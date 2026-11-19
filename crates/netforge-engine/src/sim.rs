//! Motor de simulação (Fase 3): fila de eventos com tempo virtual, hosts com ARP/IPv4/ICMP
//! e switches com aprendizado de MAC.
//!
//! Regras desta fase (ver docs/PROTOCOLS.md e docs/TESTING_STRATEGY.md):
//! - Cada link leva `LINK_LATENCY_US` de tempo virtual. Não há perda de pacotes.
//! - Hosts têm uma interface; não há roteamento: o destino precisa estar na mesma sub-rede.
//! - Switches não implementam STP: um laço gera tempestade de broadcast, que é interrompida
//!   pelo limite `MAX_EVENTS` e reportada como erro.
//! - A captura PCAP é feita na interface do PC de origem (quadros transmitidos e recebidos).
//! - Cada chamada de `run_ping` cria uma simulação nova: ARP e MAC não persistem entre chamadas.

use crate::arp::{ArpPacket, OP_REPLY, OP_REQUEST};
use crate::ethernet::{mac_for, mac_text, Frame, Mac, BROADCAST, ETHERTYPE_ARP, ETHERTYPE_IPV4};
use crate::icmp::{IcmpEcho, TYPE_ECHO_REPLY, TYPE_ECHO_REQUEST};
use crate::ipv4::{Ipv4Packet, DEFAULT_TTL, PROTO_ICMP};
use crate::pcap::PcapWriter;
use crate::topology::{netmask, parse_address, Address, DeviceKind, Topology};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::net::Ipv4Addr;

