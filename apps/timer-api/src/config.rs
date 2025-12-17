use std::time::Duration;

/// Configuration for the timer's tick behavior.
#[derive(Clone, Debug, PartialEq)]
pub struct TimerConfig {
    /// Interval between timer ticks. Determines how frequently elapsed time is calculated.
    pub tick_interval: Duration,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(100),
        }
    }
}

impl TimerConfig {
    pub fn for_testing() -> Self {
        Self {
            tick_interval: Duration::from_millis(1), // Much faster for tests
        }
    }
}

/// Configuration for WebSocket connection handlers that manage client connections.
///
/// Handlers use a heartbeat mechanism to detect disconnected clients:
/// - Periodically sends ping messages to clients based on `heartbeat_interval`
/// - Tracks when pong responses are received from clients
/// - Disconnects clients that haven't responded within `client_timeout`
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use timer_api::config::HandlerConfig;
///
/// // Production configuration with 5-second heartbeats
/// let config = HandlerConfig::default();
/// assert_eq!(config.heartbeat_interval, Duration::from_secs(5));
///
/// // Fast configuration for testing
/// let test_config = HandlerConfig::for_testing();
/// assert_eq!(test_config.heartbeat_interval, Duration::from_millis(10));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerConfig {
    /// Interval at which heartbeat ping messages are sent to connected clients.
    /// Shorter intervals detect disconnections faster but increase network traffic.
    pub heartbeat_interval: Duration,
    /// Maximum duration without receiving a pong response before considering a client timed out.
    /// Should be larger than `heartbeat_interval` to allow for network latency.
    pub client_timeout: Duration,
}

impl Default for HandlerConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(5),
            client_timeout: Duration::from_secs(10),
        }
    }
}

impl HandlerConfig {
    pub fn for_testing() -> Self {
        Self {
            heartbeat_interval: Duration::from_millis(10), // Much faster for tests
            client_timeout: Duration::from_millis(50),
        }
    }
}

/// Configuration for the timer server's resource management and cleanup behavior.
///
/// The server manages multiple timer instances identified by UUIDs. To prevent memory leaks
/// from abandoned timers (e.g., when all clients disconnect and never reconnect), the server
/// periodically cleans up old idle timers.
///
/// A timer is considered eligible for cleanup when:
/// - It has no connected clients
/// - It has existed for longer than `max_timer_age`
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use timer_api::config::ServerConfig;
///
/// // Production: keep idle timers for 30 minutes
/// let config = ServerConfig::default();
/// assert_eq!(config.max_timer_age, Duration::from_secs(30 * 60));
///
/// // Testing: cleanup after 100ms for faster tests
/// let test_config = ServerConfig::for_testing();
/// assert_eq!(test_config.max_timer_age, Duration::from_millis(100));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ServerConfig {
    /// Maximum age of idle timers (those with no connected clients) before they become
    /// eligible for cleanup. Longer durations use more memory but allow clients to reconnect
    /// to existing timers. Shorter durations save memory but require clients to create new
    /// timers if they reconnect after this duration.
    pub max_timer_age: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_timer_age: Duration::from_secs(30 * 60), // 30 minutes
        }
    }
}

impl ServerConfig {
    pub fn for_testing() -> Self {
        Self {
            max_timer_age: Duration::from_millis(100), // Much faster cleanup for tests
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_config_defaults() {
        let config = TimerConfig::default();
        assert_eq!(config.tick_interval, Duration::from_millis(100));
    }

    #[test]
    fn timer_config_for_testing() {
        let config = TimerConfig::for_testing();
        assert_eq!(config.tick_interval, Duration::from_millis(1));
    }

    #[test]
    fn handler_config_defaults() {
        let config = HandlerConfig::default();
        assert_eq!(config.heartbeat_interval, Duration::from_secs(5));
        assert_eq!(config.client_timeout, Duration::from_secs(10));
    }

    #[test]
    fn handler_config_for_testing() {
        let config = HandlerConfig::for_testing();
        assert_eq!(config.heartbeat_interval, Duration::from_millis(10));
        assert_eq!(config.client_timeout, Duration::from_millis(50));
    }

    #[test]
    fn server_config_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.max_timer_age, Duration::from_secs(30 * 60));
    }

    #[test]
    fn server_config_for_testing() {
        let config = ServerConfig::for_testing();
        assert_eq!(config.max_timer_age, Duration::from_millis(100));
    }
}