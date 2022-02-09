use crate::block::BlockId;
use crate::node::PeerId;
use crate::primitives::{CCDifficulty, CCTimestamp};
use std::fmt::{Debug, Formatter, Result};

#[derive(Clone, PartialEq)]
pub struct Challenge {
  pub difficulty: CCDifficulty,
  //mine using the lagged difficulty and store the expected difficulty for the next block.
  pub next_difficulty: CCDifficulty,
  pub timestamp: CCTimestamp,
  pub block_id: BlockId,
  pub peer_id: PeerId,
}

impl Debug for Challenge {
  fn fmt(&self, f: &mut Formatter) -> Result {
    f.debug_struct("Challenge")
      .field("difficulty", &self.difficulty)
      .field("next_difficulty", &self.next_difficulty)
      .field("timestamp", &self.timestamp)
      .field("block_id", &dbg_hex!(&self.block_id))
      .field("peer_id", &dbg_hex!(&self.peer_id))
      .finish()
  }
}
