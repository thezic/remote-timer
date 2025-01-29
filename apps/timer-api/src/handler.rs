use std::{
    pin::pin,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use actix_ws::{CloseCode, CloseReason, MessageStream, Session};
use futures_util::StreamExt as _;
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

use anyhow::Result;

use crate::server::ServerHandle;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handler(
    mut session: Session,
    stream: MessageStream,
    server_handle: ServerHandle,
    timer_id: Uuid,
) {
    info!("connected");

    let mut stream = stream.aggregate_continuations();
    let mut stream = pin!(stream);

    let mut last_heartbeat = Instant::now();
    let mut interval = interval(HEARTBEAT_INTERVAL);

    let (conn_id, mut timer_msg) = match server_handle.connect(timer_id).await {
        Ok(stuff) => stuff,
        Err(err) => {
            let _ = session
                .close(Some(CloseReason {
                    code: CloseCode::Error,
                    description: Some(format!("Failed to connect to timer: {err}")),
                }))
                .await;
            return;
        }
    };

    let reason: Option<CloseReason> = loop {
        let tick = interval.tick();

        tokio::select! {
            msg = stream.next() => {
                info!("received message: {msg:?}");
                match msg {
                    Some(Ok(msg)) => match msg {
                        actix_ws::AggregatedMessage::Text(text) => {
                            info!("Recived text: {text}");
                            if let Err(err) = handle_text_message(conn_id, &text, server_handle.clone()) {
                                error!("Error: {err}");
                            }
                        },
                        actix_ws::AggregatedMessage::Binary(_) => warn!("Received unexpected binary message"),
                        actix_ws::AggregatedMessage::Ping(bytes) => if session.pong(&bytes).await.is_err() {
                            break Some(CloseCode::Abnormal.into())
                        },
                        actix_ws::AggregatedMessage::Pong(_) => last_heartbeat = Instant::now(),
                        actix_ws::AggregatedMessage::Close(reason) => break reason,
                    },
                    Some(Err(err)) => {
                        error!("Error: {err}");
                        break Some(CloseReason { code: actix_ws::CloseCode::Error, description: Some(err.to_string()) });
                    }
                    None => break Some(CloseReason { code: actix_ws::CloseCode::Normal, description: None }),
                }
            },

            msg = timer_msg.recv() => {
                info!("Timer message: {msg:?}");
                let json = serde_json::to_string(&msg).unwrap();
                // TODO: Handle error
                _ = session.text(json).await;
            }

            _tick = tick => {
                info!("tick");
                if Instant::now() - last_heartbeat > CLIENT_TIMEOUT {
                    break Some(CloseReason { code: actix_ws::CloseCode::Error, description: Some("Client timeout".to_string()) });
                }

                session.ping(b"").await.unwrap();
            }
        };
    };

    info!("Disconnected: {reason:?}");
    let _ = session.close(reason).await;
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Message {
    StartTimer,
    StopTimer,
    SetTime { time: u32 },
}

fn handle_text_message(conn_id: Uuid, message: &str, server_handle: ServerHandle) -> Result<()> {
    let msg: Message = serde_json::from_str(message)?;

    match msg {
        Message::StartTimer => todo!(),
        Message::StopTimer => todo!(),
        Message::SetTime { time } => server_handle.set_time(conn_id, time),
    };
    Ok(())
}
