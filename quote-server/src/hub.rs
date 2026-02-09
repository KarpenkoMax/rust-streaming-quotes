use crate::config::ClientId;
use crossbeam_channel::{Receiver, Sender, TrySendError};
use quote_core::StockQuote;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum HubError {
    #[error("client already exists: {0}")]
    ClientAlreadyExists(ClientId),
}

#[derive(Debug)]
pub(crate) struct BroadcastStats {
    sent: usize,
    dropped_full: usize,
    dropped_dead: usize,
}

impl fmt::Display for BroadcastStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sent={} dropped_full={} dropped_dead={}",
            self.sent, self.dropped_full, self.dropped_dead
        )
    }
}

impl BroadcastStats {
    pub(crate) fn not_empty(&self) -> bool {
        if self.sent + self.dropped_dead + self.dropped_full > 0 {
            return true;
        }
        false
    }
}

pub(crate) struct Hub {
    clients: Mutex<HashMap<ClientId, Sender<Arc<StockQuote>>>>,
    capacity_per_client: usize,
}

impl Hub {
    pub(crate) fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            capacity_per_client: 256,
        }
    }

    pub(crate) fn add_client(&self, cid: ClientId) -> Result<Receiver<Arc<StockQuote>>, HubError> {
        let mut clients = match self.clients.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(), // продолжаем, несмотря на poison
        };

        match clients.entry(cid) {
            Entry::Vacant(e) => {
                let (tx, rx) = crossbeam_channel::bounded(self.capacity_per_client);
                e.insert(tx);
                Ok(rx)
            }
            Entry::Occupied(_) => Err(HubError::ClientAlreadyExists(cid)),
        }
    }

    pub(crate) fn remove_client(&self, cid: ClientId) -> bool {
        let mut clients = match self.clients.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(), // продолжаем, несмотря на poison
        };

        clients.remove(&cid).is_some()
    }

    pub(crate) fn broadcast(&self, q: StockQuote) -> BroadcastStats {
        let q = Arc::new(q);

        let clients_snapshot: Vec<(ClientId, Sender<Arc<StockQuote>>)> = {
            let clients = match self.clients.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            clients.iter().map(|(&cid, tx)| (cid, tx.clone())).collect()
        };

        let mut sent: usize = 0;
        let mut dropped_full: usize = 0;
        let mut dropped_disconnected: Vec<ClientId> = Vec::new();

        for (cid, tx) in clients_snapshot.iter() {
            match tx.try_send(q.clone()) {
                Ok(()) => sent += 1,
                Err(TrySendError::Disconnected(_)) => dropped_disconnected.push(*cid),
                Err(TrySendError::Full(_)) => dropped_full += 1,
            }
        }

        for cid in &dropped_disconnected {
            self.remove_client(*cid);
        }

        BroadcastStats {
            sent,
            dropped_full,
            dropped_dead: dropped_disconnected.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn mk_quote(ticker: &str, price: i64) -> StockQuote {
        StockQuote {
            ticker: ticker.to_string(),
            price,
            volume: 1,
            timestamp_ms: 1,
        }
    }

    #[test]
    fn add_client_ok_and_duplicate_fails() {
        let hub = Hub::new();

        let _rx = hub.add_client(1).expect("add_client should succeed");

        let err = hub.add_client(1).unwrap_err();
        assert!(matches!(err, HubError::ClientAlreadyExists(1)));
    }

    #[test]
    fn remove_client_returns_bool() {
        let hub = Hub::new();

        // нет такого клиента
        assert!(!hub.remove_client(42));

        // добавили -> удалили
        let _rx = hub.add_client(42).unwrap();
        assert!(hub.remove_client(42));

        // уже удалён
        assert!(!hub.remove_client(42));
    }

    #[test]
    fn broadcast_delivers_to_client() {
        let hub = Hub::new();
        let rx = hub.add_client(1).unwrap();

        let q = mk_quote("AAPL", 123_4500);
        let st = hub.broadcast(q.clone());

        assert_eq!(st.sent, 1);
        assert_eq!(st.dropped_full, 0);
        assert_eq!(st.dropped_dead, 0);

        let got = rx
            .recv_timeout(Duration::from_millis(200))
            .expect("should receive quote");
        assert_eq!(*got, q);
    }

    #[test]
    fn broadcast_counts_full_drop_when_client_not_reading() {
        let hub = Hub {
            clients: Mutex::new(HashMap::new()),
            capacity_per_client: 1,
        };

        let _rx = hub.add_client(1).unwrap();

        // Первый broadcast заполнит буфер
        let st1 = hub.broadcast(mk_quote("AAPL", 1));
        assert_eq!(st1.sent, 1);
        assert_eq!(st1.dropped_full, 0);

        // Второй broadcast не влезет, т.к. rx не читает
        let st2 = hub.broadcast(mk_quote("AAPL", 2));
        assert_eq!(st2.sent, 0);
        assert_eq!(st2.dropped_full, 1);
        assert_eq!(st2.dropped_dead, 0);
    }

    #[test]
    fn broadcast_removes_disconnected_client() {
        // capacity не важен
        let hub = Hub::new();

        // добавили и сразу дропнули receiver -> sender станет disconnected
        let rx = hub.add_client(1).unwrap();
        drop(rx);

        let st = hub.broadcast(mk_quote("AAPL", 1));

        assert_eq!(st.sent, 0);
        assert_eq!(st.dropped_dead, 1);

        // После broadcast хаб должен почистить реестр
        assert!(!hub.remove_client(1));
    }
}
