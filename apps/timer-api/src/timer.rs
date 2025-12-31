use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::{interval, Instant};
use uuid::Uuid;

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

pub struct Timer {
    time: i32,
    target_time: i32,
    cmd_rx: mpsc::UnboundedReceiver<Command>,
    listeners: HashMap<Uuid, mpsc::UnboundedSender<TimerMessage>>,
    tick_interval: std::time::Duration,
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

impl Timer {
    pub fn new(tick_interval: std::time::Duration) -> (Self, TimerHandle) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let timer = Self {
            time: 0,
            target_time: 0,
            cmd_rx,
            listeners: HashMap::new(),
            tick_interval,
        };
        (timer, TimerHandle { cmd_tx })
    }

    fn broadcast(&mut self, msg: TimerMessage) {
        self.listeners.retain(move |_, tx| tx.send(msg).is_ok());
    }

    pub async fn run(mut self) {
        let mut interval_stream = interval(self.tick_interval);
        interval_stream.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_tick = Instant::now();
        let mut is_counting = false;

        loop {
            tokio::select! {
                msg = self.cmd_rx.recv() => {
                    match msg {
                        Some(Command::Subscribe(client_id, tx)) => {
                            self.listeners.insert(client_id, tx);
                        },
                        Some(Command::StartCounter) => {
                            is_counting = true;
                            last_tick = Instant::now();
                        },
                        Some(Command::StopCounter) => {
                            is_counting = false;
                        },
                        Some(Command::SetTime(time)) => {
                            self.time = 0;
                            self.target_time = time;
                        },
                        Some(Command::Unsubscribe(client_id)) => {
                            self.listeners.remove(&client_id);
                        },
                        Some(Command::Close) => break,
                        None => break,
                    };
                }

                _ = interval_stream.tick(), if is_counting => {
                    let now = Instant::now();
                    let elapsed = now.duration_since(last_tick);
                    // Prevent overflow: clamp elapsed time to i32::MAX milliseconds (~24.8 days)
                    let elapsed_ms = elapsed.as_millis().min(i32::MAX as u128) as i32;
                    self.time = self.time.saturating_add(elapsed_ms);
                    last_tick = now;
                }
            }
            self.broadcast(TimerMessage {
                is_running: is_counting,
                current_time: self.time,
                target_time: self.target_time,
                client_count: self.listeners.len(),
            });
        }
    }
}

#[cfg(test)]
pub mod test {
    use tokio::time::{sleep, Duration};

    async fn drain_to_latest_msg(rx: &mut tokio::sync::mpsc::UnboundedReceiver<super::TimerMessage>) -> super::TimerMessage {
        let mut msg = rx.recv().await.unwrap();
        while let Ok(new_msg) = rx.try_recv() {
            msg = new_msg;
        }
        msg
    }

    #[tokio::test]
    async fn test_timer() {
        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(100));

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

    #[tokio::test]
    async fn test_timer_broadcasts_state() {
        tokio::time::pause();

        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(1));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::advance(Duration::from_millis(1)).await;

        let msg = msg_rx.recv().await.unwrap();
        assert_eq!(msg.current_time, 0);
        assert_eq!(msg.target_time, 0);
        assert!(!msg.is_running);
        assert_eq!(msg.client_count, 1);

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_set_time() {
        tokio::time::pause();

        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(1));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::advance(Duration::from_millis(1)).await;

        let msg = msg_rx.recv().await.unwrap();
        assert_eq!(msg.target_time, 0);

        timer_handle.set_time(5000).unwrap();
        tokio::time::advance(Duration::from_millis(1)).await;

        let msg = msg_rx.recv().await.unwrap();
        assert_eq!(msg.target_time, 5000);
        assert_eq!(msg.current_time, 0);

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_start_stop() {
        tokio::time::pause();

        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(1));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::advance(Duration::from_millis(5)).await;
        drain_to_latest_msg(&mut msg_rx).await;

        timer_handle.start_counter().unwrap();
        tokio::time::advance(Duration::from_millis(5)).await;
        let msg = drain_to_latest_msg(&mut msg_rx).await;
        assert!(msg.is_running, "Timer should be running after start");

        timer_handle.stop_counter().unwrap();
        tokio::time::advance(Duration::from_millis(5)).await;
        let msg = drain_to_latest_msg(&mut msg_rx).await;
        assert!(!msg.is_running, "Timer should be stopped after stop");

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_counting_with_paused_time() {
        tokio::time::pause();

        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(1));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::advance(Duration::from_millis(5)).await;
        drain_to_latest_msg(&mut msg_rx).await;

        timer_handle.set_time(1000).unwrap();
        tokio::time::advance(Duration::from_millis(5)).await;
        drain_to_latest_msg(&mut msg_rx).await;

        timer_handle.start_counter().unwrap();
        tokio::time::advance(Duration::from_millis(5)).await;
        drain_to_latest_msg(&mut msg_rx).await;

