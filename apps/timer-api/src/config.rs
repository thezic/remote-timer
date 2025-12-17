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

/// Configuration for WebSocket connection handlers.
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerConfig {
    /// Interval at which heartbeat pings are sent to clients.
    pub heartbeat_interval: Duration,
    /// Duration after which an unresponsive client is considered timed out.
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

/// Configuration for the timer server's cleanup behavior.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerConfig {
    /// Maximum age of idle timers before they become eligible for cleanup.
    /// Timers with no connected clients older than this duration may be removed.
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