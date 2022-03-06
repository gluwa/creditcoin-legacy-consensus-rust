pub use sawtooth_sdk::consensus::engine::PeerId;
use sawtooth_sdk::consensus::{
  engine::{Error, StartupState, Update},
  service::Service,
};

#[cfg(not(feature = "test-futures"))]
use std::borrow::Cow;

use crate::node::{PowConfig, PowService, PowState};
use crate::{block::BlockPrinter as Printer, futures::EventResult, miner::Miner};
#[cfg(not(feature = "test-futures"))]
use crate::{
  block::{Block, BlockAncestors, BlockConsensus, BlockHeader, BlockId},
  node::Guard,
  utils::to_hex,
};

use super::EventPublishResult;

#[cfg(not(feature = "test-futures"))]
pub const NULL_BLOCK_IDENTIFIER: [u8; 8] = [0; 8];

pub struct PowNode {
  pub config: PowConfig,
  pub service: PowService,
  state: PowState,
  #[cfg_attr(feature = "test-futures", allow(dead_code))]
  miner: Miner,
}

#[cfg(feature = "test-futures")]
impl PowNode {
  pub fn handle_update(&mut self, update: Update) -> Result<EventResult, Error> {
    let (_, res) = match update {
      Update::BlockNew(..) => ("BlockNew", Ok(EventResult::Continue)),
      Update::BlockValid(..) => ("BlockValid", Ok(EventResult::Continue)),
      Update::BlockInvalid(..) => ("BlockInvalid", Ok(EventResult::Continue)),
      Update::BlockCommit(..) => ("BlockCommit", Ok(EventResult::Restart(true))),
      Update::Shutdown => ("Shutdown", Ok(EventResult::Shutdown)),
      Update::PeerConnected(..) | Update::PeerDisconnected(..) | Update::PeerMessage(..) => {
        // ignore peer-related messages
        ("PeerGenericUpdate", Ok(EventResult::Continue))
      }
    };
    res
  }

  pub fn try_publish(&mut self) -> Result<EventPublishResult, Error> {
    Ok(EventPublishResult::Published)
  }
}

#[cfg(not(feature = "test-futures"))]
impl PowNode {
  pub fn handle_update(&mut self, update: Update) -> Result<EventResult, Error> {
    match update {
      Update::BlockNew(block) => self.on_block_new(block),
      Update::BlockValid(block_id) => self.on_block_valid(block_id),
      Update::BlockInvalid(block_id) => self.on_block_invalid(block_id),
      Update::BlockCommit(block_id) => self.on_block_commit(block_id),
      Update::Shutdown => Ok(EventResult::Shutdown),
      Update::PeerConnected(..) | Update::PeerDisconnected(..) | Update::PeerMessage(..) => {
        // ignore peer-related messages
        Ok(EventResult::Continue)
      }
    }
  }

  #[allow(clippy::ptr_arg)]
  fn on_block_new_error_handler(
    &mut self,
    block_id: &BlockId,
    error: impl std::error::Error,
  ) -> Result<(), Error> {
    debug!("Failed consensus check: {} - {:?}", to_hex(block_id), error);
    self.service.fail_block(block_id.to_owned())
  }

  /// Called when a new block is received; call for validation or fail the block.
  /// Handle a `BlockValid` update from the Validator
  ///
  /// The block has been verified by the validator, so mark it as validated in the log and
  /// attempt to handle the block.

  fn on_block_new(&mut self, block: Block) -> Result<EventResult, Error> {
    // This should never happen under normal circumstances
    if block.previous_id == NULL_BLOCK_IDENTIFIER {
      error!("Received Update::BlockNew for genesis block!");
      return Ok(EventResult::Continue);
    }

    debug!("Checking block consensus: {}", Printer(&block));

    let header = match BlockHeader::borrowed(&block) {
      // Ensure the block consensus is valid
      Ok(h) => h,
      Err(e) => {
        self.on_block_new_error_handler(&block.block_id, e)?;
        return Ok(EventResult::Continue);
      }
    };

    let expected_min_diff = header.consensus.expected_difficulty;
    // Ensure that the minimum difficulty has been reached.
    // The block must pass the difficulty filter, use the lagged difficulty stored in the predecessor.
    match header.validate(expected_min_diff) {
      Ok(_) => (),
      Err(e) => {
        debug!(
          "Failed(bypassed) consensus check: {} - {:?}",
          to_hex(&block.block_id),
          e
        );
      }
    }

    debug!("Passed consensus check: {}", Printer(&block));
    // Request block validation
    self.service.check_blocks(vec![block.block_id])?;

    Ok(EventResult::Continue)
  }

