use std::time::{Duration, Instant};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::time::{interval, sleep, Interval};
use tokio_stream::Stream;

pub trait TimeSource: Send + Sync + Clone + 'static {
    type IntervalStream: Stream<Item = ()> + Send + Unpin;
    type SleepFuture: Future<Output = ()> + Send;

    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration) -> Self::SleepFuture;
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
        SystemIntervalStream {
            interval: interval(duration),
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
    use std::sync::{Arc, Mutex};
    use std::task::Waker;

    #[derive(Clone)]
    pub struct MockTime {
        state: Arc<Mutex<MockTimeState>>,
    }

    struct MockTimeState {
        current_time: Instant,
        pending_sleeps: Vec<(Instant, Waker)>,
        intervals: Vec<Arc<Mutex<MockIntervalState>>>,
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
                })),
            }
        }

        pub fn advance(&self, duration: Duration) {
            let mut state = self.state.lock().unwrap();
            state.current_time += duration;

            // Wake up completed sleeps
            let mut completed_sleeps = Vec::new();
            for (i, (wake_time, waker)) in state.pending_sleeps.iter().enumerate() {
                if *wake_time <= state.current_time {
                    completed_sleeps.push((i, waker.clone()));
                }
            }

            // Remove completed sleeps in reverse order to maintain indices
            for (index, waker) in completed_sleeps.into_iter().rev() {
                state.pending_sleeps.remove(index);
                waker.wake();
            }

            // Check intervals
            for interval_state in &state.intervals {
                let mut interval = interval_state.lock().unwrap();
                while interval.next_tick <= state.current_time {
                    if let Some(waker) = interval.pending_waker.take() {
                        waker.wake();
                    }
                    let duration = interval.interval_duration;
                    interval.next_tick += duration;
                }
            }
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
            MockSleep {
                time_source: self.clone(),
                wake_time,
                completed: false,
            }
        }

        fn interval(&self, duration: Duration) -> Self::IntervalStream {
            let interval_state = Arc::new(Mutex::new(MockIntervalState {
                next_tick: self.now() + duration,
                interval_duration: duration,
                pending_waker: None,
            }));

            self.state.lock().unwrap().intervals.push(interval_state.clone());

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
                self.time_source.state.lock().unwrap()
                    .pending_sleeps.push((self.wake_time, cx.waker().clone()));
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
                Poll::Ready(Some(()))
            } else {
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
        
        let start = Instant::now();
        interval_stream.next().await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(3)); // Allow some tolerance
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
        
        // Give it a moment, should not complete
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
        
        // Create a task to listen for interval ticks
        let mut tick_count = 0;
        let handle = tokio::spawn(async move {
            while let Some(_) = interval_stream.next().await {
                tick_count += 1;
                if tick_count >= 2 {
                    break;
                }
            }
            tick_count
        });
        
        // Give it a moment
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(!handle.is_finished());
        
        // Advance time to trigger first interval
        mock_time.advance(Duration::from_millis(100));
        tokio::time::sleep(Duration::from_millis(1)).await;
        
        // Advance time to trigger second interval  
        mock_time.advance(Duration::from_millis(100));
        
        // Now it should complete
        let result = handle.await.unwrap();
        assert_eq!(result, 2);
    }
}