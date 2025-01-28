use std::{
    pin::pin,
    time::{Duration, Instant},
};

use actix_ws::{CloseReason, MessageStream, Session};
use futures_util::StreamExt as _;
use tokio::time::interval;
use tracing::{error, info, warn};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handler(mut session: Session, stream: MessageStream) {
    info!("connected");

    let mut stream = stream.aggregate_continuations();
    let mut stream = pin!(stream);

    let mut last_heartbeat = Instant::now();
    let mut interval = interval(HEARTBEAT_INTERVAL);

    let reason: Option<CloseReason> = loop {
        let tick = interval.tick();

        tokio::select! {
            msg = stream.next() => {
                info!("received message: {msg:?}");
                match msg {
                    Some(Ok(msg)) => match msg {
                        actix_ws::AggregatedMessage::Text(text) => {
                            info!("Recived text: {text}");
                            session.text(text).await.unwrap();
                        },
                        actix_ws::AggregatedMessage::Binary(_) => warn!("Received unexpected binary message"),
                        actix_ws::AggregatedMessage::Ping(bytes) => session.pong(&bytes).await.unwrap(),
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
