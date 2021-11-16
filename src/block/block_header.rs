use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::ops::Deref;

use crate::block::{Block, BlockConsensus, ConsensusError};
use crate::primitives::H256;
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
    2u64.pow(self.consensus.difficulty)
  }

  pub fn validate(&self) -> Result<(), ConsensusError> {
    // The genesis block is always valid
    if self.is_genesis() {
      return Ok(());
    }

    // The block must pass the difficulty filter
    self.validate_proof_of_work()?;

    Ok(())
  }

  fn validate_proof_of_work(&self) -> Result<(), ConsensusError> {
    let hash: H256 = mkhash(
      &mut get_hasher(),
      &self.previous_id,
      &self.signer_id,
      self.consensus.nonce,
    );

    let (is_valid, difficulty) = is_valid_proof_of_work(&hash, self.consensus.difficulty);

    if is_valid {
      Ok(())
    } else {
      Err(ConsensusError::InvalidHash(format!(
        "({} / diff:{})",
        difficulty, self.consensus.difficulty
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
