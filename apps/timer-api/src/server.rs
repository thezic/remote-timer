use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::config::TimerConfig;
use crate::time::SystemTime;
use crate::timer::{self, Timer, TimerHandle, TimerMessage};
use anyhow::Result;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};
use uuid::Uuid;

const MAX_TIMER_AGE: Duration = Duration::from_secs(30 * 60); // 30 minutes

type TimerId = Uuid;
type ConnId = Uuid;

pub enum Command {
    Connect(
        TimerId,
        oneshot::Sender<(ConnId, mpsc::UnboundedReceiver<TimerMessage>)>,
    ),
    Disconnect(ConnId),
    StartCounter(ConnId, oneshot::Sender<Result<()>>),
    StopCounter(ConnId, oneshot::Sender<Result<()>>),
    SetTime(ConnId, i32, oneshot::Sender<Result<()>>),
}

pub struct TimerServer {
    command_rx: mpsc::UnboundedReceiver<Command>,
    timers: HashMap<TimerId, TimerHandle>,
    clients: HashMap<ConnId, TimerId>,
    timers_to_cleanup: HashMap<TimerId, Instant>,
}

#[derive(Debug, Clone)]
pub struct ServerHandle {
    cmd_tx: mpsc::UnboundedSender<Command>,
}

pub struct BoundServerHandle {
    handle: ServerHandle,
    connection_id: ConnId,
}

impl BoundServerHandle {
    pub fn new(connection_id: ConnId, handle: ServerHandle) -> Self {
        Self {
            handle,
            connection_id,
        }
    }

    pub async fn set_time(&self, time: i32) -> Result<()> {
        self.handle.set_time(self.connection_id, time).await
    }

    pub async fn start_counter(&self) -> Result<()> {
        self.handle.start_counter(self.connection_id).await
    }

    pub async fn stop_counter(&self) -> Result<()> {
        self.handle.stop_counter(self.connection_id).await
    }
}

impl Drop for BoundServerHandle {
    fn drop(&mut self) {
        debug!("Dropping BoundServerHandle");
        _ = self.handle.disconnect(self.connection_id);
    }
}

impl ServerHandle {
    pub async fn connect(
        &self,
        id: TimerId,
    ) -> Result<(BoundServerHandle, mpsc::UnboundedReceiver<TimerMessage>)> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(Command::Connect(id, tx))?;
        let (connection_id, timer_messages) = rx.await?;

        Ok((
            BoundServerHandle::new(connection_id, self.clone()),
            timer_messages,
        ))
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

    pub fn disconnect(&self, conn_id: ConnId) -> Result<()> {
        self.cmd_tx.send(Command::Disconnect(conn_id))?;
        debug!("Disconnevting {conn_id}");
        Ok(())
    }
}

impl TimerServer {
    pub fn new() -> (Self, ServerHandle) {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<Command>();

        let server = TimerServer {
            clients: HashMap::new(),
            command_rx: msg_rx,
            timers: HashMap::new(),
            timers_to_cleanup: HashMap::new(),
        };

        (server, ServerHandle { cmd_tx: msg_tx })
    }

    fn create_timer(&mut self, id: TimerId) -> TimerHandle {
        let (timer, handle) = Timer::new(SystemTime, TimerConfig::default());
        self.timers.insert(id, handle.clone());

        let _task_handle = tokio::spawn(timer.run());

        handle
    }

    async fn connect(
        &mut self,
        timer_id: TimerId,
    ) -> anyhow::Result<(ConnId, mpsc::UnboundedReceiver<timer::TimerMessage>)> {
        let client_id = Uuid::new_v4();

        let timer_handle = match self.timers.get(&timer_id) {
            Some(handle) => handle.clone(),
            None => self.create_timer(timer_id),
        };

        self.clients.insert(client_id, timer_id);
        self.revive_timer(timer_id);

        let receiver = timer_handle.subscribe(client_id)?;

        Ok((client_id, receiver))
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

    fn schedule_timer_cleanup(&mut self, timer_id: TimerId) {
        let now = Instant::now();
        self.timers_to_cleanup.insert(timer_id, now);
    }

    fn revive_timer(&mut self, timer_id: TimerId) {
        self.timers_to_cleanup.remove(&timer_id);
    }

    fn cleanup_timers(&mut self) {
        self.timers_to_cleanup.retain(|&timer_id, created_at| {
            if created_at.elapsed() > MAX_TIMER_AGE {
                debug!("Remove timer {timer_id}");
                if let Some(timer) = self.timers.remove(&timer_id) {
                    _ = timer.close();
                }
                false
            } else {
                true
            }
        });
    }

    async fn disconnect(&mut self, client_id: ConnId) -> Result<()> {
        if let Some(timer) = self
            .clients
            .get(&client_id)
            .and_then(|timer_id| self.timers.get(timer_id))
        {
            debug!("Unsubscribe from timer {client_id}");
            timer.unsubscribe(client_id)?;
        }

        // Remove a timer if no clients are connected
        let timer_to_remove = self.clients.remove(&client_id).and_then(|timer_id| {
            match self
                .clients
                .values()
                .any(|&client_timer_id| timer_id == client_timer_id)
            {
                true => None,
                false => Some(timer_id),
            }
        });

        debug!("Timer to remove: {timer_to_remove:?}");

        if let Some(timer_id) = timer_to_remove {
            self.schedule_timer_cleanup(timer_id);
        }

        Ok(())
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

                Command::Disconnect(conn_id) => {
                    // TODO: Handle error here
                    self.disconnect(conn_id).await.unwrap();
                }
            }

            self.cleanup_timers();
        }
    }
}
