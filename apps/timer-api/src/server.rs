use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::config::TimerConfig;
use crate::timer::{self, RealTimerFactory, TimerFactory, TimerHandle, TimerMessage};
use anyhow::Result;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};
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
    Shutdown,
}

pub struct TimerServer<F: TimerFactory> {
    command_rx: mpsc::UnboundedReceiver<Command>,
    timers: HashMap<TimerId, TimerHandle>,
    clients: HashMap<ConnId, TimerId>,
    timers_to_cleanup: HashMap<TimerId, Instant>,
    factory: F,
    config: TimerConfig,
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
        debug!("Disconnecting {conn_id}");
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        Ok(self.cmd_tx.send(Command::Shutdown)?)
    }
}

impl TimerServer<RealTimerFactory> {
    pub fn new() -> (Self, ServerHandle) {
        Self::with_factory(RealTimerFactory, TimerConfig::default())
    }
}

impl<F: TimerFactory> TimerServer<F> {
    pub fn with_factory(factory: F, config: TimerConfig) -> (Self, ServerHandle) {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<Command>();

        let server = TimerServer {
            clients: HashMap::new(),
            command_rx: msg_rx,
            timers: HashMap::new(),
            timers_to_cleanup: HashMap::new(),
            factory,
            config,
        };

        (server, ServerHandle { cmd_tx: msg_tx })
    }

