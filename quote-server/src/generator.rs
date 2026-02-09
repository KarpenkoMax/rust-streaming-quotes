use quote_core::StockQuote;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub(crate) struct GeneratorConfig {
    /// Максимальный относительный шаг цены за тик (пример: 0.002 = 0.2%)
    pub(crate) max_rel_step: f64,
    /// Минимальная допустимая цена
    pub(crate) min_price: i64,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            max_rel_step: 0.002,
            min_price: 1,
        }
    }
}

/// Внутреннее состояние тикера.
#[derive(Debug, Clone)]
struct TickerState {
    price: i64,
}

pub(crate) struct QuoteGenerator {
    cfg: GeneratorConfig,
    states: HashMap<String, TickerState>,

    /// Набор "высоколиквидных" тикеров для более крупного volume.
    high_volume: HashSet<String>,
}

impl QuoteGenerator {
    pub(crate) fn new(tickers: Vec<String>, cfg: GeneratorConfig) -> Self {
        let mut rng = rand::rng();

        let states = tickers
            .into_iter()
            .map(|t| {
                let start_price = rng.random_range(5000..50000);

                (t, TickerState { price: start_price })
            })
            .collect::<HashMap<_, _>>();

        let high_volume = ["AAPL", "MSFT", "TSLA"]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<HashSet<_>>();

        Self {
            cfg,
            states,
            high_volume,
        }
    }

    /// сгенерировать котировку для тикера
    pub(crate) fn next_quote(&mut self, ticker: &str) -> Option<StockQuote> {
        let st = self.states.get_mut(ticker)?;

        let mut rng = rand::rng();

        let delta = rng.random_range(-self.cfg.max_rel_step..self.cfg.max_rel_step);

        st.price = (((1.0 + delta) * (st.price as f64)).round() as i64).max(self.cfg.min_price);

        // volume: популярные -> больше
        let volume = if self.high_volume.contains(ticker) {
            1000 + rng.random_range(0..5000)
        } else {
            100 + rng.random_range(0..1000)
        };

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_millis();

        Some(StockQuote {
            ticker: ticker.to_string(),
            price: st.price,
            volume,
            timestamp_ms,
        })
    }

    /// сгенерировать котировки для всех тикеров
    pub(crate) fn next_batch(&mut self) -> Vec<StockQuote> {
        let keys: Vec<String> = self.states.keys().cloned().collect();

        let mut out = Vec::with_capacity(keys.len());

        for t in keys {
            if let Some(q) = self.next_quote(&t) {
                out.push(q);
            }
        }

        out
    }
}
