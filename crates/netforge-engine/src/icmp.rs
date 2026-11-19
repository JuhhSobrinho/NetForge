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

// ---- Mensagens de erro (RFC 792): destino inalcançável (3) e tempo excedido (11).
// O corpo leva o cabeçalho IPv4 do datagrama original e os 8 primeiros bytes do payload.

pub const TYPE_DEST_UNREACHABLE: u8 = 3;
pub const TYPE_TIME_EXCEEDED: u8 = 11;
pub const CODE_NET_UNREACHABLE: u8 = 0;
pub const CODE_TTL_IN_TRANSIT: u8 = 0;

/// Monta uma mensagem de erro a partir do datagrama original (bytes do IPv4 completo).
pub fn encode_error(kind: u8, code: u8, original_ip_packet: &[u8]) -> Vec<u8> {
    let take = original_ip_packet.len().min(20 + 8);
    let mut m = Vec::with_capacity(8 + take);
    m.push(kind);
    m.push(code);
    m.extend_from_slice(&[0, 0]);
    m.extend_from_slice(&[0, 0, 0, 0]); // não utilizado
    m.extend_from_slice(&original_ip_packet[..take]);
    let c = internet_checksum(&m);
    m[2..4].copy_from_slice(&c.to_be_bytes());
    m
}

/// Erro ICMP recebido: tipo, código e o início do datagrama original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcmpError {
    pub kind: u8,
    pub code: u8,
    pub original: Vec<u8>,
}

impl IcmpError {
    pub fn decode(bytes: &[u8]) -> Result<IcmpError, String> {
        if bytes.len() < 8 {
            return Err("mensagem de erro ICMP truncada".into());
        }
        if internet_checksum(bytes) != 0 {
            return Err("checksum ICMP inválido".into());
        }
        let kind = bytes[0];
        if kind != TYPE_DEST_UNREACHABLE && kind != TYPE_TIME_EXCEEDED {
            return Err(format!("tipo de erro ICMP não suportado: {kind}"));
        }
        Ok(IcmpError { kind, code: bytes[1], original: bytes[8..].to_vec() })
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn error_message_round_trips_and_keeps_original_header() {
        let original = vec![0x45u8; 40];
        let msg = encode_error(TYPE_DEST_UNREACHABLE, CODE_NET_UNREACHABLE, &original);
        assert_eq!(internet_checksum(&msg), 0);
        let e = IcmpError::decode(&msg).unwrap();
        assert_eq!(e.kind, TYPE_DEST_UNREACHABLE);
        assert_eq!(e.code, CODE_NET_UNREACHABLE);
        assert_eq!(e.original.len(), 28);
        assert_eq!(&e.original[..], &original[..28]);
    }

    #[test]
    fn echo_is_not_an_error() {
        let echo = IcmpEcho { kind: TYPE_ECHO_REQUEST, id: 1, seq: 1, data: vec![] }.encode();
        assert!(IcmpError::decode(&echo).is_err());
    }
}
