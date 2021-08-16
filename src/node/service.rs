use sawtooth_sdk::consensus::{
  engine::{Block, BlockId, Error, PeerId},
  service::Service,
};
use std::collections::HashMap;

pub struct PowService {}

impl Service for PowService {
  // ===========================================================================
  // Block Creation
  // ===========================================================================

  /// Initialize a new block built on the block with the given
  /// previous id and begin adding batches to it. If no previous
  /// id is specified, the current head will be used.
  fn initialize_block(&mut self, previous_id: Option<BlockId>) -> Result<(), Error> {
    todo!()
  }

  /// Stop adding batches to the current block and return
  /// a summary of its contents.
  fn summarize_block(&mut self) -> Result<Vec<u8>, Error> {
    todo!()
  }

  /// Insert the given consensus data into the block and sign it.
  ///
  /// Note: If this call is successful, a BlockNew update
  ///       will be received with the new block afterwards.
  fn finalize_block(&mut self, data: Vec<u8>) -> Result<BlockId, Error> {
    todo!()
  }

  /// Stop adding batches to the current block and abandon it.
  fn cancel_block(&mut self) -> Result<(), Error> {
    todo!()
  }

  // ===========================================================================
  // Block Management
  // ===========================================================================

  /// Update the prioritization of blocks to check.
  ///
  /// Note: The results of all checks will be sent
  ///       as BlockValid and BlockInvalid updates.
  fn check_blocks(&mut self, priority: Vec<BlockId>) -> Result<(), Error> {
    todo!()
  }

  /// Update the block that should be committed.
  ///
  /// Note: This block must already have been checked.
  fn commit_block(&mut self, block_id: BlockId) -> Result<(), Error> {
    todo!()
  }

  /// Signal that this block is no longer being committed.
  fn ignore_block(&mut self, block_id: BlockId) -> Result<(), Error> {
    todo!()
  }

  /// Mark this block as invalid from the perspective of consensus.
  ///
  /// Note: This will also fail all descendants.
  fn fail_block(&mut self, block_id: BlockId) -> Result<(), Error> {
    todo!()
  }

  // ===========================================================================
  // Querying
  // ===========================================================================

  /// Retrieve consensus-related information about blocks
  fn get_blocks(&mut self, block_ids: Vec<BlockId>) -> Result<HashMap<BlockId, Block>, Error> {
    todo!()
  }

  /// Get the chain head block.
  fn get_chain_head(&mut self) -> Result<Block, Error> {
    todo!()
  }

  /// Read the value of settings as of the given block
  fn get_settings(
    &mut self,
    block_id: BlockId,
    keys: Vec<String>,
  ) -> Result<HashMap<String, String>, Error> {
    todo!()
  }

  /// Read values in state as of the given block
  fn get_state(
    &mut self,
    block_id: BlockId,
    addresses: Vec<String>,
  ) -> Result<HashMap<String, Vec<u8>>, Error> {
    todo!()
  }

  // ===========================================================================
  // P2P
  // ===========================================================================
  /// Send a consensus message to a specific, connected peer.
  fn send_to(&mut self, peer: &PeerId, message_type: &str, payload: Vec<u8>) -> Result<(), Error> {
    unimplemented!();
  }

  /// Broadcast a message to all connected peers.
  fn broadcast(&mut self, Message_type: &str, payload: Vec<u8>) -> Result<(), Error> {
    unimplemented!();
  }
}

impl PowService {
  pub fn get_block(&mut self, block_id: &[u8]) -> Result<Block, Error> {
    self
      .get_blocks(vec![block_id.to_owned()]).expect(&format!("Block {}",dbg_hex!(block_id)))
      .remove(block_id)
      .ok_or(Error::UnknownBlock(String::from_utf8(block_id.to_owned()).expect(&format!("utf-8 {}",dbg_hex!(block_id)))))
  }
}
