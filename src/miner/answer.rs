use crate::block::{BlockConsensus, SerializedBlockConsensus};
use crate::miner::Challenge;

#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
  pub challenge: Challenge,
  pub nonce: u64,
}

impl From<&Answer> for SerializedBlockConsensus {
  fn from(answer: &Answer) -> Self {
    BlockConsensus::serialize(
      answer.challenge.next_difficulty,
      answer.challenge.timestamp,
      answer.nonce,
    )
  }
}
