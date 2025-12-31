use std::time::Instant;

use actix_ws::{MessageStream, Session};
use futures_util::StreamExt;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::HandlerConfig;
use crate::message_handler::{handle_client_command, parse_client_message, serialize_timer_message};
use crate::server::ServerHandle;

/// WebSocket handler for timer connections.
///
/// This is a thin integration layer that:
/// - Receives WebSocket messages and parses them using message_handler
/// - Forwards timer updates to connected clients
/// - Manages heartbeat/ping-pong for connection health
///
/// All business logic lives in message_handler (parsing/serialization) and
/// server/timer (state management). This handler is intentionally simple.
pub async fn handler(
    mut session: Session,
    stream: MessageStream,
    server_handle: ServerHandle,
    timer_id: Uuid,
    config: HandlerConfig,
) {
    info!("connected");

    let mut stream = stream.aggregate_continuations();
    let mut last_heartbeat = Instant::now();
    let mut interval = interval(config.heartbeat_interval);

    // Connect to the timer server
    let (bound_handle, mut timer_msg) = match server_handle.connect(timer_id).await {
        Ok(result) => result,
        Err(err) => {
            error!("Failed to connect to timer {timer_id}: {err}");
            let _ = session
                .close(Some(actix_ws::CloseReason {
                    code: actix_ws::CloseCode::Error,
                    description: Some(format!("Failed to connect to timer: {err}")),
                }))
                .await;
            return;
        }
    };

    // Main event loop
    let close_reason: Option<actix_ws::CloseReason> = loop {
        let tick = interval.tick();

        tokio::select! {
            // Handle incoming WebSocket messages
            msg = stream.next() => {
                debug!("received message: {msg:?}");
                match msg {
                    Some(Ok(actix_ws::AggregatedMessage::Text(text))) => {
                        match parse_client_message(&text) {
                            Ok(cmd) => {
                                if let Err(err) = handle_client_command(cmd, &bound_handle).await {
                                    error!("Error handling command: {err}");
                                }
                            }
                            Err(err) => {
                                error!("Failed to parse message: {err}");
                            }
                        }
                    }
                    Some(Ok(actix_ws::AggregatedMessage::Binary(_))) => {
                        warn!("Received unexpected binary message");
                    }
                    Some(Ok(actix_ws::AggregatedMessage::Ping(bytes))) => {
                        if session.pong(&bytes).await.is_err() {
                            break Some(actix_ws::CloseReason {
                                code: actix_ws::CloseCode::Abnormal,
                                description: Some("Pong failed".to_string()),
                            });
                        }
                    }
                    Some(Ok(actix_ws::AggregatedMessage::Pong(_))) => {
                        last_heartbeat = Instant::now();
                    }
                    Some(Ok(actix_ws::AggregatedMessage::Close(reason))) => {
                        break reason;
                    }
                    Some(Err(err)) => {
                        error!("WebSocket error: {err}");
                        break Some(actix_ws::CloseReason {
                            code: actix_ws::CloseCode::Error,
                            description: Some(err.to_string()),
                        });
                    }
                    None => {
                        break Some(actix_ws::CloseReason {
                            code: actix_ws::CloseCode::Normal,
                            description: Some("Connection closed".to_string()),
                        });
                    }
                }
            }

            // Forward timer updates to client
            msg = timer_msg.recv() => {
                let Some(msg) = msg else {
                    // Timer channel closed
                    break Some(actix_ws::CloseReason {
                        code: actix_ws::CloseCode::Normal,
                        description: Some("Timer closed".to_string()),
                    });
                };

                match serialize_timer_message(&msg) {
                    Ok(json) => {
                        if let Err(err) = session.text(json).await {
                            error!("Failed to send timer update: {err}");
                            break Some(actix_ws::CloseReason {
                                code: actix_ws::CloseCode::Error,
                                description: Some("Send failed".to_string()),
                            });
                        }
                    }
                    Err(err) => {
                        error!("Failed to serialize timer message: {err}");
                        break Some(actix_ws::CloseReason {
                            code: actix_ws::CloseCode::Error,
                            description: Some("Serialization failed".to_string()),
                        });
                    }
                }
            }

            // Heartbeat tick
            _tick = tick => {
                if Instant::now() - last_heartbeat > config.client_timeout {
                    break Some(actix_ws::CloseReason {
                        code: actix_ws::CloseCode::Error,
                        description: Some("Client timeout".to_string()),
                    });
                }

                if session.ping(b"").await.is_err() {
                    break Some(actix_ws::CloseReason {
                        code: actix_ws::CloseCode::Error,
                        description: Some("Ping failed".to_string()),
                    });
                }
            }
        }
    };

    info!("Disconnected: {close_reason:?}");
    let _ = session.close(close_reason).await;
}