  /// Called when a block check succeeds
  fn on_block_valid(&mut self, block_id: BlockId) -> Result<EventResult, Error> {
    let cur_head: Block = self.service.get_block(&self.state.chain_head)?;
    let new_head: Block = self.service.get_block(&block_id)?;

    debug!(
      "Choosing between chain heads -- current: {} -- new: {}",
      Printer(&cur_head),
      Printer(&new_head),
    );

    //fork resolution and commit block
    self.compare_forks(cur_head, new_head)?;

    Ok(EventResult::Continue)
  }

  /// Called when a block check fails
  /// The block has failed, perform cleanup of consensus' state
  fn on_block_invalid(&mut self, _block_id: BlockId) -> Result<EventResult, Error> {
    Ok(EventResult::Continue)
  }

  /// Called when a block commit completes
  fn on_block_commit(&mut self, block_id: BlockId) -> Result<EventResult, Error> {
    debug!("Chain head updated to {}", dbg_hex!(&block_id));

    let mut did_publish = false;
    //don't try to publish if we have already published.
    if !self.state.guards.contains(&Guard::Finalized) {
      //try to publish opportunistically
      match self.try_publish() {
        Ok(EventPublishResult::Published) => {
          trace!("Eager-published");
          did_publish = true;
        }
        Ok(EventPublishResult::Pending) => {
          trace!("Unsuccessful eager-publishing");
        }
        Err(e) => {
          trace!("Failed eager-publishing with Error: {}", e);
        }
      }
    }

    if !self.state.guards.contains(&Guard::Finalized) {
      // Stop adding batches to the current block and abandon it.
      self.service.cancel_block()?;
    }

    // Refresh on-chain configuration
    self.reload_configuration()?;

    // Remove publishing guards, allows starting the publishing state machine.
    self.state.guards.clear();

    // Start the PoW process for this block

    // Initialize a new block based on the updated chain head
    self.service.initialize_block(Some(block_id))?;

    Ok(EventResult::Restart(did_publish))
  }

  fn compare_forks(&mut self, cur_head: Block, new_head: Block) -> Result<(), Error> {
    if !BlockConsensus::is_pow_consensus(&new_head.payload) {
      debug!("Ignoring new block (consensus) {}", Printer(&new_head));
      self.service.ignore_block(new_head.block_id)?;
      return Ok(());
    }

    if !BlockConsensus::is_pow_consensus(&cur_head.payload) {
      // this should be only possible if we switched consensus modes and haven't yet commited a block
      let mut fork_block: Cow<Block> = Cow::Borrowed(&new_head);

      loop {
        if fork_block.previous_id == cur_head.block_id {
          debug!("Committing new block (consensus) {}", Printer(&new_head));
          self.wrapper_service_commit_block(new_head.block_id)?;
          break;
        } else if !BlockConsensus::is_pow_consensus(&fork_block.payload) {
          // also happens with genesis blocks
          debug!("Ignoring new block (consensus) {}", Printer(&new_head));
          self.service.ignore_block(new_head.block_id)?;
          break;
        }

        fork_block = Cow::Owned(self.service.get_block(&fork_block.previous_id)?);
      }
    } else if new_head.block_num == cur_head.block_num + 1
      && new_head.previous_id == cur_head.block_id
    {
      debug!("Committing new block (next) {}", Printer(&new_head));
      self.wrapper_service_commit_block(new_head.block_id)?;
    } else {
      self.resolve_fork(cur_head, new_head)?;
    }

    Ok(())
  }

