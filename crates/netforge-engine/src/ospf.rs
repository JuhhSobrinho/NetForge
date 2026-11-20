//! OSPFv2 reduzido (RFC 2328), área única. Mensagens Hello (tipo 1) e Link State Update (tipo 4)
//! com LSAs de roteador (tipo 1). Sem DD/LSR/LSAck, sem eleição de DR/BDR (rede ponto a ponto),
//! checksum com a soma da Internet (o RFC usa Fletcher para LSAs). Ver docs/PROTOCOLS.md.

use crate::checksum::internet_checksum;
use std::net::Ipv4Addr;

pub const PROTO_OSPF: u8 = 89;
pub const ALL_ROUTERS: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 5);
pub const HELLO_INTERVAL: u16 = 10;
pub const DEAD_INTERVAL: u32 = 40;
const VERSION: u8 = 2;
const TYPE_HELLO: u8 = 1;
const TYPE_LSU: u8 = 4;
const HEADER_LEN: usize = 24;
const LSA_HEADER_LEN: usize = 20;
pub const LINK_POINT_TO_POINT: u8 = 1;
pub const LINK_STUB: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub router_id: Ipv4Addr,
    pub area: Ipv4Addr,
    pub mask: Ipv4Addr,
    pub neighbors: Vec<Ipv4Addr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouterLink {
    pub kind: u8,
    pub id: Ipv4Addr,
    pub data: Ipv4Addr,
    pub metric: u16,
}

/// LSA de roteador: os enlaces ponto a ponto e as redes conectadas (stubs) de um roteador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterLsa {
    pub router_id: Ipv4Addr,
    pub seq: u32,
    pub links: Vec<RouterLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OspfMessage {
    Hello(Hello),
    Update { router_id: Ipv4Addr, lsas: Vec<RouterLsa> },
}

fn header(kind: u8, router_id: Ipv4Addr, body: &[u8]) -> Vec<u8> {
    let len = (HEADER_LEN + body.len()) as u16;
    let mut out = vec![VERSION, kind];
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&router_id.octets());
    out.extend_from_slice(&Ipv4Addr::UNSPECIFIED.octets()); // área 0.0.0.0
    out.extend_from_slice(&[0, 0]); // checksum
    out.extend_from_slice(&[0, 0]); // autenticação (nula)
    out.extend_from_slice(&[0; 8]); // autenticação
    out.extend_from_slice(body);
    let c = internet_checksum(&out);
    out[12..14].copy_from_slice(&c.to_be_bytes());
    out
}

fn encode_lsa(lsa: &RouterLsa) -> Vec<u8> {
    let mut body = vec![0u8, 0, 0, lsa.links.len() as u8]; // flags, zero, número de enlaces
    for l in &lsa.links {
        body.extend_from_slice(&l.id.octets());
        body.extend_from_slice(&l.data.octets());
        body.push(l.kind);
        body.push(0); // TOS
        body.extend_from_slice(&l.metric.to_be_bytes());
    }
    let len = (LSA_HEADER_LEN + body.len()) as u16;
    let mut out = vec![0, 1, 0x02, 1]; // idade 0, opções, tipo 1 (roteador)
    out.extend_from_slice(&lsa.router_id.octets()); // link state id
    out.extend_from_slice(&lsa.router_id.octets()); // roteador anunciante
    out.extend_from_slice(&lsa.seq.to_be_bytes());
    out.extend_from_slice(&[0, 0]); // checksum (não verificado nesta versão reduzida)
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&body);
    out
}

fn decode_lsa(b: &[u8]) -> Result<(RouterLsa, usize), String> {
    if b.len() < LSA_HEADER_LEN + 4 {
        return Err("LSA truncado".into());
    }
    if b[3] != 1 {
        return Err("somente LSAs de roteador são suportados".into());
    }
    let len = u16::from_be_bytes([b[18], b[19]]) as usize;
    if len < LSA_HEADER_LEN + 4 || len > b.len() {
        return Err("comprimento de LSA inválido".into());
    }
    let router_id = Ipv4Addr::new(b[8], b[9], b[10], b[11]);
    let seq = u32::from_be_bytes([b[12], b[13], b[14], b[15]]);
    let count = b[LSA_HEADER_LEN + 3] as usize;
    let mut links = Vec::new();
    let mut pos = LSA_HEADER_LEN + 4;
    for _ in 0..count {
        let l = b.get(pos..pos + 12).ok_or("enlace de LSA truncado")?;
        links.push(RouterLink {
            id: Ipv4Addr::new(l[0], l[1], l[2], l[3]),
            data: Ipv4Addr::new(l[4], l[5], l[6], l[7]),
            kind: l[8],
            metric: u16::from_be_bytes([l[10], l[11]]),
        });
        pos += 12;
    }
    Ok((RouterLsa { router_id, seq, links }, len))
}

