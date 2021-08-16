use crate::miner::Challenge;
use crate::block::{BlockConsensus,SerializedBlockConsensus};


#[derive(Clone, Debug)]
pub struct Answer {
  pub challenge: Challenge,
  pub nonce: u64,
}

impl From<&Answer> for SerializedBlockConsensus{
    fn from(answer: &Answer) -> Self {
      BlockConsensus::serialize(
      answer.challenge.difficulty,
      answer.challenge.timestamp,
      answer.nonce
      )
    }
}