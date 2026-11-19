//! Quadro Ethernet II (IEEE 802.3, sem FCS). Ver docs/PROTOCOLS.md.

pub type Mac = [u8; 6];

pub const BROADCAST: Mac = [0xff; 6];
pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
const HEADER_LEN: usize = 14;
/// Tamanho mínimo do quadro sem FCS (64 bytes com FCS). Quadros menores recebem padding.
pub const MIN_FRAME_LEN: usize = 60;

/// MAC locamente administrado (bit 0x02) derivado do índice do equipamento e da porta.
/// Não corresponde a nenhum fabricante real.
pub fn mac_for(device_index: u16, port_index: u8) -> Mac {
    [0x02, 0x4e, 0x46, (device_index >> 8) as u8, device_index as u8, port_index]
}

pub fn mac_text(mac: &Mac) -> String {
    mac.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub dst: Mac,
    pub src: Mac,
    pub ethertype: u16,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len());
        out.extend_from_slice(&self.dst);
        out.extend_from_slice(&self.src);
        out.extend_from_slice(&self.ethertype.to_be_bytes());
        out.extend_from_slice(&self.payload);
        if out.len() < MIN_FRAME_LEN {
            out.resize(MIN_FRAME_LEN, 0);
        }
        out
    }

    /// Lê um quadro. O payload inclui o padding; cada protocolo usa o próprio tamanho.
    pub fn decode(bytes: &[u8]) -> Result<Frame, String> {
        if bytes.len() < HEADER_LEN {
            return Err("quadro menor que o cabeçalho Ethernet".into());
        }
        let mut dst = [0u8; 6];
        let mut src = [0u8; 6];
        dst.copy_from_slice(&bytes[0..6]);
        src.copy_from_slice(&bytes[6..12]);
        Ok(Frame {
            dst,
            src,
            ethertype: u16::from_be_bytes([bytes[12], bytes[13]]),
            payload: bytes[HEADER_LEN..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_header_and_pads_to_minimum() {
        let f = Frame { dst: BROADCAST, src: mac_for(1, 0), ethertype: ETHERTYPE_ARP, payload: vec![1, 2, 3] };
        let bytes = f.encode();
        assert_eq!(bytes.len(), MIN_FRAME_LEN);
        assert_eq!(&bytes[0..6], &BROADCAST);
        assert_eq!(&bytes[12..14], &[0x08, 0x06]);
        assert_eq!(&bytes[14..17], &[1, 2, 3]);
    }

    #[test]
    fn decode_round_trips_header_fields() {
        let f = Frame { dst: mac_for(2, 1), src: mac_for(1, 0), ethertype: ETHERTYPE_IPV4, payload: vec![9; 60] };
        let back = Frame::decode(&f.encode()).unwrap();
        assert_eq!(back.dst, f.dst);
        assert_eq!(back.src, f.src);
        assert_eq!(back.ethertype, ETHERTYPE_IPV4);
    }

    #[test]
    fn rejects_truncated_frames() {
        assert!(Frame::decode(&[0u8; 10]).is_err());
    }

    #[test]
    fn mac_text_is_lowercase_hex() {
        assert_eq!(mac_text(&mac_for(1, 2)), "02:4e:46:00:01:02");
    }
}
