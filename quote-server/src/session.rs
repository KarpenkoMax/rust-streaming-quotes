use crate::config::ClientId;
use crate::config::{PING_TIMEOUT, UDP_SOCKET_TICK};
use crate::udp_ping::LastPingMap;
use crossbeam_channel::Receiver;
use log::{info, warn};
use quote_core::StockQuote;
use quote_core::wire::{UdpPacketV1, encode_v1};
use std::collections::HashSet;
use std::time::Instant;
use std::{
    net::UdpSocket,
    sync::{Arc, atomic::AtomicBool, atomic::Ordering},
};

const BACK_TO_BACK_SEND_ERR_LIMIT: usize = 20;

pub(crate) fn run_session(
    cid: ClientId,
    rx: Receiver<Arc<StockQuote>>,
    udp_target: std::net::SocketAddr,
    udp: Arc<UdpSocket>,
    tickers: HashSet<String>,
    last_ping: LastPingMap,
    shutdown: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let session_start = Instant::now();
    let mut back_to_back_err_count = 0;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            info!("shutting down {cid} {udp_target}");
            break;
        }

        if ping_expired(&last_ping, udp_target, session_start) {
            info!("ping timeout for {cid} {udp_target}; stopping session");
            break;
        }

        // разгребаем очередь
        for q in rx.try_iter() {
            handle_quote(
                &udp,
                udp_target,
                q,
                &tickers,
                &mut back_to_back_err_count,
                cid,
            )?;
        }
        // ждём ещё одно сообщение + роль sleep
        match rx.recv_timeout(UDP_SOCKET_TICK) {
            Ok(q) => {
                handle_quote(
                    &udp,
                    udp_target,
                    q,
                    &tickers,
                    &mut back_to_back_err_count,
                    cid,
                )?;
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // ничего, просто тик
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
    }

    let mut map = match last_ping.write() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    map.remove(&udp_target);

    Ok(())
}

fn send_quote(
    sock: &std::net::UdpSocket,
    target: std::net::SocketAddr,
    q: &StockQuote,
) -> anyhow::Result<()> {
    let pkt = UdpPacketV1::Quote(q.clone());
    let bytes = encode_v1(&pkt)?;
    sock.send_to(&bytes, target)?;
    Ok(())
}

fn handle_quote(
    sock: &std::net::UdpSocket,
    target: std::net::SocketAddr,
    q: Arc<StockQuote>,
    tickers_fltr: &HashSet<String>,
    err_count: &mut usize,
    cid: ClientId,
) -> anyhow::Result<()> {
    if tickers_fltr.contains(&q.ticker) {
        match send_quote(sock, target, &q) {
            Ok(()) => *err_count = 0,
            Err(e) => {
                warn!("Failed to send quote to {cid} {target} due to {e}");
                *err_count += 1;
                if *err_count >= BACK_TO_BACK_SEND_ERR_LIMIT {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

fn ping_expired(
    last_ping: &LastPingMap,
    target: std::net::SocketAddr,
    session_start: Instant,
) -> bool {
    let last = {
        let map = match last_ping.read() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        map.get(&target).copied()
    };

    let age = match last {
        Some(t) => t.elapsed(),
        None => session_start.elapsed(), // ещё не было ни одного ping
    };

    age > PING_TIMEOUT
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote_core::wire::{UdpPacketV1, decode};
    use std::net::{SocketAddr, UdpSocket};
    use std::sync::RwLock;
    use std::time::{Duration, Instant};

    fn mk_quote(ticker: &str) -> StockQuote {
        StockQuote {
            ticker: ticker.to_string(),
            price: 123_4500,
            volume: 10,
            timestamp_ms: 1,
        }
    }

    #[test]
    fn handle_quote_sends_when_ticker_matches_and_resets_err_count() {
        let send_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let recv_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        recv_sock
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();

        let target = recv_sock.local_addr().unwrap();

        let mut tickers = HashSet::new();
        tickers.insert("AAPL".to_string());

        let mut err_count = 999;
        let cid: ClientId = 1;

        handle_quote(
            &send_sock,
            target,
            Arc::new(mk_quote("AAPL")),
            &tickers,
            &mut err_count,
            cid,
        )
        .unwrap();
        assert_eq!(err_count, 0);

        let mut buf = [0u8; 2048];
        let (n, _src) = recv_sock.recv_from(&mut buf).unwrap();

        let pkt = decode(&buf[..n]).unwrap();
        match pkt {
            UdpPacketV1::Quote(q) => assert_eq!(q.ticker, "AAPL"),
            _ => panic!("expected Quote packet"),
        }
    }

    #[test]
    fn handle_quote_does_not_send_when_ticker_not_in_filter() {
        let send_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let recv_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        recv_sock
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();

        let target = recv_sock.local_addr().unwrap();

        let mut tickers = HashSet::new();
        tickers.insert("TSLA".to_string()); // AAPL не входит

        let mut err_count = 0;
        let cid: ClientId = 1;

        handle_quote(
            &send_sock,
            target,
            Arc::new(mk_quote("AAPL")),
            &tickers,
            &mut err_count,
            cid,
        )
        .unwrap();

        let mut buf = [0u8; 2048];
        let res = recv_sock.recv_from(&mut buf);
        assert!(res.is_err(), "expected no UDP packet to be received");
    }

    #[test]
    fn handle_quote_increments_err_count_and_fails_on_limit() {
        // IPv4 сокет + IPv6 адрес => гарантированная ошибка send_to
        let send_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let target: SocketAddr = "[::1]:12345".parse().unwrap();

        let mut tickers = HashSet::new();
        tickers.insert("AAPL".to_string());

        let mut err_count = 0usize;
        let cid: ClientId = 1;

        // первые (LIMIT-1) раз Ok, на LIMIT-й — Err
        for _ in 0..(BACK_TO_BACK_SEND_ERR_LIMIT - 1) {
            let r = handle_quote(
                &send_sock,
                target,
                Arc::new(mk_quote("AAPL")),
                &tickers,
                &mut err_count,
                cid,
            );
            assert!(r.is_ok());
        }

        let r = handle_quote(
            &send_sock,
            target,
            Arc::new(mk_quote("AAPL")),
            &tickers,
            &mut err_count,
            cid,
        );
        assert!(r.is_err());
    }

    #[test]
    fn run_session_removes_last_ping_entry_on_keepalive_timeout() {
        let cid: ClientId = 1;
        let udp_target: SocketAddr = "127.0.0.1:34567".parse().unwrap();
        let udp = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());

        let (_tx, rx) = crossbeam_channel::bounded::<Arc<StockQuote>>(1);
        let tickers = HashSet::new();
        let shutdown = Arc::new(AtomicBool::new(false));

        let last_ping: LastPingMap = Arc::new(RwLock::new(std::collections::HashMap::new()));
        {
            let mut map = last_ping.write().unwrap();
            map.insert(
                udp_target,
                Instant::now() - PING_TIMEOUT - Duration::from_millis(1),
            );
        }

        run_session(
            cid,
            rx,
            udp_target,
            udp,
            tickers,
            last_ping.clone(),
            shutdown,
        )
        .unwrap();

        let map = last_ping.read().unwrap();
        assert!(
            !map.contains_key(&udp_target),
            "last_ping entry must be removed after session stops on timeout"
        );
    }
}
