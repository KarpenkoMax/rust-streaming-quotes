// quote-server/src/udp_ping.rs
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use log::{debug, warn};

use quote_core::wire::{UdpPacketV1, decode};

pub(crate) type LastPingMap = Arc<RwLock<HashMap<SocketAddr, Instant>>>;

/// Один поток на весь сервер:
/// - читает UDP пакеты (recv_from) с общего сокета
/// - принимает только Ping
/// - обновляет last_ping[src_addr] = Instant::now()
pub(crate) fn run_udp_ping_listener(
    udp: Arc<UdpSocket>,
    last_ping: LastPingMap,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<()> {
    udp.set_read_timeout(Some(Duration::from_millis(200)))?;

    let mut buf = vec![0u8; 2048];

    while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        match udp.recv_from(&mut buf) {
            Ok((n, src)) => {
                // decode проверяет версию + postcard payload
                match decode(&buf[..n]) {
                    Ok(UdpPacketV1::Ping) => {
                        // обновляем last ping для src (IP:port клиента)
                        let mut map = match last_ping.write() {
                            Ok(g) => g,
                            Err(poisoned) => {
                                warn!("last_ping map lock poisoned; continuing");
                                poisoned.into_inner()
                            }
                        };
                        map.insert(src, Instant::now());
                        debug!("Ping from {src}");
                    }
                    Ok(UdpPacketV1::Quote(_)) => {
                        // по протоколу клиент не должен слать Quote на сервер
                    }
                    Err(e) => {
                        // не валим сервер из-за мусора в UDP
                        warn!("Bad UDP packet from {src}: {e}");
                    }
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // тик: просто проверим shutdown и продолжим
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}
