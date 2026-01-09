use serde::{Deserialize, Serialize};

use crate::types::StockQuote;

pub const WIRE_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UdpPacketV1 {
    Quote(StockQuote),
    Ping,
}

pub fn encode_v1(pkt: &UdpPacketV1) -> Result<Vec<u8>, postcard::Error> {
    let mut out = Vec::new();
    out.push(WIRE_VERSION);
    out.extend_from_slice(&postcard::to_allocvec(pkt)?);
    Ok(out)
}

pub fn decode(buf: &[u8]) -> Result<UdpPacketV1, String> {
    let (&ver, payload) = buf.split_first().ok_or("packet too short")?;
    if ver != WIRE_VERSION {
        return Err(format!("unsupported wire version: {ver}"));
    }
    postcard::from_bytes(payload).map_err(|e| e.to_string())
}
