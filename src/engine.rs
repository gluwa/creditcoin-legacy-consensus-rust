use sawtooth_sdk::consensus::{
  engine::{Engine, Error, StartupState, Update},
  service::Service,
};
use std::sync::mpsc::Receiver;

use crate::{
  futures::{Builder, Runtime, UpdateStream},
  node::{PowConfig, PowNode},
  Duration,
};

const ENGINE_NAME: &str = "PoW";
const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct PowEngine {
  config: Option<PowConfig>,
}

impl PowEngine {
  pub const fn new() -> Self {
    Self { config: None }
  }

  pub const fn with_config(config: PowConfig) -> Self {
    Self {
      config: Some(config),
    }
  }
}

impl Engine for PowEngine {
  fn start(
    &mut self,
    updates: Receiver<Update>,
    service: Box<dyn Service>,
    startup: StartupState,
  ) -> Result<(), Error> {
    // Create a new PoW node, using the engine config if one exists.
    let node: PowNode = self.init_node(service, startup)?;

    let rt = PowEngine::build_rt();

    {
      let time_til_publishing = Duration::from_secs(node.config.seconds_between_blocks);
      let stream = UpdateStream::new(updates, node, time_til_publishing);

      rt.block_on(stream.update_loop());
    }

    Ok(())
  }

  fn name(&self) -> String {
    ENGINE_NAME.to_string()
  }

  fn version(&self) -> String {
    let idx = ENGINE_VERSION.rfind('.').expect("PATCH");
    ENGINE_VERSION[0..idx].into()
  }

  fn additional_protocols(&self) -> Vec<(String, String)> {
    Vec::new()
  }
}

impl PowEngine {
  fn init_node(
    &mut self,
    service: Box<dyn Service>,
    startup: StartupState,
  ) -> Result<PowNode, Error> {
    // Create a new PoW node, using the engine config if one exists.
    let mut node: PowNode = match self.config.take() {
      Some(config) => PowNode::with_config(config, service),
      None => PowNode::new(service),
    };

    // Initialize the PoW based on the current startup state received from the
    // validator - an error here is considered fatal and prevents startup.
    //
    // Note: Errors from this call don't propagate due to conflicting types,
    // this means we need to handle them explicity.
    if let Err(error) = node.initialize(startup) {
      error!("Init Error: {}", error);
      Err(error)
    } else {
      Ok(node)
    }
  }

  fn build_rt() -> Runtime {
    Builder::new_multi_thread()
      .worker_threads(1)
      .enable_all()
      .thread_name("engine-runtime")
      .build()
      .expect("Async runtime")
  }
}
