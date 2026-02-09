use clap::{ArgGroup, Parser};
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::config;

/// Quote Server - раздаёт котировки по UDP, управляется по TCP командой STREAM.
#[derive(Parser, Debug, Clone)]
#[command(name = "quote-server", version, about)]
#[command(
    group(
        ArgGroup::new("tickers_source")
            .required(false)
            .multiple(false)
            .args(["tickers_file", "tickers"])
    )
)]
pub(crate) struct Args {
    /// TCP bind address, например 0.0.0.0:5555
    #[arg(long, default_value = config::TCP_BIND_ADDR)]
    pub(crate) tcp_bind: SocketAddr,

    /// UDP bind address, например 0.0.0.0:5556
    #[arg(long, default_value = config::UDP_BIND_ADDR)]
    pub(crate) udp_bind: SocketAddr,

    /// Источник тикеров: файл (по одному тикеру на строку, поддержка # комментариев)
    #[arg(long, conflicts_with = "tickers")]
    pub(crate) tickers_file: Option<PathBuf>,

    /// Источник тикеров: текст. Поддерживает:
    /// - CSV: "AAPL, TSLA, GOOG"
    /// - многострочный текст: "AAPL\nTSLA\n#comment\nGOOG"
    #[arg(long, conflicts_with = "tickers_file")]
    pub(crate) tickers: Option<String>,
}
