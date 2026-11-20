//! RIPv2 (RFC 2453): mensagens de requisição e resposta com entradas IPv4 (máscara e próximo
//! salto). Métrica 16 significa inalcançável. Ver docs/PROTOCOLS.md.

use std::net::Ipv4Addr;

pub const PORT: u16 = 520;
pub const MULTICAST: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 9);
pub const INFINITY: u8 = 16;
const CMD_REQUEST: u8 = 1;
const CMD_RESPONSE: u8 = 2;
const HEADER_LEN: usize = 4;
const ENTRY_LEN: usize = 20;
const MAX_ENTRIES: usize = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RipEntry {
    pub network: Ipv4Addr,
    pub mask: Ipv4Addr,
    pub next_hop: Ipv4Addr,
    pub metric: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RipMessage {
    pub command: u8,
    pub entries: Vec<RipEntry>,
}

impl RipMessage {
    pub fn response(entries: Vec<RipEntry>) -> RipMessage {
        RipMessage { command: CMD_RESPONSE, entries }
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        if self.entries.len() > MAX_ENTRIES {
            return Err(format!("mensagem RIP com mais de {MAX_ENTRIES} entradas"));
        }
        let mut out = vec![self.command, 2, 0, 0]; // comando, versão 2, zero
        for e in &self.entries {
            out.extend_from_slice(&2u16.to_be_bytes()); // família IPv4
            out.extend_from_slice(&[0, 0]); // route tag
            out.extend_from_slice(&e.network.octets());
            out.extend_from_slice(&e.mask.octets());
            out.extend_from_slice(&e.next_hop.octets());
            out.extend_from_slice(&(e.metric.min(INFINITY) as u32).to_be_bytes());
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<RipMessage, String> {
        if bytes.len() < HEADER_LEN || (bytes.len() - HEADER_LEN) % ENTRY_LEN != 0 {
            return Err("mensagem RIP com tamanho inválido".into());
        }
        if bytes[1] != 2 {
            return Err("somente RIPv2 é suportado".into());
        }
        let command = bytes[0];
        if command != CMD_REQUEST && command != CMD_RESPONSE {
            return Err(format!("comando RIP desconhecido: {command}"));
        }
        let mut entries = Vec::new();
        for chunk in bytes[HEADER_LEN..].chunks_exact(ENTRY_LEN) {
            let family = u16::from_be_bytes([chunk[0], chunk[1]]);
            if family != 2 {
                continue;
            }
            let metric = u32::from_be_bytes([chunk[16], chunk[17], chunk[18], chunk[19]]);
            if metric == 0 || metric > INFINITY as u32 {
                return Err("métrica RIP fora de 1–16".into());
            }
            entries.push(RipEntry {
                network: Ipv4Addr::new(chunk[4], chunk[5], chunk[6], chunk[7]),
                mask: Ipv4Addr::new(chunk[8], chunk[9], chunk[10], chunk[11]),
                next_hop: Ipv4Addr::new(chunk[12], chunk[13], chunk[14], chunk[15]),
                metric: metric as u8,
            });
        }
        Ok(RipMessage { command, entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(net: [u8; 4], prefix_mask: [u8; 4], metric: u8) -> RipEntry {
        RipEntry {
            network: Ipv4Addr::from(net),
            mask: Ipv4Addr::from(prefix_mask),
            next_hop: Ipv4Addr::UNSPECIFIED,
            metric,
        }
    }

    #[test]
    fn response_layout_matches_rfc2453() {
        let m = RipMessage::response(vec![entry([10, 0, 2, 0], [255, 255, 255, 0], 1)]);
        let b = m.encode().unwrap();
        assert_eq!(&b[0..4], &[2, 2, 0, 0]);
        assert_eq!(&b[4..6], &[0, 2]);
        assert_eq!(&b[8..12], &[10, 0, 2, 0]);
        assert_eq!(&b[12..16], &[255, 255, 255, 0]);
        assert_eq!(&b[16..20], &[0, 0, 0, 0]);
        assert_eq!(&b[20..24], &[0, 0, 0, 1]);
    }

    #[test]
    fn round_trip_keeps_entries_and_metric() {
        let m = RipMessage::response(vec![entry([10, 0, 1, 0], [255, 255, 255, 0], 2), entry([10, 0, 9, 0], [255, 255, 255, 0], 16)]);
        assert_eq!(RipMessage::decode(&m.encode().unwrap()).unwrap(), m);
    }

    #[test]
    fn rejects_v1_bad_metric_and_bad_length() {
        let mut b = RipMessage::response(vec![entry([10, 0, 1, 0], [255, 255, 255, 0], 1)]).encode().unwrap();
        b[1] = 1;
        assert!(RipMessage::decode(&b).is_err());
        let mut b = RipMessage::response(vec![entry([10, 0, 1, 0], [255, 255, 255, 0], 1)]).encode().unwrap();
        b[23] = 17;
        assert!(RipMessage::decode(&b).is_err());
        assert!(RipMessage::decode(&[2, 2, 0]).is_err());
    }

    #[test]
    fn too_many_entries_are_rejected() {
        let many = vec![entry([10, 0, 1, 0], [255, 255, 255, 0], 1); 26];
        assert!(RipMessage::response(many).encode().is_err());
    }
}
