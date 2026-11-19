//! IPv4 sem opções e sem fragmentação (RFC 791). Ver docs/PROTOCOLS.md.

use super::checksum::internet_checksum;
use std::net::Ipv4Addr;

pub const PROTO_ICMP: u8 = 1;
pub const HEADER_LEN: usize = 20;
pub const DEFAULT_TTL: u8 = 64;
/// Bit "Don't Fragment" no campo flags/offset.
const FLAG_DF: u16 = 0x4000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4Packet {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub ttl: u8,
    pub protocol: u8,
    pub id: u16,
    pub payload: Vec<u8>,
}

impl Ipv4Packet {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let total = HEADER_LEN + self.payload.len();
        if total > u16::MAX as usize {
            return Err("pacote IPv4 maior que 65535 bytes".into());
        }
        let mut h = Vec::with_capacity(total);
        h.push(0x45); // versão 4, IHL 5
        h.push(0); // TOS
        h.extend_from_slice(&(total as u16).to_be_bytes());
        h.extend_from_slice(&self.id.to_be_bytes());
        h.extend_from_slice(&FLAG_DF.to_be_bytes());
        h.push(self.ttl);
        h.push(self.protocol);
        h.extend_from_slice(&[0, 0]); // checksum (preenchido abaixo)
        h.extend_from_slice(&self.src.octets());
        h.extend_from_slice(&self.dst.octets());
        let checksum = internet_checksum(&h[..HEADER_LEN]);
        h[10..12].copy_from_slice(&checksum.to_be_bytes());
        h.extend_from_slice(&self.payload);
        Ok(h)
    }

    pub fn decode(bytes: &[u8]) -> Result<Ipv4Packet, String> {
        if bytes.len() < HEADER_LEN {
            return Err("pacote IPv4 truncado".into());
        }
        if bytes[0] != 0x45 {
            return Err("IPv4 com opções ou versão diferente de 4 não é suportado".into());
        }
        let total = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
        if total < HEADER_LEN || total > bytes.len() {
            return Err("comprimento total do IPv4 inválido".into());
        }
        if internet_checksum(&bytes[..HEADER_LEN]) != 0 {
            return Err("checksum do cabeçalho IPv4 inválido".into());
        }
        Ok(Ipv4Packet {
            src: Ipv4Addr::new(bytes[12], bytes[13], bytes[14], bytes[15]),
            dst: Ipv4Addr::new(bytes[16], bytes[17], bytes[18], bytes[19]),
            ttl: bytes[8],
            protocol: bytes[9],
            id: u16::from_be_bytes([bytes[4], bytes[5]]),
            payload: bytes[HEADER_LEN..total].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Ipv4Packet {
        Ipv4Packet {
            src: Ipv4Addr::new(192, 168, 1, 10),
            dst: Ipv4Addr::new(192, 168, 1, 20),
            ttl: DEFAULT_TTL,
            protocol: PROTO_ICMP,
            id: 7,
            payload: vec![8, 0, 0, 0],
        }
    }

    #[test]
    fn header_checksum_verifies_to_zero() {
        let bytes = sample().encode().unwrap();
        assert_eq!(internet_checksum(&bytes[..HEADER_LEN]), 0);
        assert_eq!(bytes[0], 0x45);
        assert_eq!(u16::from_be_bytes([bytes[2], bytes[3]]) as usize, bytes.len());
    }

    #[test]
    fn decode_round_trips() {
        assert_eq!(Ipv4Packet::decode(&sample().encode().unwrap()).unwrap(), sample());
    }

    #[test]
    fn detects_corruption() {
        let mut bytes = sample().encode().unwrap();
        bytes[15] ^= 0xff;
        assert!(Ipv4Packet::decode(&bytes).is_err());
    }
}
