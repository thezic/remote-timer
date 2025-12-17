use std::time::{Duration, Instant};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::time::{interval, sleep, Interval};
use tokio_stream::Stream;

/// Abstraction for time operations, enabling deterministic testing with mock implementations.
///
/// This trait provides an interface for time-related operations that can be swapped between
/// real system time (production) and mock controllable time (testing). This enables:
/// - Fast, deterministic tests that don't depend on real time passage
/// - Consistent behavior across different systems and loads
/// - Testing of timeout and interval behavior without waiting
///
/// # Implementations
///
/// - [`SystemTime`]: Uses real system time via tokio. Suitable for production.
/// - [`mock::MockTime`]: Allows explicit control of time progression. Suitable for testing.
///
/// # Contract
///
/// Implementations must satisfy these requirements:
///
/// ## Thread Safety
/// All implementations must be `Send + Sync + Clone + 'static`. Cloning should be cheap
/// (typically cloning an `Arc` internally).
///
/// ## Interval Behavior
/// The `interval()` method creates a stream that yields at regular intervals. Important behaviors:
/// - **First tick is immediate**: The first call to `poll_next` on the stream returns `Ready`
/// - **Subsequent ticks**: Occur at `duration` intervals from the previous tick
/// - **Polling from different contexts**: The stream must handle being polled from different
///   async task contexts (wakers must be updated correctly)
///
/// ## Sleep Behavior
/// The `sleep()` method returns a future that completes after the specified duration:
/// - The duration is measured from when `sleep()` is called, not from when the future is polled
/// - Polling from different contexts must work correctly (wakers must be updated)
///
/// # Examples
///
/// ```
/// use timer_api::time::{TimeSource, SystemTime};
/// use std::time::Duration;
///
/// # async fn example() {
/// // Production usage with real time
/// let time = SystemTime;
/// let now = time.now();
///
/// // Sleep for 100ms
/// time.sleep(Duration::from_millis(100)).await;
///
/// // Create an interval that ticks every 100ms
/// let mut interval = time.interval(Duration::from_millis(100));
/// # }
/// ```
pub trait TimeSource: Send + Sync + Clone + 'static {
    /// Stream type that yields `()` at regular intervals.
    type IntervalStream: Stream<Item = ()> + Send + Unpin;

    /// Future type that completes after a specified duration.
    type SleepFuture: Future<Output = ()> + Send;

    /// Returns the current instant in time.
    ///
    /// For [`SystemTime`], this returns `Instant::now()`.
    /// For [`mock::MockTime`], this returns the current simulated time.
    fn now(&self) -> Instant;

    /// Returns a future that completes after the specified duration.
    ///
    /// The duration is measured from when this method is called, not from when
    /// the returned future is first polled.
    fn sleep(&self, duration: Duration) -> Self::SleepFuture;

    /// Creates a stream that yields `()` at regular intervals.
    ///
    /// # First Tick
    ///
    /// The first tick occurs **immediately** when the stream is first polled.
    /// Subsequent ticks occur at `duration` intervals.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut interval = time_source.interval(Duration::from_millis(100));
    /// interval.next().await;  // Returns immediately
    /// interval.next().await;  // Waits ~100ms (or until time advances in mock)
    /// interval.next().await;  // Waits another ~100ms
    /// ```
    fn interval(&self, duration: Duration) -> Self::IntervalStream;
}

#[derive(Clone, Debug)]
pub struct SystemTime;

impl TimeSource for SystemTime {
    type IntervalStream = SystemIntervalStream;
    type SleepFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) -> Self::SleepFuture {
        Box::pin(sleep(duration))
    }

    fn interval(&self, duration: Duration) -> Self::IntervalStream {
        let mut interval_inner = interval(duration);
        interval_inner.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        SystemIntervalStream {
            interval: interval_inner,
        }
    }
}

pub struct SystemIntervalStream {
    interval: Interval,
}

impl Stream for SystemIntervalStream {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.interval.poll_tick(cx) {
            Poll::Ready(_) => Poll::Ready(Some(())),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::{Arc, Mutex, Weak};
    use std::task::Waker;

    #[derive(Clone)]
    pub struct MockTime {
        state: Arc<Mutex<MockTimeState>>,
    }

