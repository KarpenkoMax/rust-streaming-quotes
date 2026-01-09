use std::io;
use std::io::Cursor;
use std::path::PathBuf;

const DEFAULT_TICKERS: &str = include_str!("../assets/tickers.txt");

pub fn load_server_tickers(path: Option<PathBuf>) -> io::Result<Vec<String>> {
    match path {
        Some(p) => quote_core::tickers::read_tickers_from_path(p),
        None => quote_core::tickers::read_tickers(Cursor::new(DEFAULT_TICKERS)),
    }
}
