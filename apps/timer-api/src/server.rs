use std::collections::{HashMap, HashSet};

use tokio::sync::mpsc;
use uuid::Uuid;

pub enum Command {
    Connect,
    StartCounter,
    StopCounter,
    SetTime(u32),
}

pub struct TimerServer {
    sessions: HashMap<Uuid, i32>,
    command_rx: mpsc::UnboundedReceiver<Command>,
}

#[derive(Debug, Clone)]
pub struct ServerHandle {
    cmd_tx: mpsc::UnboundedSender<Command>,
}

impl TimerServer {
    pub fn new() -> (Self, ServerHandle) {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<Command>();

        let server = TimerServer {
            sessions: HashMap::new(),
            command_rx: msg_rx,
        };

        (server, ServerHandle { cmd_tx: msg_tx })
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.command_rx.recv().await {
            match msg {
                Command::Connect => todo!(),
                Command::StartCounter => todo!(),
                Command::StopCounter => todo!(),
                Command::SetTime(_) => todo!(),
            }
        }
    }
}