pub const LINK_LATENCY_US: u64 = 1_000;
pub const MAX_PING_COUNT: u8 = 10;
/// Limite de eventos por execução: protege contra tempestade de broadcast (laço sem STP).
pub const MAX_EVENTS: u64 = 20_000;
/// Limite de linhas de log devolvidas ao frontend.
pub const MAX_LOG_ENTRIES: usize = 5_000;
const ECHO_ID: u16 = 0x4e46;
const ECHO_DATA_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Endpoint {
    dev: usize,
    port: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogKind {
    Tx,
    Rx,
    Learn,
    Flood,
    Forward,
    Drop,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time_us: u64,
    pub device: String,
    pub kind: LogKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingReply {
    pub seq: u16,
    pub rtt_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingOutcome {
    pub target: String,
    pub transmitted: u8,
    pub replies: Vec<PingReply>,
    pub error: Option<String>,
    pub log: Vec<LogEntry>,
    pub log_truncated: bool,
    pub pcap: Vec<u8>,
}

struct Pending {
    at: u64,
    seq: u64,
    to: Endpoint,
    bytes: Vec<u8>,
}

impl PartialEq for Pending {
    fn eq(&self, o: &Self) -> bool {
        self.at == o.at && self.seq == o.seq
    }
}
impl Eq for Pending {}
impl PartialOrd for Pending {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Pending {
    // BinaryHeap é máximo: invertemos para retirar sempre o menor (tempo, sequência).
    fn cmp(&self, o: &Self) -> Ordering {
        (o.at, o.seq).cmp(&(self.at, self.seq))
    }
}

struct Runtime {
    name: String,
    kind: DeviceKind,
    macs: Vec<Mac>,
    addrs: Vec<Option<Address>>,
    arp: HashMap<Ipv4Addr, Mac>,
    /// Pacote IPv4 aguardando a resolução ARP do próximo salto.
    pending: Option<(Ipv4Addr, Ipv4Packet)>,
    mac_table: HashMap<Mac, usize>,
}

struct EchoState {
    seq: u16,
    started_at: u64,
    reply_at: Option<u64>,
}

struct Simulation {
    devices: Vec<Runtime>,
    wires: HashMap<Endpoint, Endpoint>,
    queue: BinaryHeap<Pending>,
    now: u64,
    counter: u64,
    ip_id: u16,
    events: u64,
    storm: bool,
    log: Vec<LogEntry>,
    log_truncated: bool,
    capture: Endpoint,
    pcap: PcapWriter,
    echo: Option<EchoState>,
    target: Ipv4Addr,
}

/// Executa `count` echo requests de `source_id` para `target` e devolve o resultado.
/// Erros de entrada (topologia inválida, origem sem endereço) retornam `Err`.
/// Falhas de rede (sem resposta, laço) retornam `Ok` com `error` preenchido.
pub fn run_ping(topology: &Topology, source_id: &str, target: &str, count: u8) -> Result<PingOutcome, String> {
    topology.validate()?;
    if count == 0 || count > MAX_PING_COUNT {
        return Err(format!("contagem de pings deve ser de 1 a {MAX_PING_COUNT}"));
    }
    let target_ip: Ipv4Addr = target
        .trim()
        .parse()
        .map_err(|_| format!("endereço de destino inválido: {target}"))?;
    let src = topology
        .devices
        .iter()
        .position(|d| d.id == source_id)
        .ok_or_else(|| "equipamento de origem não existe".to_string())?;

    let mut sim = Simulation::build(topology, Endpoint { dev: src, port: 0 })?;
    sim.target = target_ip;
    sim.check_source(src, target_ip)?;

    let mut replies = Vec::new();
    let mut error = None;
    let mut transmitted = 0u8;
    for seq in 1..=count as u16 {
        transmitted += 1;
        sim.echo = Some(EchoState { seq, started_at: sim.now, reply_at: None });
        if let Err(e) = sim.send_echo_request(src, target_ip, seq) {
            error = Some(e);
            break;
        }
        sim.run_until_reply();
        if sim.storm {
            error = Some(format!(
                "limite de {MAX_EVENTS} eventos atingido: possível laço de camada 2 (STP não implementado)"
            ));
            break;
        }
        match sim.echo.take() {
            Some(EchoState { started_at, reply_at: Some(at), .. }) => {
                replies.push(PingReply { seq, rtt_us: at - started_at });
            }
            _ => {
                error = Some(format!("sem resposta para icmp_seq={seq}"));
                break;
            }
        }
    }

    Ok(PingOutcome {
        target: target_ip.to_string(),
        transmitted,
        replies,
        error,
        log: sim.log,
        log_truncated: sim.log_truncated,
        pcap: sim.pcap.into_bytes(),
    })
}

fn endpoint_of(topology: &Topology, index: &HashMap<&str, usize>, device: &str, port: &str) -> Result<Endpoint, String> {
    let dev = *index.get(device).ok_or_else(|| format!("equipamento inexistente: {device}"))?;
    let port_pos = topology.devices[dev]
        .ports
        .iter()
        .position(|p| p.id == port)
        .ok_or_else(|| format!("porta inexistente: {port}"))?;
    Ok(Endpoint { dev, port: port_pos })
}

impl Simulation {
    fn build(t: &Topology, capture: Endpoint) -> Result<Simulation, String> {
        let mut devices = Vec::with_capacity(t.devices.len());
        for (i, d) in t.devices.iter().enumerate() {
            let addrs = d
                .ports
                .iter()
                .map(parse_address)
                .collect::<Result<Vec<_>, _>>()?;
            devices.push(Runtime {
                name: d.name.clone(),
                kind: d.kind,
                macs: (0..d.ports.len()).map(|p| mac_for(i as u16, p as u8)).collect(),
                addrs,
                arp: HashMap::new(),
                pending: None,
                mac_table: HashMap::new(),
            });
        }
        let index: HashMap<&str, usize> = t.devices.iter().enumerate().map(|(i, d)| (d.id.as_str(), i)).collect();
        let mut wires = HashMap::new();
        for l in &t.links {
            let a = endpoint_of(t, &index, &l.from.device_id, &l.from.port_id)?;
            let b = endpoint_of(t, &index, &l.to.device_id, &l.to.port_id)?;
            wires.insert(a, b);
            wires.insert(b, a);
        }
        Ok(Simulation {
            devices,
            wires,
            queue: BinaryHeap::new(),
            now: 0,
            counter: 0,
            ip_id: 1,
            events: 0,
            storm: false,
            log: Vec::new(),
            log_truncated: false,
            capture,
            pcap: PcapWriter::new(),
            echo: None,
            target: Ipv4Addr::UNSPECIFIED,
        })
    }

    fn check_source(&self, src: usize, target: Ipv4Addr) -> Result<(), String> {
        let dev = &self.devices[src];
        if !dev.kind.is_host() {
            return Err(format!("{} não envia ping: somente PCs e servidores nesta fase", dev.name));
        }
        let (ip, prefix) = dev.addrs[0].ok_or_else(|| format!("{} não tem endereço IP", dev.name))?;
        if !self.wires.contains_key(&Endpoint { dev: src, port: 0 }) {
            return Err(format!("{} não está ligado a nenhum equipamento", dev.name));
        }
        if ip == target {
            return Err("o destino é o próprio endereço da origem".into());
        }
        let mask = netmask(prefix);
        if u32::from(ip) & mask != u32::from(target) & mask {
            return Err(format!(
                "destino {target} fora da sub-rede de {ip}/{prefix}: roteamento chega na Fase 4"
            ));
        }
        Ok(())
    }

    fn next_seq(&mut self) -> u64 {
        self.counter += 1;
        self.counter
    }

    fn push_log(&mut self, time_us: u64, device: usize, kind: LogKind, detail: String) {
        if self.log.len() >= MAX_LOG_ENTRIES {
            self.log_truncated = true;
            return;
        }
        let name = self.devices[device].name.clone();
        self.log.push(LogEntry { time_us, device: name, kind, detail });
    }

    fn new_ipv4(&mut self, src: Ipv4Addr, dst: Ipv4Addr, protocol: u8, payload: Vec<u8>) -> Ipv4Packet {
        let id = self.ip_id;
        self.ip_id = self.ip_id.wrapping_add(1);
        Ipv4Packet { src, dst, ttl: DEFAULT_TTL, protocol, id, payload }
    }

    fn send_echo_request(&mut self, host: usize, dst: Ipv4Addr, seq: u16) -> Result<(), String> {
        let (src_ip, _) = self.devices[host].addrs[0].ok_or("origem sem endereço IP")?;
        let icmp = IcmpEcho { kind: TYPE_ECHO_REQUEST, id: ECHO_ID, seq, data: vec![0xab; ECHO_DATA_LEN] }.encode();
        let pkt = self.new_ipv4(src_ip, dst, PROTO_ICMP, icmp);
        self.host_send_ip(host, dst, pkt)
    }

    /// Envia um pacote IPv4 pela interface do host. Usa ARP se o MAC do destino não é conhecido.
    fn host_send_ip(&mut self, host: usize, dst: Ipv4Addr, pkt: Ipv4Packet) -> Result<(), String> {
        let port0 = Endpoint { dev: host, port: 0 };
        if !self.wires.contains_key(&port0) {
            return Err("a interface não está ligada a nenhum equipamento".into());
        }
        let my_mac = self.devices[host].macs[0];
        if let Some(mac) = self.devices[host].arp.get(&dst).copied() {
            let payload = pkt.encode()?;
            let bytes = Frame { dst: mac, src: my_mac, ethertype: ETHERTYPE_IPV4, payload }.encode();
            self.transmit(port0, bytes, LogKind::Tx);
        } else {
            let (my_ip, _) = self.devices[host].addrs[0].ok_or("origem sem endereço IP")?;
            let request = ArpPacket { op: OP_REQUEST, sender_mac: my_mac, sender_ip: my_ip, target_mac: [0; 6], target_ip: dst };
            let frame = Frame { dst: BROADCAST, src: my_mac, ethertype: ETHERTYPE_ARP, payload: request.encode() };
            self.devices[host].pending = Some((dst, pkt));
            self.transmit(port0, frame.encode(), LogKind::Tx);
        }
        Ok(())
    }

    /// Coloca o quadro na fila do outro lado do link. Registra o log e a captura.
    fn transmit(&mut self, from: Endpoint, bytes: Vec<u8>, kind: LogKind) {
        let Some(to) = self.wires.get(&from).copied() else {
            let detail = "porta sem link; quadro descartado".to_string();
            self.push_log(self.now, from.dev, LogKind::Drop, detail);
            return;
        };
        if from == self.capture {
            self.pcap.record(self.now, &bytes);
        }
        let detail = describe(&bytes);
        self.push_log(self.now, from.dev, kind, detail);
        let seq = self.next_seq();
        let at = self.now + LINK_LATENCY_US;
        self.queue.push(Pending { at, seq, to, bytes });
    }

    fn run_until_reply(&mut self) {
        loop {
            if self.echo.as_ref().is_some_and(|e| e.reply_at.is_some()) {
                break;
            }
            let Some(p) = self.queue.pop() else { break };
            self.events += 1;
            if self.events > MAX_EVENTS {
                self.storm = true;
                break;
            }
            self.now = p.at;
            self.deliver(p.to, p.bytes);
        }
    }

    fn deliver(&mut self, to: Endpoint, bytes: Vec<u8>) {
        if to == self.capture {
            self.pcap.record(self.now, &bytes);
        }
        match self.devices[to.dev].kind {
            DeviceKind::Switch => self.switch_rx(to, bytes),
            _ => self.host_rx(to, bytes),
        }
    }

    fn switch_rx(&mut self, at: Endpoint, bytes: Vec<u8>) {
        let dev = at.dev;
        let frame = match Frame::decode(&bytes) {
            Ok(f) => f,
            Err(e) => {
                self.push_log(self.now, dev, LogKind::Drop, e);
                return;
            }
        };
        if frame.src[0] & 1 == 0 {
            let previous = self.devices[dev].mac_table.insert(frame.src, at.port);
            if previous != Some(at.port) {
                let detail = format!("aprendeu {} na porta {}", mac_text(&frame.src), at.port + 1);
                self.push_log(self.now, dev, LogKind::Learn, detail);
            }
        }
        let known = if frame.dst == BROADCAST || frame.dst[0] & 1 == 1 {
            None
        } else {
            self.devices[dev].mac_table.get(&frame.dst).copied()
        };
        match known {
            Some(p) if p == at.port => {
                self.push_log(self.now, dev, LogKind::Drop, "destino na mesma porta de origem".into());
            }
            Some(p) => self.transmit(Endpoint { dev, port: p }, bytes, LogKind::Forward),
            None => {
                for p in 0..self.devices[dev].macs.len() {
                    let out = Endpoint { dev, port: p };
                    if p != at.port && self.wires.contains_key(&out) {
                        self.transmit(out, bytes.clone(), LogKind::Flood);
                    }
                }
            }
        }
    }

    fn host_rx(&mut self, at: Endpoint, bytes: Vec<u8>) {
        let h = at.dev;
        let frame = match Frame::decode(&bytes) {
            Ok(f) => f,
            Err(e) => {
                self.push_log(self.now, h, LogKind::Drop, e);
                return;
            }
        };
        let my_mac = self.devices[h].macs[0];
        if frame.dst != my_mac && frame.dst != BROADCAST {
            self.push_log(self.now, h, LogKind::Drop, "quadro não endereçado a esta interface".into());
            return;
        }
        self.push_log(self.now, h, LogKind::Rx, describe(&bytes));
        match frame.ethertype {
            ETHERTYPE_ARP => self.arp_rx(h, &frame),
            ETHERTYPE_IPV4 => self.ipv4_rx(h, &frame),
            other => self.push_log(self.now, h, LogKind::Drop, format!("EtherType {other:#06x} não suportado")),
        }
    }

    fn arp_rx(&mut self, h: usize, frame: &Frame) {
        let arp = match ArpPacket::decode(&frame.payload) {
            Ok(a) => a,
            Err(e) => {
                self.push_log(self.now, h, LogKind::Drop, e);
                return;
            }
        };
        let my_mac = self.devices[h].macs[0];
        let Some((my_ip, _)) = self.devices[h].addrs[0] else {
            self.push_log(self.now, h, LogKind::Drop, "interface sem endereço IP; ARP ignorado".into());
            return;
        };
        if arp.target_ip != my_ip {
            self.push_log(self.now, h, LogKind::Drop, format!("ARP para {} não é deste host ({my_ip})", arp.target_ip));
        }
        if arp.target_ip == my_ip {
            self.devices[h].arp.insert(arp.sender_ip, arp.sender_mac);
            if arp.op == OP_REQUEST {
                let reply = ArpPacket {
                    op: OP_REPLY,
                    sender_mac: my_mac,
                    sender_ip: my_ip,
                    target_mac: arp.sender_mac,
                    target_ip: arp.sender_ip,
                };
                let f = Frame { dst: arp.sender_mac, src: my_mac, ethertype: ETHERTYPE_ARP, payload: reply.encode() };
                self.transmit(Endpoint { dev: h, port: 0 }, f.encode(), LogKind::Tx);
            }
        }
        let waiting = self.devices[h]
            .pending
            .as_ref()
            .is_some_and(|(dst, _)| self.devices[h].arp.contains_key(dst));
        if waiting {
            if let Some((dst, pkt)) = self.devices[h].pending.take() {
                if let Err(e) = self.host_send_ip(h, dst, pkt) {
                    self.push_log(self.now, h, LogKind::Drop, e);
                }
            }
        }
    }

    fn ipv4_rx(&mut self, h: usize, frame: &Frame) {
        let pkt = match Ipv4Packet::decode(&frame.payload) {
            Ok(p) => p,
            Err(e) => {
                self.push_log(self.now, h, LogKind::Drop, e);
                return;
            }
        };
        let Some((my_ip, _)) = self.devices[h].addrs[0] else {
            self.push_log(self.now, h, LogKind::Drop, "interface sem endereço IP; pacote ignorado".into());
            return;
        };
        if pkt.dst != my_ip {
            self.push_log(self.now, h, LogKind::Drop, format!("IPv4 destinado a {}", pkt.dst));
            return;
        }
        if pkt.protocol != PROTO_ICMP {
            self.push_log(self.now, h, LogKind::Drop, format!("protocolo IP {} não suportado", pkt.protocol));
            return;
        }
        let echo = match IcmpEcho::decode(&pkt.payload) {
            Ok(e) => e,
            Err(e) => {
                self.push_log(self.now, h, LogKind::Drop, e);
                return;
            }
        };
        match echo.kind {
            TYPE_ECHO_REQUEST => {
                let icmp = IcmpEcho { kind: TYPE_ECHO_REPLY, id: echo.id, seq: echo.seq, data: echo.data }.encode();
                let reply = self.new_ipv4(my_ip, pkt.src, PROTO_ICMP, icmp);
                if let Err(e) = self.host_send_ip(h, pkt.src, reply) {
                    self.push_log(self.now, h, LogKind::Drop, e);
                }
            }
            TYPE_ECHO_REPLY => {
                let now = self.now;
                let target = self.target;
                if let Some(state) = self.echo.as_mut() {
                    if echo.seq == state.seq && echo.id == ECHO_ID && pkt.src == target && state.reply_at.is_none() {
                        state.reply_at = Some(now);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Resumo legível de um quadro, usado no log e na inspeção.
pub fn describe(bytes: &[u8]) -> String {
    let frame = match Frame::decode(bytes) {
        Ok(f) => f,
        Err(e) => return format!("quadro inválido: {e}"),
    };
    let head = format!("{} > {}", mac_text(&frame.src), mac_text(&frame.dst));
    let body = match frame.ethertype {
        ETHERTYPE_ARP => match ArpPacket::decode(&frame.payload) {
            Ok(a) if a.op == OP_REQUEST => format!("ARP quem tem {} pergunta por {}", a.target_ip, a.sender_ip),
            Ok(a) => format!("ARP {} está em {}", a.sender_ip, mac_text(&a.sender_mac)),
            Err(e) => format!("ARP inválido: {e}"),
        },
        ETHERTYPE_IPV4 => match Ipv4Packet::decode(&frame.payload) {
            Ok(p) if p.protocol == PROTO_ICMP => match IcmpEcho::decode(&p.payload) {
                Ok(e) if e.kind == TYPE_ECHO_REQUEST => format!("IPv4 {} > {} ICMP echo request seq={}", p.src, p.dst, e.seq),
                Ok(e) => format!("IPv4 {} > {} ICMP echo reply seq={}", p.src, p.dst, e.seq),
                Err(e) => format!("IPv4 {} > {} ICMP inválido: {e}", p.src, p.dst),
            },
            Ok(p) => format!("IPv4 {} > {} protocolo {}", p.src, p.dst, p.protocol),
            Err(e) => format!("IPv4 inválido: {e}"),
        },
        other => format!("EtherType {other:#06x}"),
    };
    format!("{head}: {body}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{sample_pc, Device, DeviceKind, Link, Port, PortRef, SCHEMA_VERSION};

    fn switch(id: &str) -> Device {
        Device {
            id: id.into(),
            kind: DeviceKind::Switch,
            name: format!("Switch {id}"),
            x: 0.0,
            y: 0.0,
            ports: (1..=8).map(|i| Port { id: format!("p{i}"), name: format!("Porta {i}"), ip: None, prefix_len: None }).collect(),
        }
    }

    fn link(id: &str, a: (&str, &str), b: (&str, &str)) -> Link {
        Link {
            id: id.into(),
            from: PortRef { device_id: a.0.into(), port_id: a.1.into() },
            to: PortRef { device_id: b.0.into(), port_id: b.1.into() },
        }
    }

    /// PC A (10.0.0.1) e PC B (10.0.0.2) ligados a um switch.
    fn two_pcs_and_switch() -> Topology {
        Topology {
            schema_version: SCHEMA_VERSION,
            devices: vec![sample_pc("a", Some("10.0.0.1")), sample_pc("b", Some("10.0.0.2")), switch("sw")],
            links: vec![link("l1", ("a", "p1"), ("sw", "p1")), link("l2", ("b", "p1"), ("sw", "p2"))],
        }
    }

    fn pcap_record_count(pcap: &[u8]) -> usize {
        let mut pos = 24;
        let mut n = 0;
        while pos + 16 <= pcap.len() {
            let len = u32::from_le_bytes([pcap[pos + 8], pcap[pos + 9], pcap[pos + 10], pcap[pos + 11]]) as usize;
            pos += 16 + len;
            n += 1;
        }
        n
    }

    #[test]
    fn ping_through_switch_resolves_arp_and_replies() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 1).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.replies.len(), 1);
        // ARP (4000 µs ida e volta até B, mais o retorno) + echo: 8 saltos de 1000 µs.
        assert_eq!(out.replies[0].rtt_us, 8_000);
    }

    #[test]
    fn event_sequence_is_deterministic_and_ordered() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 1).unwrap();
        let kinds: Vec<(LogKind, &str)> = out.log.iter().map(|e| (e.kind, e.device.as_str())).collect();
        assert_eq!(
            kinds,
            vec![
                (LogKind::Tx, "PC a"),        // ARP request de A
                (LogKind::Learn, "Switch sw"), // switch aprende A na porta 1
                (LogKind::Flood, "Switch sw"), // destino desconhecido: inunda
                (LogKind::Rx, "PC b"),         // B recebe o ARP request
                (LogKind::Tx, "PC b"),         // B responde ARP
                (LogKind::Learn, "Switch sw"), // switch aprende B na porta 2
                (LogKind::Forward, "Switch sw"),
                (LogKind::Rx, "PC a"),         // A recebe o ARP reply
                (LogKind::Tx, "PC a"),         // echo request
                (LogKind::Forward, "Switch sw"),
                (LogKind::Rx, "PC b"),
                (LogKind::Tx, "PC b"),         // echo reply
                (LogKind::Forward, "Switch sw"),
                (LogKind::Rx, "PC a"),
            ]
        );
    }

    #[test]
    fn pcap_captures_the_source_interface_only() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 1).unwrap();
        assert_eq!(&out.pcap[0..4], &[0xd4, 0xc3, 0xb2, 0xa1]);
        assert_eq!(pcap_record_count(&out.pcap), 4);
        // Dois quadros ARP de 60 bytes e dois ICMP de 74 bytes, cada um precedido de 16 bytes de cabeçalho.
        assert_eq!(out.pcap.len(), 24 + 2 * (16 + 60) + 2 * (16 + 74));
    }

    #[test]
    fn repeated_pings_keep_the_cache_and_rtt_is_stable() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 3).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.replies.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![1, 2, 3]);
        // Só o primeiro echo espera ARP; os seguintes levam 4 saltos.
        assert_eq!(out.replies[0].rtt_us, 8_000);
        assert_eq!(out.replies[1].rtt_us, 4_000);
        assert_eq!(out.replies[2].rtt_us, 4_000);
    }

    #[test]
    fn destination_outside_subnet_is_rejected() {
        let err = run_ping(&two_pcs_and_switch(), "a", "192.168.9.9", 1).unwrap_err();
        assert!(err.contains("fora da sub-rede"), "{err}");
    }

    #[test]
    fn unknown_host_in_subnet_gets_no_reply() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.50", 2).unwrap();
        assert!(out.replies.is_empty());
        assert_eq!(out.transmitted, 1);
        assert_eq!(out.error.as_deref(), Some("sem resposta para icmp_seq=1"));
    }

    #[test]
    fn source_without_link_or_address_is_rejected() {
        let mut t = two_pcs_and_switch();
        t.links.remove(0);
        assert!(run_ping(&t, "a", "10.0.0.2", 1).unwrap_err().contains("não está ligado"));

        let mut t = two_pcs_and_switch();
        t.devices[0].ports[0].ip = None;
        t.devices[0].ports[0].prefix_len = None;
        assert!(run_ping(&t, "a", "10.0.0.2", 1).unwrap_err().contains("não tem endereço"));
    }

    #[test]
    fn switch_cannot_be_a_ping_source() {
        let err = run_ping(&two_pcs_and_switch(), "sw", "10.0.0.2", 1).unwrap_err();
        assert!(err.contains("não envia ping"), "{err}");
    }

    #[test]
    fn invalid_count_and_target_are_rejected() {
        assert!(run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 0).is_err());
        assert!(run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 11).is_err());
        assert!(run_ping(&two_pcs_and_switch(), "a", "não é ip", 1).is_err());
    }

    #[test]
    fn switch_loop_is_bounded_and_reported() {
        // PC A - SW1 ; SW1 p2 - SW2 p1 ; SW2 p2 - SW1 p3 (laço) ; PC B - SW2 p3.
        let t = Topology {
            schema_version: SCHEMA_VERSION,
            devices: vec![
                sample_pc("a", Some("10.0.0.1")),
                sample_pc("b", Some("10.0.0.2")),
                switch("s1"),
                switch("s2"),
            ],
            links: vec![
                link("l1", ("a", "p1"), ("s1", "p1")),
                link("l2", ("s1", "p2"), ("s2", "p1")),
                link("l3", ("s2", "p2"), ("s1", "p3")),
                link("l4", ("b", "p1"), ("s2", "p3")),
            ],
        };
        let out = run_ping(&t, "a", "10.0.0.2", 1).unwrap();
        // Sem STP a inundação pode impedir a resposta; o motor deve terminar e reportar.
        assert!(out.replies.len() == 1 || out.error.is_some());
        assert!(out.log.len() <= MAX_LOG_ENTRIES);
    }

    #[test]
    fn invalid_topology_is_rejected_before_simulating() {
        let mut t = two_pcs_and_switch();
        t.schema_version = 7;
        assert!(run_ping(&t, "a", "10.0.0.2", 1).is_err());
    }

    #[test]
    fn describe_summarises_arp_and_icmp() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 1).unwrap();
        assert!(out.log.iter().any(|e| e.detail.contains("ARP quem tem 10.0.0.2 pergunta por 10.0.0.1")));
        assert!(out.log.iter().any(|e| e.detail.contains("ICMP echo request seq=1")));
        assert!(out.log.iter().any(|e| e.detail.contains("ICMP echo reply seq=1")));
    }
}
