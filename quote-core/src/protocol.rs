use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Stream {
        udp_target: SocketAddr,
        tickers: Vec<String>,
    },
}

/// Парсит строку вида:
/// "STREAM udp://127.0.0.1:34254 AAPL,TSLA"
pub fn parse_command(line: &str) -> Result<Command, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("empty command".into());
    }

    let mut parts = line.split_whitespace();
    let cmd = parts.next().ok_or("missing command")?;

    match cmd {
        "STREAM" => {
            let udp_uri = parts.next().ok_or("missing udp target")?;
            let tickers_raw = parts.next().ok_or("missing tickers list")?;

            // udp://127.0.0.1:34254 -> 127.0.0.1:34254
            let addr_str = udp_uri
                .strip_prefix("udp://")
                .ok_or("udp target must start with udp://")?;

            let udp_target: SocketAddr = addr_str
                .parse()
                .map_err(|_| format!("invalid udp address: {addr_str}"))?;

            let tickers: Vec<String> = tickers_raw
                .split(',')
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .map(|t| t.to_ascii_uppercase())
                .collect();

            if tickers.is_empty() {
                return Err("tickers list is empty".into());
            }

            Ok(Command::Stream {
                udp_target,
                tickers,
            })
        }
        other => Err(format!("unknown command: {other}")),
    }
}

pub fn format_stream_command(udp_target: SocketAddr, tickers: &[String]) -> String {
    let list = tickers.join(",");
    format!("STREAM udp://{} {}", udp_target, list)
}
