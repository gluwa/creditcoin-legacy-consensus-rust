use sawtooth_sdk::consensus::{
  engine::{Block, Error},
  service::Service,
};

use std::ops::{Deref, DerefMut};

pub struct PowService {
  service: Box<dyn Service>,
}

impl Deref for PowService {
  type Target = Box<dyn Service>;

  fn deref(&self) -> &Self::Target {
    &(self.service)
  }
}

impl DerefMut for PowService {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut (self.service)
  }
}

impl PowService {
  pub fn new(service: Box<dyn Service>) -> Self {
    PowService { service }
  }

  pub fn get_block(&mut self, block_id: &[u8]) -> Result<Block, Error> {
    self
      .service
      .get_blocks(vec![block_id.to_owned()])
      .expect(&format!("Block {}", dbg_hex!(block_id)))
      .remove(block_id)
      .ok_or(Error::UnknownBlock(
        String::from_utf8(block_id.to_owned()).expect(&format!("utf-8 {}", dbg_hex!(block_id))),
      ))
  }
}
