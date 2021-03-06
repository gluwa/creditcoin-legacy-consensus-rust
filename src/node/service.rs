use sawtooth_sdk::consensus::{
  engine::{Block, Error},
  service::Service,
};

use crate::utils::to_hex;
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
      .and_then(|mut map| {
        map
          .remove(block_id)
          .ok_or_else(|| Error::UnknownBlock(to_hex(block_id)))
      })
  }
}

#[cfg(test)]
pub mod tests {
  use crate::consensus::engine::{Block, BlockId, Error, PeerId};
  pub use sawtooth_sdk::consensus::engine::StartupState;
  pub use sawtooth_sdk::consensus::service::*;
  use std::collections::hash_map::HashMap;

  //Mock Service is a copy-paste from sawtooth-sdk, check licensing.
  pub struct MockService {}

  impl Service for MockService {
    fn send_to(
      &mut self,
      _peer: &PeerId,
      _message_type: &str,
      _payload: Vec<u8>,
    ) -> Result<(), Error> {
      Ok(())
    }
    fn broadcast(&mut self, _message_type: &str, _payload: Vec<u8>) -> Result<(), Error> {
      Ok(())
    }
    fn initialize_block(&mut self, _previous_id: Option<BlockId>) -> Result<(), Error> {
      Ok(())
    }
    fn summarize_block(&mut self) -> Result<Vec<u8>, Error> {
      Ok(Default::default())
    }
    fn finalize_block(&mut self, _data: Vec<u8>) -> Result<BlockId, Error> {
      Ok(Default::default())
    }
    fn cancel_block(&mut self) -> Result<(), Error> {
      Ok(())
    }
    fn check_blocks(&mut self, _priority: Vec<BlockId>) -> Result<(), Error> {
      Ok(())
    }
    fn commit_block(&mut self, _block_id: BlockId) -> Result<(), Error> {
      Ok(())
    }
    fn ignore_block(&mut self, _block_id: BlockId) -> Result<(), Error> {
      Ok(())
    }
    fn fail_block(&mut self, _block_id: BlockId) -> Result<(), Error> {
      Ok(())
    }
    fn get_blocks(&mut self, block_ids: Vec<BlockId>) -> Result<HashMap<BlockId, Block>, Error> {
      let mut map = HashMap::new();
      for k in block_ids {
        map.insert(k, Block::default());
      }
      Ok(map)
    }
    fn get_chain_head(&mut self) -> Result<Block, Error> {
      Ok(Default::default())
    }
    fn get_settings(
      &mut self,
      _block_id: BlockId,
      _settings: Vec<String>,
    ) -> Result<HashMap<String, String>, Error> {
      Ok(Default::default())
    }
    fn get_state(
      &mut self,
      _block_id: BlockId,
      _addresses: Vec<String>,
    ) -> Result<HashMap<String, Vec<u8>>, Error> {
      Ok(Default::default())
    }
  }
}
