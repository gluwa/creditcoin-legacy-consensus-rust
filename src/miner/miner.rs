use std::cell::RefCell;
use std::fmt::{Debug, Formatter, Result as FmtResult};

use sawtooth_sdk::consensus::engine::Error;

use crate::utils::utc_seconds_f64;
use crate::work::get_difficulty;
use crate::{
  block::{Block, BlockHeader, BlockId, SerializedBlockConsensus},
  node::PowService,
};
use crate::{
  miner::{Answer, Challenge, Worker},
  node::{PeerId, PowConfig},
};

use super::MessageToMiner;

#[derive(Default)]
pub struct Miner {
  worker: Worker,
  answer: RefCell<Option<Answer>>,
}

impl Miner {
  pub fn try_create_consensus(&self) -> Option<SerializedBlockConsensus> {
    // Drain answers from the worker thread
    while let Some(msg) = self.worker.try_recv() {
      match msg {
        MessageToMiner::Solved(answer) => {
          self.answer.borrow_mut().replace(answer);
        }
        MessageToMiner::Started => {
          self.clear_answer();
        }
      };
    }

    self.answer.take().as_ref().map(|answer| answer.into())
  }

  pub fn mine(
    &mut self,
    block_id: BlockId,
    peer_id: PeerId,
    service: &mut PowService,
    config: &PowConfig,
  ) -> Result<(), Error> {
    let block: Block = service.get_block(&block_id)?;
    let header: BlockHeader = BlockHeader::borrowed(&block).expect("Block Header");

    let timestamp: f64 = utc_seconds_f64();
    let difficulty: u32 = get_difficulty(&header, timestamp, service, config);

    let challenge: Challenge = Challenge {
      difficulty,
      timestamp,
      block_id,
      peer_id,
    };

    self.worker.send(challenge);

    Ok(())
  }

  fn clear_answer(&self) {
    *self.answer.borrow_mut() = None;
  }
}

impl Debug for Miner {
  fn fmt(&self, f: &mut Formatter) -> FmtResult {
    f.debug_struct("Miner")
      .field("worker", &self.worker)
      .field("answer", &self.answer)
      .finish()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::block::SerializedBlockConsensus;
  #[test]
  fn default_miner() {
    let m = Miner::default();
    assert!(m.answer.take().is_none());
  }
  #[test]
  /// Worker shouldn't stop unless shutdown.
  /// Refine the answer until it receives a new challenge.
  /// It shouldn't yield an answer twice.
  fn worker_wont_stop() -> Result<(), Error> {
    let miner = Miner::default();
    let block_id = b"1111111111111111".iter().copied().collect();
    let peer_id = b"1111111111111111".iter().copied().collect();

    let timestamp: f64 = utc_seconds_f64();

    let challenge: Challenge = Challenge {
      difficulty: 12,
      timestamp,
      block_id,
      peer_id,
    };

    miner.worker.send(challenge.clone());
    let mut consensus: SerializedBlockConsensus;
    loop {
      if let Some(new) = miner.try_create_consensus() {
        consensus = new;
        break;
      };
    }
    //Don't return the same answer when polled again
    loop {
      if let Some(new) = miner.try_create_consensus() {
        assert_ne!(consensus, new);
        break;
      };
    }
    //Restart challenge
    miner.worker.send(challenge.clone());
    loop {
      if let Some(new) = miner.try_create_consensus() {
        assert_ne!(consensus, new);
        consensus = new;
        break;
      };
    }
    loop {
      if let Some(new) = miner.try_create_consensus() {
        assert_ne!(consensus, new);
        break;
      };
    }

    Ok(())
  }
}
