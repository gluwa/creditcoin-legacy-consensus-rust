use crate::consensus::engine::Update;
use crate::primitives::AtomicFlag;

use crate::futures::event_result::EventResult;
use crate::futures::*;
use crate::node::EventPublishResult;
use futures::pin_mut;
use futures::{future::Fuse, select, FutureExt};

use crate::node::PowNode;

pub struct UpdateStream {
  updates: Receiver<Update>,
  node: PowNode,
  publishing_flag: AtomicFlag,
  new_chainhead_flag: AtomicFlag,
  time_til_publishing: Duration,
}

impl UpdateStream {
  pub fn new(updates: Receiver<Update>, node: PowNode, time_til_publishing: Duration) -> Self {
    let publishing_flag = Arc::new(AtomicBool::new(false));
    let new_chainhead_flag = Arc::new(AtomicBool::new(false));
    #[cfg(feature = "test-futures")]
    let time_til_publishing = Duration::from_secs(time_til_publishing.as_secs() / 60 * 2);
    Self {
      updates,
      node,
      publishing_flag,
      new_chainhead_flag,
      time_til_publishing,
    }
  }

  fn toggle_off_reactor(flag: AtomicFlag) -> impl futures::Future<Output = ()> {
    async move {
      while flag.load(Ordering::Acquire) {
        sleep(Duration::from_millis(10)).await
      }
    }
  }

  fn toggle_on_reactor(flag: AtomicFlag) -> impl futures::Future<Output = ()> {
    async move {
      while !flag.load(Ordering::Acquire) {
        sleep(Duration::from_millis(10)).await
      }
    }
  }

  /*
  TODO
  When new_chainhead:
    reset timer.
    if new_chainhead is not ours:
      try fast-publish. abstract cancel block logic, insert cancel() if fast_publish fails.
  */
  pub async fn update_loop(mut self) {
    let publishing_flag = self.publishing_flag.clone();
    let time = self.time_til_publishing;
    let commit_flag = self.new_chainhead_flag.clone();
    //publishing timer
    let scheduler =
      { PublishSchedulerFuture::schedule_publishing(publishing_flag.clone(), time).fuse() };

    //update calls from the validator
    let updater = async move {
      while let EventResult::Continue = self.update_call().await {}
      EventResult::Shutdown
    }
    .fuse();

    //publishing resetter
    let schedule_timer = Fuse::terminated();
    //commit listener
    let commiter = UpdateStream::toggle_on_reactor(commit_flag.clone()).fuse();

    pin_mut!(scheduler, schedule_timer, updater, commiter);

    //Schedule a publishing timer, when the timer yields, toggle on a flag and spawn a task that waits for the flag to turn off.
    //Schedule a commit reactor, when the flag turns on, force-reset the publishing timer.
    loop {
      select! {
        // timer
        () = scheduler=>{
          //Publishing time, schedule reset.
            schedule_timer.set(UpdateStream::toggle_off_reactor(publishing_flag.clone()).fuse());
        },
        () = schedule_timer =>{
          scheduler.set( PublishSchedulerFuture::schedule_publishing(publishing_flag.clone(), time).fuse());
        },
        () = commiter =>{
          //reset publisher timer
            scheduler.set( PublishSchedulerFuture::schedule_publishing(publishing_flag.clone(), time).fuse());
            schedule_timer.set(Fuse::terminated());
          //reschedule the reactor;
            commit_flag.clone().store(false,Ordering::Release);
            commiter.set(UpdateStream::toggle_on_reactor(commit_flag.clone()).fuse());
          //force publish TODO?

        },
        _ = updater => break,
        complete =>{}
      }
    }
  }
}

impl UpdateStream {
  async fn update_call(&mut self) -> EventResult {
    if self.publishing_flag.load(Ordering::Acquire) {
      match self.node.try_publish() {
        Ok(EventPublishResult::Published) => {
          trace!("Resetting publishing flag");
          #[cfg(feature = "test-futures")]
          println!("Resetting publishing flag");
          self.publishing_flag.store(false, Ordering::Release);
        }
        Ok(..) => {}
        Err(e) => {
          warn!(
            "Publishing Error {}. Consensus event handler is stopping.",
            e
          );
          return EventResult::Shutdown;
        }
      }
    }

    match self.updates.try_recv() {
      Ok(update) => {
        trace!("Incoming update {:?}", update);
        match self.node.handle_update(update) {
          Ok(true) => EventResult::Continue,
          Ok(false) => EventResult::Shutdown,
          Err(error) => {
            error!("Update Error: {}", error);
            EventResult::Continue
          }
        }
      }
      Err(TryRecvError::Disconnected) => {
        error!("Disconnected from validator");
        EventResult::Shutdown
      }
      Err(TryRecvError::Empty) => {
        sleep(Duration::from_millis(10)).await;
        EventResult::Continue
      }
    }
  }
}

#[cfg(all(test, feature = "test-futures"))]
///Don't forget to turn the "test-futures" flag to run these tests
/// e.g cargo test --features "test-futures" --package ccconsensus --lib -- futures::update_stream::tests --nocapture
mod tests {
  use super::*;
  use crate::block::Block;
  use crate::engine::PowEngine;
  use crate::node::tests::MockService;
  use crate::node::tests::{Service, StartupState};
  use sawtooth_sdk::consensus::engine::{Engine, PeerInfo};
  use std::boxed::Box;
  use std::sync::mpsc::channel;
  use std::thread;

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

  #[test]
  fn test_update_stream_future() {
    let (sx, rx) = channel::<Update>();
    let t = Update::BlockCommit(vec![]);
    let _ = sx.send(t);
    let t = Update::BlockInvalid(vec![]);
    let _ = sx.send(t);
    let t = Update::BlockValid(vec![]);
    let _ = sx.send(t);

    thread::spawn(move || {
      thread::sleep(Duration::from_secs(5));
      let t = Update::Shutdown;
      let _ = sx.send(t);
    });

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
