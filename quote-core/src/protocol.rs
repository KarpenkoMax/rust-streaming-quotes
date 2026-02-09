use crate::error::ProtocolError;
use crate::tickers::parse_tickers_csv;
use std::net::SocketAddr;

/// Команды, принимаемые сервером
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Начать стриминг тикеров
    Stream {
        /// UDP-адрес клиента
        udp_target: SocketAddr,
        /// Запрошенный список тикеров
        tickers: Vec<String>,
    },
}

/// Парсит строку вида:
/// "STREAM udp://127.0.0.1:34254 AAPL,TSLA"
pub fn parse_command(line: &str) -> Result<Command, ProtocolError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(ProtocolError::EmptyCommand);
    }

    let mut parts = line.split_whitespace();
    let cmd = parts.next().ok_or(ProtocolError::MissingCommand)?;

    match cmd {
        "STREAM" => {
            let udp_uri = parts.next().ok_or(ProtocolError::MissingUdpTarget)?;

            // забираем ВСЁ остальное как строку тикеров (включая пробелы)
            let tickers_raw = parts.collect::<Vec<_>>().join(" ");
            if tickers_raw.trim().is_empty() {
                return Err(ProtocolError::MissingTickers);
            }

            let addr_str = udp_uri
                .strip_prefix("udp://")
                .ok_or(ProtocolError::BadUdpScheme)?;

            let udp_target: SocketAddr = addr_str
                .parse()
                .map_err(|_| ProtocolError::InvalidUdpAddress(addr_str.to_string()))?;

            let tickers = parse_tickers_csv(&tickers_raw);
            if tickers.is_empty() {
                return Err(ProtocolError::EmptyTickers);
            }

            Ok(Command::Stream {
                udp_target,
                tickers,
            })
        }
        other => Err(ProtocolError::UnknownCommand(other.to_string())),
    }
}

/// Формирует команду для стриминга котировок.
pub fn format_stream_command(udp_target: SocketAddr, tickers: &[String]) -> String {
    let list = tickers.join(",");
    format!("STREAM udp://{} {}", udp_target, list)
}

/// Формирует команду + конец строки для стриминга котировок.
/// Используется клиентом.
pub fn format_stream_command_line(udp_target: SocketAddr, tickers: &[String]) -> String {
    format!("{}\n", format_stream_command(udp_target, tickers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stream_happy_path() {
        let cmd = parse_command("STREAM udp://127.0.0.1:34254 AAPL,TSLA").unwrap();

        assert_eq!(
            cmd,
            Command::Stream {
                udp_target: "127.0.0.1:34254".parse().unwrap(),
                tickers: vec!["AAPL".to_string(), "TSLA".to_string()],
            }
        );
    }

    #[test]
    fn parse_stream_trims_and_uppercases_and_filters_empty() {
        let cmd = parse_command("  STREAM   udp://127.0.0.1:1   aapl,  tsla , ,goog  ").unwrap();

        assert_eq!(
            cmd,
            Command::Stream {
                udp_target: "127.0.0.1:1".parse().unwrap(),
                tickers: vec!["AAPL".to_string(), "GOOG".to_string(), "TSLA".to_string()],
            }
        );
    }

    #[test]
    fn parse_empty_line_is_error() {
        let err = parse_command("").unwrap_err();
        assert!(matches!(err, ProtocolError::EmptyCommand));

        let err = parse_command("   \t\n  ").unwrap_err();
        assert!(matches!(err, ProtocolError::EmptyCommand));
    }

    #[test]
    fn parse_missing_udp_target() {
        let err = parse_command("STREAM").unwrap_err();
        assert!(matches!(err, ProtocolError::MissingUdpTarget));
    }

    #[test]
    fn parse_missing_tickers() {
        let err = parse_command("STREAM udp://127.0.0.1:1").unwrap_err();
        assert!(matches!(err, ProtocolError::MissingTickers));
    }

    #[test]
    fn parse_bad_udp_scheme() {
        let err = parse_command("STREAM tcp://127.0.0.1:1 AAPL").unwrap_err();
        assert!(matches!(err, ProtocolError::BadUdpScheme));
    }

    #[test]
    fn parse_invalid_udp_address() {
        let err = parse_command("STREAM udp://127.0.0.1:notaport AAPL").unwrap_err();
        assert!(matches!(err, ProtocolError::InvalidUdpAddress(s) if s == "127.0.0.1:notaport"));
    }

    #[test]
    fn parse_empty_tickers_is_error() {
        // parts.next() возьмёт "," как tickers_raw -> после split/filter список станет пустым
        let err = parse_command("STREAM udp://127.0.0.1:1 ,").unwrap_err();
        assert!(matches!(err, ProtocolError::EmptyTickers));
    }

    #[test]
    fn parse_unknown_command() {
        let err = parse_command("PING udp://127.0.0.1:1 AAPL").unwrap_err();
        assert!(matches!(err, ProtocolError::UnknownCommand(s) if s == "PING"));
    }

    #[test]
    fn format_stream_command_formats_as_expected() {
        let addr: SocketAddr = "127.0.0.1:34254".parse().unwrap();
        let tickers = vec!["AAPL".to_string(), "TSLA".to_string()];

        let s = format_stream_command(addr, &tickers);
        assert_eq!(s, "STREAM udp://127.0.0.1:34254 AAPL,TSLA");
    }

    #[test]
    fn roundtrip_parse_format_parse() {
        let addr: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let tickers = vec!["aapl".to_string(), "TsLa".to_string()];

        // format не нормализует тикеры, но parse нормализует при разборе
        let s = format_stream_command(addr, &tickers);
        let cmd = parse_command(&s).unwrap();

        assert_eq!(
            cmd,
            Command::Stream {
                udp_target: addr,
                tickers: vec!["AAPL".to_string(), "TSLA".to_string()],
            }
        );
    }
}
