//! Точка входа `quote-server`.
//!
//! Жизненный цикл:
//! - парсинг CLI и установка обработчика `Ctrl+C`
//! - запуск общего UDP-сокета и потока приёма ping
//! - запуск потока генерации котировок и рассылки в сессии
//! - запуск TCP-listener: `STREAM` и создание сессии на клиента
//! - при shutdown: корректное завершение и `join` фоновых потоков

use std::collections::HashMap;
use std::io::Cursor;
use std::net::UdpSocket;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, RwLock,
};
use std::thread;
use clap::Parser;
use log::{info, warn};

mod cli;
mod config;
mod generator;
mod hub;
mod session;
mod tcp;
mod udp_ping;

use crate::cli::Args;
use crate::hub::Hub;
use crate::udp_ping::{run_udp_ping_listener, LastPingMap};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    let shutdown = Arc::new(AtomicBool::new(false));

    // Ctrl+C => ставим shutdown=true
    {
        let shutdown = shutdown.clone();
        ctrlc::set_handler(move || {
            shutdown.store(true, Ordering::Relaxed);
            info!("shutting down...");
        })?;
    }

    // shared state
    let hub = Arc::new(Hub::new());
    let curr_client_id = Arc::new(AtomicU64::new(1));
    let last_ping: LastPingMap = Arc::new(RwLock::new(HashMap::new()));

    // общий UDP-сокет
    let udp = Arc::new(UdpSocket::bind(args.udp_bind)?);
    info!("UDP bound on {}", args.udp_bind);

    let mut handles = Vec::new();

    // слушаем PING по UDP и обновляем last_ping
    {
        let udp = udp.clone();
        let last_ping = last_ping.clone();
        let shutdown = shutdown.clone();
        handles.push(thread::spawn(move || {
            if let Err(e) = run_udp_ping_listener(udp, last_ping, shutdown) {
                warn!("udp ping listener stopped: {e}");
            }
        }));
    }

    // тикеры генератора: default / файл / текст
    let tickers = load_server_tickers_from_args(&args)?;

    // генерация котировок + broadcast в hub
    {
        let hub = hub.clone();
        let shutdown = shutdown.clone();

        handles.push(thread::spawn(move || {
            let gen_cfg = generator::GeneratorConfig::default();
            let mut q_gen = generator::QuoteGenerator::new(tickers, gen_cfg);

            while !shutdown.load(Ordering::Relaxed) {
                let quote_batch = q_gen.next_batch();
                for q in quote_batch.into_iter() {
                    let stats = hub.broadcast(q);
                    if stats.not_empty() {
                        info!("{}", stats);
                    }
                }

                thread::sleep(config::QUOTE_INTERVAL);
            }

            info!("generator stopped");
        }));
    }

    // TCP listener
    info!("TCP listening on {}", args.tcp_bind);
    crate::tcp::run_tcp_listener(
        args.tcp_bind,
        hub,
        udp,
        curr_client_id,
        last_ping,
        shutdown.clone(),
    )?;

    // shutdown
    shutdown.store(true, Ordering::Relaxed); // гарантия
    for h in handles {
        if let Err(panic) = h.join() {
            warn!("background thread panicked: {:?}", panic);
        }
    }

    info!("server stopped");
    Ok(())
}

fn load_server_tickers_from_args(args: &Args) -> anyhow::Result<Vec<String>> {
    // 1) файл
    if let Some(p) = &args.tickers_file {
        let v = config::load_server_tickers(Some(p.clone()))?;
        if v.is_empty() {
            anyhow::bail!("tickers list is empty (file: {:?})", p);
        }
        return Ok(v);
    }

    // 2) текст (CSV или многострочный)
    if let Some(raw) = &args.tickers {
        let raw_trimmed = raw.trim();
        if raw_trimmed.is_empty() {
            anyhow::bail!("tickers text is empty");
        }

        // Если есть перевод строки или комментарии - трактуем как "по одному на строку"
        let v = if raw_trimmed.contains('\n') || raw_trimmed.contains('#') {
            quote_core::tickers::read_tickers(Cursor::new(raw_trimmed))?
        } else {
            quote_core::tickers::parse_tickers_csv(raw_trimmed)
        };

        if v.is_empty() {
            anyhow::bail!("tickers list is empty (--tickers)");
        }
        return Ok(v);
    }

    // 3) default (встроенный DEFAULT_TICKERS)
    let v = config::load_server_tickers(None)?;
    if v.is_empty() {
        anyhow::bail!("default tickers list is empty (DEFAULT_TICKERS)");
    }
    Ok(v)
}
