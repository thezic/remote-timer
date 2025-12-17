use std::{
    collections::HashMap,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::config::TimerConfig;
use crate::time::{SystemTime, TimeSource};

#[derive(Debug)]
pub enum Command {
    StartCounter,
    StopCounter,
    SetTime(i32),
    Close,
    Subscribe(Uuid, mpsc::UnboundedSender<TimerMessage>),
    Unsubscribe(Uuid),
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct TimerMessage {
    pub is_running: bool,
    pub current_time: i32,
    pub target_time: i32,
    pub client_count: usize,
}

pub struct Timer<T: TimeSource> {
    time: i32,
    target_time: i32,
    cmd_rx: mpsc::UnboundedReceiver<Command>,
    listeners: HashMap<Uuid, mpsc::UnboundedSender<TimerMessage>>,
    time_source: T,
    config: TimerConfig,
}

#[derive(Clone)]
pub struct TimerHandle {
    cmd_tx: mpsc::UnboundedSender<Command>,
}

impl TimerHandle {
    pub fn start_counter(&self) -> Result<()> {
        Ok(self.cmd_tx.send(Command::StartCounter)?)
    }

    pub fn stop_counter(&self) -> Result<()> {
        Ok(self.cmd_tx.send(Command::StopCounter)?)
    }

    pub fn set_time(&self, time: i32) -> Result<()> {
        Ok(self.cmd_tx.send(Command::SetTime(time))?)
    }
    pub fn close(&self) -> Result<()> {
        Ok(self.cmd_tx.send(Command::Close)?)
    }

    pub fn subscribe(&self, client_id: Uuid) -> Result<mpsc::UnboundedReceiver<TimerMessage>> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.cmd_tx.send(Command::Subscribe(client_id, tx))?;
        Ok(rx)
    }

    pub fn unsubscribe(&self, client_id: Uuid) -> Result<()> {
        Ok(self.cmd_tx.send(Command::Unsubscribe(client_id))?)
    }
}

pub trait TimerFactory: Clone + Send + 'static {
    fn create_timer(&self, config: TimerConfig) -> TimerHandle;
}

#[derive(Clone)]
pub struct RealTimerFactory;

impl TimerFactory for RealTimerFactory {
    fn create_timer(&self, config: TimerConfig) -> TimerHandle {
        let (timer, handle) = Timer::new(SystemTime, config);
        tokio::spawn(timer.run());
        handle
    }
}

impl<T: TimeSource> Timer<T> {
    pub fn new(time_source: T, config: TimerConfig) -> (Self, TimerHandle) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let timer = Self {
            time: 0,
            target_time: 0,
            cmd_rx,
            listeners: HashMap::new(),
            time_source,
            config,
        };
        (timer, TimerHandle { cmd_tx })
    }

    fn broadcast(&mut self, msg: TimerMessage) {
        self.listeners.retain(move |_, tx| tx.send(msg).is_ok());
    }

    /// Processes a single iteration of the timer's event loop.
    ///
    /// This method is primarily exposed for fine-grained testing, allowing tests to control
    /// the execution of the timer one step at a time rather than running the full async loop.
    ///
    /// # Parameters
    ///
    /// - `interval_stream`: The interval stream that produces timer ticks
    /// - `last_tick`: The timestamp of the last tick, updated when the timer is running
    /// - `is_counting`: Whether the timer is currently counting (running)
    ///
    /// # Returns
    ///
    /// Returns `true` if the timer should continue running, `false` if it should stop
    /// (e.g., when a `Close` command is received or the command channel is closed).
    ///
    /// # Example Usage in Tests
    ///
    /// ```ignore
    /// let (timer, handle) = Timer::new(mock_time, config);
    /// let mut interval = mock_time.interval(config.tick_interval);
    /// let mut last_tick = mock_time.now();
    /// let mut is_counting = false;
    ///
    /// // Process one iteration
    /// timer.run_single_iteration(&mut interval, &mut last_tick, &mut is_counting).await;
    /// ```
    pub async fn run_single_iteration(
        &mut self,
        interval_stream: &mut T::IntervalStream,
        last_tick: &mut std::time::Instant,
        is_counting: &mut bool,
    ) -> bool {
        tokio::select! {
            msg = self.cmd_rx.recv() => {
                match msg {
                    Some(Command::Subscribe(client_id, tx)) => {
                        self.listeners.insert(client_id, tx);
                    },
                    Some(Command::StartCounter) => {
                        *is_counting = true;
                        *last_tick = self.time_source.now();
                    },
                    Some(Command::StopCounter) => {
                        *is_counting = false;
                    },
                    Some(Command::SetTime(time)) => {
                        self.time = 0;
                        self.target_time = time;
                    },
                    Some(Command::Unsubscribe(client_id)) => {
                        self.listeners.remove(&client_id);
                    },
                    Some(Command::Close) => return false,
                    None => return false,
                };
            }

            _ = interval_stream.next(), if *is_counting => {
                let now = self.time_source.now();
                let elapsed = now.duration_since(*last_tick);
                // Prevent overflow: clamp elapsed time to i32::MAX milliseconds (~24.8 days)
                let elapsed_ms = elapsed.as_millis().min(i32::MAX as u128) as i32;
                self.time = self.time.saturating_add(elapsed_ms);
                *last_tick = now;
            }
        }
        self.broadcast(TimerMessage {
            is_running: *is_counting,
            current_time: self.time,
            target_time: self.target_time,
            client_count: self.listeners.len(),
        });
        true
    }

    pub async fn run(mut self) {
        let mut interval_stream = self.time_source.interval(self.config.tick_interval);
        let mut last_tick = self.time_source.now();
        let mut is_counting = false;

        while self.run_single_iteration(&mut interval_stream, &mut last_tick, &mut is_counting).await {}
    }
}

