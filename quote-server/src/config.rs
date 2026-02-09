use std::io;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_TICKERS: &str = include_str!("../assets/tickers.txt");

pub(crate) const UDP_SOCKET_TICK: Duration = Duration::from_millis(10);
pub(crate) use quote_core::PING_TIMEOUT;

pub(crate) const QUOTE_INTERVAL: Duration = Duration::from_millis(500);

pub(crate) const TCP_BIND_ADDR: &str = "0.0.0.0:5555";
pub(crate) const UDP_BIND_ADDR: &str = "0.0.0.0:5556";

pub(crate) fn load_server_tickers(path: Option<PathBuf>) -> io::Result<Vec<String>> {
    match path {
        Some(p) => quote_core::tickers::read_tickers_from_path(p),
        None => quote_core::tickers::read_tickers(Cursor::new(DEFAULT_TICKERS)),
    }
}

pub(crate) type ClientId = u64;
