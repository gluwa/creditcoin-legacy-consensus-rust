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
    let header = BlockHeader::borrowed(&block).expect("Chain head Header");

    let timestamp: f64 = utc_seconds_f64();
    let difficulty = 19;
    warn!("Mining low difficulty: {}", difficulty);

    let next_difficulty: u32 = get_difficulty(&header, timestamp, service, config);

    let challenge: Challenge = Challenge {
      difficulty,
      timestamp,
      block_id,
      peer_id,
      next_difficulty,
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
  use crate::block::{BlockConsensus, SerializedBlockConsensus};
  use crate::primitives::H256;
  use crate::work::{get_hasher, is_valid_proof_of_work, mkhash};

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
      next_difficulty: 0,
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
  #[test]
  ///The worker should return a challenge with the next's block expected difficulty.
  fn worker_returns_challenge_with_expected_difficulty() -> Result<(), String> {
    let miner = Miner::default();
    let block_id: Vec<u8> = b"1111111111111111".iter().copied().collect();
    let peer_id: Vec<u8> = b"1111111111111111".iter().copied().collect();

    let timestamp: f64 = utc_seconds_f64();

    let challenge: Challenge = Challenge {
      difficulty: 2,
      // doesnt matter how high it is, this block won't use it, that's next block's problem.
      next_difficulty: 100,
      timestamp,
      block_id: block_id.clone(),
      peer_id: peer_id.clone(),
    };

    miner.worker.send(challenge.clone());
    while None == miner.try_create_consensus() {}

    let consensus: BlockConsensus;
    //the second answer if any is guaranteed to be higher than the expected difficulty but the expected diff should stay the same
    loop {
      if let Some(new) = miner.try_create_consensus() {
        consensus = BlockConsensus::deserialize(new.as_slice()).unwrap();
        break;
      };
    }

    assert_eq!(consensus.expected_difficulty, challenge.next_difficulty);

    let hash: H256 = mkhash(&mut get_hasher(), &block_id, &peer_id, consensus.nonce);

    let (is_valid, realized_difficulty) =
    //this block's expected difficulty is on the predecessor, in this case, the same challenge's value.
      is_valid_proof_of_work(&hash, challenge.difficulty);
    assert!(is_valid);
    assert!(realized_difficulty > challenge.difficulty);

    //a new challenge should reset the current difficulty in the worker
    miner.worker.send(challenge.clone());

    while Some(MessageToMiner::Started) != miner.worker.try_recv() {}

    std::thread::sleep(std::time::Duration::from_millis(250));

    if let MessageToMiner::Solved(ans) = miner.worker.try_recv().unwrap() {
      let hash: H256 = mkhash(&mut get_hasher(), &block_id, &peer_id, ans.nonce);
      let (_, new_realized_difficulty) = is_valid_proof_of_work(&hash, ans.challenge.difficulty);
      assert!(realized_difficulty > new_realized_difficulty);
      Ok(())
    } else {
      Err("No answer received".into())
    }
  }

  use crate::node::tests::MockService;

  #[test]
  //first pow block will pull difficulty from a default blockheader.
  // The difficulty mined should be the on-chain initial difficulty.
  fn first_pow_block_difficulty() -> Result<(), Error> {
    let config = PowConfig::new();
    let mut service = PowService::new(Box::new(MockService {}));
    let mut miner = Miner::default();
    let block_id = b"1111111111111111".iter().copied().collect();
    let peer_id = b"2222222222222222".iter().copied().collect();
    miner.mine(block_id, peer_id, &mut service, &config)?;
    loop {
      match miner.worker.try_recv() {
        Some(MessageToMiner::Solved(ans)) => {
          // first block's difficulty should be pulled from config
          assert_eq!(ans.challenge.difficulty, config.initial_difficulty);
          break;
        }
        _ => {}
      }
    }

    Ok(())
  }
}
