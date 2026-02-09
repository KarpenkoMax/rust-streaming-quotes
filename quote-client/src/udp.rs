use std::net::{SocketAddr, UdpSocket};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use log::{debug, warn, info};

use quote_core::PING_INTERVAL;
use quote_core::wire::{decode, encode_v1, UdpPacketV1};
use crossbeam_channel::{Sender, Receiver, TrySendError};
use std::thread;

const TICK_RATE_MS: u64 = 200;

pub(crate) fn run_udp_receiver(
    bind_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let sock = UdpSocket::bind(bind_addr)?;
    sock.set_read_timeout(Some(Duration::from_millis(TICK_RATE_MS))).ok();

    // clone после bind, чтобы ping шёл с того же local port
    let ping_sock = sock.try_clone()?;

    let mut buf = [0u8; 2048];
    let mut connected = false;

    let (tx, rx): (Sender<SocketAddr>, Receiver<SocketAddr>) = crossbeam_channel::bounded(1);

    let sd = shutdown.clone();
    let h = thread::spawn(move || {
         if let Err(e) = run_ping(ping_sock, rx, PING_INTERVAL, sd) {
            warn!("keep-alive error: {e}");
         }
    });

    let result: anyhow::Result<()> = loop {
        if shutdown.load(Ordering::Relaxed) {
            info!("shutting down...");
            break Ok(());
        }

        if !connected {
            // первый пакет
            match sock.recv_from(&mut buf) {
                Ok((n, src)) => {
                    match decode(&buf[..n]) {
                        Ok(pkt) => {
                            if let Err(e) = sock.connect(src) {
                                break Err(e.into());
                            }
                            connected = true;
                            match tx.try_send(src) {
                                Ok(()) => {}
                                Err(TrySendError::Full(_)) => {
                                    // адрес уже был отправлен ранее
                                }
                                Err(TrySendError::Disconnected(_)) => {
                                    warn!("ping channel disconnected; keep-alive will not be sent");
                                }
                            };
                            handle_pkt(pkt);
                        }
                        Err(e) => {
                            debug!("bad udp packet from {src}: {e}");
                        }
                    }
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // просто "тик" цикла, ничего не делаем
                    continue;
                }
                Err(e) => {
                    break Err(e.into());
                }
            }
        } else {
            // sock.connect уже выполнен
            match sock.recv(&mut buf) {
                Ok(n) => match decode(&buf[..n]) {
                    Ok(pkt) => {
                        handle_pkt(pkt);
                    }
                    Err(e) => {
                        warn!("error decoding packet: {e}")
                    }
                },
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // просто "тик" цикла, ничего не делаем
                    continue;
                }
                Err(e) => {
                    break Err(e.into());
                }
            }
        }
    };

    shutdown.store(true, Ordering::Relaxed);
    drop(tx);
    let _ = h.join();
    result
}


fn handle_pkt(pkt: UdpPacketV1) {
    match pkt {
        UdpPacketV1::Ping => {},
        UdpPacketV1::Quote(quote) => {
            info!("{}", quote);
        }
    }
}

fn run_ping(
    sock: UdpSocket,
    rx: Receiver<SocketAddr>,
    interval: Duration,
    shutdown: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    // Ждём адрес сервера (полученный из udp recv_from)
    let server_addr = loop {
        if shutdown.load(Ordering::Relaxed) {
            return Ok(());
        }

        match rx.recv_timeout(Duration::from_millis(TICK_RATE_MS)) {
            Ok(addr) => break addr,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                // UDP-ресивер завершился, адрес так и не пришёл - выходим
                return Ok(());
            }
        }
    };

    let bytes = encode_v1(&UdpPacketV1::Ping)?;

    let tick = Duration::from_millis(TICK_RATE_MS);

    while !shutdown.load(Ordering::Relaxed) {
        sock.send_to(&bytes, server_addr)?;
        debug!("PING");

        let mut slept = Duration::ZERO;
        while slept < interval && !shutdown.load(Ordering::Relaxed) {
            let step = (interval - slept).min(tick);
            std::thread::sleep(step);
            slept += step;
        }
    }

    Ok(())
}