impl OspfMessage {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            OspfMessage::Hello(h) => {
                let mut body = Vec::new();
                body.extend_from_slice(&h.mask.octets());
                body.extend_from_slice(&HELLO_INTERVAL.to_be_bytes());
                body.push(0); // opções
                body.push(0); // prioridade
                body.extend_from_slice(&DEAD_INTERVAL.to_be_bytes());
                body.extend_from_slice(&[0; 8]); // DR e BDR
                for n in &h.neighbors {
                    body.extend_from_slice(&n.octets());
                }
                header(TYPE_HELLO, h.router_id, &body)
            }
            OspfMessage::Update { router_id, lsas } => {
                let mut body = (lsas.len() as u32).to_be_bytes().to_vec();
                for l in lsas {
                    body.extend_from_slice(&encode_lsa(l));
                }
                header(TYPE_LSU, *router_id, &body)
            }
        }
    }

    pub fn decode(b: &[u8]) -> Result<OspfMessage, String> {
        if b.len() < HEADER_LEN {
            return Err("pacote OSPF truncado".into());
        }
        if b[0] != VERSION {
            return Err("somente OSPFv2 é suportado".into());
        }
        if internet_checksum(&[&b[..12], &[0, 0][..], &b[14..]].concat()) != u16::from_be_bytes([b[12], b[13]]) {
            return Err("checksum OSPF inválido".into());
        }
        let router_id = Ipv4Addr::new(b[4], b[5], b[6], b[7]);
        let body = &b[HEADER_LEN..];
        match b[1] {
            TYPE_HELLO => {
                if body.len() < 20 {
                    return Err("Hello truncado".into());
                }
                let mask = Ipv4Addr::new(body[0], body[1], body[2], body[3]);
                let neighbors = body[20..]
                    .chunks_exact(4)
                    .map(|c| Ipv4Addr::new(c[0], c[1], c[2], c[3]))
                    .collect();
                Ok(OspfMessage::Hello(Hello { router_id, area: Ipv4Addr::UNSPECIFIED, mask, neighbors }))
            }
            TYPE_LSU => {
                if body.len() < 4 {
                    return Err("atualização truncada".into());
                }
                let count = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
                let mut lsas = Vec::new();
                let mut pos = 4;
                for _ in 0..count {
                    let (lsa, used) = decode_lsa(&body[pos..])?;
                    lsas.push(lsa);
                    pos += used;
                }
                Ok(OspfMessage::Update { router_id, lsas })
            }
            other => Err(format!("tipo de pacote OSPF não suportado: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_round_trips_with_neighbors() {
        let h = OspfMessage::Hello(Hello {
            router_id: Ipv4Addr::new(1, 1, 1, 1),
            area: Ipv4Addr::UNSPECIFIED,
            mask: Ipv4Addr::new(255, 255, 255, 0),
            neighbors: vec![Ipv4Addr::new(2, 2, 2, 2)],
        });
        assert_eq!(OspfMessage::decode(&h.encode()).unwrap(), h);
    }

    #[test]
    fn update_round_trips_router_lsas() {
        let lsa = RouterLsa {
            router_id: Ipv4Addr::new(1, 1, 1, 1),
            seq: 7,
            links: vec![
                RouterLink { kind: LINK_POINT_TO_POINT, id: Ipv4Addr::new(2, 2, 2, 2), data: Ipv4Addr::new(10, 0, 9, 1), metric: 1 },
                RouterLink { kind: LINK_STUB, id: Ipv4Addr::new(10, 0, 1, 0), data: Ipv4Addr::new(255, 255, 255, 0), metric: 1 },
            ],
        };
        let m = OspfMessage::Update { router_id: Ipv4Addr::new(1, 1, 1, 1), lsas: vec![lsa] };
        assert_eq!(OspfMessage::decode(&m.encode()).unwrap(), m);
    }

    #[test]
    fn corrupted_packet_is_rejected() {
        let mut b = OspfMessage::Hello(Hello {
            router_id: Ipv4Addr::new(1, 1, 1, 1),
            area: Ipv4Addr::UNSPECIFIED,
            mask: Ipv4Addr::new(255, 255, 255, 0),
            neighbors: vec![],
        })
        .encode();
        b[5] ^= 1;
        assert!(OspfMessage::decode(&b).is_err());
    }
}
