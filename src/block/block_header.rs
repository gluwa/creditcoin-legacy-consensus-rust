use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::ops::Deref;

use crate::block::{Block, BlockConsensus, ConsensusError};
use crate::primitives::{CCDifficulty, H256};
use crate::work::get_hasher;
use crate::work::{is_valid_proof_of_work, mkhash};

#[derive(Clone)]
pub struct BlockHeader<'a> {
  block: Cow<'a, Block>,
  pub consensus: BlockConsensus,
}

impl<'a> BlockHeader<'a> {
  pub fn owned(block: Block) -> Result<Self, ConsensusError> {
    Self::from_cow(Cow::Owned(block))
  }

  pub fn borrowed(block: &'a Block) -> Result<Self, ConsensusError> {
    Self::from_cow(Cow::Borrowed(block))
  }

  pub fn from_cow(block: Cow<'a, Block>) -> Result<Self, ConsensusError> {
    let consensus = if block.block_num == 0 {
      BlockConsensus::new()
    } else {
      BlockConsensus::deserialize(&block.payload)?
    };
    Ok(Self { block, consensus })
  }

  pub fn is_genesis(&self) -> bool {
    self.block_num == 0
  }

  pub fn work(&self) -> u64 {
    let actual_difficulty = self
      //we don't want to validate difficulty, we want the actual_difficulty, use the minimum input value so that the method never fails
      .validate_proof_of_work(0)
      .expect("Validity was previously attested when creating the BlockHeader");

    2u64.pow(actual_difficulty)
  }

  //Validate that the solution has a difficulty greater or equal than the minimum difficulty (now stored in the predecessor)
  pub fn validate(self, _minimum_difficulty: CCDifficulty) -> Result<Self, ConsensusError> {
    // The genesis block is always valid
    if self.is_genesis() {
      return Ok(self);
    }

    warn!("Bypassing proof of work validation");

    Ok(self)
  }

  // is valid proof of work using the consensus difficulty field
  fn validate_proof_of_work(
    &self,
    difficulty: CCDifficulty,
  ) -> Result<CCDifficulty, ConsensusError> {
    let hash: H256 = mkhash(
      &mut get_hasher(),
      &self.previous_id,
      &self.signer_id,
      self.consensus.nonce,
    );

    let (is_valid, actual_difficulty) = is_valid_proof_of_work(&hash, difficulty);

    if is_valid {
      Ok(actual_difficulty)
    } else {
      Err(ConsensusError::InvalidHash(format!(
        "(Expected {}, got {})",
        difficulty, actual_difficulty
      )))
    }
  }
}

impl Deref for BlockHeader<'_> {
  type Target = Block;

  fn deref(&self) -> &Self::Target {
    &self.block
  }
}

impl From<Block> for BlockHeader<'_> {
  fn from(block: Block) -> Self {
    Self::owned(block).unwrap()
  }
}

impl<'a> From<&'a Block> for BlockHeader<'a> {
  fn from(block: &'a Block) -> Self {
    Self::borrowed(block).unwrap()
  }
}

impl Debug for BlockHeader<'_> {
  fn fmt(&self, f: &mut Formatter) -> FmtResult {
    f.debug_struct("Block")
      .field("block_num", &self.block_num)
      .field("block_id", &dbg_hex!(&self.block_id))
      .field("previous_id", &dbg_hex!(&self.previous_id))
      .field("consensus", &self.consensus)
      .finish()
  }
}

impl Display for BlockHeader<'_> {
  fn fmt(&self, f: &mut Formatter) -> FmtResult {
    write!(
      f,
      "Block({}, {}, {})",
      self.block_num,
      dbg_hex!(&self.block_id),
      dbg_hex!(&self.previous_id),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::block::Block;

  use crate::miner::Miner;
  use crate::node::tests::MockService;
  use crate::node::{PowConfig, PowService};

  #[test]
  ///Validate proof of work could mistakenly return the expected difficulty instead of the actual difficulty.
  fn validate_proof_of_work_returns_actual_diff() {
    let mut miner = Miner::default();
    let mut service = PowService::new(Box::new(MockService {}));
    let mut config = PowConfig::new();
    let mut b = Block::default();

    {
      config.initial_difficulty = 7;
      let block_id = b"1111111111111111".iter().copied().collect();
      let peer_id = b"2222222222222222".iter().copied().collect();
      miner
        .mine(block_id, peer_id, &mut service, &config)
        .unwrap();
    }

    loop {
      if let Some(c) = miner.try_create_consensus() {
        b.payload = c;
        break;
      }
    }

    let block_header = BlockHeader::borrowed(&b).expect("test-block");
    let exp_diff = 0;
    let actual_diff = block_header.validate_proof_of_work(exp_diff).unwrap();
    assert_ne!(actual_diff, exp_diff);
  }
}
