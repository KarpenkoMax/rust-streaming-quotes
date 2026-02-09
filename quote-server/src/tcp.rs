use crate::hub::Hub;
use crate::session::run_session;
use crate::udp_ping::LastPingMap;
use anyhow::Context;
use log::{info, warn};
use quote_core::protocol::{Command, parse_command};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, atomic::AtomicBool, atomic::AtomicU64, atomic::Ordering};
use std::thread;
use std::time::Duration;

const TCP_READ_TIMEOUT_S: u64 = 5;
const TCP_WRITE_TIMEOUT_S: u64 = 5;

// accept loop + чтение команд по TCP
pub(crate) fn run_tcp_listener(
    tcp_addr: SocketAddr,
    hub: Arc<Hub>,
    udp: Arc<UdpSocket>,
    curr_client_id: Arc<AtomicU64>,
    last_ping: LastPingMap,
    shutdown: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let listener =
        TcpListener::bind(tcp_addr).with_context(|| format!("bind TCP listener {}", tcp_addr))?;
    listener
        .set_nonblocking(true)
        .context("listener.set_nonblocking(true)")?;
    let mut session_handles = Vec::new();

    loop {
        reap_finished_sessions(&mut session_handles);

        if shutdown.load(Ordering::Relaxed) {
            info!("shutting down tcp listener");
            break;
        }

        match listener.accept() {
            Ok((stream, _addr)) => {
                stream
                    .set_nonblocking(false)
                    .context("stream.set_nonblocking(false)")?;

                stream.set_nodelay(true).ok();
                stream
                    .set_read_timeout(Some(Duration::from_secs(TCP_READ_TIMEOUT_S)))
                    .ok();
                stream
                    .set_write_timeout(Some(Duration::from_secs(TCP_WRITE_TIMEOUT_S)))
                    .ok();

                let hub = hub.clone();
                let udp = udp.clone();
                let curr_client_id = curr_client_id.clone();
                let last_ping = last_ping.clone();
                let shutdown = shutdown.clone();

                let h = thread::spawn(move || {
                    if let Err(e) =
                        handle_conn(stream, hub, curr_client_id, udp, last_ping, shutdown)
                    {
                        warn!("handle_conn error: {e}");
                    }
                });
                session_handles.push(h);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // нет новых соединений прямо сейчас
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                warn!("accept error: {e}");
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    for h in session_handles {
        if let Err(panic) = h.join() {
            warn!("session thread panicked: {:?}", panic);
        }
    }

    Ok(())
}

fn reap_finished_sessions(handles: &mut Vec<thread::JoinHandle<()>>) {
    let mut i = 0;
    while i < handles.len() {
        if handles[i].is_finished() {
            let h = handles.swap_remove(i);
            if let Err(panic) = h.join() {
                warn!("session thread panicked: {:?}", panic);
            }
        } else {
            i += 1;
        }
    }
}

fn extract_command(stream: &mut TcpStream) -> anyhow::Result<Command> {
    let mut line = String::new();

    {
        let mut reader = BufReader::new(stream);
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            anyhow::bail!("client closed connection");
        }
    }

    parse_command(&line).map_err(|e| anyhow::anyhow!(e))
}

fn handle_conn(
    mut stream: TcpStream,
    hub: Arc<Hub>,
    curr_client_id: Arc<AtomicU64>,
    udp: Arc<UdpSocket>,
    last_ping: LastPingMap,
    shutdown: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    // парсинг команды
    let cmd = match extract_command(&mut stream) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("ERR {e}\n");
            let _ = stream.write_all(msg.as_bytes());
            return Ok(());
        }
    };

    match cmd {
        Command::Stream {
            udp_target,
            tickers,
        } => {
            let cid = curr_client_id.fetch_add(1, Ordering::Relaxed);

            let rx = match hub.add_client(cid) {
                Ok(rx) => rx,
                Err(e) => {
                    let msg = format!("ERR {e}\n");
                    let _ = stream.write_all(msg.as_bytes());
                    return Ok(());
                }
            };

            if let Err(e) = stream.write_all(b"OK\n") {
                hub.remove_client(cid);
                return Err(e.into());
            }
            stream.flush()?;
            stream.shutdown(std::net::Shutdown::Both).ok();
            drop(stream);

            let tickers_hs: HashSet<String> = tickers.into_iter().collect();

            let res = run_session(cid, rx, udp_target, udp, tickers_hs, last_ping, shutdown);

            if let Err(e) = res {
                warn!("session {cid} ended with error: {e}");
            }

            hub.remove_client(cid);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream, UdpSocket};
    use std::sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU64},
    };
    use std::time::Duration;

    fn connect_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();

        (client, server)
    }

    fn read_reply(mut client: TcpStream) -> String {
        client
            .set_read_timeout(Some(Duration::from_millis(300)))
            .unwrap();
        let mut buf = [0u8; 256];
        let n = client.read(&mut buf).unwrap_or(0);
        String::from_utf8_lossy(&buf[..n]).to_string()
    }

    #[test]
    fn handle_conn_writes_err_on_garbage_command() {
        let (mut client, server) = connect_pair();
        client.write_all(b"GARBAGE\n").unwrap();

        let hub = Arc::new(Hub::new());
        let udp = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
        let last_ping: LastPingMap = Arc::new(RwLock::new(HashMap::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let cid = Arc::new(AtomicU64::new(1));

        handle_conn(server, hub, cid, udp, last_ping, shutdown).unwrap();

        let reply = read_reply(client);
        assert!(
            reply.starts_with("ERR "),
            "expected ERR reply, got: {reply:?}"
        );
        assert!(
            reply.ends_with('\n'),
            "reply must end with newline: {reply:?}"
        );
    }

    #[test]
    fn handle_conn_writes_ok_on_stream_and_exits_fast_on_shutdown() {
        let (mut client, server) = connect_pair();

        // валидная команда (парсинг проверяется в quote-core)
        client
            .write_all(b"STREAM udp://127.0.0.1:34254 AAPL\n")
            .unwrap();

        let hub = Arc::new(Hub::new());
        let udp = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
        let last_ping: LastPingMap = Arc::new(RwLock::new(HashMap::new()));

        // shutdown=true => run_session не зависнет
        let shutdown = Arc::new(AtomicBool::new(true));

        let cid = Arc::new(AtomicU64::new(1));

        handle_conn(server, hub, cid, udp, last_ping, shutdown).unwrap();

        let reply = read_reply(client);
        assert_eq!(reply, "OK\n");
    }

    #[test]
    fn handle_conn_writes_err_on_eof_before_command() {
        let (client, server) = connect_pair();
        drop(client); // клиент сразу закрыл соединение => EOF

        let hub = Arc::new(Hub::new());
        let udp = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
        let last_ping: LastPingMap = Arc::new(RwLock::new(HashMap::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let cid = Arc::new(AtomicU64::new(1));

        // просто проверяем, что не паникует и корректно завершается
        handle_conn(server, hub, cid, udp, last_ping, shutdown).unwrap();
    }
}
