use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

/// Чтение тикеров
pub fn read_tickers<R: io::Read>(reader: R) -> io::Result<Vec<String>> {
    let mut set = BTreeSet::new();
    let buf = BufReader::new(reader);

    for line in buf.lines() {
        let line = line?;
        if let Some(t) = normalize_line(&line) {
            set.insert(t);
        }
    }

    Ok(set.into_iter().collect())
}

/// Чтение тикеров из файла
pub fn read_tickers_from_path(path: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let f = File::open(path)?;
    read_tickers(f)
}

fn normalize_line(line: &str) -> Option<String> {
    let s = line.trim();
    if s.is_empty() || s.starts_with('#') {
        return None;
    }

    // Поддержка inline-комментариев: "AAPL # comment"
    let s = s.split('#').next().unwrap_or("").trim();
    if s.is_empty() {
        return None;
    }

    Some(s.to_ascii_uppercase())
}

/// Парсит список тикеров из строки вида "AAPL, TSLA, ,GOOG".
/// Правила:
/// - разделитель: запятая
/// - trim пробелов
/// - пустые элементы игнорируются
/// - нормализация: ASCII uppercase
/// - результат: отсортирован + уникален (BTreeSet)
pub fn parse_tickers_csv(raw: &str) -> Vec<String> {
    let mut set = BTreeSet::new();

    for part in raw.split(',') {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        set.insert(t.to_ascii_uppercase());
    }

    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor, Read};
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{fs, path::PathBuf};

    #[test]
    fn parse_tickers_csv_sorts_and_dedups_and_filters_empty() {
        let got = parse_tickers_csv(" aapl, TSLA, ,goog ,AAPL,, tsla ");
        assert_eq!(got, vec!["AAPL", "GOOG", "TSLA"]);
    }

    #[test]
    fn parse_tickers_csv_empty_gives_empty_vec() {
        assert!(parse_tickers_csv("").is_empty());
        assert!(parse_tickers_csv(" , ,  ,").is_empty());
    }

    #[test]
    fn normalize_line_ignores_empty_and_full_line_comments() {
        assert_eq!(normalize_line(""), None);
        assert_eq!(normalize_line("   "), None);
        assert_eq!(normalize_line("# comment"), None);
        assert_eq!(normalize_line("   # comment"), None);
        assert_eq!(normalize_line("#"), None);
        assert_eq!(normalize_line("   #"), None);
    }

    #[test]
    fn normalize_line_supports_inline_comments_and_uppercase() {
        assert_eq!(normalize_line("aapl"), Some("AAPL".to_string()));
        assert_eq!(normalize_line("  aapl  "), Some("AAPL".to_string()));
        assert_eq!(normalize_line("aapl # long comment"), Some("AAPL".to_string()));
        assert_eq!(normalize_line("tsla#comment"), Some("TSLA".to_string()));
        assert_eq!(normalize_line("  tsla#comment  "), Some("TSLA".to_string()));
        assert_eq!(normalize_line("   # only comment after trim"), None);
        assert_eq!(normalize_line("   #only"), None);
        assert_eq!(normalize_line("   #only  # still"), None);
        assert_eq!(normalize_line("AAPL #"), Some("AAPL".to_string()));
        assert_eq!(normalize_line("AAPL#"), Some("AAPL".to_string()));
    }

    #[test]
    fn read_tickers_sorts_and_deduplicates() {
        let input = "\
msft
aapl
GOOG
AAPL
  goog
# ignored
";
        let got = read_tickers(Cursor::new(input)).unwrap();

        // BTreeSet => сортировка + уникальность
        assert_eq!(got, vec!["AAPL", "GOOG", "MSFT"]);
    }

    #[test]
    fn read_tickers_ignores_blank_lines_and_comments() {
        let input = "\n   \n# one\n   # two\n#\n   #\n";
        let got = read_tickers(Cursor::new(input)).unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn read_tickers_parses_inline_comments_and_trimming() {
        let input = "\
  aapl   # comment
#full comment
 tsla#x
   nvda   # ok
";
        let got = read_tickers(Cursor::new(input)).unwrap();
        assert_eq!(got, vec!["AAPL", "NVDA", "TSLA"]);
    }

    #[test]
    fn read_tickers_from_path_reads_file() {
        // делаем уникальный путь в temp без сторонних crate
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path: PathBuf = std::env::temp_dir();
        path.push(format!("quote_core_tickers_test_{nanos}_{}.txt", std::process::id()));

        let input = "aapl\nmsft\n#comment\nAAPL\n";
        fs::write(&path, input).unwrap();

        let got = read_tickers_from_path(&path).unwrap();
        assert_eq!(got, vec!["AAPL", "MSFT"]);

        // cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn read_tickers_propagates_read_error() {
        // Подсунем reader, который читает немного, а потом падает
        struct FailingReader {
            inner: Cursor<Vec<u8>>,
            fail_after: usize,
            read_total: usize,
        }

        impl Read for FailingReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.read_total >= self.fail_after {
                    return Err(io::Error::new(io::ErrorKind::Other, "boom"));
                }

                let remaining_before_fail = self.fail_after - self.read_total;
                let to_read = buf.len().min(remaining_before_fail);

                let n = self.inner.read(&mut buf[..to_read])?;
                self.read_total += n;

                // если данных больше нет — обычный EOF
                Ok(n)
            }
        }

        let data = b"aapl\nmsft\nGOOG\n".to_vec();
        let reader = FailingReader {
            inner: Cursor::new(data),
            fail_after: 6, // успеем прочитать "aapl\nm" и упадём в процессе
            read_total: 0,
        };

        let err = read_tickers(reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert!(err.to_string().contains("boom"));
    }
}
