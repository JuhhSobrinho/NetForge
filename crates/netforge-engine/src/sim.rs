//! Motor de simulação: fila de eventos com tempo virtual, hosts e roteadores com ARP/IPv4/ICMP,
//! switches com aprendizado de MAC.
//!
//! Regras desta fase (ver docs/PROTOCOLS.md e docs/TESTING_STRATEGY.md):
//! - Cada link leva `LINK_LATENCY_US` de tempo virtual. Não há perda de pacotes.
//! - Hosts usam o gateway padrão para destinos fora da sub-rede; roteadores usam rotas
//!   conectadas e estáticas (maior prefixo vence; rota conectada vence rota estática empatada).
//! - Roteadores decrementam o TTL e respondem com ICMP (destino inalcançável, tempo excedido).
//! - Switches não implementam STP: um laço gera tempestade de broadcast, que é interrompida
//!   pelo limite `MAX_EVENTS` e reportada como erro.
//! - A captura PCAP é feita na interface de saída da origem (quadros transmitidos e recebidos).
//! - Cada chamada de `run_ping` cria uma simulação nova: ARP e MAC não persistem entre chamadas.

use crate::arp::{ArpPacket, OP_REPLY, OP_REQUEST};
use crate::ethernet::{mac_for, mac_text, Frame, Mac, BROADCAST, ETHERTYPE_ARP, ETHERTYPE_IPV4};
use crate::icmp::{
    encode_error, IcmpEcho, IcmpError, CODE_NET_UNREACHABLE, CODE_TTL_IN_TRANSIT, TYPE_DEST_UNREACHABLE,
    TYPE_ECHO_REPLY, TYPE_ECHO_REQUEST, TYPE_TIME_EXCEEDED,
};
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
pub(crate) const ECHO_ID: u16 = 0x4e46;
const ECHO_DATA_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Endpoint {
    pub dev: usize,
    pub port: usize,
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

struct Iface {
    mac: Mac,
    addr: Option<Address>,
    /// Ligada a algum link.
    linked: bool,
    /// Ligada e não administrativamente desligada.
    up: bool,
}

struct Route {
    net: u32,
    prefix: u8,
    via: Ipv4Addr,
}

struct Runtime {
    name: String,
    kind: DeviceKind,
    ifaces: Vec<Iface>,
    arp: HashMap<Ipv4Addr, Mac>,
    /// Pacotes esperando a resolução ARP do próximo salto: próximo salto → (porta, pacote).
    waiting: HashMap<Ipv4Addr, Vec<(usize, Ipv4Packet)>>,
    mac_table: HashMap<Mac, usize>,
    routes: Vec<Route>,
}

struct EchoState {
    target: Ipv4Addr,
    seq: u16,
    started_at: u64,
    reply_at: Option<u64>,
    error: Option<String>,
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
    capture: Option<Endpoint>,
    pcap: PcapWriter,
    echo: Option<EchoState>,
}

/// Executa `count` echo requests de `source_id` para `target` e devolve o resultado.
/// Erros de entrada (topologia inválida, origem sem rota) retornam `Err`.
/// Falhas de rede (sem resposta, erro ICMP, laço) retornam `Ok` com `error` preenchido.
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

    let mut sim = Simulation::build(topology)?;
    sim.check_source(src, target_ip)?;
    let (egress, _) = sim.lookup(src, target_ip).ok_or("sem rota")?;
    sim.capture = Some(Endpoint { dev: src, port: egress });

    let mut replies = Vec::new();
    let mut error = None;
    let mut transmitted = 0u8;
    for seq in 1..=count as u16 {
        transmitted += 1;
        sim.echo = Some(EchoState { target: target_ip, seq, started_at: sim.now, reply_at: None, error: None });
        if let Err(e) = sim.send_echo_request(src, target_ip, seq) {
            error = Some(e);
            break;
        }
        sim.run_until_done();
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
            Some(EchoState { error: Some(e), .. }) => {
                error = Some(e);
                break;
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
    fn build(t: &Topology) -> Result<Simulation, String> {
        let index: HashMap<&str, usize> = t.devices.iter().enumerate().map(|(i, d)| (d.id.as_str(), i)).collect();
        let mut wires = HashMap::new();
        for l in &t.links {
            let a = endpoint_of(t, &index, &l.from.device_id, &l.from.port_id)?;
            let b = endpoint_of(t, &index, &l.to.device_id, &l.to.port_id)?;
            wires.insert(a, b);
            wires.insert(b, a);
        }
        let mut devices = Vec::with_capacity(t.devices.len());
        for (i, d) in t.devices.iter().enumerate() {
            let mut ifaces = Vec::with_capacity(d.ports.len());
            for (p, port) in d.ports.iter().enumerate() {
                let linked = wires.contains_key(&Endpoint { dev: i, port: p });
                ifaces.push(Iface {
                    mac: mac_for(i as u16, p as u8),
                    addr: parse_address(port)?,
                    linked,
                    up: linked && !port.shutdown,
                });
            }
            let mut routes = Vec::new();
            for r in &d.routes {
                let addr: Ipv4Addr = r.network.parse().map_err(|_| "rede de rota inválida")?;
                let via: Ipv4Addr = r.next_hop.parse().map_err(|_| "próximo salto inválido")?;
                routes.push(Route { net: u32::from(addr) & netmask(r.prefix_len), prefix: r.prefix_len, via });
            }
            if let Some(gw) = &d.gateway {
                let via: Ipv4Addr = gw.parse().map_err(|_| "gateway inválido")?;
                routes.push(Route { net: 0, prefix: 0, via });
            }
            devices.push(Runtime {
                name: d.name.clone(),
                kind: d.kind,
                ifaces,
                arp: HashMap::new(),
                waiting: HashMap::new(),
                mac_table: HashMap::new(),
                routes,
            });
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
            capture: None,
            pcap: PcapWriter::new(),
            echo: None,
        })
    }

    /// Escolhe a interface de saída e o próximo salto para um destino.
    /// Interfaces conectadas vencem por maior prefixo; rotas estáticas precisam de um próximo
    /// salto alcançável por uma interface conectada.
    fn lookup(&self, dev: usize, dst: Ipv4Addr) -> Option<(usize, Ipv4Addr)> {
        let rt = &self.devices[dev];
        // (prefixo, tipo: 0 = conectada, 1 = estática, porta, próximo salto)
        let mut best: Option<(u8, u8, usize, Ipv4Addr)> = None;
        let mut consider = |cand: (u8, u8, usize, Ipv4Addr)| {
            let better = match best {
                None => true,
                Some((bp, bk, _, _)) => cand.0 > bp || (cand.0 == bp && cand.1 < bk),
            };
            if better {
                best = Some(cand);
            }
        };
        for (i, f) in rt.ifaces.iter().enumerate() {
            if let (true, Some((ip, p))) = (f.up, f.addr) {
                if u32::from(dst) & netmask(p) == u32::from(ip) & netmask(p) {
                    consider((p, 0, i, dst));
                }
            }
        }
        for r in &rt.routes {
            let egress = rt.ifaces.iter().enumerate().find(|(_, f)| {
                f.up && f.addr.is_some_and(|(ip, p)| u32::from(r.via) & netmask(p) == u32::from(ip) & netmask(p))
            });
            if let Some((port, _)) = egress {
                if u32::from(dst) & netmask(r.prefix) == r.net {
                    consider((r.prefix, 1, port, r.via));
                }
            }
        }
        best.map(|(_, _, port, nh)| (port, nh))
    }

    fn is_local(&self, dev: usize, ip: Ipv4Addr) -> bool {
        self.devices[dev].ifaces.iter().any(|f| f.up && f.addr.is_some_and(|(a, _)| a == ip))
    }

    fn check_source(&self, src: usize, target: Ipv4Addr) -> Result<(), String> {
        let dev = &self.devices[src];
        if dev.kind == DeviceKind::Switch {
            return Err(format!("{} não envia ping: switches não têm endereço IP", dev.name));
        }
        if !dev.ifaces.iter().any(|f| f.addr.is_some()) {
            return Err(format!("{} não tem endereço IP", dev.name));
        }
        if !dev.ifaces.iter().any(|f| f.linked) {
            return Err(format!("{} não está ligado a nenhum equipamento", dev.name));
        }
        if !dev.ifaces.iter().any(|f| f.up) {
            return Err(format!("{} não tem interface ativa (verifique shutdown)", dev.name));
        }
        if self.is_local(src, target) {
            return Err("o destino é o próprio endereço da origem".into());
        }
        if self.lookup(src, target).is_none() {
            return Err(format!(
                "{} não tem rota para {target}: defina ip default-gateway ou uma rota estática",
                dev.name
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
        let (port, _) = self.lookup(host, dst).ok_or("sem rota")?;
        let (src_ip, _) = self.devices[host].ifaces[port].addr.ok_or("interface sem endereço IP")?;
        let icmp = IcmpEcho { kind: TYPE_ECHO_REQUEST, id: ECHO_ID, seq, data: vec![0xab; ECHO_DATA_LEN] }.encode();
        let pkt = self.new_ipv4(src_ip, dst, PROTO_ICMP, icmp);
        self.send_ip(host, pkt)
    }

    /// Envia um pacote originado no próprio equipamento, pela interface que a rota escolher.
    fn send_ip(&mut self, dev: usize, pkt: Ipv4Packet) -> Result<(), String> {
        let (port, nh) = self.lookup(dev, pkt.dst).ok_or_else(|| format!("sem rota para {}", pkt.dst))?;
        self.send_via(dev, port, nh, pkt)
    }

    /// Entrega o pacote ao próximo salto pela porta dada. Se o MAC do próximo salto não é
    /// conhecido, envia ARP e espera a resposta.
    fn send_via(&mut self, dev: usize, port: usize, nh: Ipv4Addr, pkt: Ipv4Packet) -> Result<(), String> {
        let ep = Endpoint { dev, port };
        if !self.devices[dev].ifaces[port].up {
            return Err("interface de saída desligada".into());
        }
        let my_mac = self.devices[dev].ifaces[port].mac;
        if let Some(mac) = self.devices[dev].arp.get(&nh).copied() {
            let payload = pkt.encode()?;
            let bytes = Frame { dst: mac, src: my_mac, ethertype: ETHERTYPE_IPV4, payload }.encode();
            self.transmit(ep, bytes, LogKind::Tx);
        } else {
            let first = !self.devices[dev].waiting.contains_key(&nh);
            self.devices[dev].waiting.entry(nh).or_default().push((port, pkt));
            if first {
                let (my_ip, _) = self.devices[dev].ifaces[port].addr.ok_or("interface sem endereço IP")?;
                let request = ArpPacket { op: OP_REQUEST, sender_mac: my_mac, sender_ip: my_ip, target_mac: [0; 6], target_ip: nh };
                let frame = Frame { dst: BROADCAST, src: my_mac, ethertype: ETHERTYPE_ARP, payload: request.encode() };
                self.transmit(ep, frame.encode(), LogKind::Tx);
            }
        }
        Ok(())
    }

    /// Coloca o quadro na fila do outro lado do link. Registra o log e a captura.
    fn transmit(&mut self, from: Endpoint, bytes: Vec<u8>, kind: LogKind) {
        if !self.devices[from.dev].ifaces[from.port].up {
            self.push_log(self.now, from.dev, LogKind::Drop, "interface desligada; quadro descartado".into());
            return;
        }
        let Some(to) = self.wires.get(&from).copied() else {
            self.push_log(self.now, from.dev, LogKind::Drop, "porta sem link; quadro descartado".into());
            return;
        };
        if self.capture == Some(from) {
            self.pcap.record(self.now, &bytes);
        }
        let detail = describe(&bytes);
        self.push_log(self.now, from.dev, kind, detail);
        let seq = self.next_seq();
        let at = self.now + LINK_LATENCY_US;
        self.queue.push(Pending { at, seq, to, bytes });
    }

    fn run_until_done(&mut self) {
        loop {
            if self.echo.as_ref().is_some_and(|e| e.reply_at.is_some() || e.error.is_some()) {
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
        if !self.devices[to.dev].ifaces[to.port].up {
            self.push_log(self.now, to.dev, LogKind::Drop, "interface desligada; quadro descartado".into());
            return;
        }
        if self.capture == Some(to) {
            self.pcap.record(self.now, &bytes);
        }
        match self.devices[to.dev].kind {
            DeviceKind::Switch => self.switch_rx(to, bytes),
            _ => self.device_rx(to, bytes),
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
                for p in 0..self.devices[dev].ifaces.len() {
                    let out = Endpoint { dev, port: p };
                    if p != at.port && self.wires.contains_key(&out) {
                        self.transmit(out, bytes.clone(), LogKind::Flood);
                    }
                }
            }
        }
    }

    /// Recebimento em hosts e roteadores.
    fn device_rx(&mut self, at: Endpoint, bytes: Vec<u8>) {
        let dev = at.dev;
        let frame = match Frame::decode(&bytes) {
            Ok(f) => f,
            Err(e) => {
                self.push_log(self.now, dev, LogKind::Drop, e);
                return;
            }
        };
        let my_mac = self.devices[dev].ifaces[at.port].mac;
        if frame.dst != my_mac && frame.dst != BROADCAST {
            self.push_log(self.now, dev, LogKind::Drop, "quadro não endereçado a esta interface".into());
            return;
        }
        self.push_log(self.now, dev, LogKind::Rx, describe(&bytes));
        match frame.ethertype {
            ETHERTYPE_ARP => self.arp_rx(at, &frame),
            ETHERTYPE_IPV4 => self.ip_rx(at, &frame),
            other => self.push_log(self.now, dev, LogKind::Drop, format!("EtherType {other:#06x} não suportado")),
        }
    }

    fn learn(&mut self, dev: usize, ip: Ipv4Addr, mac: Mac) {
        self.devices[dev].arp.insert(ip, mac);
        if let Some(list) = self.devices[dev].waiting.remove(&ip) {
            for (port, pkt) in list {
                if let Err(e) = self.send_via(dev, port, ip, pkt) {
                    self.push_log(self.now, dev, LogKind::Drop, e);
                }
            }
        }
    }

    fn arp_rx(&mut self, at: Endpoint, frame: &Frame) {
        let dev = at.dev;
        let arp = match ArpPacket::decode(&frame.payload) {
            Ok(a) => a,
            Err(e) => {
                self.push_log(self.now, dev, LogKind::Drop, e);
                return;
            }
        };
        let Some((my_ip, _)) = self.devices[dev].ifaces[at.port].addr else {
            self.push_log(self.now, dev, LogKind::Drop, "interface sem endereço IP; ARP ignorado".into());
            return;
        };
        if arp.target_ip != my_ip {
            self.push_log(self.now, dev, LogKind::Drop, format!("ARP para {} não é desta interface", arp.target_ip));
            return;
        }
        self.learn(dev, arp.sender_ip, arp.sender_mac);
        if arp.op == OP_REQUEST {
            let my_mac = self.devices[dev].ifaces[at.port].mac;
            let reply = ArpPacket {
                op: OP_REPLY,
                sender_mac: my_mac,
                sender_ip: my_ip,
                target_mac: arp.sender_mac,
                target_ip: arp.sender_ip,
            };
            let f = Frame { dst: arp.sender_mac, src: my_mac, ethertype: ETHERTYPE_ARP, payload: reply.encode() };
            self.transmit(at, f.encode(), LogKind::Tx);
        }
    }

    fn ip_rx(&mut self, at: Endpoint, frame: &Frame) {
        let dev = at.dev;
        let pkt = match Ipv4Packet::decode(&frame.payload) {
            Ok(p) => p,
            Err(e) => {
                self.push_log(self.now, dev, LogKind::Drop, e);
                return;
            }
        };
        if self.is_local(dev, pkt.dst) {
            if pkt.protocol != PROTO_ICMP {
                self.push_log(self.now, dev, LogKind::Drop, format!("protocolo IP {} não suportado", pkt.protocol));
                return;
            }
            self.icmp_rx(dev, &pkt);
            return;
        }
        if self.devices[dev].kind != DeviceKind::Router {
            self.push_log(self.now, dev, LogKind::Drop, format!("IPv4 destinado a {}", pkt.dst));
            return;
        }
        self.forward(dev, at.port, pkt);
    }

    /// Encaminhamento de um roteador: decrementa o TTL, consulta a tabela e envia.
    fn forward(&mut self, dev: usize, in_port: usize, mut pkt: Ipv4Packet) {
        let ingress_ip = self.devices[dev].ifaces[in_port].addr.map(|a| a.0);
        if pkt.ttl <= 1 {
            self.push_log(self.now, dev, LogKind::Drop, format!("TTL expirado para {}", pkt.dst));
            if let Some(from) = ingress_ip {
                self.send_icmp_error(dev, from, &pkt, TYPE_TIME_EXCEEDED, CODE_TTL_IN_TRANSIT);
            }
            return;
        }
        pkt.ttl -= 1;
        match self.lookup(dev, pkt.dst) {
            None => {
                self.push_log(self.now, dev, LogKind::Drop, format!("sem rota para {}", pkt.dst));
                if let Some(from) = ingress_ip {
                    self.send_icmp_error(dev, from, &pkt, TYPE_DEST_UNREACHABLE, CODE_NET_UNREACHABLE);
                }
            }
            Some((port, nh)) => {
                if let Err(e) = self.send_via(dev, port, nh, pkt) {
                    self.push_log(self.now, dev, LogKind::Drop, e);
                }
            }
        }
    }

    /// Envia uma mensagem de erro ICMP ao remetente do datagrama original.
    fn send_icmp_error(&mut self, dev: usize, from: Ipv4Addr, original: &Ipv4Packet, kind: u8, code: u8) {
        let original_bytes = match original.encode() {
            Ok(b) => b,
            Err(_) => return,
        };
        let body = encode_error(kind, code, &original_bytes);
        let p = self.new_ipv4(from, original.src, PROTO_ICMP, body);
        if let Err(e) = self.send_ip(dev, p) {
            self.push_log(self.now, dev, LogKind::Drop, e);
        }
    }

    fn icmp_rx(&mut self, dev: usize, pkt: &Ipv4Packet) {
        if let Ok(echo) = IcmpEcho::decode(&pkt.payload) {
            match echo.kind {
                TYPE_ECHO_REQUEST => {
                    let icmp = IcmpEcho { kind: TYPE_ECHO_REPLY, id: echo.id, seq: echo.seq, data: echo.data }.encode();
                    let reply = self.new_ipv4(pkt.dst, pkt.src, PROTO_ICMP, icmp);
                    if let Err(e) = self.send_ip(dev, reply) {
                        self.push_log(self.now, dev, LogKind::Drop, e);
                    }
                }
                TYPE_ECHO_REPLY => {
                    let now = self.now;
                    if let Some(st) = self.echo.as_mut() {
                        if st.reply_at.is_none()
                            && st.error.is_none()
                            && echo.seq == st.seq
                            && echo.id == ECHO_ID
                            && pkt.src == st.target
                        {
                            st.reply_at = Some(now);
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        match IcmpError::decode(&pkt.payload) {
            Ok(err) => self.icmp_error_rx(&err, pkt.src),
            Err(e) => self.push_log(self.now, dev, LogKind::Drop, format!("ICMP inválido: {e}")),
        }
    }

    /// Erro ICMP recebido: se refere ao nosso echo em curso, encerra o ping com o motivo.
    fn icmp_error_rx(&mut self, err: &IcmpError, from: Ipv4Addr) {
        // O datagrama original começa com o cabeçalho IPv4 (20 bytes) seguido do ICMP.
        if err.original.len() < 28 {
            return;
        }
        let orig_dst = Ipv4Addr::new(err.original[16], err.original[17], err.original[18], err.original[19]);
        let orig_id = u16::from_be_bytes([err.original[24], err.original[25]]);
        let orig_seq = u16::from_be_bytes([err.original[26], err.original[27]]);
        let reason = if err.kind == TYPE_DEST_UNREACHABLE {
            format!("destino inalcançável (código {}) informado por {from}", err.code)
        } else {
            format!("tempo excedido em trânsito informado por {from}")
        };
        if let Some(st) = self.echo.as_mut() {
            if st.reply_at.is_none() && st.error.is_none() && orig_dst == st.target && orig_id == ECHO_ID && orig_seq == st.seq {
                st.error = Some(reason);
            }
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
                Err(_) => match IcmpError::decode(&p.payload) {
                    Ok(e) if e.kind == TYPE_DEST_UNREACHABLE => format!("IPv4 {} > {} ICMP destino inalcançável", p.src, p.dst),
                    Ok(_) => format!("IPv4 {} > {} ICMP tempo excedido", p.src, p.dst),
                    Err(e) => format!("IPv4 {} > {} ICMP inválido: {e}", p.src, p.dst),
                },
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
    use crate::topology::{sample_pc, Device, DeviceKind, Link, Port, PortRef, StaticRoute, SCHEMA_VERSION};

    fn switch(id: &str) -> Device {
        Device {
            id: id.into(),
            kind: DeviceKind::Switch,
            name: format!("Switch {id}"),
            x: 0.0,
            y: 0.0,
            ports: (1..=8).map(|i| port(&format!("p{i}"), None, None)).collect(),
            gateway: None,
            routes: Vec::new(),
        }
    }

    fn port(id: &str, ip: Option<&str>, prefix: Option<u8>) -> Port {
        Port { id: id.into(), name: format!("Porta {id}"), ip: ip.map(String::from), prefix_len: prefix, shutdown: false }
    }

    fn router(id: &str, ports: Vec<Port>, routes: Vec<StaticRoute>) -> Device {
        Device {
            id: id.into(),
            kind: DeviceKind::Router,
            name: format!("Roteador {id}"),
            x: 0.0,
            y: 0.0,
            ports,
            gateway: None,
            routes,
        }
    }

    fn host(id: &str, ip: &str, prefix: u8, gateway: Option<&str>) -> Device {
        let mut d = sample_pc(id, Some(ip));
        d.ports[0].prefix_len = Some(prefix);
        d.gateway = gateway.map(String::from);
        d
    }

    fn link(id: &str, a: (&str, &str), b: (&str, &str)) -> Link {
        Link {
            id: id.into(),
            from: PortRef { device_id: a.0.into(), port_id: a.1.into() },
            to: PortRef { device_id: b.0.into(), port_id: b.1.into() },
        }
    }

    fn topo(devices: Vec<Device>, links: Vec<Link>) -> Topology {
        Topology { schema_version: SCHEMA_VERSION, devices, links }
    }

    /// PC A (10.0.0.1) e PC B (10.0.0.2) ligados a um switch.
    fn two_pcs_and_switch() -> Topology {
        topo(
            vec![sample_pc("a", Some("10.0.0.1")), sample_pc("b", Some("10.0.0.2")), switch("sw")],
            vec![link("l1", ("a", "p1"), ("sw", "p1")), link("l2", ("b", "p1"), ("sw", "p2"))],
        )
    }

    /// PC A (10.0.1.10) - Roteador (10.0.1.1 / 10.0.2.1) - PC B (10.0.2.10), via switches.
    fn routed_pair() -> Topology {
        topo(
            vec![
                host("a", "10.0.1.10", 24, Some("10.0.1.1")),
                host("b", "10.0.2.10", 24, Some("10.0.2.1")),
                router("r", vec![port("p1", Some("10.0.1.1"), Some(24)), port("p2", Some("10.0.2.1"), Some(24))], vec![]),
            ],
            vec![link("l1", ("a", "p1"), ("r", "p1")), link("l2", ("b", "p1"), ("r", "p2"))],
        )
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
        // ARP (ida e volta) + echo (ida e volta): 8 saltos de 1000 µs.
        assert_eq!(out.replies[0].rtt_us, 8_000);
    }

    #[test]
    fn event_sequence_is_deterministic_and_ordered() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 1).unwrap();
        let kinds: Vec<(LogKind, &str)> = out.log.iter().map(|e| (e.kind, e.device.as_str())).collect();
        assert_eq!(
            kinds,
            vec![
                (LogKind::Tx, "PC a"),
                (LogKind::Learn, "Switch sw"),
                (LogKind::Flood, "Switch sw"),
                (LogKind::Rx, "PC b"),
                (LogKind::Tx, "PC b"),
                (LogKind::Learn, "Switch sw"),
                (LogKind::Forward, "Switch sw"),
                (LogKind::Rx, "PC a"),
                (LogKind::Tx, "PC a"),
                (LogKind::Forward, "Switch sw"),
                (LogKind::Rx, "PC b"),
                (LogKind::Tx, "PC b"),
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
        assert_eq!(out.pcap.len(), 24 + 2 * (16 + 60) + 2 * (16 + 74));
    }

    #[test]
    fn repeated_pings_keep_the_cache_and_rtt_is_stable() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 3).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.replies.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(out.replies[0].rtt_us, 8_000);
        assert_eq!(out.replies[1].rtt_us, 4_000);
        assert_eq!(out.replies[2].rtt_us, 4_000);
    }

    #[test]
    fn destination_without_route_is_rejected_before_simulating() {
        // Sem gateway padrão, um host não alcança redes fora da sua sub-rede.
        let err = run_ping(&two_pcs_and_switch(), "a", "192.168.9.9", 1).unwrap_err();
        assert!(err.contains("não tem rota"), "{err}");
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
        let t = topo(
            vec![
                sample_pc("a", Some("10.0.0.1")),
                sample_pc("b", Some("10.0.0.2")),
                switch("s1"),
                switch("s2"),
            ],
            vec![
                link("l1", ("a", "p1"), ("s1", "p1")),
                link("l2", ("s1", "p2"), ("s2", "p1")),
                link("l3", ("s2", "p2"), ("s1", "p3")),
                link("l4", ("b", "p1"), ("s2", "p3")),
            ],
        );
        let out = run_ping(&t, "a", "10.0.0.2", 1).unwrap();
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

    #[test]
    fn router_forwards_between_two_subnets() {
        let out = run_ping(&routed_pair(), "a", "10.0.2.10", 2).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.replies.len(), 2);
        assert!(out.log.iter().any(|e| e.device == "Roteador r" && e.kind == LogKind::Tx));
    }

    #[test]
    fn router_without_route_answers_with_unreachable() {
        let out = run_ping(&routed_pair(), "a", "172.16.0.1", 1);
        // 172.16.0.1 não está na sub-rede de A, então A usa o gateway; o roteador não tem rota.
        let out = out.unwrap();
        assert!(out.replies.is_empty());
        let err = out.error.unwrap();
        assert!(err.contains("destino inalcançável"), "{err}");
        assert!(err.contains("10.0.1.1"), "{err}");
    }

    #[test]
    fn static_routes_in_both_directions_route_across_two_routers() {
        let t = topo(
            vec![
                host("a", "10.0.1.10", 24, Some("10.0.1.1")),
                host("b", "10.0.2.10", 24, Some("10.0.2.1")),
                router(
                    "r1",
                    vec![port("p1", Some("10.0.1.1"), Some(24)), port("p2", Some("10.0.9.1"), Some(24))],
                    vec![StaticRoute { network: "10.0.2.0".into(), prefix_len: 24, next_hop: "10.0.9.2".into() }],
                ),
                router(
                    "r2",
                    vec![port("p1", Some("10.0.9.2"), Some(24)), port("p2", Some("10.0.2.1"), Some(24))],
                    vec![StaticRoute { network: "10.0.1.0".into(), prefix_len: 24, next_hop: "10.0.9.1".into() }],
                ),
            ],
            vec![
                link("l1", ("a", "p1"), ("r1", "p1")),
                link("l2", ("r1", "p2"), ("r2", "p1")),
                link("l3", ("r2", "p2"), ("b", "p1")),
            ],
        );
        let out = run_ping(&t, "a", "10.0.2.10", 1).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.replies.len(), 1);
    }

    #[test]
    fn shutdown_interface_blocks_the_ping() {
        let mut t = routed_pair();
        t.devices[2].ports[1].shutdown = true; // desliga a interface do roteador para B
        let err = run_ping(&t, "a", "10.0.2.10", 1).unwrap();
        assert!(err.replies.is_empty());
        assert!(err.error.is_some());
    }

    #[test]
    fn shutdown_source_interface_is_rejected() {
        let mut t = routed_pair();
        t.devices[0].ports[0].shutdown = true;
        let err = run_ping(&t, "a", "10.0.2.10", 1).unwrap_err();
        assert!(err.contains("interface ativa"), "{err}");
    }
}
