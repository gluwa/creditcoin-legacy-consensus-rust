use crate::block::BlockId;
use crate::node::PeerId;
use crate::primitives::{CCDifficulty, CCTimestamp};
use std::fmt::{Debug, Formatter, Result};

#[derive(Clone)]
pub struct Challenge {
  pub difficulty: CCDifficulty,
  pub timestamp: CCTimestamp,
  pub block_id: BlockId,
  pub peer_id: PeerId,
}

impl Debug for Challenge {
  fn fmt(&self, f: &mut Formatter) -> Result {
    f.debug_struct("Challenge")
      .field("difficulty", &self.difficulty)
      .field("timestamp", &self.timestamp)
      .field("block_id", &dbg_hex!(&self.block_id))
      .field("peer_id", &dbg_hex!(&self.peer_id))
      .finish()
  }
}
