//! TCP reduzido (RFC 9293 no cabeçalho): cabeçalho de 20 bytes, flags SYN/ACK/PSH/FIN/RST,
//! checksum com pseudo-cabeçalho. Sem opções, sem janela dinâmica, sem retransmissão, sem
//! controle de congestionamento. Ver docs/PROTOCOLS.md.

use crate::checksum::internet_checksum;
use std::net::Ipv4Addr;

pub const PROTO_TCP: u8 = 6;
pub const HEADER_LEN: usize = 20;
pub const FLAG_FIN: u8 = 0x01;
pub const FLAG_SYN: u8 = 0x02;
pub const FLAG_RST: u8 = 0x04;
pub const FLAG_PSH: u8 = 0x08;
pub const FLAG_ACK: u8 = 0x10;
const WINDOW: u16 = 65_535;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq: u32,
    pub ack: u32,
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Segment {
    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    /// Quantidade de número de sequência consumida: payload mais 1 por SYN e por FIN.
    pub fn seq_len(&self) -> u32 {
        let mut n = self.payload.len() as u32;
        if self.has(FLAG_SYN) {
            n += 1;
        }
        if self.has(FLAG_FIN) {
            n += 1;
        }
        n
    }

    pub fn encode(&self, src: Ipv4Addr, dst: Ipv4Addr) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len());
        out.extend_from_slice(&self.src_port.to_be_bytes());
        out.extend_from_slice(&self.dst_port.to_be_bytes());
        out.extend_from_slice(&self.seq.to_be_bytes());
        out.extend_from_slice(&self.ack.to_be_bytes());
        out.push(5 << 4); // deslocamento de dados: 5 palavras
        out.push(self.flags);
        out.extend_from_slice(&WINDOW.to_be_bytes());
        out.extend_from_slice(&[0, 0]); // checksum
        out.extend_from_slice(&[0, 0]); // urgente
        out.extend_from_slice(&self.payload);
        let c = checksum_with_pseudo(src, dst, &out);
        out[16..18].copy_from_slice(&c.to_be_bytes());
        out
    }

    pub fn decode(bytes: &[u8], src: Ipv4Addr, dst: Ipv4Addr) -> Result<Segment, String> {
        if bytes.len() < HEADER_LEN {
            return Err("segmento TCP truncado".into());
        }
        if (bytes[12] >> 4) != 5 {
            return Err("opções TCP não são suportadas".into());
        }
        if checksum_with_pseudo(src, dst, bytes) != 0 {
            return Err("checksum TCP inválido".into());
        }
        Ok(Segment {
            src_port: u16::from_be_bytes([bytes[0], bytes[1]]),
            dst_port: u16::from_be_bytes([bytes[2], bytes[3]]),
            seq: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            ack: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            flags: bytes[13],
            payload: bytes[HEADER_LEN..].to_vec(),
        })
    }
}

/// Checksum do segmento com o pseudo-cabeçalho IPv4 (RFC 9293, seção 3.1).
fn checksum_with_pseudo(src: Ipv4Addr, dst: Ipv4Addr, segment: &[u8]) -> u16 {
    let mut buf = Vec::with_capacity(12 + segment.len() + 1);
    buf.extend_from_slice(&src.octets());
    buf.extend_from_slice(&dst.octets());
    buf.push(0);
    buf.push(PROTO_TCP);
    buf.extend_from_slice(&(segment.len() as u16).to_be_bytes());
    buf.extend_from_slice(segment);
    internet_checksum(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(flags: u8, payload: &[u8]) -> Segment {
        Segment { src_port: 40000, dst_port: 7, seq: 1000, ack: 2000, flags, payload: payload.to_vec() }
    }

    #[test]
    fn header_layout_and_checksum_verify() {
        let src = Ipv4Addr::new(10, 0, 0, 1);
        let dst = Ipv4Addr::new(10, 0, 0, 2);
        let b = seg(FLAG_SYN | FLAG_ACK, b"hi").encode(src, dst);
        assert_eq!(b.len(), HEADER_LEN + 2);
        assert_eq!(b[12], 0x50);
        assert_eq!(b[13], FLAG_SYN | FLAG_ACK);
        assert_eq!(Segment::decode(&b, src, dst).unwrap(), seg(FLAG_SYN | FLAG_ACK, b"hi"));
    }

    #[test]
    fn checksum_depends_on_addresses_and_detects_corruption() {
        let src = Ipv4Addr::new(10, 0, 0, 1);
        let dst = Ipv4Addr::new(10, 0, 0, 2);
        let b = seg(FLAG_ACK, b"x").encode(src, dst);
        assert!(Segment::decode(&b, src, Ipv4Addr::new(10, 0, 0, 3)).is_err());
        let mut bad = b.clone();
        bad[20] ^= 1;
        assert!(Segment::decode(&bad, src, dst).is_err());
    }

    #[test]
    fn sequence_length_counts_syn_and_fin() {
        assert_eq!(seg(FLAG_SYN, b"").seq_len(), 1);
        assert_eq!(seg(FLAG_FIN | FLAG_ACK, b"abc").seq_len(), 4);
        assert_eq!(seg(FLAG_ACK, b"abc").seq_len(), 3);
    }
}