  fn resolve_fork(&mut self, cur_head: Block, new_head: Block) -> Result<(), Error> {
    let cur_diff_size: u64 = cur_head.block_num.saturating_sub(new_head.block_num);
    let new_diff_size: u64 = new_head.block_num.saturating_sub(cur_head.block_num);

    debug!(
      "Resolve fork with height ({}/{})",
      cur_diff_size, new_diff_size,
    );

    // Fetch all blocks from the current chain AFTER the head of the new chain
    // Inverse of `new_chain_orphans`.
    let cur_chain_orphans: Vec<BlockHeader> =
      BlockAncestors::new(&cur_head.previous_id, &mut self.service)
        .take(cur_diff_size as usize)
        .take_while(|block| block.consensus.is_pow())
        .collect();

    // Fetch all blocks from the new chain AFTER the head of the current chain.
    // Inverse of `cur_chain_orphans`.
    let new_chain_orphans: Vec<BlockHeader> =
      BlockAncestors::new(&new_head.previous_id, &mut self.service)
        .take(new_diff_size as usize)
        .take_while(|block| block.consensus.is_pow())
        .collect();

    // Convert both chain heads to `BlockHeader`s. Propagate errors since
    // PoW validation should have been an earlier step.
    let cur_header: BlockHeader = BlockHeader::borrowed(&cur_head)
      .unwrap_or_else(|_| panic!("Cur_header {}", Printer::from(&cur_head)));
    let new_header: BlockHeader = BlockHeader::borrowed(&new_head)
      .unwrap_or_else(|_| panic!("New_header {}", Printer::from(&new_head)));

    // Fetch the earliest block from both orphan chains; default to the current head
    let cur_fork_head: &BlockHeader = cur_chain_orphans.last().unwrap_or(&cur_header);
    let new_fork_head: &BlockHeader = new_chain_orphans.last().unwrap_or(&new_header);

    debug_assert_eq!(cur_fork_head.block_num, new_fork_head.block_num);

    // Construct a `ForkChain` to quickly traverse ancestors in pairs.
    // Traverse until:
    //   1. A common ancestor is found
    //   2. Either block is a genesis block
    //   3. Either block is NOT a PoW block
    let cur_ancestors = BlockAncestors::new(&cur_fork_head.block_id, &mut self.service);
    let (cur_fork_blocks, new_fork_blocks): (Vec<_>, Vec<_>) = cur_ancestors
      .paired_fork_iter(&new_fork_head.block_id)
      .take_while(|(block_a, block_b)| block_a.block_id != block_b.block_id)
      .take_while(|(block_a, block_b)| !block_a.is_genesis() && !block_b.is_genesis())
      .take_while(|(block_a, block_b)| block_a.consensus.is_pow() && block_b.consensus.is_pow())
      .unzip();

    // Chain the new orphan chain with any uncommon
    // ancestors; sum the total amount of work.
    let new_work: u64 = new_chain_orphans
      .iter()
      .chain(new_fork_blocks.iter())
      .fold(0, |total, block| total + block.work());

    // Chain the current orphan chain with any uncommon
    // ancestors; sum the total amount of work.
    let cur_work: u64 = cur_chain_orphans
      .iter()
      .chain(cur_fork_blocks.iter())
      .fold(0, |total, block| total + block.work());

    // Commit the new fork if it has greater work
    if new_work > cur_work {
      debug!(
        "Committing new fork (work {}/{}) {}",
        new_work,
        cur_work,
        Printer(&new_head),
      );

      self.wrapper_service_commit_block(new_head.block_id)?;
    } else {
      debug!(
        "Ignoring new fork (work {}/{}) {}",
        new_work,
        cur_work,
        Printer(&new_head),
      );

      self.service.ignore_block(new_head.block_id)?;
    }

    Ok(())
  }