    fn create_timer(&mut self, id: TimerId) -> TimerHandle {
        let handle = self.factory.create_timer(self.config.clone());
        self.timers.insert(id, handle.clone());
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

    pub async fn run_single_iteration(&mut self) -> bool {
        let msg = match self.command_rx.recv().await {
            Some(msg) => msg,
            None => return false,
        };

        match msg {
            Command::Connect(timer_id, signal) => {
                match self.connect(timer_id).await {
                    Ok((conn_id, receiver)) => {
                        if signal.send((conn_id, receiver)).is_err() {
                            warn!("failed to send connection id for timer {timer_id}");
                        }
                    }
                    Err(err) => {
                        error!("failed to connect to timer {timer_id}: {err}");
                    }
                }
            }
            Command::StartCounter(conn_id, signal) => {
                match self.start_counter(conn_id).await {
                    Ok(()) => {
                        if signal.send(Ok(())).is_err() {
                            warn!("failed to send start counter response to client {conn_id}");
                        }
                    }
                    Err(err) => {
                        error!("failed to start counter for client {conn_id}: {err}");
                        if signal.send(Err(err)).is_err() {
                            debug!("Failed to send error response for start_counter, receiver dropped");
                        }
                    }
                }
            }
            Command::StopCounter(conn_id, signal) => {
                match self.stop_counter(conn_id).await {
                    Ok(()) => {
                        if signal.send(Ok(())).is_err() {
                            warn!("failed to send stop counter response to client {conn_id}");
                        }
                    }
                    Err(err) => {
                        error!("failed to stop counter for client {conn_id}: {err}");
                        if signal.send(Err(err)).is_err() {
                            debug!("Failed to send error response for stop_counter, receiver dropped");
                        }
                    }
                }
            }
            Command::SetTime(conn_id, time, signal) => {
                match self.set_time(conn_id, time) {
                    Ok(()) => {
                        if signal.send(Ok(())).is_err() {
                            warn!("failed to send set time response to {conn_id}");
                        }
                    }
                    Err(err) => {
                        error!("failed to set time for client {conn_id}: {err}");
                        if signal.send(Err(err)).is_err() {
                            debug!("Failed to send error response for set_time, receiver dropped");
                        }
                    }
                }
            }
            Command::Disconnect(conn_id) => {
                if let Err(err) = self.disconnect(conn_id).await {
                    error!("failed to disconnect client {conn_id}: {err}");
                }
            }
            Command::Shutdown => return false,
        }

        self.cleanup_timers();
        true
    }

    pub async fn run(mut self) {
        while self.run_single_iteration().await {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::test::MockTimerFactory;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_server_creates_timer_on_first_connection() {
        let factory = MockTimerFactory::new();
        let (server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        tokio::spawn(server.run());

        let timer_id = Uuid::new_v4();
        let result = handle.connect(timer_id).await;
        assert!(result.is_ok());

        sleep(Duration::from_millis(10)).await;
        assert_eq!(factory.created_count(), 1, "Should create one timer on first connection");
    }

    #[tokio::test]
    async fn test_server_reuses_timer_for_same_id() {
        let factory = MockTimerFactory::new();
        let (server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        tokio::spawn(server.run());

        let timer_id = Uuid::new_v4();
        let _conn1 = handle.connect(timer_id).await.unwrap();
        sleep(Duration::from_millis(10)).await;

        let _conn2 = handle.connect(timer_id).await.unwrap();
        sleep(Duration::from_millis(10)).await;

        assert_eq!(factory.created_count(), 1, "Should reuse existing timer for same timer_id");
    }

    #[tokio::test]
    async fn test_server_creates_different_timers_for_different_ids() {
        let factory = MockTimerFactory::new();
        let (server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        tokio::spawn(server.run());

        let timer_id1 = Uuid::new_v4();
        let timer_id2 = Uuid::new_v4();

        let _conn1 = handle.connect(timer_id1).await.unwrap();
        sleep(Duration::from_millis(10)).await;

        let _conn2 = handle.connect(timer_id2).await.unwrap();
        sleep(Duration::from_millis(10)).await;

        assert_eq!(factory.created_count(), 2, "Should create separate timers for different timer_ids");
    }

    #[tokio::test]
    async fn test_bound_handle_sends_commands() {
        let factory = MockTimerFactory::new();
        let (server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        tokio::spawn(server.run());

        let timer_id = Uuid::new_v4();
        let (bound_handle, mut msg_rx) = handle.connect(timer_id).await.unwrap();

        sleep(Duration::from_millis(10)).await;

        bound_handle.set_time(5000).await.unwrap();
        sleep(Duration::from_millis(10)).await;

        bound_handle.start_counter().await.unwrap();
        sleep(Duration::from_millis(10)).await;

        bound_handle.stop_counter().await.unwrap();
        sleep(Duration::from_millis(10)).await;

        while let Ok(_) = msg_rx.try_recv() {}
    }

    #[tokio::test]
    async fn test_server_shutdown() {
        let factory = MockTimerFactory::new();
        let (mut server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        handle.shutdown().unwrap();

        let continues = server.run_single_iteration().await;
        assert!(!continues, "Server should stop after shutdown command");
    }

    #[tokio::test]
    async fn test_server_single_iteration_processes_connect() {
        let factory = MockTimerFactory::new();
        let (mut server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        let timer_id = Uuid::new_v4();
        tokio::spawn(async move {
            handle.connect(timer_id).await.unwrap();
        });

        sleep(Duration::from_millis(5)).await;
        let continues = server.run_single_iteration().await;
        assert!(continues, "Server should continue after processing connect");
        assert_eq!(factory.created_count(), 1, "Should have created one timer");
    }

    #[tokio::test]
    async fn test_server_handles_error_for_invalid_client() {
        let factory = MockTimerFactory::new();
        let (mut server, _handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        let invalid_conn_id = Uuid::new_v4();

        let result = server.set_time(invalid_conn_id, 1000);
        assert!(result.is_err(), "Should return error for invalid client");

        let result = server.start_counter(invalid_conn_id).await;
        assert!(result.is_err(), "Should return error for invalid client");

        let result = server.stop_counter(invalid_conn_id).await;
        assert!(result.is_err(), "Should return error for invalid client");
    }

    #[tokio::test]
    async fn test_multiple_clients_receive_same_timer_updates() {
        let factory = MockTimerFactory::new();
        let (server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        tokio::spawn(server.run());

        let timer_id = Uuid::new_v4();
        let (bound_handle1, mut msg_rx1) = handle.connect(timer_id).await.unwrap();
        let (bound_handle2, mut msg_rx2) = handle.connect(timer_id).await.unwrap();

        sleep(Duration::from_millis(10)).await;

        bound_handle1.set_time(3000).await.unwrap();
        sleep(Duration::from_millis(10)).await;

        while msg_rx1.try_recv().is_ok() {}
        while msg_rx2.try_recv().is_ok() {}

        bound_handle2.start_counter().await.unwrap();
        sleep(Duration::from_millis(10)).await;

        let msg1 = msg_rx1.try_recv();
        let msg2 = msg_rx2.try_recv();

        assert!(msg1.is_ok(), "Client 1 should receive timer update");
        assert!(msg2.is_ok(), "Client 2 should receive timer update");

        let msg1 = msg1.unwrap();
        let msg2 = msg2.unwrap();

        assert_eq!(msg1.target_time, 3000);
        assert_eq!(msg2.target_time, 3000);
        assert!(msg1.is_running);
        assert!(msg2.is_running);
        assert_eq!(msg1.client_count, 2);
        assert_eq!(msg2.client_count, 2);
    }

    #[tokio::test]
    async fn test_client_disconnect_reduces_count() {
        let factory = MockTimerFactory::new();
        let (server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        tokio::spawn(server.run());

        let timer_id = Uuid::new_v4();
        let (bound_handle1, mut msg_rx1) = handle.connect(timer_id).await.unwrap();
        let (bound_handle2, mut msg_rx2) = handle.connect(timer_id).await.unwrap();

        sleep(Duration::from_millis(10)).await;
        while msg_rx1.try_recv().is_ok() {}
        while msg_rx2.try_recv().is_ok() {}

        drop(bound_handle2);
        drop(msg_rx2);

        sleep(Duration::from_millis(10)).await;

        bound_handle1.set_time(1000).await.unwrap();
        sleep(Duration::from_millis(10)).await;

        while let Ok(msg) = msg_rx1.try_recv() {
            if msg.client_count == 1 {
                return;
            }
        }

        sleep(Duration::from_millis(20)).await;
        let msg = msg_rx1.try_recv().unwrap();
        assert_eq!(msg.client_count, 1, "Client count should be 1 after disconnect");
    }

    #[tokio::test]
    async fn test_different_timers_are_independent() {
        let factory = MockTimerFactory::new();
        let (server, handle) = TimerServer::with_factory(factory.clone(), TimerConfig::for_testing());

        tokio::spawn(server.run());

        let timer_id1 = Uuid::new_v4();
        let timer_id2 = Uuid::new_v4();

        let (bound_handle1, mut msg_rx1) = handle.connect(timer_id1).await.unwrap();
        let (bound_handle2, mut msg_rx2) = handle.connect(timer_id2).await.unwrap();

        sleep(Duration::from_millis(10)).await;

        bound_handle1.set_time(1000).await.unwrap();
        bound_handle2.set_time(2000).await.unwrap();

        sleep(Duration::from_millis(10)).await;

        while msg_rx1.try_recv().is_ok() {}
        while msg_rx2.try_recv().is_ok() {}

        bound_handle1.start_counter().await.unwrap();
        bound_handle2.set_time(3000).await.unwrap();

        sleep(Duration::from_millis(10)).await;

        let mut timer1_msg = None;
        let mut timer2_msg = None;

        while let Ok(msg) = msg_rx1.try_recv() {
            timer1_msg = Some(msg);
        }
        while let Ok(msg) = msg_rx2.try_recv() {
            timer2_msg = Some(msg);
        }

        assert!(timer1_msg.is_some(), "Should receive timer 1 message");
        assert!(timer2_msg.is_some(), "Should receive timer 2 message");

        let timer1_msg = timer1_msg.unwrap();
        let timer2_msg = timer2_msg.unwrap();

        assert!(timer1_msg.is_running, "Timer 1 should be running");
        assert_eq!(timer1_msg.target_time, 1000, "Timer 1 should have target time 1000");
        assert!(!timer2_msg.is_running, "Timer 2 should not be running");
        assert_eq!(timer2_msg.target_time, 3000, "Timer 2 should have target time 3000");
    }
}
