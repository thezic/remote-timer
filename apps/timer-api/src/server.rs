use std::collections::HashMap;

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
    SetTime(ConnId, i32, oneshot::Sender<Result<()>>),
}

pub struct TimerServer {
    command_rx: mpsc::UnboundedReceiver<Command>,
    timers: HashMap<TimerId, TimerHandle>,
    clients: HashMap<ConnId, TimerId>,
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

    pub async fn set_time(&self, timer_id: ConnId, time: i32) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(Command::SetTime(timer_id, time, tx))?;
        rx.await?
    }

    pub async fn start_counter(&self, conn_id: ConnId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(Command::StartCounter(conn_id, tx))?;
        rx.await?
    }

    pub async fn stop_counter(&self, conn_id: ConnId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(Command::StopCounter(conn_id, tx))?;
        rx.await?
    }
}

impl TimerServer {
    pub fn new() -> (Self, ServerHandle) {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<Command>();

        let server = TimerServer {
            // sessions: HashMap::new(),
            // timer_connections: HashMap::new(),
            clients: HashMap::new(),
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

        self.clients.insert(conn_id, id);

        let receiver = timer_handle.subscribe()?;

        Ok((conn_id, receiver))
    }

    fn set_time(&mut self, conn_id: ConnId, time: i32) -> Result<()> {
        let timer_handle = self.get_timer_handle_for_client(conn_id)?;
        timer_handle.set_time(time)?;

        Ok(())
    }

    async fn start_counter(&mut self, _conn_id: ConnId) -> Result<()> {
        let timer_handle = self.get_timer_handle_for_client(_conn_id)?;
        timer_handle.start_counter()?;
        Ok(())
    }

    async fn stop_counter(&mut self, _conn_id: ConnId) -> Result<()> {
        let timer_handle = self.get_timer_handle_for_client(_conn_id)?;
        timer_handle.stop_counter()?;
        Ok(())
    }

    fn get_timer_handle_for_client(&self, conn_id: ConnId) -> Result<&TimerHandle> {
        let handle = self
            .clients
            .get(&conn_id)
            .and_then(|timer_id| self.timers.get(timer_id));

        if let Some(handle) = handle {
            Ok(handle)
        } else {
            Err(anyhow::anyhow!("No timer found for client {conn_id}"))
        }
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
                Command::StartCounter(conn_id, signal) => {
                    // TODO: Handle error here
                    self.start_counter(conn_id).await.unwrap();
                    if signal.send(Ok(())).is_err() {
                        warn!("failed to send start counter response to client {conn_id}");
                    }
                }
                Command::StopCounter(conn_id, signal) => {
                    // TODO: Handle error here
                    self.stop_counter(conn_id).await.unwrap();
                    if signal.send(Ok(())).is_err() {
                        warn!("failed to send stop counter response to client {conn_id}");
                    }
                }
                Command::SetTime(conn_id, time, signal) => {
                    // TODO: Handle error here
                    self.set_time(conn_id, time).unwrap();
                    if let Err(err) = signal.send(Ok(())) {
                        warn!("failed to send set time response to {conn_id}: {err:?}");
                    }
                }
            }
        }
    }
}
