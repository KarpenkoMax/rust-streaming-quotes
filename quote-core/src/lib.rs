//! # quote-core
//!
//! Базовые типы и протокол для Quote Server / Quote Client.
//!
//! Этот крейт содержит:
//!
//! - [`protocol`] — парсинг и форматирование текстовых команд
//! - [`tickers`] — чтение и нормализация списка тикеров из текста/файла
//! - [`wire`] — компактный UDP wire-формат (версия + бинарный payload)
//! - [`types`] — доменные типы
//! - [`error`] — типы ошибок, которые возвращают компоненты `quote-core`
//!
//! ## Быстрый пример: парсинг команды `STREAM`
//!
//! ```rust
//! use quote_core::protocol::{parse_command, Command};
//!
//! let cmd = parse_command("STREAM udp://127.0.0.1:34254 AAPL,TSLA").unwrap();
//! match cmd {
//!     Command::Stream { udp_target, tickers } => {
//!         assert_eq!(udp_target, "127.0.0.1:34254".parse().unwrap());
//!         assert_eq!(tickers, vec!["AAPL".to_string(), "TSLA".to_string()]);
//!     }
//! }
//! ```
//!
//! ## Пример: чтение тикеров
//!
//! ```rust
//! use quote_core::tickers::read_tickers;
//! use std::io::Cursor;
//!
//! let input = "aapl\n# comment\n tsla \nAAPL\n";
//! let tickers = read_tickers(Cursor::new(input)).unwrap();
//! assert_eq!(tickers, vec!["AAPL".to_string(), "TSLA".to_string()]);
//! ```
//! ## Пример: wire-формат (UDP)
//!
//! ```rust
//! use quote_core::wire::{encode_v1, decode, UdpPacketV1};
//! use quote_core::StockQuote;
//!
//! let pkt = UdpPacketV1::Quote(StockQuote {
//!     ticker: "AAPL".to_string(),
//!     price: 123_4500,
//!     volume: 1500,
//!     timestamp_ms: 1_700_000_000_000,
//! });
//!
//! let bytes = encode_v1(&pkt).unwrap();
//! let decoded = decode(&bytes).unwrap();
//! assert_eq!(decoded, pkt);
//! ```
//!
//! ## Дизайн
//!
//! `quote-core` задуман как “нулевая” зависимость для всех частей системы:
//! сервер, клиент, утилиты, тесты. Поэтому здесь держим только:
//! чистые типы, парсинг/сериализацию и простую утилитарщину,
//! без runtime/async и без тяжёлых зависимостей.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Текстовый протокол команд (например `STREAM udp://... AAPL,TSLA`).
pub mod protocol;

/// Чтение/нормализация списка тикеров из текста и файлов.
pub mod tickers;

/// Доменные типы (например котировка).
pub mod types;

/// Wire-уровень (сериализация/десериализация сообщений), если используется.
pub mod wire;

/// Ошибки `quote-core`.
pub mod error;

/// Общие константы
mod constants;
pub use constants::{PING_INTERVAL, PING_TIMEOUT};

// --- Re-exports (публичный фасад API) ---

pub use crate::error::{ProtocolError, QuoteCoreError, WireError};
pub use crate::protocol::Command;
pub use crate::types::StockQuote;
