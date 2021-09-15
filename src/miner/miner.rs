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

pub struct Miner {
  worker: Worker,
  answer: RefCell<Option<Answer>>,
}

impl Miner {
  pub fn new() -> Self {
    Self {
      worker: Worker::new(),
      answer: RefCell::new(None),
    }
  }

  pub fn try_create_consensus(&self) -> Option<SerializedBlockConsensus> {
    // Drain answers from the worker thread
    while let Some(answer) = self.worker.recv() {
      self.answer.borrow_mut().replace(answer);
    }

    match self.answer.borrow().as_ref() {
      Some(answer) => Some(answer.into()),
      None => None,
    }
  }

  pub fn reset(&self) {
    self.clear_answer();
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
    self.clear_answer();

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
