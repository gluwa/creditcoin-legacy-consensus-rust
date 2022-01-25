use crate::block::Block;
use crate::block::BlockId;
use anyhow::Result;
use sha2::Digest;
use sha2::Sha256;
use std::borrow::Cow;

use crate::block::BlockConsensus;
use crate::block::BlockHeader;
use crate::node::PowConfig;
use crate::node::PowService;
use crate::primitives::{CCDifficulty, CCNonce, CCTimestamp, H256};

pub type Hasher = Sha256;

pub fn get_hasher() -> Hasher {
  Sha256::new()
}

pub fn mkhash(hasher: &mut Hasher, block_id: &[u8], peer_id: &[u8], nonce: CCNonce) -> H256 {
  let mut output: H256 = H256::new();

  mkhash_into(hasher, &mut output, block_id, peer_id, nonce);

  output
}

pub fn mkhash_into(
  hasher: &mut Hasher,
  output: &mut H256,
  block_id: &[u8],
  peer_id: &[u8],
  nonce: CCNonce,
) {
  hasher.update(block_id);
  hasher.update(peer_id);
  hasher.update(nonce.to_string().as_bytes());
  output.copy_from_slice(&*hasher.finalize_reset());
}

pub fn is_valid_proof_of_work(hash: &H256, difficulty: CCDifficulty) -> (bool, CCDifficulty) {
  let digest = digest_score(hash);
  (digest >= difficulty, digest)
}

pub fn get_difficulty(
  header: &BlockHeader,
  timestamp: CCTimestamp,
  service: &mut PowService,
  config: &PowConfig,
) -> CCDifficulty {
  if header.is_genesis() {
    return config.initial_difficulty;
  }
  calculate_difficulty(header, timestamp, service, config).unwrap_or(config.initial_difficulty)
}

fn calculate_difficulty(
  header: &BlockHeader,
  timestamp: CCTimestamp,
  service: &mut PowService,
  config: &PowConfig,
) -> Result<CCDifficulty> {
  if is_tuning_block(header, config) {
    calculate_tuning_difficulty(header, timestamp, service, config)
  } else if is_adjustment_block(header, config) {
    calculate_adjustment_difficulty(header, timestamp, service, config)
  } else {
    Ok(header.consensus.expected_difficulty)
  }
}

fn calculate_tuning_difficulty(
  header: &BlockHeader,
  timestamp: CCTimestamp,
  service: &mut PowService,
  config: &PowConfig,
) -> Result<CCDifficulty> {
  let (time_taken, time_expected) = elapsed_time(
    header,
    service,
    timestamp,
    config.difficulty_tuning_block_count,
    config.seconds_between_blocks,
  )?;

  let difficulty: u32 = header.consensus.expected_difficulty;

  if time_taken < time_expected && difficulty < 255 {
    Ok(difficulty + 1)
  } else if time_taken > time_expected && difficulty > 0 {
    Ok(difficulty - 1)
  } else {
    Ok(difficulty)
  }
}

fn calculate_adjustment_difficulty(
  header: &BlockHeader,
  timestamp: CCTimestamp,
  service: &mut PowService,
  config: &PowConfig,
) -> Result<CCDifficulty> {
  let (time_taken, time_expected) = elapsed_time(
    header,
    service,
    timestamp,
    config.difficulty_adjustment_block_count,
    config.seconds_between_blocks,
  )?;

  let difficulty: u32 = header.consensus.expected_difficulty;

  if time_taken < time_expected / 2.0 && difficulty < 255 {
    Ok(difficulty + 1)
  } else if time_taken > time_expected * 2.0 && difficulty > 0 {
    Ok(difficulty - 1)
  } else {
    Ok(difficulty)
  }
}

fn is_tuning_block(header: &BlockHeader, config: &PowConfig) -> bool {
  header.block_num % config.difficulty_tuning_block_count == 0
}

fn is_adjustment_block(header: &BlockHeader, config: &PowConfig) -> bool {
  header.block_num % config.difficulty_adjustment_block_count == 0
}

fn elapsed_time(
  header: &BlockHeader,
  service: &mut PowService,
  current_time: CCTimestamp,
  total_count: u64,
  expected_interval: u64,
) -> Result<(f64, f64)> {
  let mut count: u64 = 1;
  let mut previous_time: f64 = header.consensus.timestamp;
  let mut block_id: Cow<BlockId> = Cow::Borrowed(&header.previous_id);

  loop {
    let block: Block = service.get_block(&block_id)?;

    if !BlockConsensus::is_pow_consensus(&block.payload) {
      break;
    }

    let timestamp: f64 = match BlockConsensus::deserialize(&block.payload) {
      Ok(consensus) => consensus.timestamp,
      Err(error) => panic!("Failed to parse PoW consensus: {}", error),
    };

    count += 1;
    block_id = Cow::Owned(block.previous_id);
    previous_time = timestamp;

    if count >= total_count {
      break;
    }
  }

  let time_taken: f64 = current_time - previous_time;
  let time_expected: f64 = (count * expected_interval) as f64;

  Ok((time_taken, time_expected))
}

pub fn digest_score(digest: &H256) -> u32 {
  let mut score: u32 = 0;

  for byte in digest.iter().copied() {
    if byte > 0 {
      if byte >= 128 {
        continue;
      } else if byte >= 64 {
        score += 1;
      } else if byte >= 32 {
        score += 2;
      } else if byte >= 16 {
        score += 3;
      } else if byte >= 8 {
        score += 4;
      } else if byte >= 4 {
        score += 5;
      } else if byte >= 2 {
        score += 6;
      } else {
        score += 7;
      }
      break;
    } else {
      score += 8;
    }
  }

  score
}
