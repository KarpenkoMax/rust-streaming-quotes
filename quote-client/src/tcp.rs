use quote_core::protocol::format_stream_command_line;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const TCP_READ_TIMEOUT_S: u64 = 5;
const TCP_WRITE_TIMEOUT_S: u64 = 5;

pub(crate) fn send_stream_command(
    server_tcp_addr: SocketAddr,
    udp_target: SocketAddr,
    tickers: &[String],
) -> anyhow::Result<()> {
    let mut stream = TcpStream::connect(server_tcp_addr)?;

    stream.set_nodelay(true).ok();
    stream
        .set_read_timeout(Some(Duration::from_secs(TCP_READ_TIMEOUT_S)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_secs(TCP_WRITE_TIMEOUT_S)))
        .ok();

    // отправляем команду
    let cmd = format_stream_command_line(udp_target, tickers);

    stream.write_all(cmd.as_bytes())?;
    stream.flush()?;

    // обрабатываем ответ
    let mut reader = BufReader::new(&mut stream);

    let mut line = String::new();
    let n = reader.read_line(&mut line)?;

    if n == 0 {
        anyhow::bail!("server closed connection without response");
    }

    let resp = line.trim_end_matches(&['\r', '\n'][..]);

    if resp == "OK" {
        return Ok(());
    }

    if let Some(rest) = resp.strip_prefix("ERR") {
        anyhow::bail!("server error: {}", rest.trim());
    }

    anyhow::bail!("unexpected server response: {:?}", resp);
}
