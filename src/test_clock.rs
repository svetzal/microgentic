//! Task-local manual time for boundary tests; never included in a binary.
use std::{
    future::{Future, poll_fn},
    sync::{Arc, Mutex},
    task::{Poll, Waker},
    time::Duration,
};
#[derive(Default)]
pub(crate) struct Clock {
    state: Mutex<State>,
    registered: tokio::sync::Notify,
}
#[derive(Default)]
struct State {
    now: Duration,
    waiters: Vec<Waker>,
    requested: Vec<Duration>,
}
tokio::task_local! { pub(crate) static ACTIVE: Arc<Clock>; }
impl Clock {
    pub(crate) fn advance(&self, delta: Duration) {
        let mut state = self.state.lock().unwrap();
        state.now += delta;
        for waker in state.waiters.drain(..) {
            waker.wake();
        }
    }
    pub(crate) async fn wait_for(&self, duration: Duration) {
        loop {
            let notified = self.registered.notified();
            if self.requested().contains(&duration) {
                return;
            }
            notified.await;
        }
    }
    pub(crate) fn requested(&self) -> Vec<Duration> {
        self.state.lock().unwrap().requested.clone()
    }
}
#[derive(Debug)]
pub(crate) struct Elapsed;
impl std::fmt::Display for Elapsed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("deadline has elapsed")
    }
}
impl std::error::Error for Elapsed {}
pub(crate) async fn timeout<F: Future>(
    duration: Duration,
    future: F,
) -> Result<F::Output, Elapsed> {
    let Ok(clock) = ACTIVE.try_with(Arc::clone) else {
        return tokio::time::timeout(duration, future)
            .await
            .map_err(|_| Elapsed);
    };
    let deadline = {
        let mut state = clock.state.lock().unwrap();
        state.requested.push(duration);
        clock.registered.notify_one();
        state.now + duration
    };
    let timer = poll_fn(|cx| {
        let mut state = clock.state.lock().unwrap();
        if state.now >= deadline {
            Poll::Ready(())
        } else {
            state.waiters.push(cx.waker().clone());
            Poll::Pending
        }
    });
    tokio::pin!(future, timer);
    // Tokio's timeout checks the operation before its timer.
    tokio::select! { biased; result = &mut future => Ok(result), _ = &mut timer => Err(Elapsed) }
}
