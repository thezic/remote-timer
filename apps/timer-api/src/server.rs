use std::collections::{HashMap, HashSet};

use crate::timer::{self, Timer, TimerHandle, TimerMessage};
use anyhow::Result;
use tokio::sync::{mpsc, oneshot};
use tracing::warn;
use uuid::Uuid;

type TimerId = Uuid;
type ConnId = Uuid;

pub enum Command {
    Connect(
        TimerId,
        oneshot::Sender<(ConnId, mpsc::UnboundedReceiver<TimerMessage>)>,
    ),
    StartCounter(ConnId, oneshot::Sender<Result<()>>),
    StopCounter(ConnId, oneshot::Sender<Result<()>>),
    SetTime(ConnId, u32, oneshot::Sender<Result<()>>),
}

pub struct TimerServer {
    command_rx: mpsc::UnboundedReceiver<Command>,
    timers: HashMap<TimerId, TimerHandle>,
    timer_connections: HashMap<TimerId, HashSet<ConnId>>,
}

#[derive(Debug, Clone)]
pub struct ServerHandle {
    cmd_tx: mpsc::UnboundedSender<Command>,
}

impl ServerHandle {
    pub async fn connect(
        &self,
        id: TimerId,
    ) -> Result<(ConnId, mpsc::UnboundedReceiver<TimerMessage>)> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(Command::Connect(id, tx))?;
        Ok(rx.await?)
    }

    pub async fn set_time(&self, id: ConnId, time: u32) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(Command::SetTime(id, time, tx))?;
        rx.await?
    }

    // pub fn start_counter(&self, id: TimerId) {
    //     self.cmd_tx.send(Command::StartCounter(id)).unwrap();
    // }

    // pub fn stop_counter(&self, id: TimerId) {
    //     self.cmd_tx.send(Command::StopCounter(id)).unwrap();
    // }

    // pub fn set_time(&self, id: TimerId, time: u32) {
    //     self.cmd_tx.send(Command::SetTime(id, time)).unwrap();
    // }
}

impl TimerServer {
    pub fn new() -> (Self, ServerHandle) {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<Command>();

        let server = TimerServer {
            // sessions: HashMap::new(),
            timer_connections: HashMap::new(),
            command_rx: msg_rx,
            timers: HashMap::new(),
        };

        (server, ServerHandle { cmd_tx: msg_tx })
    }

    fn create_timer(&mut self, id: TimerId) -> TimerHandle {
        let (timer, handle) = Timer::new();
        self.timers.insert(id, handle.clone());

        let _task_handle = tokio::spawn(timer.run());

        handle
    }

    async fn connect(
        &mut self,
        id: TimerId,
    ) -> anyhow::Result<(ConnId, mpsc::UnboundedReceiver<timer::TimerMessage>)> {
        let conn_id = Uuid::new_v4();

        let timer_handle = match self.timers.get(&id) {
            Some(handle) => handle.clone(),
            None => self.create_timer(id),
        };

        self.timer_connections
            .entry(id)
            .or_default()
            .insert(conn_id);

        let receiver = timer_handle.subscribe()?;

        Ok((conn_id, receiver))
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.command_rx.recv().await {
            match msg {
                Command::Connect(timer_id, signal) => {
                    // TODO: Handle error here
                    let (conn_id, receiver) = self.connect(timer_id).await.unwrap();

                    if signal.send((conn_id, receiver)).is_err() {
                        // TODO: We should probably cleanup this connection
                        warn!("failed to send connection id");
                    };
                }
                Command::StartCounter(_, _) => todo!(),
                Command::StopCounter(_, _) => todo!(),
                Command::SetTime(id, time, signal) => todo!(),
            }
        }
    }
}
