use sawtooth_sdk::consensus::engine::Error;
use std::collections::HashMap;
use std::str::FromStr;

use crate::block::BlockId;
use crate::node::PowService;
use crate::Duration;

const INITIAL_DIFFICULTY: u32 = 22;
const SECONDS_BETWEEN_BLOCKS: u64 = 60;
const DIFFICULTY_ADJUSTMENT_BLOCK_COUNT: u64 = 10;
const DIFFICULTY_TUNING_BLOCK_COUNT: u64 = 100;
const UPDATE_RECV_TIMEOUT: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub struct PowConfig {
  pub initial_difficulty: u32,
  pub seconds_between_blocks: u64,
  pub difficulty_adjustment_block_count: u64,
  pub difficulty_tuning_block_count: u64,
  pub update_recv_timeout: Duration,
}

impl Default for PowConfig {
  fn default() -> Self {
    Self {
      initial_difficulty: INITIAL_DIFFICULTY,
      seconds_between_blocks: SECONDS_BETWEEN_BLOCKS,
      difficulty_adjustment_block_count: DIFFICULTY_ADJUSTMENT_BLOCK_COUNT,
      difficulty_tuning_block_count: DIFFICULTY_TUNING_BLOCK_COUNT,
      update_recv_timeout: UPDATE_RECV_TIMEOUT,
    }
  }
}

impl PowConfig {
  pub fn new() -> Self {
    Self::default()
  }

  fn consensus_chain_settings() -> Vec<String> {
    vec![
      conf_key!("seconds_between_blocks").to_string(),
      conf_key!("difficulty_adjustment_block_count").to_string(),
      conf_key!("difficulty_tuning_block_count").to_string(),
      conf_key!("initial_difficulty").to_string(),
    ]
  }

  pub fn consensus_settings_view(
    service: &mut PowService,
    block_id: BlockId,
  ) -> Result<Self, Error> {
    let keys = Self::consensus_chain_settings();
    let settings: HashMap<String, String> = service.get_settings(block_id, keys)?;
    let mut out = Self::default();

    #[allow(unused_macros)]
    macro_rules! insert_settings {
      ($out:ident,$map:ident,$($settings_literals:ident),*) => {
        $(
        if let Some(e) = $map.get(conf_key!(stringify!($settings_literals))).and_then(|s| s.parse().ok())
        {
          $out.$settings_literals = e;
        }
        )*
      };
    }

    insert_settings!(
      out,
      settings,
      seconds_between_blocks,
      difficulty_adjustment_block_count,
      difficulty_tuning_block_count,
      initial_difficulty
    );

    Ok(out)
  }

  pub fn load(&mut self, service: &mut PowService, block_id: BlockId) -> Result<(), Error> {
    let keys = Self::consensus_chain_settings();

    let settings: HashMap<String, String> = service.get_settings(block_id, keys)?;
    let mut changes: bool = false;

    if let Some(value) = get_setting(conf_key!("seconds_between_blocks"), &settings) {
      if self.seconds_between_blocks != value {
        self.seconds_between_blocks = value;
        changes = true;
      }
    }

    if let Some(value) = get_setting(conf_key!("difficulty_adjustment_block_count"), &settings) {
      if self.difficulty_adjustment_block_count != value {
        self.difficulty_adjustment_block_count = value;
        changes = true;
      }
    }

    if let Some(value) = get_setting(conf_key!("difficulty_tuning_block_count"), &settings) {
      if self.difficulty_tuning_block_count != value {
        self.difficulty_tuning_block_count = value;
        changes = true;
      }
    }

    if let Some(value) = get_setting(conf_key!("initial_difficulty"), &settings) {
      if self.initial_difficulty != value {
        self.initial_difficulty = value;
        changes = true;
      }
    }

    if changes {
      trace!("PoW Config = {:?}", self);
    }

    Ok(())
  }
}

fn get_setting<T: FromStr>(key: &str, settings: &HashMap<String, String>) -> Option<T> {
  settings.get(key).and_then(|string| string.parse().ok())
}
