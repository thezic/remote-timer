#[cfg(test)]
pub mod test_utils {
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc;
    use tokio::time::timeout;
    
    use crate::config::{HandlerConfig, ServerConfig, TimerConfig};

    /// Returns a tuple of testing configurations for timer, handler, and server.
    ///
    /// These configurations use faster intervals suitable for testing:
    /// - Timer ticks every 1ms (vs 100ms in production)
    /// - Heartbeats every 10ms (vs 5s in production)
    /// - Cleanup happens after 100ms (vs 30 minutes in production)
    pub fn fast_configs() -> (TimerConfig, HandlerConfig, ServerConfig) {
        (
            TimerConfig::for_testing(),
            HandlerConfig::for_testing(),
            ServerConfig::for_testing(),
        )
    }

    /// Creates a pair of unbounded MPSC channels for testing.
    ///
    /// Returns a tuple of (sender, receiver) that can be used in tests
    /// to simulate async message passing between components.
    pub fn create_test_channels<T>() -> (mpsc::UnboundedSender<T>, mpsc::UnboundedReceiver<T>) {
        mpsc::unbounded_channel()
    }

    /// Polls a synchronous condition function until it returns true or times out.
    ///
    /// # Arguments
    /// * `condition` - A mutable closure that returns true when the desired condition is met
    /// * `timeout_duration` - Maximum duration to wait before timing out
    ///
    /// # Returns
    /// * `Ok(())` if the condition becomes true within the timeout
    /// * `Err(&'static str)` if the timeout is reached before the condition is met
    ///
    /// The condition is polled approximately every 1ms.
    pub async fn wait_for_condition<F>(
        mut condition: F,
        timeout_duration: Duration,
    ) -> Result<(), &'static str>
    where
        F: FnMut() -> bool,
    {
        let start = Instant::now();

        while start.elapsed() < timeout_duration {
            if condition() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }

        Err("Condition not met within timeout")
    }

    /// Polls an asynchronous condition function until it returns true or times out.
    ///
    /// # Arguments
    /// * `condition` - A mutable closure that returns a Future<Output = bool>
    /// * `timeout_duration` - Maximum duration to wait before timing out
    ///
    /// # Returns
    /// * `Ok(())` if the condition becomes true within the timeout
    /// * `Err(&'static str)` if the timeout is reached before the condition is met
    ///
    /// Similar to [`wait_for_condition`] but for async conditions. Polls approximately every 1ms.
    pub async fn wait_for_async_condition<F, Fut>(
        mut condition: F,
        timeout_duration: Duration,
    ) -> Result<(), &'static str>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let result = timeout(timeout_duration, async {
            loop {
                if condition().await {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }).await;

        match result {
            Ok(_) => Ok(()),
            Err(_) => Err("Async condition not met within timeout"),
        }
    }

    /// Asserts that a synchronous operation completes within an expected duration with tolerance.
    ///
    /// # Arguments
    /// * `operation` - A closure containing the operation to time
    /// * `expected_duration` - The expected duration for the operation to complete
    /// * `tolerance` - Allowed variance above or below the expected duration
    ///
    /// # Panics
    /// Panics if the operation duration falls outside the range:
    /// `[expected_duration - tolerance, expected_duration + tolerance]`
    ///
    /// Useful for testing timing-sensitive code while accounting for system variance.
    pub fn assert_within_duration<F>(operation: F, expected_duration: Duration, tolerance: Duration)
    where
        F: FnOnce(),
    {
        let start = Instant::now();
        operation();
        let elapsed = start.elapsed();

        assert!(
            elapsed >= expected_duration.saturating_sub(tolerance) &&
            elapsed <= expected_duration + tolerance,
            "Operation took {:?}, expected {:?} ± {:?}",
            elapsed,
            expected_duration,
            tolerance
        );
    }

    /// Asserts that an asynchronous operation completes within an expected duration with tolerance.
    ///
    /// # Arguments
    /// * `operation` - A closure that returns a Future to time
    /// * `expected_duration` - The expected duration for the operation to complete
    /// * `tolerance` - Allowed variance above or below the expected duration
    ///
    /// # Panics
    /// Panics if the operation duration falls outside the range:
    /// `[expected_duration - tolerance, expected_duration + tolerance]`
    ///
    /// Similar to [`assert_within_duration`] but for async operations.
    pub async fn assert_within_duration_async<F, Fut>(
        operation: F,
        expected_duration: Duration,
        tolerance: Duration,
    )
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let start = Instant::now();
        operation().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed >= expected_duration.saturating_sub(tolerance) &&
            elapsed <= expected_duration + tolerance,
            "Async operation took {:?}, expected {:?} ± {:?}",
            elapsed,
            expected_duration,
            tolerance
        );
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[test]
    fn fast_configs_returns_testing_configs() {
        let (timer_config, handler_config, server_config) = fast_configs();
        
        assert_eq!(timer_config, crate::config::TimerConfig::for_testing());
        assert_eq!(handler_config, crate::config::HandlerConfig::for_testing());
        assert_eq!(server_config, crate::config::ServerConfig::for_testing());
    }

    #[test]
    fn create_test_channels_works() {
        let (tx, mut rx) = create_test_channels::<i32>();
        
        tx.send(42).unwrap();
        let received = rx.try_recv().unwrap();
        assert_eq!(received, 42);
    }

    #[tokio::test]
    async fn wait_for_condition_succeeds_when_condition_met() {
        let mut counter = 0;
        let result = wait_for_condition(
            || {
                counter += 1;
                counter >= 3
            },
            Duration::from_millis(100),
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(counter, 3);
    }

    #[tokio::test]
    async fn wait_for_condition_fails_on_timeout() {
        let result = wait_for_condition(
            || false, // Never true
            Duration::from_millis(10),
        ).await;
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Condition not met within timeout");
    }

    #[tokio::test]
    async fn wait_for_async_condition_works() {
        use std::sync::{Arc, Mutex};
        
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        let result = wait_for_async_condition(
            move || {
                let counter = counter_clone.clone();
                async move {
                    let mut c = counter.lock().unwrap();
                    *c += 1;
                    *c >= 2
                }
            },
            Duration::from_millis(100),
        ).await;
        
        assert!(result.is_ok());
        assert!(*counter.lock().unwrap() >= 2);
    }

    #[test]
    fn assert_within_duration_passes_for_correct_timing() {
        assert_within_duration(
            || std::thread::sleep(Duration::from_millis(10)),
            Duration::from_millis(10),
            Duration::from_millis(5),
        );
    }

    #[tokio::test]
    async fn assert_within_duration_async_passes_for_correct_timing() {
        assert_within_duration_async(
            || async { sleep(Duration::from_millis(10)).await },
            Duration::from_millis(10),
            Duration::from_millis(5),
        ).await;
    }
}