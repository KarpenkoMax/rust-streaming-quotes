use serde::{Deserialize, Serialize};

use crate::error::WireError;
use crate::types::StockQuote;

/// Версия протокола
pub const WIRE_VERSION: u8 = 1;

/// Возможный payload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UdpPacketV1 {
    /// Котировка
    Quote(StockQuote),
    /// Пинг (keep-alive)
    Ping,
}

/// Закодировать payload
pub fn encode_v1(pkt: &UdpPacketV1) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    out.push(WIRE_VERSION);
    out.extend_from_slice(&postcard::to_allocvec(pkt)?);
    Ok(out)
}

/// Распаковать payload
pub fn decode(buf: &[u8]) -> Result<UdpPacketV1, WireError> {
    let (&ver, payload) = buf.split_first().ok_or(WireError::PacketTooShort)?;
    if ver != WIRE_VERSION {
        return Err(WireError::UnsupportedWireVersion(ver));
    }
    let pkt = postcard::from_bytes(payload)?;
    Ok(pkt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_quote() {
        let q = StockQuote {
            ticker: "AAPL".to_string(),
            price: 123_4500,
            volume: 1500,
            timestamp_ms: 1_700_000_000_000,
        };

        let pkt = UdpPacketV1::Quote(q.clone());

        let bytes = encode_v1(&pkt).expect("encode");
        let decoded = decode(&bytes).expect("decode");

        assert_eq!(decoded, UdpPacketV1::Quote(q));
    }

    #[test]
    fn roundtrip_ping() {
        let pkt = UdpPacketV1::Ping;

        let bytes = encode_v1(&pkt).expect("encode");
        let decoded = decode(&bytes).expect("decode");

        assert_eq!(decoded, UdpPacketV1::Ping);
    }

    #[test]
    fn decode_rejects_unknown_version() {
        let pkt = UdpPacketV1::Ping;
        let mut bytes = encode_v1(&pkt).expect("encode");

        // портим версию
        bytes[0] = WIRE_VERSION.wrapping_add(1);

        let err = decode(&bytes).unwrap_err();
        assert!(matches!(err, WireError::UnsupportedWireVersion(_)));
    }

    #[test]
    fn decode_rejects_too_short_packet() {
        let err = decode(&[]).unwrap_err();
        assert!(matches!(err, WireError::PacketTooShort));
    }
}