    struct MockTimeState {
        current_time: Instant,
        pending_sleeps: Vec<(usize, Instant, Waker)>, // (sleep_id, wake_time, waker)
        intervals: Vec<Weak<Mutex<MockIntervalState>>>,
        next_sleep_id: usize,
    }

    struct MockIntervalState {
        next_tick: Instant,
        interval_duration: Duration,
        pending_waker: Option<Waker>,
    }

    impl MockTime {
        pub fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(MockTimeState {
                    current_time: Instant::now(),
                    pending_sleeps: Vec::new(),
                    intervals: Vec::new(),
                    next_sleep_id: 0,
                })),
            }
        }

        pub fn advance(&self, duration: Duration) {
            let mut state = self.state.lock().unwrap();
            state.current_time += duration;

            let mut completed_sleeps = Vec::new();
            for (i, (sleep_id, wake_time, waker)) in state.pending_sleeps.iter().enumerate() {
                if *wake_time <= state.current_time {
                    completed_sleeps.push((i, *sleep_id, waker.clone()));
                }
            }

            for (index, _sleep_id, waker) in completed_sleeps.into_iter().rev() {
                state.pending_sleeps.remove(index);
                waker.wake();
            }

            let current_time = state.current_time;
            state.intervals.retain(|weak_interval| {
                if let Some(interval_state) = weak_interval.upgrade() {
                    let mut interval = interval_state.lock().unwrap();
                    if interval.next_tick <= current_time {
                        if let Some(waker) = interval.pending_waker.take() {
                            waker.wake();
                        }
                    }
                    true
                } else {
                    false
                }
            });
        }

        pub fn set_time(&self, time: Instant) {
            let mut state = self.state.lock().unwrap();
            state.current_time = time;
        }
    }

    impl TimeSource for MockTime {
        type IntervalStream = MockIntervalStream;
        type SleepFuture = MockSleep;

        fn now(&self) -> Instant {
            self.state.lock().unwrap().current_time
        }

        fn sleep(&self, duration: Duration) -> Self::SleepFuture {
            let wake_time = self.now() + duration;
            let sleep_id = {
                let mut state = self.state.lock().unwrap();
                let id = state.next_sleep_id;
                state.next_sleep_id += 1;
                id
            };
            MockSleep {
                time_source: self.clone(),
                wake_time,
                completed: false,
                sleep_id,
            }
        }

        fn interval(&self, duration: Duration) -> Self::IntervalStream {
            let interval_state = Arc::new(Mutex::new(MockIntervalState {
                next_tick: self.now(),
                interval_duration: duration,
                pending_waker: None,
            }));

            self.state.lock().unwrap().intervals.push(Arc::downgrade(&interval_state));

            MockIntervalStream {
                time_source: self.clone(),
                state: interval_state,
            }
        }
    }

    pub struct MockSleep {
        time_source: MockTime,
        wake_time: Instant,
        completed: bool,
        sleep_id: usize,
    }

    impl Future for MockSleep {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.completed {
                return Poll::Ready(());
            }

            let current_time = self.time_source.now();
            if current_time >= self.wake_time {
                self.completed = true;
                Poll::Ready(())
            } else {
                // Always update the waker for this sleep to handle context changes
                let mut state = self.time_source.state.lock().unwrap();

                // Find existing entry for this sleep_id
                if let Some(entry) = state.pending_sleeps.iter_mut().find(|(id, _, _)| *id == self.sleep_id) {
                    // Update the waker (wake_time should be the same, but update it anyway)
                    *entry = (self.sleep_id, self.wake_time, cx.waker().clone());
                } else {
                    // First registration for this sleep_id
                    state.pending_sleeps.push((self.sleep_id, self.wake_time, cx.waker().clone()));
                }

                Poll::Pending
            }
        }
    }

    pub struct MockIntervalStream {
        time_source: MockTime,
        state: Arc<Mutex<MockIntervalState>>,
    }

    impl Stream for MockIntervalStream {
        type Item = ();

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let current_time = self.time_source.now();
            let mut state = self.state.lock().unwrap();

            if current_time >= state.next_tick {
                let duration = state.interval_duration;
                state.next_tick += duration;
                // Clear any pending waker since we're returning Ready
                state.pending_waker = None;
                Poll::Ready(Some(()))
            } else {
                // Update waker for when time advances
                state.pending_waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn system_time_now_works() {
        let time_source = SystemTime;
        let now1 = time_source.now();
        tokio::time::sleep(Duration::from_millis(1)).await;
        let now2 = time_source.now();
        assert!(now2 > now1);
    }

    #[tokio::test]
    async fn system_time_sleep_works() {
        let time_source = SystemTime;
        let start = Instant::now();
        time_source.sleep(Duration::from_millis(10)).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(8)); // Allow some tolerance
    }

    #[tokio::test]
    async fn system_time_interval_works() {
        let time_source = SystemTime;
        let mut interval_stream = time_source.interval(Duration::from_millis(5));

        interval_stream.next().await;

        let start = Instant::now();
        interval_stream.next().await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(3));
    }

    #[tokio::test]
    async fn mock_time_now_returns_set_time() {
        let mock_time = mock::MockTime::new();
        let expected_time = Instant::now() + Duration::from_secs(100);
        mock_time.set_time(expected_time);
        assert_eq!(mock_time.now(), expected_time);
    }

    #[tokio::test]
    async fn mock_time_advance_changes_now() {
        let mock_time = mock::MockTime::new();
        let initial_time = mock_time.now();
        mock_time.advance(Duration::from_secs(10));
        assert_eq!(mock_time.now(), initial_time + Duration::from_secs(10));
    }

    #[tokio::test]
    async fn mock_time_sleep_completes_after_advance() {
        let mock_time = mock::MockTime::new();
        
        // Test that sleep doesn't complete before time advance
        let sleep_future = mock_time.sleep(Duration::from_millis(100));
        
        // Start the sleep task
        let handle = tokio::spawn(async move {
            sleep_future.await;
            "completed"
        });

        // Brief sleep for async task coordination - allows spawned task to be scheduled
        // by the tokio runtime and begin polling the MockSleep future
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(!handle.is_finished());
        
        // Advance time to complete the sleep
        mock_time.advance(Duration::from_millis(100));
        
        // Now it should complete
        let result = handle.await.unwrap();
        assert_eq!(result, "completed");
    }

    #[tokio::test]
    async fn mock_time_interval_works() {
        let mock_time = mock::MockTime::new();
        let mut interval_stream = mock_time.interval(Duration::from_millis(100));

        let mut tick_count = 0;
        let handle = tokio::spawn(async move {
            while interval_stream.next().await.is_some() {
                tick_count += 1;
                if tick_count >= 3 {
                    break;
                }
            }
            tick_count
        });

        // Allow spawned task to be scheduled and poll the interval
        tokio::time::sleep(Duration::from_millis(1)).await;

        mock_time.advance(Duration::from_millis(100));
        // Allow interval to process the time advance and wake
        tokio::time::sleep(Duration::from_millis(1)).await;

        mock_time.advance(Duration::from_millis(100));
        // Allow interval to process the time advance and wake
        tokio::time::sleep(Duration::from_millis(1)).await;

        let result = handle.await.unwrap();
        assert_eq!(result, 3);
    }

    #[tokio::test]
    async fn mock_time_interval_multiple_ticks_from_single_advance() {
        let mock_time = mock::MockTime::new();
        let mut interval_stream = mock_time.interval(Duration::from_millis(100));

        // First tick (immediate)
        let first_tick = interval_stream.next().await;
        assert!(first_tick.is_some(), "Should get immediate first tick");

        // Advance by 1000ms and collect all available ticks
        mock_time.advance(Duration::from_millis(1000));

        // Poll repeatedly to collect all ticks that should be ready
        let mut tick_count = 0;
        for _ in 0..15 {  // Try up to 15 times (should get 10 ticks)
            tokio::select! {
                result = interval_stream.next() => {
                    if result.is_some() {
                        tick_count += 1;
                    } else {
                        break;  // Stream ended
                    }
                }
                // Use real sleep as a timeout - if we have to wait, we've gotten all available ticks
                _ = tokio::time::sleep(Duration::from_millis(1)) => {
                    break;
                }
            }
        }

        // We should get approximately 10 ticks (1000ms / 100ms)
        // Allow some tolerance since timing can be tricky
        assert!(
            tick_count >= 9 && tick_count <= 11,
            "Expected 9-11 ticks after advancing 1000ms with 100ms interval, got {}",
            tick_count
        );
    }
}