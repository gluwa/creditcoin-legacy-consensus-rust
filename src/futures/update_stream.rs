use crate::consensus::engine::Update;
use crate::primitives::AtomicFlag;

use crate::futures::*;
use crate::node::EventPublishResult;
use futures::pin_mut;
use futures::{select, FutureExt};

#[cfg(feature = "test-futures")]
use std::sync::atomic::AtomicUsize;

use crate::node::PowNode;

pub struct UpdateStream {
  updates: Receiver<Update>,
  node: PowNode,
  publishing_flag: AtomicFlag,
  new_chainhead_flag: AtomicFlag,
  time_til_publishing: Duration,
}

#[cfg(feature = "test-futures")]
static COUNT_COMMITTER: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "test-futures")]
use std::println as trace;

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

  #[allow(dead_code)]
  async fn toggle_off_reactor(flag: AtomicFlag) {
    while flag.load(Ordering::Acquire) {
      sleep(Duration::from_millis(10)).await
    }
  }

  async fn toggle_on_reactor(flag: AtomicFlag) {
    while !flag.load(Ordering::Acquire) {
      sleep(Duration::from_millis(10)).await
    }
  }

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

    //commit listener
    let committer = UpdateStream::toggle_on_reactor(commit_flag.clone()).fuse();

    pin_mut!(scheduler, updater, committer);

    //Schedule a publishing timer, when the timer yields, toggle on the publishing flag.
    //Schedule a commit reactor, when the flag turns on, start a publishing timer.
    loop {
      select! {
        // timer
        () = scheduler=>{},
        //new block commited as the new chain head
        () = committer =>{
          #[cfg(feature = "test-futures")]
          trace!("Commiter fut");
          commit_flag.clone().store(false, Ordering::SeqCst);
          //publishing timer kicked in but publishing was unsuccessful, and a new chain head arrived.
          publishing_flag.clone().store(false,Ordering::Release);
          //reset publisher timer
          scheduler.set(PublishSchedulerFuture::schedule_publishing(publishing_flag.clone(), time).fuse());
          #[cfg(feature = "test-futures")]
          COUNT_COMMITTER.fetch_add(1usize, Ordering::Relaxed);
          committer.set(UpdateStream::toggle_on_reactor(commit_flag.clone()).fuse());
        },
        _ = updater => break,
        complete =>{}
      }
    }
  }
}

#[cfg(feature = "test-futures")]
static COUNT_PUBLISHED: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "test-futures")]
static COUNT_UPDATED: AtomicUsize = AtomicUsize::new(0);

impl UpdateStream {
  async fn update_call(&mut self) -> EventResult {
    if self.publishing_flag.load(Ordering::Acquire) {
      match self.node.try_publish() {
        Ok(EventPublishResult::Published) => {
          #[cfg(feature = "test-futures")]
          {
            trace!("Resetting publishing flag");
            COUNT_PUBLISHED.fetch_add(1usize, Ordering::Relaxed);
          }
          self.publishing_flag.store(false, Ordering::Release);
        }
        Ok(EventPublishResult::Pending) => {}
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
        #[cfg(feature = "test-futures")]
        COUNT_UPDATED.fetch_add(1usize, Ordering::Relaxed);
        match self.node.handle_update(update) {
          Ok(EventResult::Continue) => EventResult::Continue,
          Ok(EventResult::Shutdown) => EventResult::Shutdown,
          Ok(EventResult::Restart(eager_publish)) => {
            #[cfg(feature = "test-futures")]
            trace!("restart publishing");
            self.new_chainhead_flag.store(true, Ordering::SeqCst);
            if eager_publish {
              self.publishing_flag.store(false, Ordering::Release)
            }
            EventResult::Continue
          }
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

  macro_rules! one_of_each_update {
    ($sx:ident, $update: ident) => {
      let _ = $sx.send(Update::$update(vec![]));
    };
    ($sx:ident, $update: ident, $($rest:ident),+) => {
      let _ = $sx.send(Update::$update(vec![]));
      one_of_each_update!($sx, $($rest),+)
      };
  }

  macro_rules! simple_engine_loop {
    ($sx:ident, $rx:ident, $message_pattern:expr, $($updates:ident),* ) => {
      one_of_each_update!($sx, $($updates),*);

      thread::spawn(move || $message_pattern);

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
        .start($rx, service as Box<dyn Service>, startup_state)
        .expect("start");
    };
  }

  #[test]
  ///The event loop processed `COUNT_UPDATED` events.
  /// Will fail if run in bulk due to the shared static update event counter.
  /// use --test-threads 1 instead
  fn singled_out_update_events() {
    let (sx, rx) = channel::<Update>();
    simple_engine_loop!(
      sx,
      rx,
      {
        let _ = sx.send(Update::Shutdown);
      },
      PeerDisconnected,
      BlockValid,
      BlockInvalid,
      BlockCommit
    );

    assert_eq!(COUNT_UPDATED.load(Ordering::Acquire), 5);
  }

  #[test]
  ///The event loop attempted to publish `COUNT_PUBLISHED` and started the publisher interval `COUNT_COMMITTER` times.
  fn test_publishing_event() {
    let (sx, rx) = channel::<Update>();
    simple_engine_loop!(
      sx,
      rx,
      {
        let _ = sx.send(Update::BlockCommit(vec![]));
        //allow time for another publication
        thread::sleep(Duration::from_secs(3));
        let _ = sx.send(Update::BlockCommit(vec![]));
        //allow time for another publication
        thread::sleep(Duration::from_secs(3));
        let _ = sx.send(Update::Shutdown);
      },
      BlockValid,
      BlockValid,
      BlockValid,
      BlockValid
    );

    assert_eq!(COUNT_PUBLISHED.load(Ordering::Acquire), 2);
    assert_eq!(COUNT_COMMITTER.load(Ordering::Acquire), 2);
  }

  /// Try to publish, leave it incomplete, then on_block_commit, finishes it,
  /// test that the publishing event is properly reset and try_publish is not retried.
  #[allow(dead_code)]
  fn partial_publishing_then_eager_publishing() {}

  ///it is publishing time, publishing doesn't finish, on_block_commit runs, eager_publish also doesn't finish.
  /// test that the publishing timer is properly reset.
  #[allow(dead_code)]
  fn publisher_timer_resets_after_timed_and_eager_publishing_fail() {}

  /// publishing timer will activate just as on_block_commit is being executed.
  /// Unluckily the tasks executed go in the following order,
  /// on_block_commit -> publishing_timer -> updater -> committer
  /// The miner is reset, publishing will be tried, a consensus won't be ready, the committer reesets publishing.
  /// Relies on the consensus not finding a valid solution on time.
  #[allow(dead_code)]
  fn publisher_timer_slightly_ahead_of_committer_event() {}
}
