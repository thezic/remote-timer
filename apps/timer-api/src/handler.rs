use std::time::Instant;

use serde::{Deserialize, Serialize};

use actix_ws::{MessageStream, Session};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use anyhow::Result;

use crate::config::HandlerConfig;
use crate::server::{BoundServerHandle, ServerHandle};
use crate::transport::{Transport, TransportMessage, WebSocketTransport};

pub async fn handler(
    session: Session,
    stream: MessageStream,
    server_handle: ServerHandle,
    timer_id: Uuid,
    config: HandlerConfig,
) {
    let mut transport = WebSocketTransport::new(session, stream);
    handler_with_transport(&mut transport, server_handle, timer_id, config).await
}

async fn handler_with_transport<T: Transport>(
    transport: &mut T,
    server_handle: ServerHandle,
    timer_id: Uuid,
    config: HandlerConfig,
) {
    info!("connected");

    let mut last_heartbeat = Instant::now();
    let mut interval = interval(config.heartbeat_interval);

    let (bound_handle, mut timer_msg) = match server_handle.connect(timer_id).await {
        Ok(stuff) => stuff,
        Err(err) => {
            let _ = transport
                .close(Some(&format!("Failed to connect to timer: {err}")))
                .await;
            return;
        }
    };

    let close_reason: Option<String> = loop {
        let tick = interval.tick();

        tokio::select! {
            msg = transport.recv() => {
                debug!("received message: {msg:?}");
                match msg {
                    Ok(Some(msg)) => match msg {
                        TransportMessage::Text(text) => {
                            if let Err(err) = handle_text_message(&text, &bound_handle).await {
                                error!("Error: {err}");
                            }
                        },
                        TransportMessage::Binary(_) => warn!("Received unexpected binary message"),
                        TransportMessage::Ping(bytes) => {
                            if transport.pong(&bytes).await.is_err() {
                                break Some("Pong failed".to_string());
                            }
                        },
                        TransportMessage::Pong(_) => last_heartbeat = Instant::now(),
                        TransportMessage::Close(reason) => break reason,
                    },
                    Ok(None) => break Some("Connection closed".to_string()),
                    Err(err) => {
                        error!("Error: {err}");
                        break Some(err.to_string());
                    }
                }
            },

            msg = timer_msg.recv() => {
                let json = match serde_json::to_string(&msg) {
                    Ok(json) => json,
                    Err(err) => {
                        error!("Failed to serialize message: {err}");
                        break Some("Serialization failed".to_string());
                    }
                };
                if let Err(err) = transport.send_text(&json).await {
                    error!("Failed to send message: {err}");
                    break Some("Send failed".to_string());
                }
            }

            _tick = tick => {
                if Instant::now() - last_heartbeat > config.client_timeout {
                    break Some("Client timeout".to_string());
                }

                if transport.ping(b"").await.is_err() {
                    break Some("Ping failed".to_string());
                }
            }
        };
    };

    info!("Disconnected: {close_reason:?}");
    let _ = transport.close(close_reason.as_deref()).await;
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Message {
    StartTimer,
    StopTimer,
    SetTime { time: i32 },
}

async fn handle_text_message(message: &str, server_handle: &BoundServerHandle) -> Result<()> {
    let msg: Message = serde_json::from_str(message)?;

    match msg {
        Message::StartTimer => server_handle.start_counter().await,
        Message::StopTimer => server_handle.stop_counter().await,
        Message::SetTime { time } => server_handle.set_time(time).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::MockTransport;
    use crate::transport::TransportMessage;

    #[tokio::test]
    async fn test_mock_transport_receives_text() {
        let mut transport = MockTransport::new();

        transport.queue_message(TransportMessage::Text(r#"{"type":"StartTimer"}"#.to_string()));
        transport.queue_message(TransportMessage::Close(Some("test done".to_string())));

        let msg1 = transport.recv().await.unwrap();
        assert!(matches!(msg1, Some(TransportMessage::Text(s)) if s.contains("StartTimer")));

        let msg2 = transport.recv().await.unwrap();
        assert!(matches!(msg2, Some(TransportMessage::Close(_))));
    }

    #[tokio::test]
    async fn test_mock_transport_send_and_receive() {
        let mut transport = MockTransport::new();

        transport.send_text("hello").await.unwrap();
        transport.send_text("world").await.unwrap();

        let sent = transport.get_sent_messages();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], "hello");
        assert_eq!(sent[1], "world");
    }
}
