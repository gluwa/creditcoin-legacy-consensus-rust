use std::collections::BTreeSet;

use crate::block::BlockId;
use crate::node::Guard;
use crate::node::PeerId;

#[derive(Debug, Default)]
pub struct PowState {
  pub chain_head: BlockId,
  pub peer_id: PeerId,
  pub guards: BTreeSet<Guard>,
}

impl PowState {
  pub fn new() -> Self {
    Self::default()
  }
}
