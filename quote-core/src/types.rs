use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockQuote {
    pub ticker: String,
    pub price: f64,
    pub volume: u32,
    pub timestamp_ms: u64,
}

impl StockQuote {
    /// Текстовый wire-формат: TICKER|PRICE|VOLUME|TIMESTAMP_MS
    pub fn to_wire(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.ticker, self.price, self.volume, self.timestamp_ms
        )
    }

    pub fn from_wire(s: &str) -> Option<Self> {
        let mut it = s.split('|');
        Some(Self {
            ticker: it.next()?.to_string(),
            price: it.next()?.parse().ok()?,
            volume: it.next()?.parse().ok()?,
            timestamp_ms: it.next()?.parse().ok()?,
        })
    }
}

pub const PING_MSG: &[u8] = b"PING";

pub const PING_TIMEOUT: Duration = Duration::from_secs(5);
pub const PING_INTERVAL: Duration = Duration::from_secs(2);
pub const QUOTE_INTERVAL: Duration = Duration::from_millis(500);
