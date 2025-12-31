use std::time::Duration;

/// Application configuration for timer behavior, WebSocket heartbeats, and cleanup.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// How often the timer ticks to update elapsed time (default: 100ms).
    pub tick_interval: Duration,
    /// How often to send WebSocket ping messages (default: 5s).
    pub heartbeat_interval: Duration,
    /// How long to wait for pong before timing out client (default: 10s).
    pub client_timeout: Duration,
    /// How long to keep idle timers before cleanup (default: 30 minutes).
    pub max_timer_age: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(100),
            heartbeat_interval: Duration::from_secs(5),
            client_timeout: Duration::from_secs(10),
            max_timer_age: Duration::from_secs(30 * 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = Config::default();
        assert_eq!(config.tick_interval, Duration::from_millis(100));
        assert_eq!(config.heartbeat_interval, Duration::from_secs(5));
        assert_eq!(config.client_timeout, Duration::from_secs(10));
        assert_eq!(config.max_timer_age, Duration::from_secs(30 * 60));
    }
}
