use std::fmt;
use serde::{Deserialize, Serialize};

/// структура с данными по акциям для одного тикера
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockQuote {
    /// наименование тикера, например AMZN NVDA TSLA
    pub ticker: String,
    /// цена (целое, в "копейках")
    pub price: i64,
    /// кол-во акций
    pub volume: u32,
    /// время формирования
    pub timestamp_ms: u128,
}

impl fmt::Display for StockQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs: u64 = self.price.unsigned_abs();
        let major = abs / 100;
        let minor = abs % 100;

        let sign = if self.price < 0 { "-" } else { "" };

        write!(
            f,
            "{} price={}{}.{:02} volume={} ts_ms={}",
            self.ticker, sign, major, minor, self.volume, self.timestamp_ms
        )
    }
}
