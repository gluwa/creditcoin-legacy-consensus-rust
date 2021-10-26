/*!
A future that will flag the publishing time using an atomic construct.
*/
use crate::futures::*;
#[cfg(feature = "test-futures")]
use std::println as trace;

///schedule a task to mark publishing time, after publishing, reschedule a new publishing future.
pub struct PublishSchedulerFuture {
  flag: AtomicFlag,
  sleep: Pin<Box<Sleep>>,
}

impl PublishSchedulerFuture {
  pub fn schedule_publishing(flag: AtomicFlag, time_til_publishing: Duration) -> Self {
    PublishSchedulerFuture {
      flag,
      sleep: Box::pin(sleep(time_til_publishing)),
    }
  }
}

///A future that flags publishing time through an atomic bool.
///After publishing don't forget to set the flag to false and schedule a new publishing future.
impl Future for PublishSchedulerFuture {
  type Output = ();

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let PublishSchedulerFuture { flag, sleep } = self.get_mut();
    if Pin::new(sleep).poll(cx).is_pending() {
      return Poll::Pending;
    }

    #[cfg(feature = "test-futures")]
    trace!("Publishing time!");
    //set atomic
    flag.store(true, Ordering::Release);
    Poll::Ready(())
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;

  #[test]
  fn publishing_future() {
    let flag = Arc::new(AtomicBool::new(false));
    let time_til_publishing = Duration::from_millis(1000);
    let rt = runtime::Runtime::new().unwrap();
    {
      let flag = flag.clone();
      let fut = async move {
        PublishSchedulerFuture::schedule_publishing(flag, time_til_publishing).await;
      };
      rt.block_on(fut);
    }
    assert!(flag.load(std::sync::atomic::Ordering::Acquire))
  }
}
