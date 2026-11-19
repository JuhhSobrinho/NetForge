//! DHCP (RFC 2131) em formato BOOTP com as opções usadas pelo NetForge: tipo de mensagem (53),
//! identificador do servidor (54), endereço pedido (50), máscara (1), roteador (3) e
//! tempo de concessão (51). Só Discover, Offer, Request e Ack. Ver docs/PROTOCOLS.md.

use crate::ethernet::Mac;
use std::net::Ipv4Addr;

pub const SERVER_PORT: u16 = 67;
pub const CLIENT_PORT: u16 = 68;
const FIXED_LEN: usize = 236;
const MAGIC: [u8; 4] = [99, 130, 83, 99];

pub const MSG_DISCOVER: u8 = 1;
pub const MSG_OFFER: u8 = 2;
pub const MSG_REQUEST: u8 = 3;
pub const MSG_ACK: u8 = 5;

pub const OP_REQUEST: u8 = 1;
pub const OP_REPLY: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhcpMessage {
    pub op: u8,
    pub xid: u32,
    pub ciaddr: Ipv4Addr,
    pub yiaddr: Ipv4Addr,
    pub siaddr: Ipv4Addr,
    pub chaddr: Mac,
    pub msg_type: u8,
    pub server_id: Option<Ipv4Addr>,
    pub requested_ip: Option<Ipv4Addr>,
    pub subnet_mask: Option<Ipv4Addr>,
    pub router: Option<Ipv4Addr>,
    pub lease_time: Option<u32>,
}

impl DhcpMessage {
    pub fn new(op: u8, msg_type: u8, xid: u32, chaddr: Mac) -> DhcpMessage {
        DhcpMessage {
            op,
            xid,
            ciaddr: Ipv4Addr::UNSPECIFIED,
            yiaddr: Ipv4Addr::UNSPECIFIED,
            siaddr: Ipv4Addr::UNSPECIFIED,
            chaddr,
            msg_type,
            server_id: None,
            requested_ip: None,
            subnet_mask: None,
            router: None,
            lease_time: None,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![0u8; FIXED_LEN];
        out[0] = self.op;
        out[1] = 1; // Ethernet
        out[2] = 6; // comprimento do endereço
        out[4..8].copy_from_slice(&self.xid.to_be_bytes());
        out[10..12].copy_from_slice(&0x8000u16.to_be_bytes()); // flag de broadcast
        out[12..16].copy_from_slice(&self.ciaddr.octets());
        out[16..20].copy_from_slice(&self.yiaddr.octets());
        out[20..24].copy_from_slice(&self.siaddr.octets());
        out[28..34].copy_from_slice(&self.chaddr);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&[53, 1, self.msg_type]);
        if let Some(a) = self.server_id {
            push_addr(&mut out, 54, a);
        }
        if let Some(a) = self.requested_ip {
            push_addr(&mut out, 50, a);
        }
        if let Some(a) = self.subnet_mask {
            push_addr(&mut out, 1, a);
        }
        if let Some(a) = self.router {
            push_addr(&mut out, 3, a);
        }
        if let Some(t) = self.lease_time {
            out.extend_from_slice(&[51, 4]);
            out.extend_from_slice(&t.to_be_bytes());
        }
        out.push(255);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<DhcpMessage, String> {
        if bytes.len() < FIXED_LEN + 4 + 1 {
            return Err("mensagem DHCP truncada".into());
        }
        if bytes[FIXED_LEN..FIXED_LEN + 4] != MAGIC {
            return Err("cookie DHCP inválido".into());
        }
        let mut chaddr = [0u8; 6];
        chaddr.copy_from_slice(&bytes[28..34]);
        let mut msg = DhcpMessage::new(bytes[0], 0, u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]), chaddr);
        msg.ciaddr = addr_at(bytes, 12);
        msg.yiaddr = addr_at(bytes, 16);
        msg.siaddr = addr_at(bytes, 20);
        let mut i = FIXED_LEN + 4;
        let mut seen_type = false;
        while i < bytes.len() {
            let code = bytes[i];
            if code == 255 {
                break;
            }
            if code == 0 {
                i += 1;
                continue;
            }
            let len = *bytes.get(i + 1).ok_or("opção DHCP truncada")? as usize;
            let value = bytes.get(i + 2..i + 2 + len).ok_or("opção DHCP truncada")?;
            match code {
                53 if len == 1 => {
                    msg.msg_type = value[0];
                    seen_type = true;
                }
                54 if len == 4 => msg.server_id = Some(ip_of(value)),
                50 if len == 4 => msg.requested_ip = Some(ip_of(value)),
                1 if len == 4 => msg.subnet_mask = Some(ip_of(value)),
                3 if len >= 4 => msg.router = Some(ip_of(value)),
                51 if len == 4 => msg.lease_time = Some(u32::from_be_bytes([value[0], value[1], value[2], value[3]])),
                _ => {}
            }
            i += 2 + len;
        }
        if !seen_type {
            return Err("mensagem DHCP sem tipo".into());
        }
        Ok(msg)
    }
}

fn push_addr(out: &mut Vec<u8>, code: u8, a: Ipv4Addr) {
    out.push(code);
    out.push(4);
    out.extend_from_slice(&a.octets());
}

fn addr_at(bytes: &[u8], at: usize) -> Ipv4Addr {
    Ipv4Addr::new(bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3])
}

fn ip_of(v: &[u8]) -> Ipv4Addr {
    Ipv4Addr::new(v[0], v[1], v[2], v[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_round_trips() {
        let mut m = DhcpMessage::new(OP_REQUEST, MSG_DISCOVER, 0xdead_beef, [2, 0, 0, 0, 0, 1]);
        m.requested_ip = None;
        let b = m.encode();
        assert_eq!(&b[FIXED_LEN..FIXED_LEN + 4], &MAGIC);
        assert_eq!(DhcpMessage::decode(&b).unwrap(), m);
    }

    #[test]
    fn offer_carries_lease_options() {
        let mut m = DhcpMessage::new(OP_REPLY, MSG_OFFER, 7, [2, 0, 0, 0, 0, 2]);
        m.yiaddr = Ipv4Addr::new(10, 0, 1, 50);
        m.server_id = Some(Ipv4Addr::new(10, 0, 1, 1));
        m.subnet_mask = Some(Ipv4Addr::new(255, 255, 255, 0));
        m.router = Some(Ipv4Addr::new(10, 0, 1, 1));
        m.lease_time = Some(3600);
        let back = DhcpMessage::decode(&m.encode()).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn rejects_bad_cookie_and_missing_type() {
        let mut b = DhcpMessage::new(OP_REPLY, MSG_ACK, 1, [0; 6]).encode();
        b[FIXED_LEN] = 0;
        assert!(DhcpMessage::decode(&b).is_err());
        let mut b = DhcpMessage::new(OP_REPLY, MSG_ACK, 1, [0; 6]).encode();
        b[FIXED_LEN + 4] = 255; // remove a opção 53
        assert!(DhcpMessage::decode(&b).is_err());
    }
}
