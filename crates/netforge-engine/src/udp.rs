//! UDP (RFC 768). O checksum é enviado como zero, o que o RFC permite em IPv4.
//! Ver docs/PROTOCOLS.md.

pub const PROTO_UDP: u8 = 17;
pub const HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpDatagram {
    pub src_port: u16,
    pub dst_port: u16,
    pub payload: Vec<u8>,
}

impl UdpDatagram {
    pub fn encode(&self) -> Vec<u8> {
        let len = (HEADER_LEN + self.payload.len()) as u16;
        let mut out = Vec::with_capacity(len as usize);
        out.extend_from_slice(&self.src_port.to_be_bytes());
        out.extend_from_slice(&self.dst_port.to_be_bytes());
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&[0, 0]); // checksum opcional em IPv4
        out.extend_from_slice(&self.payload);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<UdpDatagram, String> {
        if bytes.len() < HEADER_LEN {
            return Err("datagrama UDP truncado".into());
        }
        let len = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        if len < HEADER_LEN || len > bytes.len() {
            return Err("comprimento UDP inválido".into());
        }
        Ok(UdpDatagram {
            src_port: u16::from_be_bytes([bytes[0], bytes[1]]),
            dst_port: u16::from_be_bytes([bytes[2], bytes[3]]),
            payload: bytes[HEADER_LEN..len].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_rfc768() {
        let d = UdpDatagram { src_port: 68, dst_port: 67, payload: vec![1, 2, 3] };
        let b = d.encode();
        assert_eq!(&b[0..4], &[0, 68, 0, 67]);
        assert_eq!(&b[4..6], &[0, 11]);
        assert_eq!(&b[6..8], &[0, 0]);
        assert_eq!(&b[8..], &[1, 2, 3]);
    }

    #[test]
    fn decode_round_trips_and_rejects_bad_length() {
        let d = UdpDatagram { src_port: 1234, dst_port: 53, payload: vec![9; 20] };
        assert_eq!(UdpDatagram::decode(&d.encode()).unwrap(), d);
        let mut bad = d.encode();
        bad[5] = 200;
        assert!(UdpDatagram::decode(&bad).is_err());
        assert!(UdpDatagram::decode(&[0u8; 4]).is_err());
    }
}
