//! DNS (RFC 1035) sobre UDP: consultas e respostas do tipo A, classe IN. Sem compressão de
//! nomes na escrita; a leitura aceita apenas nomes sem ponteiros. Ver docs/PROTOCOLS.md.

use std::net::Ipv4Addr;

pub const PORT: u16 = 53;
const TYPE_A: u16 = 1;
const CLASS_IN: u16 = 1;
const HEADER_LEN: usize = 12;
const FLAG_RD: u16 = 0x0100;
const FLAG_RESPONSE: u16 = 0x8000;
const FLAG_RA: u16 = 0x0080;
pub const RCODE_NXDOMAIN: u8 = 3;
pub const MAX_NAME_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;
const ANSWER_TTL: u32 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Query { id: u16, name: String },
    Response { id: u16, name: String, answer: Option<Ipv4Addr>, rcode: u8 },
}

/// Normaliza e valida um nome (letras, dígitos e hífens nos rótulos).
pub fn normalize_name(name: &str) -> Result<String, String> {
    let lower = name.trim().trim_end_matches('.').to_lowercase();
    if lower.is_empty() || lower.len() > MAX_NAME_LEN {
        return Err(format!("nome DNS inválido: {name}"));
    }
    for label in lower.split('.') {
        if label.is_empty() || label.len() > MAX_LABEL_LEN {
            return Err(format!("rótulo DNS inválido em {name}"));
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || label.starts_with('-') || label.ends_with('-') {
            return Err(format!("caracteres inválidos no nome {name}"));
        }
    }
    Ok(lower)
}

fn encode_name(name: &str, out: &mut Vec<u8>) -> Result<(), String> {
    let name = normalize_name(name)?;
    for label in name.split('.') {
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    Ok(())
}

fn decode_name(bytes: &[u8], mut pos: usize) -> Result<(String, usize), String> {
    let mut labels = Vec::new();
    loop {
        let len = *bytes.get(pos).ok_or("nome DNS truncado")? as usize;
        if len & 0xC0 != 0 {
            return Err("ponteiros de compressão não são suportados".into());
        }
        pos += 1;
        if len == 0 {
            break;
        }
        let raw = bytes.get(pos..pos + len).ok_or("rótulo DNS truncado")?;
        labels.push(String::from_utf8_lossy(raw).to_string());
        pos += len;
    }
    Ok((labels.join(".").to_lowercase(), pos))
}

pub fn encode_query(id: u16, name: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&FLAG_RD.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    out.extend_from_slice(&[0; 6]); // ANCOUNT, NSCOUNT, ARCOUNT
    encode_name(name, &mut out)?;
    out.extend_from_slice(&TYPE_A.to_be_bytes());
    out.extend_from_slice(&CLASS_IN.to_be_bytes());
    Ok(out)
}

pub fn encode_response(id: u16, name: &str, answer: Option<Ipv4Addr>) -> Result<Vec<u8>, String> {
    let rcode = if answer.is_some() { 0 } else { RCODE_NXDOMAIN };
    let flags = FLAG_RESPONSE | FLAG_RD | FLAG_RA | rcode as u16;
    let mut out = Vec::new();
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&flags.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&(answer.is_some() as u16).to_be_bytes());
    out.extend_from_slice(&[0; 4]);
    encode_name(name, &mut out)?;
    out.extend_from_slice(&TYPE_A.to_be_bytes());
    out.extend_from_slice(&CLASS_IN.to_be_bytes());
    if let Some(ip) = answer {
        encode_name(name, &mut out)?;
        out.extend_from_slice(&TYPE_A.to_be_bytes());
        out.extend_from_slice(&CLASS_IN.to_be_bytes());
        out.extend_from_slice(&ANSWER_TTL.to_be_bytes());
        out.extend_from_slice(&4u16.to_be_bytes());
        out.extend_from_slice(&ip.octets());
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Message, String> {
    if bytes.len() < HEADER_LEN {
        return Err("mensagem DNS truncada".into());
    }
    let id = u16::from_be_bytes([bytes[0], bytes[1]]);
    let flags = u16::from_be_bytes([bytes[2], bytes[3]]);
    let qd = u16::from_be_bytes([bytes[4], bytes[5]]);
    let an = u16::from_be_bytes([bytes[6], bytes[7]]);
    if qd != 1 {
        return Err("mensagem DNS deve ter exatamente uma pergunta".into());
    }
    let (name, mut pos) = decode_name(bytes, HEADER_LEN)?;
    if bytes.len() < pos + 4 {
        return Err("pergunta DNS truncada".into());
    }
    pos += 4; // QTYPE e QCLASS
    if flags & FLAG_RESPONSE == 0 {
        return Ok(Message::Query { id, name });
    }
    let rcode = (flags & 0x000f) as u8;
    let mut answer = None;
    if an > 0 {
        let (_, p) = decode_name(bytes, pos)?;
        let rtype = u16::from_be_bytes([*bytes.get(p).ok_or("resposta truncada")?, *bytes.get(p + 1).ok_or("resposta truncada")?]);
        let rdlen = u16::from_be_bytes([*bytes.get(p + 8).ok_or("resposta truncada")?, *bytes.get(p + 9).ok_or("resposta truncada")?]) as usize;
        let rdata = bytes.get(p + 10..p + 10 + rdlen).ok_or("resposta truncada")?;
        if rtype == TYPE_A && rdlen == 4 {
            answer = Some(Ipv4Addr::new(rdata[0], rdata[1], rdata[2], rdata[3]));
        }
    }
    Ok(Message::Response { id, name, answer, rcode })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_layout_matches_rfc1035() {
        let q = encode_query(0x1234, "srv.lab").unwrap();
        assert_eq!(&q[0..2], &[0x12, 0x34]);
        assert_eq!(&q[2..4], &[0x01, 0x00]); // RD
        assert_eq!(&q[4..6], &[0, 1]);
        assert_eq!(&q[12..], &[3, b's', b'r', b'v', 3, b'l', b'a', b'b', 0, 0, 1, 0, 1]);
    }

    #[test]
    fn query_and_answer_round_trip() {
        let q = encode_query(7, "Web.Lab").unwrap();
        assert_eq!(decode(&q).unwrap(), Message::Query { id: 7, name: "web.lab".into() });
        let r = encode_response(7, "web.lab", Some(Ipv4Addr::new(10, 0, 5, 80))).unwrap();
        assert_eq!(
            decode(&r).unwrap(),
            Message::Response { id: 7, name: "web.lab".into(), answer: Some(Ipv4Addr::new(10, 0, 5, 80)), rcode: 0 }
        );
    }

    #[test]
    fn nxdomain_has_no_answer() {
        let r = encode_response(9, "nada.lab", None).unwrap();
        assert_eq!(decode(&r).unwrap(), Message::Response { id: 9, name: "nada.lab".into(), answer: None, rcode: RCODE_NXDOMAIN });
    }

    #[test]
    fn invalid_names_are_rejected() {
        assert!(normalize_name("").is_err());
        assert!(normalize_name("a..b").is_err());
        assert!(normalize_name("-x.lab").is_err());
        assert!(normalize_name("with space.lab").is_err());
        assert!(normalize_name(&format!("{}.lab", "a".repeat(64))).is_err());
        assert!(encode_query(1, "ok.lab").is_ok());
    }

    #[test]
    fn truncated_messages_are_errors() {
        assert!(decode(&[0u8; 5]).is_err());
        let q = encode_query(1, "x.lab").unwrap();
        assert!(decode(&q[..q.len() - 3]).is_err());
    }
}
