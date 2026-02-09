use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::cli::Args;

#[derive(Debug, Error)]
pub(crate) enum TickersError {
    /// Clap-логика должна гарантировать источник тикеров, но на всякий случай
    #[error("tickers source is missing: provide either --tickers-file or --tickers")]
    MissingSource,

    #[error("tickers list is empty (file: {path:?})")]
    EmptyFromFile { path: PathBuf },

    #[error("tickers list is empty (--tickers value: {raw:?})")]
    EmptyFromArg { raw: String },

    #[error("failed to read tickers file: {path:?}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) type Result<T> = std::result::Result<T, TickersError>;

/// Загружает тикеры из источника, выбранного в CLI:
/// - `--tickers-file` -> quote_core::tickers::read_tickers_from_path
/// - `--tickers`      -> quote_core::tickers::parse_tickers_csv
pub(crate) fn load_tickers(args: &Args) -> Result<Vec<String>> {
    if let Some(path) = &args.tickers_file {
        load_from_file(path)
    } else if let Some(raw) = &args.tickers {
        load_from_arg(raw)
    } else {
        Err(TickersError::MissingSource)
    }
}

fn load_from_file(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let path = path.as_ref().to_path_buf();

    let tickers = quote_core::tickers::read_tickers_from_path(&path).map_err(|e| {
        TickersError::ReadFile {
            path: path.clone(),
            source: e,
        }
    })?;

    if tickers.is_empty() {
        return Err(TickersError::EmptyFromFile { path });
    }

    Ok(tickers)
}

fn load_from_arg(raw: &str) -> Result<Vec<String>> {
    let tickers = quote_core::tickers::parse_tickers_csv(raw);

    if tickers.is_empty() {
        return Err(TickersError::EmptyFromArg {
            raw: raw.to_string(),
        });
    }

    Ok(tickers)
}
