use crate::consensus::engine::Update;

use crate::futures::Receiver;
use crate::futures::{sleep, Duration, RecvTimeoutError};

use crate::node::PowNode;

pub struct UpdateStream {
  updates: Receiver<Update>,
  node: PowNode,
}

impl UpdateStream {
  pub fn new(updates: Receiver<Update>, node: PowNode) -> Self {
    Self { updates, node }
  }

  pub async fn update_loop(mut self) {
    while let Some(..) = self.update_call().await {}
  }
}

impl UpdateStream {
  async fn update_call(&mut self) -> Option<()> {
    let call = self.updates.recv_timeout(Duration::from_millis(0));
    match call {
      Ok(update) => {
        trace!("Incoming update {:?}", update);
        match self.node.handle_update(update) {
          Ok(true) => Some(()),
          Ok(false) => None,
          Err(error) => {
            error!("Update Error: {}", error);
            Some(())
          }
        }
      }
      Err(RecvTimeoutError::Disconnected) => {
        error!("Disconnected from validator");
        None
      }
      Err(RecvTimeoutError::Timeout) => {
        sleep(Duration::from_millis(10)).await;
        Some(())
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::block::Block;
  use crate::engine::PowEngine;
  use crate::node::tests::MockService;
  use crate::node::tests::{Service, StartupState};
  use sawtooth_sdk::consensus::engine::{Engine, PeerInfo};
  use std::boxed::Box;
  use std::sync::mpsc::channel;

  #[test]
  fn test_update_future() {
    let (sx, rx) = channel::<Update>();
    let t = Update::BlockCommit(vec![]);
    let _ = sx.send(t);
    let t = Update::BlockInvalid(vec![]);
    let _ = sx.send(t);
    let t = Update::BlockValid(vec![]);
    let _ = sx.send(t);
    let t = Update::Shutdown;
    let _ = sx.send(t);

    let mut engine = PowEngine::new();
    let service = Box::new(MockService {});

    //dummy block
    let chain_head = Block::default();
    let startup_state: StartupState = StartupState {
      chain_head,
      peers: vec![],
      local_peer_info: PeerInfo::default(),
    };

    engine
      .start(rx, service as Box<dyn Service>, startup_state)
      .expect("start")
  }
}
