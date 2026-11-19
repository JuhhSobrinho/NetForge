//! ARP para Ethernet/IPv4 (RFC 826). Apenas request e reply; sem gratuitous ARP nem proxy ARP.

use super::ethernet::Mac;
use std::net::Ipv4Addr;

pub const OP_REQUEST: u16 = 1;
pub const OP_REPLY: u16 = 2;
pub const PACKET_LEN: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArpPacket {
    pub op: u16,
    pub sender_mac: Mac,
    pub sender_ip: Ipv4Addr,
    pub target_mac: Mac,
    pub target_ip: Ipv4Addr,
}

impl ArpPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(PACKET_LEN);
        out.extend_from_slice(&1u16.to_be_bytes()); // HTYPE Ethernet
        out.extend_from_slice(&0x0800u16.to_be_bytes()); // PTYPE IPv4
        out.push(6); // HLEN
        out.push(4); // PLEN
        out.extend_from_slice(&self.op.to_be_bytes());
        out.extend_from_slice(&self.sender_mac);
        out.extend_from_slice(&self.sender_ip.octets());
        out.extend_from_slice(&self.target_mac);
        out.extend_from_slice(&self.target_ip.octets());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<ArpPacket, String> {
        if bytes.len() < PACKET_LEN {
            return Err("pacote ARP truncado".into());
        }
        if bytes[0..2] != [0, 1] || bytes[2..4] != [0x08, 0x00] || bytes[4] != 6 || bytes[5] != 4 {
            return Err("ARP com tipos de endereço não suportados".into());
        }
        let op = u16::from_be_bytes([bytes[6], bytes[7]]);
        if op != OP_REQUEST && op != OP_REPLY {
            return Err(format!("operação ARP desconhecida: {op}"));
        }
        let mut sender_mac = [0u8; 6];
        let mut target_mac = [0u8; 6];
        sender_mac.copy_from_slice(&bytes[8..14]);
        target_mac.copy_from_slice(&bytes[18..24]);
        Ok(ArpPacket {
            op,
            sender_mac,
            sender_ip: Ipv4Addr::new(bytes[14], bytes[15], bytes[16], bytes[17]),
            target_mac,
            target_ip: Ipv4Addr::new(bytes[24], bytes[25], bytes[26], bytes[27]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_has_the_rfc826_layout() {
        let p = ArpPacket {
            op: OP_REQUEST,
            sender_mac: [0x02, 0, 0, 0, 0, 1],
            sender_ip: Ipv4Addr::new(192, 168, 1, 10),
            target_mac: [0; 6],
            target_ip: Ipv4Addr::new(192, 168, 1, 20),
        };
        let b = p.encode();
        assert_eq!(b.len(), PACKET_LEN);
        assert_eq!(&b[0..8], &[0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01]);
        assert_eq!(&b[14..18], &[192, 168, 1, 10]);
        assert_eq!(&b[24..28], &[192, 168, 1, 20]);
    }

    #[test]
    fn decode_round_trips() {
        let p = ArpPacket {
            op: OP_REPLY,
            sender_mac: [0x02, 0x4e, 0x46, 0, 1, 0],
            sender_ip: Ipv4Addr::new(10, 0, 0, 2),
            target_mac: [0x02, 0x4e, 0x46, 0, 0, 0],
            target_ip: Ipv4Addr::new(10, 0, 0, 1),
        };
        assert_eq!(ArpPacket::decode(&p.encode()).unwrap(), p);
    }

    #[test]
    fn rejects_unknown_operation_and_truncation() {
        let mut b = ArpPacket {
            op: OP_REQUEST,
            sender_mac: [0; 6],
            sender_ip: Ipv4Addr::UNSPECIFIED,
            target_mac: [0; 6],
            target_ip: Ipv4Addr::UNSPECIFIED,
        }
        .encode();
        b[7] = 9;
        assert!(ArpPacket::decode(&b).is_err());
        assert!(ArpPacket::decode(&b[..10]).is_err());
    }
}
