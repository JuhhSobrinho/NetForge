//! ICMP echo request/reply (RFC 792). Apenas os tipos 0 e 8.

use super::checksum::internet_checksum;

pub const TYPE_ECHO_REPLY: u8 = 0;
pub const TYPE_ECHO_REQUEST: u8 = 8;
const HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcmpEcho {
    pub kind: u8,
    pub id: u16,
    pub seq: u16,
    pub data: Vec<u8>,
}

impl IcmpEcho {
    pub fn encode(&self) -> Vec<u8> {
        let mut m = Vec::with_capacity(HEADER_LEN + self.data.len());
        m.push(self.kind);
        m.push(0); // código
        m.extend_from_slice(&[0, 0]); // checksum (preenchido abaixo)
        m.extend_from_slice(&self.id.to_be_bytes());
        m.extend_from_slice(&self.seq.to_be_bytes());
        m.extend_from_slice(&self.data);
        let c = internet_checksum(&m);
        m[2..4].copy_from_slice(&c.to_be_bytes());
        m
    }

    pub fn decode(bytes: &[u8]) -> Result<IcmpEcho, String> {
        if bytes.len() < HEADER_LEN {
            return Err("mensagem ICMP truncada".into());
        }
        if internet_checksum(bytes) != 0 {
            return Err("checksum ICMP inválido".into());
        }
        let kind = bytes[0];
        if kind != TYPE_ECHO_REPLY && kind != TYPE_ECHO_REQUEST {
            return Err(format!("tipo ICMP não suportado: {kind}"));
        }
        Ok(IcmpEcho {
            kind,
            id: u16::from_be_bytes([bytes[4], bytes[5]]),
            seq: u16::from_be_bytes([bytes[6], bytes[7]]),
            data: bytes[HEADER_LEN..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_round_trips_with_valid_checksum() {
        let e = IcmpEcho { kind: TYPE_ECHO_REQUEST, id: 0x4e46, seq: 3, data: vec![0xab; 32] };
        let bytes = e.encode();
        assert_eq!(internet_checksum(&bytes), 0);
        assert_eq!(IcmpEcho::decode(&bytes).unwrap(), e);
    }

    #[test]
    fn rejects_corrupted_message() {
        let mut bytes = IcmpEcho { kind: TYPE_ECHO_REPLY, id: 1, seq: 1, data: vec![1, 2] }.encode();
        bytes[6] ^= 1;
        assert!(IcmpEcho::decode(&bytes).is_err());
    }

    #[test]
    fn rejects_unsupported_type() {
        let mut bytes = IcmpEcho { kind: TYPE_ECHO_REQUEST, id: 1, seq: 1, data: vec![] }.encode();
        bytes[0] = 3;
        bytes[2] = 0;
        bytes[3] = 0;
        let c = internet_checksum(&bytes);
        bytes[2..4].copy_from_slice(&c.to_be_bytes());
        assert!(IcmpEcho::decode(&bytes).is_err());
    }
}