#[cfg(test)]
pub mod test {
    use tokio::time::{sleep, Duration};
    use tokio::sync::mpsc;
    use crate::time::{SystemTime, TimeSource};
    use crate::config::TimerConfig;
    use std::sync::{Arc, Mutex};
    use super::{TimerFactory, TimerHandle};

    #[tokio::test]
    async fn test_timer() {
        let (timer, timer_handle) = super::Timer::new(SystemTime, TimerConfig::default());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                println!("Received: {:?}", msg);
            }
            println!("Receiver Done");
        });

        let handle = tokio::spawn(timer.run());

        timer_handle.set_time(1000).unwrap();
        timer_handle.start_counter().unwrap();
        sleep(Duration::from_millis(100)).await;
        timer_handle.stop_counter().unwrap();
        sleep(Duration::from_millis(200)).await;
        timer_handle.start_counter().unwrap();
        sleep(Duration::from_millis(500)).await;
        timer_handle.set_time(500).unwrap();

        sleep(Duration::from_millis(1000)).await;
        timer_handle.close().unwrap();

        _ = handle.await;
    }

    use crate::time::mock::MockTime;

    async fn drain_to_latest_msg(rx: &mut mpsc::UnboundedReceiver<super::TimerMessage>) -> super::TimerMessage {
        let mut msg = rx.recv().await.unwrap();
        while let Ok(new_msg) = rx.try_recv() {
            msg = new_msg;
        }
        msg
    }

    #[tokio::test]
    async fn test_timer_broadcasts_state() {
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (timer, timer_handle) = super::Timer::new(mock_time.clone(), config);

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::sleep(Duration::from_millis(5)).await;

        let msg = msg_rx.recv().await.unwrap();
        assert_eq!(msg.current_time, 0);
        assert_eq!(msg.target_time, 0);
        assert!(!msg.is_running);
        assert_eq!(msg.client_count, 1);

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_set_time() {
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (timer, timer_handle) = super::Timer::new(mock_time.clone(), config);

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::sleep(Duration::from_millis(5)).await;

        let msg = msg_rx.recv().await.unwrap();
        assert_eq!(msg.target_time, 0);

        timer_handle.set_time(5000).unwrap();

        let msg = msg_rx.recv().await.unwrap();
        assert_eq!(msg.target_time, 5000);
        assert_eq!(msg.current_time, 0);

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_start_stop() {
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (timer, timer_handle) = super::Timer::new(mock_time.clone(), config);

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;
        drain_to_latest_msg(&mut msg_rx).await;

        timer_handle.start_counter().unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let msg = drain_to_latest_msg(&mut msg_rx).await;
        assert!(msg.is_running, "Timer should be running after start");

        timer_handle.stop_counter().unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let msg = drain_to_latest_msg(&mut msg_rx).await;
        assert!(!msg.is_running, "Timer should be stopped after stop");

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_counting_with_mock_time() {
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (timer, timer_handle) = super::Timer::new(mock_time.clone(), config);

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;
        drain_to_latest_msg(&mut msg_rx).await;

        timer_handle.set_time(1000).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        drain_to_latest_msg(&mut msg_rx).await;

        timer_handle.start_counter().unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        drain_to_latest_msg(&mut msg_rx).await;

        mock_time.advance(Duration::from_millis(100));
        tokio::time::sleep(Duration::from_millis(20)).await;

        let msg = drain_to_latest_msg(&mut msg_rx).await;
        assert!(
            msg.current_time >= 90 && msg.current_time <= 110,
            "Expected ~100ms, got {}ms",
            msg.current_time
        );

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_multiple_subscribers() {
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (timer, timer_handle) = super::Timer::new(mock_time.clone(), config);

        tokio::spawn(timer.run());

        let client_id1 = uuid::Uuid::new_v4();
        let client_id2 = uuid::Uuid::new_v4();
        let mut msg_rx1 = timer_handle.subscribe(client_id1).unwrap();
        let mut msg_rx2 = timer_handle.subscribe(client_id2).unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;

        let msg1 = drain_to_latest_msg(&mut msg_rx1).await;
        let msg2 = drain_to_latest_msg(&mut msg_rx2).await;
        assert_eq!(msg1.client_count, 2, "First subscriber should see 2 clients");
        assert_eq!(msg2.client_count, 2, "Second subscriber should see 2 clients");

        timer_handle.unsubscribe(client_id1).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        timer_handle.set_time(100).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let msg2 = drain_to_latest_msg(&mut msg_rx2).await;
        assert_eq!(msg2.client_count, 1, "Should only have 1 client after unsubscribe");

        tokio::time::sleep(Duration::from_millis(5)).await;
        while msg_rx1.try_recv().is_ok() {}

        timer_handle.set_time(200).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let result = msg_rx1.try_recv();
        assert!(result.is_err(), "Unsubscribed client should not receive new messages after unsubscribe");

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_single_iteration_processes_command() {
        use crate::time::mock::MockTime;
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (mut timer, timer_handle) = super::Timer::new(mock_time.clone(), config.clone());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        let mut interval_stream = mock_time.interval(config.tick_interval);
        let mut last_tick = mock_time.now();
        let mut is_counting = false;

        timer.run_single_iteration(&mut interval_stream, &mut last_tick, &mut is_counting).await;

        let msg = msg_rx.recv().await.unwrap();
        assert_eq!(msg.client_count, 1);

        timer_handle.set_time(1000).unwrap();

        timer.run_single_iteration(&mut interval_stream, &mut last_tick, &mut is_counting).await;

        let msg = msg_rx.recv().await.unwrap();
        assert_eq!(msg.target_time, 1000);
    }

    #[tokio::test]
    async fn test_timer_single_iteration_stops_on_close() {
        use crate::time::mock::MockTime;
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (mut timer, timer_handle) = super::Timer::new(mock_time.clone(), config.clone());

        let mut interval_stream = mock_time.interval(config.tick_interval);
        let mut last_tick = mock_time.now();
        let mut is_counting = false;

        timer_handle.close().unwrap();

        let continues = timer.run_single_iteration(&mut interval_stream, &mut last_tick, &mut is_counting).await;
        assert!(!continues, "Timer should stop after close command");
    }

    #[tokio::test]
    async fn test_timer_handles_rapid_commands() {
        use crate::time::mock::MockTime;
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (timer, timer_handle) = super::Timer::new(mock_time.clone(), config);

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        for i in 0..10 {
            timer_handle.set_time(i * 100).unwrap();
            timer_handle.start_counter().unwrap();
            timer_handle.stop_counter().unwrap();
        }

        tokio::time::sleep(Duration::from_millis(50)).await;

        while msg_rx.try_recv().is_ok() {}

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_handles_large_time_values() {
        use crate::time::mock::MockTime;
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (timer, timer_handle) = super::Timer::new(mock_time.clone(), config);

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;
        while msg_rx.try_recv().is_ok() {}

        timer_handle.set_time(i32::MAX).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let msg = msg_rx.try_recv();
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.target_time, i32::MAX);

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_closes_receiver_on_shutdown() {
        use crate::time::mock::MockTime;
        let mock_time = MockTime::new();
        let config = TimerConfig::for_testing();
        let (timer, timer_handle) = super::Timer::new(mock_time.clone(), config);

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        timer_handle.close().unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        let mut received_none = false;
        for _ in 0..5 {
            if msg_rx.recv().await.is_none() {
                received_none = true;
                break;
            }
        }

        assert!(received_none, "Receiver should be closed after timer shutdown");
    }

    #[derive(Clone)]
    pub struct MockTimerFactory {
        created_timers: Arc<Mutex<Vec<TimerHandle>>>,
    }

    impl MockTimerFactory {
        pub fn new() -> Self {
            Self {
                created_timers: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn created_count(&self) -> usize {
            self.created_timers.lock().unwrap().len()
        }

        pub fn get_timer(&self, index: usize) -> Option<TimerHandle> {
            self.created_timers.lock().unwrap().get(index).cloned()
        }
    }

    impl TimerFactory for MockTimerFactory {
        fn create_timer(&self, config: TimerConfig) -> TimerHandle {
            use crate::time::mock::MockTime;
            let (timer, handle) = super::Timer::new(MockTime::new(), config);
            self.created_timers.lock().unwrap().push(handle.clone());
            tokio::spawn(timer.run());
            handle
        }
    }
}
