use std::path::PathBuf;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use anyhow::{bail, Context, Result};
use clap::{ArgGroup, Parser};

/// Quote Client - подписка на котировки через quote-server.
///
/// TCP используется один раз: отправляем STREAM и ждём OK/ERR.
/// Дальше принимаем котировки по UDP и шлём Ping keep-alive.
#[derive(Parser, Debug, Clone)]
#[command(name = "quote-client", version, about)]
#[command(
    group(
        ArgGroup::new("tickers_source")
            .required(true)
            .args(["tickers_file", "tickers"])
    )
)]
pub(crate) struct Args {
    /// TCP адрес quote-server, например 127.0.0.1:5555 или host.example.com:5555
    #[arg(long)]
    pub(crate) server: String,

    /// Локальный UDP порт, на который будут приходить котировки
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=65535))]
    pub(crate) udp_port: u16,

    /// IP, который клиент объявляет серверу в udp://IP:PORT
    /// (обычно 127.0.0.1 для локального запуска; в проде — реальный IP интерфейса)
    #[arg(long, default_value = "127.0.0.1")]
    pub(crate) bind_ip: IpAddr,

    /// Файл тикеров (по одному на строку). Нельзя вместе с --tickers
    #[arg(long, conflicts_with = "tickers")]
    pub(crate) tickers_file: Option<PathBuf>,

    /// Список тикеров строкой, например: "AAPL,TSLA" или "AAPL, TSLA, GOOG"
    /// Нельзя вместе с --tickers-file
    #[arg(long, conflicts_with = "tickers_file")]
    pub(crate) tickers: Option<String>,
}

impl Args {
    /// Валидация аргументов (файл существует, server выглядит как HOST:PORT и т.д.)
    pub(crate) fn validate(&self) -> Result<()> {
        if self.server.trim().is_empty() {
            bail!("--server is empty");
        }
        if !self.server.contains(':') {
            bail!("--server must look like HOST:PORT (got: {})", self.server);
        }

        if let Some(path) = &self.tickers_file {
            let md = std::fs::metadata(path)
                .with_context(|| format!("tickers file not found: {:?}", path))?;
            if !md.is_file() {
                bail!("--tickers-file must point to a file: {:?}", path);
            }
        }

        // ArgGroup уже гарантирует, что ровно один из (tickers_file|tickers) задан,
        // но оставим защиту на всякий случай:
        if self.tickers_file.is_none() && self.tickers.is_none() {
            bail!("either --tickers-file or --tickers must be provided");
        }
        if self.tickers_file.is_some() && self.tickers.is_some() {
            bail!("--tickers-file and --tickers are mutually exclusive");
        }

        Ok(())
    }

    pub(crate) fn tcp_server(&self) -> &str {
        self.server.as_str()
    }

    pub(crate) fn advertise_ip(&self) -> IpAddr {
        self.bind_ip
    }

    pub(crate) fn server_socket_addr(&self) -> std::io::Result<SocketAddr> {
        // Берём первый результат резолвинга
        self.server
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no addresses resolved"))
    }
}
