use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, time::interval};

#[derive(Debug)]
pub enum Command {
    StartCounter,
    StopCounter,
    SetTime(i32),
    Close,
    Subscribe(mpsc::UnboundedSender<TimerMessage>),
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct TimerMessage {
    is_running: bool,
    current_time: i32,
}

pub struct Timer {
    time: i32,
    cmd_rx: mpsc::UnboundedReceiver<Command>,
    listeners: Vec<mpsc::UnboundedSender<TimerMessage>>,
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

    pub fn subscribe(&self) -> Result<mpsc::UnboundedReceiver<TimerMessage>> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.cmd_tx.send(Command::Subscribe(tx))?;
        Ok(rx)
    }
}

impl Timer {
    pub fn new() -> (Self, TimerHandle) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let timer = Self {
            time: 0,
            cmd_rx,
            listeners: Vec::new(),
        };
        (timer, TimerHandle { cmd_tx })
    }

    fn broadcast(&mut self, msg: TimerMessage) {
        self.listeners.retain(move |tx| tx.send(msg).is_ok());
    }

    pub async fn run(mut self) {
        let mut interval = interval(Duration::from_millis(100));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut last_tick = Instant::now();
        let mut is_counting = false;

        loop {
            tokio::select! {
                msg = self.cmd_rx.recv() => {
                    match msg {
                        Some(Command::Subscribe(tx)) => {
                            self.listeners.push(tx);
                        },
                        Some(Command::StartCounter) => {
                            is_counting = true;
                            last_tick = Instant::now();
                        },
                        Some(Command::StopCounter) => {
                            is_counting = false;
                        },
                        Some(Command::SetTime(time)) => {
                            self.time = time;
                        },
                        Some(Command::Close) => break,
                        None => break,
                    };
                }

                _ = interval.tick(), if is_counting => {
                    self.time -= last_tick.elapsed().as_millis() as i32;
                    last_tick = Instant::now();

                    if self.time <= 0 {
                        self.time = 0;
                        is_counting = false;
                    }
                }
            }
            self.broadcast(TimerMessage {
                is_running: is_counting,
                current_time: self.time,
            });
        }
    }
}

#[cfg(test)]
mod test {
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_timer() {
        let (timer, timer_handle) = super::Timer::new();

        let mut msg_rx = timer_handle.subscribe().unwrap();
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
}
