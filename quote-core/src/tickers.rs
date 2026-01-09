use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

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
    let s = s.split('#').next().unwrap().trim();
    if s.is_empty() {
        return None;
    }

    Some(s.to_ascii_uppercase())
}
