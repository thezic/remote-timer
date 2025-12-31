use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::server::BoundServerHandle;
use crate::timer::TimerMessage;

/// Commands that can be sent from clients to control the timer.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ClientCommand {
    StartTimer,
    StopTimer,
    SetTime { time: i32 },
}

/// Parse a JSON text message from a client into a command.
///
/// # Examples
///
/// ```
/// use timer_api::message_handler::parse_client_message;
///
/// let result = parse_client_message(r#"{"type":"StartTimer"}"#);
/// assert!(result.is_ok());
/// ```
pub fn parse_client_message(text: &str) -> Result<ClientCommand> {
    let cmd: ClientCommand = serde_json::from_str(text)?;
    Ok(cmd)
}

/// Execute a client command by calling the appropriate method on the server handle.
///
/// This is a thin wrapper that maps commands to server handle method calls.
/// Most of the actual logic lives in the server and timer implementations.
pub async fn handle_client_command(
    cmd: ClientCommand,
    server_handle: &BoundServerHandle,
) -> Result<()> {
    match cmd {
        ClientCommand::StartTimer => server_handle.start_counter().await,
        ClientCommand::StopTimer => server_handle.stop_counter().await,
        ClientCommand::SetTime { time } => server_handle.set_time(time).await,
    }
}

/// Serialize a timer message to JSON for sending to clients.
///
/// # Examples
///
/// ```
/// use timer_api::message_handler::serialize_timer_message;
/// use timer_api::timer::TimerMessage;
///
/// let msg = TimerMessage {
///     is_running: false,
///     current_time: 0,
///     target_time: 5000,
///     client_count: 1,
/// };
///
/// let json = serialize_timer_message(&msg).unwrap();
/// assert!(json.contains("\"is_running\":false"));
/// ```
pub fn serialize_timer_message(msg: &TimerMessage) -> Result<String> {
    Ok(serde_json::to_string(msg)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_start_timer() {
        let result = parse_client_message(r#"{"type":"StartTimer"}"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ClientCommand::StartTimer);
    }

    #[test]
    fn test_parse_stop_timer() {
        let result = parse_client_message(r#"{"type":"StopTimer"}"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ClientCommand::StopTimer);
    }

    #[test]
    fn test_parse_set_time() {
        let result = parse_client_message(r#"{"type":"SetTime","time":5000}"#);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            ClientCommand::SetTime { time: 5000 }
        );
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_client_message("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unknown_type() {
        let result = parse_client_message(r#"{"type":"UnknownCommand"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_time_field() {
        let result = parse_client_message(r#"{"type":"SetTime"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_malformed_json() {
        let result = parse_client_message(r#"{"type":"StartTimer""#);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_string() {
        let result = parse_client_message("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_extra_fields_ignored() {
        let result = parse_client_message(r#"{"type":"StartTimer","extra":"field"}"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ClientCommand::StartTimer);
    }

    #[test]
    fn test_serialize_timer_message() {
        let msg = TimerMessage {
            is_running: false,
            current_time: 0,
            target_time: 5000,
            client_count: 1,
        };

        let json = serialize_timer_message(&msg).unwrap();

        // Check that JSON contains expected fields
        assert!(json.contains("\"is_running\":false"));
        assert!(json.contains("\"current_time\":0"));
        assert!(json.contains("\"target_time\":5000"));
        assert!(json.contains("\"client_count\":1"));
    }

    #[test]
    fn test_serialize_running_timer() {
        let msg = TimerMessage {
            is_running: true,
            current_time: 1234,
            target_time: 5000,
            client_count: 2,
        };

        let json = serialize_timer_message(&msg).unwrap();
        assert!(json.contains("\"is_running\":true"));
        assert!(json.contains("\"current_time\":1234"));
        assert!(json.contains("\"client_count\":2"));
    }

    #[test]
    fn test_roundtrip_serialization() {
        let msg = TimerMessage {
            is_running: true,
            current_time: 999,
            target_time: 3000,
            client_count: 5,
        };

        let json = serialize_timer_message(&msg).unwrap();
        let deserialized: TimerMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.is_running, msg.is_running);
        assert_eq!(deserialized.current_time, msg.current_time);
        assert_eq!(deserialized.target_time, msg.target_time);
        assert_eq!(deserialized.client_count, msg.client_count);
    }
}
