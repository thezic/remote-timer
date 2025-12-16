use std::{
    collections::HashMap,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::config::TimerConfig;
use crate::time::TimeSource;

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
    is_running: bool,
    current_time: i32,
    target_time: i32,
    client_count: usize,
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

    pub async fn run(mut self) {
        let mut interval_stream = self.time_source.interval(self.config.tick_interval);

        let mut last_tick = self.time_source.now();
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
                            last_tick = self.time_source.now();
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

                _ = interval_stream.next(), if is_counting => {
                    let now = self.time_source.now();
                    let elapsed = now.duration_since(last_tick);
                    self.time += elapsed.as_millis() as i32;
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
mod test {
    use tokio::time::{sleep, Duration};
    use tokio::sync::mpsc;
    use crate::time::SystemTime;
    use crate::config::TimerConfig;

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

    async fn get_latest_msg(rx: &mut mpsc::UnboundedReceiver<super::TimerMessage>) -> super::TimerMessage {
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
        assert_eq!(msg.is_running, false);
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
        get_latest_msg(&mut msg_rx).await;

        timer_handle.start_counter().unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let msg = get_latest_msg(&mut msg_rx).await;
        assert_eq!(msg.is_running, true, "Timer should be running after start");

        timer_handle.stop_counter().unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let msg = get_latest_msg(&mut msg_rx).await;
        assert_eq!(msg.is_running, false, "Timer should be stopped after stop");

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
        get_latest_msg(&mut msg_rx).await;

        timer_handle.set_time(1000).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        get_latest_msg(&mut msg_rx).await;

        timer_handle.start_counter().unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        get_latest_msg(&mut msg_rx).await;

        mock_time.advance(Duration::from_millis(100));
        tokio::time::sleep(Duration::from_millis(20)).await;

        let msg = get_latest_msg(&mut msg_rx).await;
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

        let msg1 = get_latest_msg(&mut msg_rx1).await;
        let msg2 = get_latest_msg(&mut msg_rx2).await;
        assert_eq!(msg1.client_count, 2, "First subscriber should see 2 clients");
        assert_eq!(msg2.client_count, 2, "Second subscriber should see 2 clients");

        timer_handle.unsubscribe(client_id1).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        timer_handle.set_time(100).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let msg2 = get_latest_msg(&mut msg_rx2).await;
        assert_eq!(msg2.client_count, 1, "Should only have 1 client after unsubscribe");

        tokio::time::sleep(Duration::from_millis(5)).await;
        while msg_rx1.try_recv().is_ok() {}

        timer_handle.set_time(200).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let result = msg_rx1.try_recv();
        assert!(result.is_err(), "Unsubscribed client should not receive new messages after unsubscribe");

        timer_handle.close().unwrap();
    }
}