        tokio::time::advance(Duration::from_millis(100)).await;

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
        tokio::time::pause();

        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(1));

        tokio::spawn(timer.run());

        let client_id1 = uuid::Uuid::new_v4();
        let client_id2 = uuid::Uuid::new_v4();
        let mut msg_rx1 = timer_handle.subscribe(client_id1).unwrap();
        let mut msg_rx2 = timer_handle.subscribe(client_id2).unwrap();

        tokio::time::advance(Duration::from_millis(10)).await;

        let msg1 = drain_to_latest_msg(&mut msg_rx1).await;
        let msg2 = drain_to_latest_msg(&mut msg_rx2).await;
        assert_eq!(msg1.client_count, 2, "First subscriber should see 2 clients");
        assert_eq!(msg2.client_count, 2, "Second subscriber should see 2 clients");

        timer_handle.unsubscribe(client_id1).unwrap();
        tokio::time::advance(Duration::from_millis(5)).await;

        timer_handle.set_time(100).unwrap();
        tokio::time::advance(Duration::from_millis(5)).await;

        let msg2 = drain_to_latest_msg(&mut msg_rx2).await;
        assert_eq!(msg2.client_count, 1, "Should only have 1 client after unsubscribe");

        while msg_rx1.try_recv().is_ok() {}

        timer_handle.set_time(200).unwrap();
        tokio::time::advance(Duration::from_millis(5)).await;

        let result = msg_rx1.try_recv();
        assert!(result.is_err(), "Unsubscribed client should not receive new messages after unsubscribe");

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_handles_rapid_commands() {
        tokio::time::pause();

        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(1));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        for i in 0..10 {
            timer_handle.set_time(i * 100).unwrap();
            timer_handle.start_counter().unwrap();
            timer_handle.stop_counter().unwrap();
        }

        tokio::time::advance(Duration::from_millis(20)).await;

        while msg_rx.try_recv().is_ok() {}

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_handles_large_time_values() {
        tokio::time::pause();

        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(1));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::advance(Duration::from_millis(5)).await;
        while msg_rx.try_recv().is_ok() {}

        timer_handle.set_time(i32::MAX).unwrap();
        tokio::time::advance(Duration::from_millis(5)).await;

        let msg = msg_rx.try_recv();
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.target_time, i32::MAX);

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_closes_receiver_on_shutdown() {
        tokio::time::pause();

        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(1));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        timer_handle.close().unwrap();

        tokio::time::advance(Duration::from_millis(5)).await;

        let mut received_none = false;
        for _ in 0..5 {
            if msg_rx.recv().await.is_none() {
                received_none = true;
                break;
            }
        }

        assert!(received_none, "Receiver should be closed after timer shutdown");
    }

    #[tokio::test]
    async fn test_timer_with_real_time() {
        // NO tokio::time::pause() - use real time!
        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(100));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        // Drain initial messages
        while msg_rx.try_recv().is_ok() {}

        timer_handle.set_time(3000).unwrap();
        timer_handle.start_counter().unwrap();

        // Actually wait real milliseconds
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Get latest message
        let mut latest_msg = None;
        while let Ok(msg) = msg_rx.try_recv() {
            latest_msg = Some(msg);
        }

        let msg = latest_msg.expect("Should have received at least one message");
        assert!(msg.is_running, "Timer should be running");
        assert_eq!(msg.target_time, 3000);
        // Real time should advance roughly 250ms (with tolerance)
        assert!(
            msg.current_time >= 200 && msg.current_time <= 350,
            "Expected ~250ms, got {}ms",
            msg.current_time
        );

        timer_handle.close().unwrap();
    }

    #[tokio::test]
    async fn test_timer_with_production_tick_interval() {
        tokio::time::pause();

        // Use production-like 100ms tick interval
        let (timer, timer_handle) = super::Timer::new(Duration::from_millis(100));

        tokio::spawn(timer.run());

        let client_id = uuid::Uuid::new_v4();
        let mut msg_rx = timer_handle.subscribe(client_id).unwrap();

        tokio::time::advance(Duration::from_millis(150)).await;
        tokio::task::yield_now().await; // Let timer task process
        drain_to_latest_msg(&mut msg_rx).await;

        timer_handle.set_time(5000).unwrap();
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        timer_handle.start_counter().unwrap();
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        // Advance by realistic amount, giving time for multiple ticks
        // With 100ms tick interval, need to advance past several ticks
        tokio::time::advance(Duration::from_millis(750)).await;
        tokio::task::yield_now().await;

        let msg = drain_to_latest_msg(&mut msg_rx).await;
        assert!(msg.is_running);
        assert_eq!(msg.target_time, 5000);
        // Should advance roughly 750-800ms
        assert!(
            msg.current_time >= 700 && msg.current_time <= 850,
            "Expected ~750ms, got {}ms",
            msg.current_time
        );

        timer_handle.close().unwrap();
    }
}
