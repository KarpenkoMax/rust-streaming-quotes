//! Точка входа `quote-client`.
//!
//! Жизненный цикл:
//! - парсинг CLI и загрузка списка тикеров
//! - одноразовый TCP-запрос `STREAM` и ожидание `OK/ERR`
//! - запуск UDP-цикла приёма котировок
//! - запуск keep-alive ping в отдельном потоке с того же UDP-порта
//! - корректная остановка по `Ctrl+C`

mod cli;
mod tickers;
mod tcp;
mod udp;
use std::net::SocketAddr;
use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};

use clap::Parser;
use log::{info};

fn main() -> anyhow::Result<()> {
    // Логи через RUST_LOG=info/trace
    env_logger::init();

    let shutdown = Arc::new(AtomicBool::new(false));

    // Ctrl+C => ставим shutdown=true
    {
        let shutdown = shutdown.clone();
        ctrlc::set_handler(move || {
            shutdown.store(true, Ordering::Relaxed);
            info!("shutting down...");
        })?;
    }

    let args = cli::Args::parse();
    args.validate()?; // оставляем как есть, если validate() у тебя на anyhow::Result

    let tickers = tickers::load_tickers(&args)
        .map_err(|e| anyhow::anyhow!(e))?;

    info!(
        "Starting quote-client: server_tcp={}, udp_port={}, advertise_ip={}, tickers={}",
        args.tcp_server(),
        args.udp_port,
        args.advertise_ip(),
        tickers.join(",")
    );

    let udp_advertise_addr = SocketAddr::new(args.advertise_ip(), args.udp_port);
    let udp_bind_addr = SocketAddr::from(([0, 0, 0, 0], args.udp_port));    

    // запрос на стрим
    tcp::send_stream_command(args.server_socket_addr()?, udp_advertise_addr, tickers.as_slice())?;

    udp::run_udp_receiver(udp_bind_addr, shutdown)?;

    Ok(())
}
