use anyhow::Result;
use async_trait::async_trait;
use std::pin::Pin;

/// Represents different types of messages that can be sent or received over a transport connection.
#[derive(Debug, Clone)]
pub enum TransportMessage {
    /// A text message containing UTF-8 encoded string data.
    Text(String),
    /// A binary message containing raw byte data.
    Binary(Vec<u8>),
    /// A ping control frame with optional payload data, used for keep-alive checks.
    Ping(Vec<u8>),
    /// A pong control frame with optional payload data, sent in response to a ping.
    Pong(Vec<u8>),
    /// A close control frame indicating connection termination, with optional reason string.
    Close(Option<String>),
}

/// Abstract interface for bidirectional message transport, typically over WebSockets.
///
/// This trait abstracts the underlying transport mechanism (WebSockets, in-memory channels, etc.)
/// to enable testing without real network connections and to decouple the handler logic from
/// the transport implementation.
///
/// # Implementations
///
/// - [`WebSocketTransport`]: Production implementation using actix-ws for real WebSocket connections
/// - [`mock::MockTransport`]: Test implementation that queues messages in memory
///
/// # Usage
///
/// Handlers use this trait to send and receive messages without knowing the underlying transport:
///
/// ```ignore
/// async fn handler<T: Transport>(transport: &mut T) {
///     // Send a message
///     transport.send_text("{\"status\":\"ok\"}").await?;
///
///     // Receive messages in a loop
///     while let Ok(Some(msg)) = transport.recv().await {
///         match msg {
///             TransportMessage::Text(text) => { /* handle */ }
///             TransportMessage::Close(_) => break,
///             _ => {}
///         }
///     }
/// }
/// ```
#[async_trait(?Send)]
pub trait Transport {
    /// Sends a text message over the transport.
    ///
    /// Returns an error if the transport is closed or if sending fails.
    async fn send_text(&mut self, message: &str) -> Result<()>;

    /// Receives the next message from the transport.
    ///
    /// Returns:
    /// - `Ok(Some(message))` if a message was received
    /// - `Ok(None)` if the transport has been cleanly closed
    /// - `Err(...)` if an error occurred while receiving
    async fn recv(&mut self) -> Result<Option<TransportMessage>>;

    /// Closes the transport connection with an optional reason.
    ///
    /// After calling close, further send/recv operations may fail.
    async fn close(&mut self, reason: Option<&str>) -> Result<()>;

    /// Sends a ping control message.
    ///
    /// Used for heartbeat/keepalive mechanisms. The peer should respond with a pong.
    async fn ping(&mut self, data: &[u8]) -> Result<()>;

    /// Sends a pong control message in response to a ping.
    async fn pong(&mut self, data: &[u8]) -> Result<()>;

    /// Returns whether the transport is still connected.
    ///
    /// Returns `false` if the transport has been explicitly closed or if the connection
    /// has been lost.
    fn is_connected(&self) -> bool;
}

pub struct WebSocketTransport {
    session: Option<actix_ws::Session>,
    stream: Pin<Box<dyn futures_util::Stream<Item = Result<actix_ws::AggregatedMessage, actix_ws::ProtocolError>>>>,
}

impl WebSocketTransport {
    pub fn new(session: actix_ws::Session, stream: actix_ws::MessageStream) -> Self {
        Self {
            session: Some(session),
            stream: Box::pin(stream.aggregate_continuations()),
        }
    }
}

#[async_trait(?Send)]
impl Transport for WebSocketTransport {
    async fn send_text(&mut self, message: &str) -> Result<()> {
        let session = self.session.as_mut().ok_or_else(|| anyhow::anyhow!("Session closed"))?;
        Ok(session.text(message).await?)
    }

