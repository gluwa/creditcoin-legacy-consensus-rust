use rand::rngs::ThreadRng;
use rand::thread_rng;
use rand::Rng;
use std::thread::Builder;
use std::thread::JoinHandle;

use crate::miner::{Answer, Challenge, Channel};
use crate::primitives::{CCNonce, H256};
use crate::utils::to_hex;
use crate::work::{get_hasher, is_valid_proof_of_work, mkhash_into, Hasher};

#[cfg(test)]
use println as debug;

type Parent = Channel<MessageToWorker, MessageToMiner>;
type Child = Channel<MessageToMiner, MessageToWorker>;

#[derive(Debug)]
pub enum MessageToWorker {
  Shutdown,
  Challenge(Challenge),
}

#[derive(Debug)]
pub enum MessageToMiner {
  Solved(Answer),
  Started,
}

#[derive(Debug)]
pub struct Worker {
  channel: Channel<MessageToWorker, MessageToMiner>,
  handle: Option<JoinHandle<()>>,
}

impl Default for Worker {
  fn default() -> Self {
    Self::new()
  }
}

impl Worker {
  fn new() -> Self {
    let (chan1, chan2): (Parent, Child) = Channel::duplex();

    let handle: JoinHandle<()> = Builder::new()
      .name("Miner".to_string())
      .spawn(Self::task(chan2))
      .expect("Worker thread failed to spawn");

    Self {
      channel: chan1,
      handle: Some(handle),
    }
  }

  pub fn send(&self, challenge: Challenge) {
    self.channel.send(MessageToWorker::Challenge(challenge));
  }

  pub fn try_recv(&self) -> Option<MessageToMiner> {
    self.channel.try_recv()
  }

  fn start(
    channel: &Channel<MessageToMiner, MessageToWorker>,
    challenge: Challenge,
    rng: &mut ThreadRng,
  ) -> (Challenge, CCNonce) {
    &channel.send(MessageToMiner::Started);
    (challenge, rng.gen_range(0..u64::MAX))
  }

  /// Mine until shutdown
  ///
  fn task(channel: Channel<MessageToMiner, MessageToWorker>) -> impl Fn() {
    move || {
      let mut hasher: Hasher = get_hasher();
      let mut output: H256 = H256::new();
      let mut rng: ThreadRng = thread_rng();

      debug!("Waiting for challenge");
      let (mut challenge, mut nonce) = match channel.recv() {
        MessageToWorker::Challenge(challenge) => Worker::start(&channel, challenge, &mut rng),
        MessageToWorker::Shutdown => return,
      };
      debug!("Received challenge: {:?}", challenge);
      let mut is_first = true;

      loop {
        mkhash_into(
          &mut hasher,
          &mut output,
          &challenge.block_id,
          &challenge.peer_id,
          nonce,
        );
        //if solved send the answer, increase diff and continue
        let (is_valid, solution_diff) = is_valid_proof_of_work(&output, challenge.difficulty);
        if is_valid && (is_first || solution_diff > challenge.difficulty) {
          debug!("Found nonce: {:?} -> {}", nonce, to_hex(&output));
          channel.send(MessageToMiner::Solved(Answer {
            challenge: challenge.clone(),
            nonce,
          }));
          is_first = false;
          challenge.difficulty = solution_diff;
        }

        //if updated, send update confirmation.
        match channel.try_recv() {
          Some(MessageToWorker::Challenge(update)) => {
            debug!("Received update: {:?}", update);
            let challenge_nonce = Worker::start(&channel, update, &mut rng);
            challenge = challenge_nonce.0;
            nonce = challenge_nonce.1;
            is_first = true;
          }
          Some(MessageToWorker::Shutdown) => {
            return;
          }
          None => {
            nonce = nonce.wrapping_add(1);
          }
        }
      }
    }
  }
}

impl Drop for Worker {
  fn drop(&mut self) {
    self.channel.send(MessageToWorker::Shutdown);

    if let Some(handle) = self.handle.take() {
      if let Err(error) = handle.join() {
        error!("Handle failed to join: {:?}", error);
      }
    } else {
      error!("Handle is `None`");
    }
  }
}