  /// Is reentrant. Can be retried at any publishing state.
  pub fn try_publish(&mut self) -> Result<EventPublishResult, Error> {
    // If we already published at this height, exit early.
    if self.state.guards.contains(&Guard::Finalized) {
      //A block has not been commited yet.
      //While we are still waiting for a block to be committed
      return Ok(EventPublishResult::Pending);
    }

    //always update consensus, i.e. never skip it.
    let consensus: Vec<u8> = match self.miner.try_create_consensus() {
      Some(consensus) => {
        self.state.guards.insert(Guard::Consensus);
        consensus
      }
      None => return Ok(EventPublishResult::Pending),
    };

    // Try summarizing the blocks contents with a digest
    //check summarize guard
    let summarized = self.state.guards.contains(&Guard::Summarized);
    if !summarized {
      match self.service.summarize_block() {
        Ok(_digest) => {
          self.state.guards.insert(Guard::Summarized);
          // Finalize the block with the current consensus
        }
        Err(Error::BlockNotReady) => {
          trace!("Cannot summarize block: not ready");
          return Ok(EventPublishResult::Pending);
        }
        Err(error) => {
          return Err(error);
        }
      }
    }
    let finalized = self.state.guards.contains(&Guard::Finalized);
    if !finalized {
      match self.service.finalize_block(consensus) {
        Ok(block_id) => {
          debug!("Publishing block: {}", dbg_hex!(&block_id));

          // Set publishing guard
          self.state.guards.insert(Guard::Finalized);

          self.state.guards.remove(&Guard::Consensus);
          self.state.guards.remove(&Guard::Summarized);

          return Ok(EventPublishResult::Published);
        }
        Err(Error::BlockNotReady) => {
          trace!("Cannot finalize block: not ready");
          return Ok(EventPublishResult::Pending);
        }
        Err(error) => {
          return Err(error);
        }
      }
    }

    unreachable!();
  }

  fn wrapper_service_commit_block(&mut self, block_id: BlockId) -> Result<(), Error> {
    self.state.chain_head = block_id.to_owned();
    self.service.commit_block(block_id)
  }
}

impl PowNode {
  pub fn new(service: Box<dyn Service>) -> Self {
    Self::with_config(PowConfig::new(), service)
  }

  pub fn with_config(config: PowConfig, service: Box<dyn Service>) -> Self {
    let state: PowState = PowState::new();
    let miner: Miner = Miner::default();

    Self {
      config,
      state,
      miner,
      service: PowService::new(service),
    }
  }

  pub fn initialize(mut self, state: StartupState) -> Result<Self, Error> {
    if state.chain_head.block_num > 1 {
      debug!("Starting from non-genesis: {}", Printer(&state.chain_head));
    }

    // Store the public key of this validator, for signing blocks
    self.state.peer_id = state.local_peer_info.peer_id;

    // Store the chain head id for quick comparisons when required
    self.state.chain_head = state.chain_head.block_id;

    #[cfg(not(feature = "test-futures"))]
    {
      // Set initial on-chain configuration
      self.reload_configuration()?;

      // Start the inital PoW process with the current chain head

      // Initialize a new block based on the current chain head
      self.service.initialize_block(None)?;
    }

    Ok(self)
  }

  /// Fetch and store on-chain settings as of the current head height
  pub fn reload_configuration(&mut self) -> Result<(), Error> {
    self
      .config
      .load(&mut self.service, self.state.chain_head.to_owned())
      .map_err(Into::into)
  }
}

#[cfg(all(test, not(feature = "test-futures")))]
mod tests {

  use super::*;
  use crate::node::tests::MockService;
  #[test]
  fn if_already_published_dont_publish_on_block_commit() -> Result<(), Error> {
    let state = {
      let mut state = PowState::new();
      state.peer_id = (&b"ffffffffffffffff"[..]).into();
      state
    };
    let mut node = PowNode {
      config: PowConfig::new(),
      service: PowService::new(Box::new(MockService {})),
      state,
      miner: Miner::default(),
    };
    //publishing finished successfully
    node.state.guards.insert(Guard::Finalized);
    let blockid = &b"ffffffffffffffff"[..];
    let res = node.on_block_commit(blockid.into())?;
    if let EventResult::Restart(published) = res {
      assert!(!published);
    }

    Ok(())
  }

  #[test]
  fn if_block_is_invalid_then_continue() -> Result<(), Error> {
    let state = {
      let mut state = PowState::new();
      state.peer_id = (&b"aaaaaaaaaaaaaaaa"[..]).into();
      state
    };

    let mut node = PowNode {
      config: PowConfig::new(),
      service: PowService::new(Box::new(MockService {})),
      state,
      miner: Miner::default(),
    };

    node.state.guards.insert(Guard::Finalized);
    let blockid = &b"aaffaaffaaffffff"[..];
    let res = node.on_block_invalid(blockid.into())?;

    assert_eq!(res, EventResult::Continue);

    Ok(())
  }
}
