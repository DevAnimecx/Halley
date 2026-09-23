//! Timeout settings (centralized — UI must not override per component).

use std::time::Duration;

/// Resolved timeout durations for the network manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutSettings {
    connect: Duration,
    request: Duration,
    idle: Duration,
}

impl TimeoutSettings {
    /// From validated network config (milliseconds).
    pub fn from_config(config: &halley_common::NetworkConfig) -> Self {
        TimeoutSettings {
            connect: Duration::from_millis(config.connect_timeout_ms.max(1)),
            request: Duration::from_millis(config.request_timeout_ms.max(1)),
            idle: Duration::from_millis(config.idle_timeout_ms.max(1)),
        }
    }

    /// Connect/handshake budget.
    pub fn connect(&self) -> Duration {
        self.connect
    }

    /// Full request budget.
    pub fn request(&self) -> Duration {
        self.request
    }

    /// Idle connection budget.
    pub fn idle(&self) -> Duration {
        self.idle
    }
}

impl From<&halley_common::NetworkConfig> for TimeoutSettings {
    fn from(config: &halley_common::NetworkConfig) -> Self {
        Self::from_config(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_not_absurdly_short() {
        let t = TimeoutSettings::from_config(&halley_common::NetworkConfig::default());
        assert!(t.connect() >= Duration::from_secs(1));
        assert!(t.request() >= t.connect());
        assert!(t.idle() >= Duration::from_secs(1));
    }

    #[test]
    fn zero_config_is_clamped_to_one_ms_not_zero() {
        // validate() rejects this at startup; from_config still never
        // produces a zero Duration (defense in depth).
        let config = halley_common::NetworkConfig {
            connect_timeout_ms: 0,
            ..Default::default()
        };
        let t = TimeoutSettings::from_config(&config);
        assert!(t.connect() >= Duration::from_millis(1));
    }
}