    async fn recv(&mut self) -> Result<Option<TransportMessage>> {
        use futures_util::StreamExt;

        match self.stream.next().await {
            Some(Ok(msg)) => {
                let transport_msg = match msg {
                    actix_ws::AggregatedMessage::Text(text) => TransportMessage::Text(text.to_string()),
                    actix_ws::AggregatedMessage::Binary(bytes) => TransportMessage::Binary(bytes.to_vec()),
                    actix_ws::AggregatedMessage::Ping(bytes) => TransportMessage::Ping(bytes.to_vec()),
                    actix_ws::AggregatedMessage::Pong(bytes) => TransportMessage::Pong(bytes.to_vec()),
                    actix_ws::AggregatedMessage::Close(reason) => {
                        TransportMessage::Close(reason.and_then(|r| r.description.map(|d| d.to_string())))
                    }
                };
                Ok(Some(transport_msg))
            }
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn close(&mut self, reason: Option<&str>) -> Result<()> {
        if let Some(session) = self.session.take() {
            let close_reason = reason.map(|r| actix_ws::CloseReason {
                code: actix_ws::CloseCode::Normal,
                description: Some(r.into()),
            });
            Ok(session.close(close_reason).await?)
        } else {
            Ok(())
        }
    }

    async fn ping(&mut self, data: &[u8]) -> Result<()> {
        let session = self.session.as_mut().ok_or_else(|| anyhow::anyhow!("Session closed"))?;
        Ok(session.ping(data).await?)
    }

    async fn pong(&mut self, data: &[u8]) -> Result<()> {
        let session = self.session.as_mut().ok_or_else(|| anyhow::anyhow!("Session closed"))?;
        Ok(session.pong(data).await?)
    }

    fn is_connected(&self) -> bool {
        self.session.is_some()
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    pub struct MockTransport {
        sent_messages: Arc<Mutex<Vec<String>>>,
        incoming_messages: Arc<Mutex<VecDeque<TransportMessage>>>,
        connected: Arc<Mutex<bool>>,
    }

    impl MockTransport {
        pub fn new() -> Self {
            Self {
                sent_messages: Arc::new(Mutex::new(Vec::new())),
                incoming_messages: Arc::new(Mutex::new(VecDeque::new())),
                connected: Arc::new(Mutex::new(true)),
            }
        }

        pub fn queue_message(&self, msg: TransportMessage) {
            self.incoming_messages.lock().unwrap().push_back(msg);
        }

        pub fn get_sent_messages(&self) -> Vec<String> {
            self.sent_messages.lock().unwrap().clone()
        }

        pub fn disconnect(&self) {
            *self.connected.lock().unwrap() = false;
        }
    }

    #[async_trait(?Send)]
    impl Transport for MockTransport {
        async fn send_text(&mut self, message: &str) -> Result<()> {
            if !*self.connected.lock().unwrap() {
                anyhow::bail!("Transport not connected");
            }
            self.sent_messages.lock().unwrap().push(message.to_string());
            Ok(())
        }

        async fn recv(&mut self) -> Result<Option<TransportMessage>> {
            if !*self.connected.lock().unwrap() {
                return Ok(None);
            }
            Ok(self.incoming_messages.lock().unwrap().pop_front())
        }

        async fn close(&mut self, _reason: Option<&str>) -> Result<()> {
            *self.connected.lock().unwrap() = false;
            Ok(())
        }

        async fn ping(&mut self, _data: &[u8]) -> Result<()> {
            if !*self.connected.lock().unwrap() {
                anyhow::bail!("Transport not connected");
            }
            Ok(())
        }

        async fn pong(&mut self, _data: &[u8]) -> Result<()> {
            if !*self.connected.lock().unwrap() {
                anyhow::bail!("Transport not connected");
            }
            Ok(())
        }

        fn is_connected(&self) -> bool {
            *self.connected.lock().unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mock::MockTransport;

    #[tokio::test]
    async fn mock_transport_send_text() {
        let mut transport = MockTransport::new();

        transport.send_text("hello").await.unwrap();
        transport.send_text("world").await.unwrap();

        let sent = transport.get_sent_messages();
        assert_eq!(sent, vec!["hello", "world"]);
    }

    #[tokio::test]
    async fn mock_transport_recv() {
        let mut transport = MockTransport::new();

        transport.queue_message(TransportMessage::Text("test".to_string()));
        transport.queue_message(TransportMessage::Ping(vec![1, 2, 3]));

        let msg1 = transport.recv().await.unwrap();
        assert!(matches!(msg1, Some(TransportMessage::Text(s)) if s == "test"));

        let msg2 = transport.recv().await.unwrap();
        assert!(matches!(msg2, Some(TransportMessage::Ping(data)) if data == vec![1, 2, 3]));

        let msg3 = transport.recv().await.unwrap();
        assert!(msg3.is_none());
    }

    #[tokio::test]
    async fn mock_transport_disconnect() {
        let mut transport = MockTransport::new();

        assert!(transport.is_connected());

        transport.send_text("hello").await.unwrap();

        transport.disconnect();

        assert!(!transport.is_connected());

        let result = transport.send_text("world").await;
        assert!(result.is_err());

        let msg = transport.recv().await.unwrap();
        assert!(msg.is_none());
    }

    #[tokio::test]
    async fn mock_transport_close() {
        let mut transport = MockTransport::new();

        assert!(transport.is_connected());

        transport.close(Some("test reason")).await.unwrap();

        assert!(!transport.is_connected());
    }
}
