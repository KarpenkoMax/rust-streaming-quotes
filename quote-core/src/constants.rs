use std::time::Duration;

/// время, после которого соединение считается "мёртвым"
pub const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Интервал ping
pub const PING_INTERVAL: Duration = Duration::from_secs(2);
