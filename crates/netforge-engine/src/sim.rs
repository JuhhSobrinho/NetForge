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
use crate::dns;
use crate::dhcp::{DhcpMessage, CLIENT_PORT, MSG_ACK, MSG_DISCOVER, MSG_OFFER, MSG_REQUEST, OP_REPLY as DHCP_REPLY, OP_REQUEST as DHCP_REQUEST, SERVER_PORT};
use crate::ipv4::{Ipv4Packet, DEFAULT_TTL, PROTO_ICMP};
use crate::udp::{UdpDatagram, PROTO_UDP};
use crate::tcp::{Segment, FLAG_ACK, FLAG_FIN, FLAG_PSH, FLAG_RST, FLAG_SYN, PROTO_TCP};
use crate::pcap::PcapWriter;
use crate::topology::{netmask, parse_address, Address, DeviceKind, DhcpPool, Topology, VlanMode};
use serde::{Deserialize, Serialize};
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
    pub device_id: String,
    pub kind: LogKind,
    pub detail: String,
    /// Quadro decodificado, presente nas transmissões.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameInfo>,
    /// Enlace percorrido pela transmissão (origem e destino por equipamento e porta).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<LinkRef>,
}

/// Campos de um quadro por camada, para a inspeção de pacotes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInfo {
    pub protocol: String,
    pub src_mac: String,
    pub dst_mac: String,
    pub ethertype: u16,
    pub bytes: usize,
    pub arp: Option<ArpInfo>,
    pub ipv4: Option<Ipv4Info>,
    pub icmp: Option<IcmpInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArpInfo {
    pub op: String,
    pub sender_mac: String,
    pub sender_ip: String,
    pub target_mac: String,
    pub target_ip: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ipv4Info {
    pub src: String,
    pub dst: String,
    pub ttl: u8,
    pub id: u16,
    pub protocol: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IcmpInfo {
    pub kind: String,
    pub code: u8,
    pub id: u16,
    pub seq: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRef {
    pub from_device: String,
    pub from_port: String,
    pub to_device: String,
    pub to_port: String,
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
    /// Tabelas ARP e MAC ao final da execução (usadas por show arp e show mac address-table).
    pub tables: Vec<DeviceTables>,
}

/// Entrada da tabela ARP de um equipamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArpEntry {
    pub address: String,
    pub mac: String,
    pub port: String,
}

/// Entrada da tabela MAC de um switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacEntry {
    pub vlan: u16,
    pub mac: String,
    pub port: String,
}

/// Tabelas de um equipamento ao final de uma execução.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceTables {
    pub device_id: String,
    pub arp: Vec<ArpEntry>,
    pub mac_table: Vec<MacEntry>,
}

impl Simulation {
    /// Interface conectada à sub-rede do endereço, usada como "porta" da entrada ARP.
    fn port_for_address(&self, dev: usize, ip: Ipv4Addr) -> String {
        let d = &self.devices[dev];
        for (i, f) in d.ifaces.iter().enumerate() {
            if let Some((addr, p)) = f.addr {
                if u32::from(ip) & netmask(p) == u32::from(addr) & netmask(p) {
                    return d.port_ids.get(i).cloned().unwrap_or_default();
                }
            }
        }
        "-".to_string()
    }

    fn tables(&self) -> Vec<DeviceTables> {
        (0..self.devices.len())
            .map(|dev| {
                let d = &self.devices[dev];
                let mut arp: Vec<ArpEntry> = d
                    .arp
                    .iter()
                    .map(|(ip, mac)| ArpEntry {
                        address: ip.to_string(),
                        mac: mac_text(mac),
                        port: self.port_for_address(dev, *ip),
                    })
                    .collect();
                arp.sort_by_key(|e| e.address.parse::<Ipv4Addr>().map(u32::from).unwrap_or(0));
                let mut mac_table: Vec<MacEntry> = d
                    .mac_table
                    .iter()
                    .map(|((vid, mac), port)| MacEntry {
                        vlan: *vid,
                        mac: mac_text(mac),
                        port: d.port_ids.get(*port).cloned().unwrap_or_default(),
                    })
                    .collect();
                mac_table.sort_by(|a, b| a.mac.cmp(&b.mac));
                DeviceTables { device_id: d.id.clone(), arp, mac_table }
            })
            .collect()
    }
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
    tcp_listen: Vec<u16>,
    ports_vlan: Vec<PortVlan>,
    pool: Option<Pool>,
    dns_records: Vec<(String, Ipv4Addr)>,
    id: String,
    port_ids: Vec<String>,
    name: String,
    kind: DeviceKind,
    ifaces: Vec<Iface>,
    arp: HashMap<Ipv4Addr, Mac>,
    /// Pacotes esperando a resolução ARP do próximo salto: próximo salto → (porta, pacote).
    waiting: HashMap<Ipv4Addr, Vec<(usize, Ipv4Packet)>>,
    mac_table: HashMap<(u16, Mac), usize>,
    routes: Vec<Route>,
}

struct EchoState {
    target: Ipv4Addr,
    seq: u16,
    started_at: u64,
    reply_at: Option<u64>,
    error: Option<String>,
    /// Equipamento que respondeu com erro ICMP (para o traceroute).
    hop: Option<Ipv4Addr>,
    hop_at: Option<u64>,
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
    dhcp: Option<DhcpClient>,
    dns: Option<DnsClient>,
    tcp_client: Option<TcpClient>,
    tcp_conns: Vec<TcpConn>,
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
        sim.echo = Some(EchoState { target: target_ip, seq, started_at: sim.now, reply_at: None, error: None, hop: None, hop_at: None });
        if let Err(e) = sim.send_echo_request(src, target_ip, seq, DEFAULT_TTL) {
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

    let tables = sim.tables();
    Ok(PingOutcome {
        target: target_ip.to_string(),
        transmitted,
        replies,
        error,
        log: sim.log,
        log_truncated: sim.log_truncated,
        pcap: sim.pcap.into_bytes(),
        tables,
    })
}

/// Limite de saltos do traceroute.
pub const MAX_TRACE_HOPS: u8 = 16;

/// Um salto do traceroute. `address` é o equipamento que respondeu neste TTL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceHop {
    pub ttl: u8,
    pub address: Option<String>,
    pub rtt_us: Option<u64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceOutcome {
    pub target: String,
    pub hops: Vec<TraceHop>,
    pub reached: bool,
    pub error: Option<String>,
    pub tables: Vec<DeviceTables>,
}

/// Traceroute: um echo por TTL crescente. Cada equipamento intermediário responde com
/// tempo excedido; o destino responde com echo reply.
pub fn run_traceroute(topology: &Topology, source_id: &str, target: &str, max_hops: u8) -> Result<TraceOutcome, String> {
    topology.validate()?;
    if max_hops == 0 || max_hops > MAX_TRACE_HOPS {
        return Err(format!("número de saltos deve ser de 1 a {MAX_TRACE_HOPS}"));
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

    let mut hops = Vec::new();
    let mut reached = false;
    let mut error = None;
    for ttl in 1..=max_hops {
        let seq = ttl as u16;
        let start = sim.now;
        sim.echo = Some(EchoState { target: target_ip, seq, started_at: start, reply_at: None, error: None, hop: None, hop_at: None });
        if let Err(e) = sim.send_echo_request(src, target_ip, seq, ttl) {
            error = Some(e);
            break;
        }
        sim.run_until_done();
        if sim.storm {
            error = Some(format!("limite de {MAX_EVENTS} eventos atingido: possível laço de camada 2"));
            break;
        }
        let st = sim.echo.take().ok_or("estado do traceroute perdido")?;
        if let Some(at) = st.reply_at {
            hops.push(TraceHop { ttl, address: Some(target_ip.to_string()), rtt_us: Some(at - start), note: None });
            reached = true;
            break;
        }
        match (st.hop, st.hop_at, st.error) {
            (Some(h), Some(at), Some(note)) => {
                let stop = note.contains("inalcançável");
                hops.push(TraceHop { ttl, address: Some(h.to_string()), rtt_us: Some(at - start), note: Some(note) });
                if stop {
                    break;
                }
            }
            _ => hops.push(TraceHop { ttl, address: None, rtt_us: None, note: Some("sem resposta".into()) }),
        }
    }
    Ok(TraceOutcome { target: target_ip.to_string(), hops, reached, error, tables: sim.tables() })
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
            let mut dns_records = Vec::new();
            for r in &d.dns_records {
                let ip: Ipv4Addr = r.ip.parse().map_err(|_| "registro DNS inválido")?;
                dns_records.push((dns::normalize_name(&r.name)?, ip));
            }
            let ports_vlan: Vec<PortVlan> = d
                .ports
                .iter()
                .map(|p| match p.vlan_mode {
                    Some(VlanMode::Trunk) => PortVlan::Trunk,
                    _ => PortVlan::Access(p.vlan.unwrap_or(1)),
                })
                .collect();
            let pool = match &d.dhcp_pool {
                Some(p) => Some(build_pool(p, &ifaces)?),
                None => None,
            };
            devices.push(Runtime {
                id: d.id.clone(),
                port_ids: d.ports.iter().map(|p| p.id.clone()).collect(),
                name: d.name.clone(),
                kind: d.kind,
                ifaces,
                arp: HashMap::new(),
                waiting: HashMap::new(),
                mac_table: HashMap::new(),
                routes,
                tcp_listen: d.tcp_listen.clone(),
                ports_vlan,
                pool,
                dns_records,
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
            dhcp: None,
            dns: None,
            tcp_client: None,
            tcp_conns: Vec::new(),
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

    fn link_ref(&self, from: Endpoint, to: Endpoint) -> LinkRef {
        let f = &self.devices[from.dev];
        let t = &self.devices[to.dev];
        LinkRef {
            from_device: f.id.clone(),
            from_port: f.port_ids.get(from.port).cloned().unwrap_or_default(),
            to_device: t.id.clone(),
            to_port: t.port_ids.get(to.port).cloned().unwrap_or_default(),
        }
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
        let device_id = self.devices[device].id.clone();
        self.log.push(LogEntry { time_us, device: name, device_id, kind, detail, frame: None, link: None });
    }

    fn new_ipv4(&mut self, src: Ipv4Addr, dst: Ipv4Addr, protocol: u8, payload: Vec<u8>) -> Ipv4Packet {
        let id = self.ip_id;
        self.ip_id = self.ip_id.wrapping_add(1);
        Ipv4Packet { src, dst, ttl: DEFAULT_TTL, protocol, id, payload }
    }

    fn send_echo_request(&mut self, host: usize, dst: Ipv4Addr, seq: u16, ttl: u8) -> Result<(), String> {
        let (port, _) = self.lookup(host, dst).ok_or("sem rota")?;
        let (src_ip, _) = self.devices[host].ifaces[port].addr.ok_or("interface sem endereço IP")?;
        let icmp = IcmpEcho { kind: TYPE_ECHO_REQUEST, id: ECHO_ID, seq, data: vec![0xab; ECHO_DATA_LEN] }.encode();
        let mut pkt = self.new_ipv4(src_ip, dst, PROTO_ICMP, icmp);
        pkt.ttl = ttl;
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
        let plain = untag(&bytes).1;
        let detail = describe(&plain);
        let frame = frame_info(&plain);
        let link = self.link_ref(from, to);
        let before = self.log.len();
        self.push_log(self.now, from.dev, kind, detail);
        if self.log.len() > before {
            if let Some(last) = self.log.last_mut() {
                last.frame = frame;
                last.link = Some(link);
            }
        }
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

}

impl Simulation {
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
        if pkt.dst == Ipv4Addr::BROADCAST {
            self.udp_broadcast_rx(at, &pkt);
            return;
        }
        if self.is_local(dev, pkt.dst) {
            match pkt.protocol {
                PROTO_ICMP => self.icmp_rx(dev, &pkt),
                PROTO_UDP => self.udp_local_rx(dev, &pkt),
                PROTO_TCP => self.tcp_local_rx(dev, &pkt),
                other => self.push_log(self.now, dev, LogKind::Drop, format!("protocolo IP {other} não suportado")),
            }
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
        let now = self.now;
        if let Some(st) = self.echo.as_mut() {
            if st.reply_at.is_none() && st.error.is_none() && orig_dst == st.target && orig_id == ECHO_ID && orig_seq == st.seq {
                st.error = Some(reason);
                st.hop = Some(from);
                st.hop_at = Some(now);
            }
        }
    }
}

/// Pool DHCP de um servidor já validado contra suas interfaces.
struct Pool {
    prefix: u8,
    server_ip: Ipv4Addr,
    start: u32,
    end: u32,
    gateway: Ipv4Addr,
    leases: HashMap<Mac, Ipv4Addr>,
}

fn build_pool(p: &DhcpPool, ifaces: &[Iface]) -> Result<Pool, String> {
    let net: Ipv4Addr = p.network.parse().map_err(|_| "pool DHCP: rede inválida")?;
    let start: Ipv4Addr = p.start.parse().map_err(|_| "pool DHCP: início inválido")?;
    let end: Ipv4Addr = p.end.parse().map_err(|_| "pool DHCP: fim inválido")?;
    let gateway: Ipv4Addr = p.gateway.parse().map_err(|_| "pool DHCP: gateway inválido")?;
    if u32::from(start) > u32::from(end) {
        return Err("pool DHCP: início maior que o fim".into());
    }
    let mask = netmask(p.prefix_len);
    let server_ip = ifaces
        .iter()
        .filter_map(|f| f.addr)
        .find(|&(a, pr)| pr == p.prefix_len && u32::from(a) & mask == u32::from(net) & mask)
        .map(|(a, _)| a)
        .ok_or("pool DHCP sem interface do servidor na sub-rede")?;
    Ok(Pool {
        prefix: p.prefix_len,
        server_ip,
        start: u32::from(start),
        end: u32::from(end),
        gateway,
        leases: HashMap::new(),
    })
}

/// Endereço para o cliente: a concessão anterior, ou o primeiro livre do pool.
fn allocate(pool: &Pool, mac: Mac) -> Option<Ipv4Addr> {
    if let Some(ip) = pool.leases.get(&mac) {
        return Some(*ip);
    }
    (pool.start..=pool.end)
        .map(Ipv4Addr::from)
        .find(|ip| !pool.leases.values().any(|used| used == ip))
}

struct DhcpClient {
    dev: usize,
    port: usize,
    mac: Mac,
    xid: u32,
    lease: Option<DhcpMessage>,
}

/// Concessão obtida por DHCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DhcpLease {
    pub ip: String,
    pub prefix_len: u8,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DhcpOutcome {
    pub lease: Option<DhcpLease>,
    pub error: Option<String>,
    pub log: Vec<LogEntry>,
    pub log_truncated: bool,
}

/// Executa o DORA (Discover, Offer, Request, Ack) para a interface `port_id` de `client_id`.
/// O resultado é a concessão; o frontend aplica o endereço à interface.
pub fn run_dhcp(topology: &Topology, client_id: &str, port_id: &str) -> Result<DhcpOutcome, String> {
    topology.validate()?;
    let dev = topology
        .devices
        .iter()
        .position(|d| d.id == client_id)
        .ok_or_else(|| "equipamento não existe".to_string())?;
    if !topology.devices[dev].kind.is_host() {
        return Err("somente PCs e servidores obtêm endereço por DHCP".into());
    }
    let port = topology.devices[dev]
        .ports
        .iter()
        .position(|p| p.id == port_id)
        .ok_or_else(|| "interface inexistente".to_string())?;
    let mut sim = Simulation::build(topology)?;
    if !sim.devices[dev].ifaces[port].up {
        return Err("a interface precisa estar ligada e sem shutdown".into());
    }
    let mac = sim.devices[dev].ifaces[port].mac;
    let xid = 0x4e46_0001;
    sim.dhcp = Some(DhcpClient { dev, port, mac, xid, lease: None });
    let discover = DhcpMessage::new(DHCP_REQUEST, MSG_DISCOVER, xid, mac).encode();
    sim.broadcast_udp(dev, port, Ipv4Addr::UNSPECIFIED, CLIENT_PORT, SERVER_PORT, discover);
    sim.run_until_dhcp_done();
    let client = sim.dhcp.take().ok_or("estado DHCP perdido")?;
    let lease = client.lease.map(|ack| DhcpLease {
        ip: ack.yiaddr.to_string(),
        prefix_len: ack.subnet_mask.map(|m| u32::from(m).leading_ones() as u8).unwrap_or(24),
        gateway: ack.router.map(|g| g.to_string()),
    });
    let error = match lease {
        Some(_) => None,
        None => Some("sem resposta do servidor DHCP (verifique o pool e a ligação)".to_string()),
    };
    Ok(DhcpOutcome { lease, error, log: sim.log, log_truncated: sim.log_truncated })
}

impl Simulation {
    fn run_until_dhcp_done(&mut self) {
        loop {
            if self.dhcp.as_ref().is_some_and(|c| c.lease.is_some()) {
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

    /// Envia uma mensagem UDP em broadcast (255.255.255.255) pela interface dada.
    fn broadcast_udp(&mut self, dev: usize, port: usize, src: Ipv4Addr, src_port: u16, dst_port: u16, payload: Vec<u8>) {
        let udp = UdpDatagram { src_port, dst_port, payload }.encode();
        let pkt = self.new_ipv4(src, Ipv4Addr::BROADCAST, PROTO_UDP, udp);
        let Ok(ip_bytes) = pkt.encode() else { return };
        let my_mac = self.devices[dev].ifaces[port].mac;
        let bytes = Frame { dst: BROADCAST, src: my_mac, ethertype: ETHERTYPE_IPV4, payload: ip_bytes }.encode();
        self.transmit(Endpoint { dev, port }, bytes, LogKind::Tx);
    }

    /// Mensagem UDP em broadcast recebida: direciona para o servidor (67) ou o cliente (68).
    fn udp_broadcast_rx(&mut self, at: Endpoint, pkt: &Ipv4Packet) {
        let dev = at.dev;
        if pkt.protocol != PROTO_UDP {
            self.push_log(self.now, dev, LogKind::Drop, "broadcast não suportado".into());
            return;
        }
        let Ok(udp) = UdpDatagram::decode(&pkt.payload) else { return };
        let Ok(msg) = DhcpMessage::decode(&udp.payload) else {
            self.push_log(self.now, dev, LogKind::Drop, "UDP em broadcast sem DHCP".into());
            return;
        };
        if udp.dst_port == SERVER_PORT {
            self.dhcp_server_rx(at, msg);
        } else if udp.dst_port == CLIENT_PORT {
            self.dhcp_client_rx(at, msg);
        }
    }

    fn dhcp_server_rx(&mut self, at: Endpoint, msg: DhcpMessage) {
        let dev = at.dev;
        let Some(pool) = self.devices[dev].pool.as_mut() else { return };
        let server_ip = pool.server_ip;
        let netmask_addr = Ipv4Addr::from(netmask(pool.prefix));
        let gateway = pool.gateway;
        let reply_type = match msg.msg_type {
            MSG_DISCOVER => MSG_OFFER,
            MSG_REQUEST => {
                if msg.server_id != Some(server_ip) {
                    return;
                }
                MSG_ACK
            }
            _ => return,
        };
        let ip = if msg.msg_type == MSG_REQUEST {
            match msg.requested_ip {
                Some(req) if allocate(pool, msg.chaddr) == Some(req) => {
                    pool.leases.insert(msg.chaddr, req);
                    req
                }
                _ => return,
            }
        } else {
            match allocate(pool, msg.chaddr) {
                Some(ip) => ip,
                None => {
                    self.push_log(self.now, dev, LogKind::Drop, "pool DHCP esgotado".into());
                    return;
                }
            }
        };
        let mut reply = DhcpMessage::new(DHCP_REPLY, reply_type, msg.xid, msg.chaddr);
        reply.yiaddr = ip;
        reply.siaddr = server_ip;
        reply.server_id = Some(server_ip);
        reply.subnet_mask = Some(netmask_addr);
        reply.router = Some(gateway);
        reply.lease_time = Some(3600);
        self.broadcast_udp(dev, at.port, server_ip, SERVER_PORT, CLIENT_PORT, reply.encode());
    }

    fn dhcp_client_rx(&mut self, at: Endpoint, msg: DhcpMessage) {
        let Some(client) = self.dhcp.as_ref() else { return };
        if client.dev != at.dev || client.port != at.port || msg.xid != client.xid || msg.chaddr != client.mac || msg.op != DHCP_REPLY {
            return;
        }
        let (dev, port, xid, mac) = (client.dev, client.port, client.xid, client.mac);
        match msg.msg_type {
            MSG_OFFER => {
                let mut req = DhcpMessage::new(DHCP_REQUEST, MSG_REQUEST, xid, mac);
                req.server_id = msg.server_id;
                req.requested_ip = Some(msg.yiaddr);
                self.broadcast_udp(dev, port, Ipv4Addr::UNSPECIFIED, CLIENT_PORT, SERVER_PORT, req.encode());
            }
            MSG_ACK => {
                if let Some(c) = self.dhcp.as_mut() {
                    c.lease = Some(msg);
                }
            }
            _ => {}
        }
    }
}

/// Cliente DNS em andamento: aguarda a resposta da consulta.
struct DnsClient {
    dev: usize,
    src_port: u16,
    id: u16,
    name: String,
    /// `None`: sem resposta; `Some(None)`: NXDOMAIN; `Some(Some(ip))`: endereço.
    answer: Option<Option<Ipv4Addr>>,
}

/// Resultado de uma consulta DNS feita por um equipamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsOutcome {
    pub name: String,
    pub address: Option<String>,
    pub nxdomain: bool,
    pub error: Option<String>,
    pub log: Vec<LogEntry>,
    pub log_truncated: bool,
}

/// Consulta `name` no servidor de nomes configurado em `client_id` (`ip name-server`).
pub fn run_dns(topology: &Topology, client_id: &str, name: &str) -> Result<DnsOutcome, String> {
    topology.validate()?;
    let name = dns::normalize_name(name)?;
    let dev = topology
        .devices
        .iter()
        .position(|d| d.id == client_id)
        .ok_or_else(|| "equipamento não existe".to_string())?;
    if !topology.devices[dev].kind.is_host() {
        return Err("somente PCs e servidores fazem consultas DNS".into());
    }
    let server_text = topology.devices[dev]
        .name_server
        .clone()
        .ok_or_else(|| "nenhum servidor de nomes configurado (ip name-server)".to_string())?;
    let server: Ipv4Addr = server_text.parse().map_err(|_| "servidor de nomes inválido".to_string())?;
    let mut sim = Simulation::build(topology)?;
    let (port, _) = sim.lookup(dev, server).ok_or("sem rota até o servidor de nomes")?;
    let (src_ip, _) = sim.devices[dev].ifaces[port].addr.ok_or("interface sem endereço IP")?;
    let id = 0x1d2e;
    let src_port = 49_153;
    sim.dns = Some(DnsClient { dev, src_port, id, name: name.clone(), answer: None });
    let query = dns::encode_query(id, &name)?;
    sim.send_udp(dev, src_ip, src_port, server, dns::PORT, query)?;
    sim.run_until_dns_done();
    let answer = sim.dns.take().and_then(|c| c.answer);
    let (address, nxdomain, error) = match answer {
        Some(Some(ip)) => (Some(ip.to_string()), false, None),
        Some(None) => (None, true, None),
        None => (None, false, Some("sem resposta do servidor de nomes".to_string())),
    };
    Ok(DnsOutcome { name, address, nxdomain, error, log: sim.log, log_truncated: sim.log_truncated })
}

impl Simulation {
    fn run_until_dns_done(&mut self) {
        loop {
            if self.dns.as_ref().is_some_and(|c| c.answer.is_some()) {
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

    /// Envia uma mensagem UDP unicast pela rota do equipamento.
    fn send_udp(&mut self, dev: usize, src: Ipv4Addr, src_port: u16, dst: Ipv4Addr, dst_port: u16, payload: Vec<u8>) -> Result<(), String> {
        let udp = UdpDatagram { src_port, dst_port, payload }.encode();
        let pkt = self.new_ipv4(src, dst, PROTO_UDP, udp);
        self.send_ip(dev, pkt)
    }

    /// UDP recebido para um endereço local: servidor DNS (53) ou resposta ao cliente DNS.
    fn udp_local_rx(&mut self, dev: usize, pkt: &Ipv4Packet) {
        let Ok(udp) = UdpDatagram::decode(&pkt.payload) else {
            self.push_log(self.now, dev, LogKind::Drop, "datagrama UDP inválido".into());
            return;
        };
        if udp.dst_port == dns::PORT {
            self.dns_server_rx(dev, pkt.dst, pkt.src, udp.src_port, &udp.payload);
            return;
        }
        let is_client_port = self.dns.as_ref().is_some_and(|c| c.dev == dev && c.src_port == udp.dst_port);
        if is_client_port {
            self.dns_client_rx(&udp.payload);
            return;
        }
        self.push_log(self.now, dev, LogKind::Drop, format!("porta UDP {} fechada", udp.dst_port));
    }

    /// Servidor DNS: responde só se o equipamento tem registros configurados.
    fn dns_server_rx(&mut self, dev: usize, local_ip: Ipv4Addr, client_ip: Ipv4Addr, client_port: u16, payload: &[u8]) {
        if self.devices[dev].dns_records.is_empty() {
            self.push_log(self.now, dev, LogKind::Drop, "porta 53 sem registros DNS".into());
            return;
        }
        let Ok(dns::Message::Query { id, name }) = dns::decode(payload) else { return };
        let answer = self.devices[dev].dns_records.iter().find(|(n, _)| *n == name).map(|(_, ip)| *ip);
        let Ok(reply) = dns::encode_response(id, &name, answer) else { return };
        if let Err(e) = self.send_udp(dev, local_ip, dns::PORT, client_ip, client_port, reply) {
            self.push_log(self.now, dev, LogKind::Drop, e);
        }
    }

    fn dns_client_rx(&mut self, payload: &[u8]) {
        let Ok(dns::Message::Response { id, name, answer, rcode }) = dns::decode(payload) else { return };
        if let Some(c) = self.dns.as_mut() {
            if id == c.id && name == c.name && c.answer.is_none() {
                c.answer = Some(if rcode == dns::RCODE_NXDOMAIN { None } else { answer });
            }
        }
    }
}

/// Modo de uma porta de switch: acesso a uma VLAN, ou tronco com quadros marcados (802.1Q).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortVlan {
    Access(u16),
    Trunk,
}

const TPID: [u8; 2] = [0x81, 0x00];

/// Lê a marca 802.1Q. Sem marca, o quadro pertence à VLAN 1 (nativa).
fn untag(bytes: &[u8]) -> (u16, Vec<u8>) {
    if bytes.len() >= 16 && bytes[12..14] == TPID {
        let vid = u16::from_be_bytes([bytes[14], bytes[15]]) & 0x0fff;
        let mut out = bytes[..12].to_vec();
        out.extend_from_slice(&bytes[16..]);
        (vid, out)
    } else {
        (1, bytes.to_vec())
    }
}

/// Insere a marca 802.1Q (prioridade 0) com o identificador da VLAN.
fn tag(untagged: &[u8], vid: u16) -> Vec<u8> {
    let mut out = untagged[..12].to_vec();
    out.extend_from_slice(&TPID);
    out.extend_from_slice(&(vid & 0x0fff).to_be_bytes());
    out.extend_from_slice(&untagged[12..]);
    out
}

impl Simulation {
    /// A porta pertence à VLAN? Tronco carrega todas.
    fn carries(&self, dev: usize, port: usize, vid: u16) -> bool {
        match self.devices[dev].ports_vlan[port] {
            PortVlan::Access(v) => v == vid,
            PortVlan::Trunk => true,
        }
    }

    /// Bytes que saem pela porta: tronco ligado a outro switch envia marcado; o resto, sem marca.
    fn egress(&self, dev: usize, port: usize, vid: u16, untagged: &[u8]) -> Vec<u8> {
        let to_switch = self
            .wires
            .get(&Endpoint { dev, port })
            .is_some_and(|p| self.devices[p.dev].kind == DeviceKind::Switch);
        match self.devices[dev].ports_vlan[port] {
            PortVlan::Trunk if to_switch => tag(untagged, vid),
            _ => untagged.to_vec(),
        }
    }

    fn switch_rx(&mut self, at: Endpoint, bytes: Vec<u8>) {
        let dev = at.dev;
        let (vid, untagged) = match self.devices[dev].ports_vlan[at.port] {
            PortVlan::Access(v) => {
                if bytes.len() >= 14 && bytes[12..14] == TPID {
                    self.push_log(self.now, dev, LogKind::Drop, "quadro marcado em porta de acesso".into());
                    return;
                }
                (v, bytes)
            }
            PortVlan::Trunk => untag(&bytes),
        };
        let frame = match Frame::decode(&untagged) {
            Ok(f) => f,
            Err(e) => {
                self.push_log(self.now, dev, LogKind::Drop, e);
                return;
            }
        };
        if frame.src[0] & 1 == 0 {
            let previous = self.devices[dev].mac_table.insert((vid, frame.src), at.port);
            if previous != Some(at.port) {
                let detail = format!("aprendeu {} na VLAN {vid}, porta {}", mac_text(&frame.src), at.port + 1);
                self.push_log(self.now, dev, LogKind::Learn, detail);
            }
        }
        let known = if frame.dst == BROADCAST || frame.dst[0] & 1 == 1 {
            None
        } else {
            self.devices[dev].mac_table.get(&(vid, frame.dst)).copied()
        };
        match known {
            Some(p) if p == at.port => {
                self.push_log(self.now, dev, LogKind::Drop, "destino na mesma porta de origem".into());
            }
            Some(p) => {
                if self.carries(dev, p, vid) {
                    let out = self.egress(dev, p, vid, &untagged);
                    self.transmit(Endpoint { dev, port: p }, out, LogKind::Forward);
                } else {
                    self.push_log(self.now, dev, LogKind::Drop, format!("destino fora da VLAN {vid}"));
                }
            }
            None => {
                for p in 0..self.devices[dev].ifaces.len() {
                    let out_ep = Endpoint { dev, port: p };
                    if p != at.port && self.wires.contains_key(&out_ep) && self.carries(dev, p, vid) {
                        let out = self.egress(dev, p, vid, &untagged);
                        self.transmit(out_ep, out, LogKind::Flood);
                    }
                }
            }
        }
    }
}

/// Estado do cliente TCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TcpState {
    SynSent,
    DataSent,
    FinWait,
}

/// Cliente TCP em andamento (uma conexão por execução de `tcp-test`).
struct TcpClient {
    dev: usize,
    src_ip: Ipv4Addr,
    src_port: u16,
    dst: Ipv4Addr,
    dst_port: u16,
    isn: u32,
    peer_seq: u32,
    state: TcpState,
    msg: Vec<u8>,
    connected: bool,
    echo: Option<Vec<u8>>,
    done: bool,
    error: Option<String>,
}

/// Conexão aceita por um servidor (uma por par origem/porta).
struct TcpConn {
    dev: usize,
    local_port: u16,
    peer_ip: Ipv4Addr,
    peer_port: u16,
    our_seq: u32,
    expected: u32,
}

const TCP_SERVER_ISN: u32 = 5_000;
const TCP_CLIENT_ISN: u32 = 1_000;
const TCP_CLIENT_PORT: u16 = 40_000;

/// Resultado de uma conexão TCP de teste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpOutcome {
    pub connected: bool,
    pub echoed: Option<String>,
    pub closed: bool,
    pub error: Option<String>,
    pub log: Vec<LogEntry>,
    pub log_truncated: bool,
}

/// Abre uma conexão TCP até `target:port`, envia `message`, lê o eco e fecha.
pub fn run_tcp(topology: &Topology, client_id: &str, target: &str, port: u16, message: &str) -> Result<TcpOutcome, String> {
    topology.validate()?;
    if message.len() > 512 {
        return Err("mensagem TCP maior que 512 bytes".into());
    }
    let dst: Ipv4Addr = target.trim().parse().map_err(|_| format!("endereço inválido: {target}"))?;
    let dev = topology
        .devices
        .iter()
        .position(|d| d.id == client_id)
        .ok_or_else(|| "equipamento não existe".to_string())?;
    if !topology.devices[dev].kind.is_host() {
        return Err("somente PCs e servidores abrem conexões TCP".into());
    }
    let mut sim = Simulation::build(topology)?;
    let (egress, _) = sim.lookup(dev, dst).ok_or("sem rota até o destino")?;
    let (src_ip, _) = sim.devices[dev].ifaces[egress].addr.ok_or("interface sem endereço IP")?;
    sim.tcp_client = Some(TcpClient {
        dev,
        src_ip,
        src_port: TCP_CLIENT_PORT,
        dst,
        dst_port: port,
        isn: TCP_CLIENT_ISN,
        peer_seq: 0,
        state: TcpState::SynSent,
        msg: message.as_bytes().to_vec(),
        connected: false,
        echo: None,
        done: false,
        error: None,
    });
    let syn = Segment { src_port: TCP_CLIENT_PORT, dst_port: port, seq: TCP_CLIENT_ISN, ack: 0, flags: FLAG_SYN, payload: Vec::new() };
    sim.send_tcp(dev, src_ip, &syn, dst)?;
    sim.run_until_tcp_done();
    let c = sim.tcp_client.take().ok_or("estado TCP perdido")?;
    let error = c.error.or_else(|| if c.done { None } else { Some("sem resposta do servidor TCP".to_string()) });
    Ok(TcpOutcome {
        connected: c.connected,
        echoed: c.echo.map(|b| String::from_utf8_lossy(&b).to_string()),
        closed: c.done,
        error,
        log: sim.log,
        log_truncated: sim.log_truncated,
    })
}

impl Simulation {
    fn run_until_tcp_done(&mut self) {
        loop {
            if self.tcp_client.as_ref().is_some_and(|c| c.done || c.error.is_some()) {
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

    /// Envia um segmento TCP pela rota do equipamento.
    fn send_tcp(&mut self, dev: usize, src: Ipv4Addr, seg: &Segment, dst: Ipv4Addr) -> Result<(), String> {
        let bytes = seg.encode(src, dst);
        let pkt = self.new_ipv4(src, dst, PROTO_TCP, bytes);
        self.send_ip(dev, pkt)
    }

    /// Segmento TCP recebido em endereço local: cliente em andamento ou servidor.
    fn tcp_local_rx(&mut self, dev: usize, pkt: &Ipv4Packet) {
        let seg = match Segment::decode(&pkt.payload, pkt.src, pkt.dst) {
            Ok(s) => s,
            Err(e) => {
                self.push_log(self.now, dev, LogKind::Drop, e);
                return;
            }
        };
        let is_client = self.tcp_client.as_ref().is_some_and(|c| c.dev == dev && c.src_port == seg.dst_port);
        if is_client {
            self.tcp_client_rx(pkt.src, &seg);
        } else {
            self.tcp_server_rx(dev, pkt.dst, pkt.src, seg);
        }
    }

    fn tcp_client_rx(&mut self, from: Ipv4Addr, seg: &Segment) {
        let Some(c) = self.tcp_client.as_mut() else { return };
        if from != c.dst || seg.src_port != c.dst_port {
            return;
        }
        if seg.has(FLAG_RST) {
            c.error = Some(format!("conexão recusada pelo destino na porta {}", c.dst_port));
            return;
        }
        let (dev, src_ip, src_port, dst, dst_port) = (c.dev, c.src_ip, c.src_port, c.dst, c.dst_port);
        match c.state {
            TcpState::SynSent => {
                if seg.has(FLAG_SYN) && seg.has(FLAG_ACK) && seg.ack == c.isn + 1 {
                    c.connected = true;
                    c.peer_seq = seg.seq.wrapping_add(1);
                    c.state = TcpState::DataSent;
                    let msg = c.msg.clone();
                    let data = Segment { src_port, dst_port, seq: c.isn + 1, ack: c.peer_seq, flags: FLAG_ACK | FLAG_PSH, payload: msg };
                    let _ = self.send_tcp(dev, src_ip, &data, dst);
                }
            }
            TcpState::DataSent => {
                if seg.payload.is_empty() {
                    return;
                }
                c.echo = Some(seg.payload.clone());
                c.peer_seq = seg.seq.wrapping_add(seg.payload.len() as u32);
                c.state = TcpState::FinWait;
                let seq = c.isn + 1 + c.msg.len() as u32;
                let peer_seq = c.peer_seq;
                let ack = Segment { src_port, dst_port, seq, ack: peer_seq, flags: FLAG_ACK, payload: Vec::new() };
                let fin = Segment { src_port, dst_port, seq, ack: peer_seq, flags: FLAG_FIN | FLAG_ACK, payload: Vec::new() };
                let _ = self.send_tcp(dev, src_ip, &ack, dst);
                let _ = self.send_tcp(dev, src_ip, &fin, dst);
            }
            TcpState::FinWait => {
                if seg.has(FLAG_FIN) {
                    c.done = true;
                    let ack = Segment {
                        src_port,
                        dst_port,
                        seq: c.isn + 2 + c.msg.len() as u32,
                        ack: seg.seq.wrapping_add(1),
                        flags: FLAG_ACK,
                        payload: Vec::new(),
                    };
                    let _ = self.send_tcp(dev, src_ip, &ack, dst);
                }
            }
        }
    }

    /// Servidor: aceita conexões nas portas de escuta, faz eco dos dados e fecha quando o par fecha.
    fn tcp_server_rx(&mut self, dev: usize, local_ip: Ipv4Addr, peer_ip: Ipv4Addr, seg: Segment) {
        if !self.devices[dev].tcp_listen.contains(&seg.dst_port) {
            if !seg.has(FLAG_RST) {
                let rst = Segment {
                    src_port: seg.dst_port,
                    dst_port: seg.src_port,
                    seq: 0,
                    ack: seg.seq.wrapping_add(seg.seq_len()),
                    flags: FLAG_RST | FLAG_ACK,
                    payload: Vec::new(),
                };
                let _ = self.send_tcp(dev, local_ip, &rst, peer_ip);
            }
            self.push_log(self.now, dev, LogKind::Drop, format!("porta TCP {} fechada", seg.dst_port));
            return;
        }
        let idx = self.tcp_conns.iter().position(|c| c.dev == dev && c.local_port == seg.dst_port && c.peer_ip == peer_ip && c.peer_port == seg.src_port);
        let Some(idx) = idx else {
            if seg.has(FLAG_SYN) && !seg.has(FLAG_ACK) {
                let expected = seg.seq.wrapping_add(1);
                self.tcp_conns.push(TcpConn {
                    dev,
                    local_port: seg.dst_port,
                    peer_ip,
                    peer_port: seg.src_port,
                    our_seq: TCP_SERVER_ISN + 1,
                    expected,
                });
                let synack = Segment { src_port: seg.dst_port, dst_port: seg.src_port, seq: TCP_SERVER_ISN, ack: expected, flags: FLAG_SYN | FLAG_ACK, payload: Vec::new() };
                let _ = self.send_tcp(dev, local_ip, &synack, peer_ip);
            }
            return;
        };
        if seg.has(FLAG_FIN) {
            let expected = seg.seq.wrapping_add(seg.payload.len() as u32).wrapping_add(1);
            let our_seq = self.tcp_conns[idx].our_seq;
            let finack = Segment { src_port: seg.dst_port, dst_port: seg.src_port, seq: our_seq, ack: expected, flags: FLAG_FIN | FLAG_ACK, payload: Vec::new() };
            let _ = self.send_tcp(dev, local_ip, &finack, peer_ip);
            self.tcp_conns.remove(idx);
            return;
        }
        if !seg.payload.is_empty() && seg.seq == self.tcp_conns[idx].expected {
            let len = seg.payload.len() as u32;
            let conn = &mut self.tcp_conns[idx];
            conn.expected = conn.expected.wrapping_add(len);
            let echo = Segment {
                src_port: seg.dst_port,
                dst_port: seg.src_port,
                seq: conn.our_seq,
                ack: conn.expected,
                flags: FLAG_ACK | FLAG_PSH,
                payload: seg.payload.clone(),
            };
            conn.our_seq = conn.our_seq.wrapping_add(len);
            let _ = self.send_tcp(dev, local_ip, &echo, peer_ip);
        }
    }
}

/// Decodifica um quadro por camada. Retorna `None` se nem o Ethernet é válido.
pub fn frame_info(bytes: &[u8]) -> Option<FrameInfo> {
    let frame = Frame::decode(bytes).ok()?;
    let mut info = FrameInfo {
        protocol: "Ethernet".into(),
        src_mac: mac_text(&frame.src),
        dst_mac: mac_text(&frame.dst),
        ethertype: frame.ethertype,
        bytes: bytes.len(),
        arp: None,
        ipv4: None,
        icmp: None,
    };
    match frame.ethertype {
        ETHERTYPE_ARP => {
            if let Ok(a) = ArpPacket::decode(&frame.payload) {
                info.protocol = "ARP".into();
                info.arp = Some(ArpInfo {
                    op: if a.op == OP_REQUEST { "request" } else { "reply" }.into(),
                    sender_mac: mac_text(&a.sender_mac),
                    sender_ip: a.sender_ip.to_string(),
                    target_mac: mac_text(&a.target_mac),
                    target_ip: a.target_ip.to_string(),
                });
            }
        }
        ETHERTYPE_IPV4 => {
            if let Ok(p) = Ipv4Packet::decode(&frame.payload) {
                info.protocol = "IPv4".into();
                info.ipv4 = Some(Ipv4Info { src: p.src.to_string(), dst: p.dst.to_string(), ttl: p.ttl, id: p.id, protocol: p.protocol });
                if p.protocol == PROTO_UDP {
                    info.protocol = "UDP".into();
                    if let Ok(u) = UdpDatagram::decode(&p.payload) {
                        if u.dst_port == SERVER_PORT || u.dst_port == CLIENT_PORT {
                            info.protocol = "DHCP".into();
                        }
                    }
                }
                if p.protocol == PROTO_TCP {
                    info.protocol = "TCP".into();
                }
                if p.protocol == PROTO_ICMP {
                    if let Ok(e) = IcmpEcho::decode(&p.payload) {
                        info.protocol = "ICMP".into();
                        let kind = if e.kind == TYPE_ECHO_REQUEST { "echo request" } else { "echo reply" };
                        info.icmp = Some(IcmpInfo { kind: kind.into(), code: 0, id: e.id, seq: e.seq });
                    } else if let Ok(e) = IcmpError::decode(&p.payload) {
                        info.protocol = "ICMP".into();
                        let kind = if e.kind == TYPE_DEST_UNREACHABLE { "destino inalcançável" } else { "tempo excedido" };
                        info.icmp = Some(IcmpInfo { kind: kind.into(), code: e.code, id: 0, seq: 0 });
                    }
                }
            }
        }
        _ => {}
    }
    Some(info)
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
            dhcp_pool: None,
            name_server: None,
            dns_records: Vec::new(),
            tcp_listen: Vec::new(),
        }
    }

    fn port(id: &str, ip: Option<&str>, prefix: Option<u8>) -> Port {
        Port { id: id.into(), name: format!("Porta {id}"), ip: ip.map(String::from), prefix_len: prefix, shutdown: false, vlan_mode: None, vlan: None }
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
            dhcp_pool: None,
            name_server: None,
            dns_records: Vec::new(),
            tcp_listen: Vec::new(),
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

    #[test]
    fn traceroute_lists_the_router_then_the_target() {
        let out = run_traceroute(&routed_pair(), "a", "10.0.2.10", 8).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert!(out.reached);
        assert_eq!(out.hops.len(), 2);
        assert_eq!(out.hops[0].address.as_deref(), Some("10.0.1.1"));
        assert_eq!(out.hops[1].address.as_deref(), Some("10.0.2.10"));
        assert!(out.hops[0].rtt_us.is_some() && out.hops[1].rtt_us.is_some());
    }

    #[test]
    fn traceroute_stops_at_unreachable_network() {
        let out = run_traceroute(&routed_pair(), "a", "172.16.0.1", 8).unwrap();
        assert!(!out.reached);
        let last = out.hops.last().unwrap();
        assert_eq!(last.address.as_deref(), Some("10.0.1.1"));
        assert!(last.note.as_deref().unwrap().contains("inalcançável"));
    }

    #[test]
    fn traceroute_rejects_invalid_hop_limit() {
        assert!(run_traceroute(&routed_pair(), "a", "10.0.2.10", 0).is_err());
        assert!(run_traceroute(&routed_pair(), "a", "10.0.2.10", 17).is_err());
    }

    #[test]
    fn ping_returns_arp_and_mac_tables() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 1).unwrap();
        let a = out.tables.iter().find(|t| t.device_id == "a").unwrap();
        assert_eq!(a.arp.len(), 1);
        assert_eq!(a.arp[0].address, "10.0.0.2");
        assert_eq!(a.arp[0].port, "p1");
        let sw = out.tables.iter().find(|t| t.device_id == "sw").unwrap();
        assert_eq!(sw.mac_table.len(), 2);
        let ports: Vec<&str> = sw.mac_table.iter().map(|m| m.port.as_str()).collect();
        assert!(ports.contains(&"p1") && ports.contains(&"p2"));
    }

    #[test]
    fn transmissions_carry_decoded_frames_and_links() {
        let out = run_ping(&two_pcs_and_switch(), "a", "10.0.0.2", 1).unwrap();
        let first = out.log.iter().find(|e| e.kind == LogKind::Tx).unwrap();
        let frame = first.frame.as_ref().expect("tx sem quadro");
        assert_eq!(frame.protocol, "ARP");
        assert_eq!(frame.arp.as_ref().unwrap().op, "request");
        assert_eq!(first.device_id, "a");
        let link = first.link.as_ref().expect("tx sem enlace");
        assert_eq!((link.from_device.as_str(), link.from_port.as_str()), ("a", "p1"));
        assert_eq!((link.to_device.as_str(), link.to_port.as_str()), ("sw", "p1"));
        let icmp = out.log.iter().filter_map(|e| e.frame.as_ref()).find(|f| f.protocol == "ICMP").unwrap();
        assert_eq!(icmp.icmp.as_ref().unwrap().kind, "echo request");
        assert_eq!(icmp.ipv4.as_ref().unwrap().ttl, DEFAULT_TTL);
    }

    #[test]
    fn frame_info_decodes_arp_and_rejects_garbage() {
        let req = ArpPacket { op: OP_REQUEST, sender_mac: [2, 0, 0, 0, 0, 1], sender_ip: Ipv4Addr::new(10, 0, 0, 1), target_mac: [0; 6], target_ip: Ipv4Addr::new(10, 0, 0, 2) };
        let bytes = Frame { dst: BROADCAST, src: [2, 0, 0, 0, 0, 1], ethertype: ETHERTYPE_ARP, payload: req.encode() }.encode();
        let info = frame_info(&bytes).unwrap();
        assert_eq!(info.protocol, "ARP");
        assert_eq!(info.arp.unwrap().target_ip, "10.0.0.2");
        assert!(frame_info(&[0u8; 5]).is_none());
    }

    /// Cliente sem endereço ligado ao switch; servidor com pool 10.0.5.100–110 e gateway 10.0.5.1.
    fn dhcp_topology(with_pool: bool) -> Topology {
        let mut server = host("s", "10.0.5.1", 24, None);
        if with_pool {
            server.dhcp_pool = Some(crate::topology::DhcpPool {
                network: "10.0.5.0".into(),
                prefix_len: 24,
                start: "10.0.5.100".into(),
                end: "10.0.5.110".into(),
                gateway: "10.0.5.1".into(),
            });
        }
        let mut client = sample_pc("c", None);
        client.ports[0].ip = None;
        client.ports[0].prefix_len = None;
        topo(
            vec![client, server, switch("sw")],
            vec![link("l1", ("c", "p1"), ("sw", "p1")), link("l2", ("s", "p1"), ("sw", "p2"))],
        )
    }

    #[test]
    fn dhcp_leases_the_first_free_address_through_a_switch() {
        let out = run_dhcp(&dhcp_topology(true), "c", "p1").unwrap();
        let lease = out.lease.expect("deve haver concessão");
        assert_eq!(lease.ip, "10.0.5.100");
        assert_eq!(lease.prefix_len, 24);
        assert_eq!(lease.gateway.as_deref(), Some("10.0.5.1"));
        assert!(out.error.is_none());
    }

    #[test]
    fn dhcp_without_server_reports_no_answer() {
        let out = run_dhcp(&dhcp_topology(false), "c", "p1").unwrap();
        assert!(out.lease.is_none());
        assert!(out.error.unwrap().contains("sem resposta do servidor DHCP"));
    }

    #[test]
    fn address_from_dhcp_allows_a_ping_to_the_server() {
        let mut t = dhcp_topology(true);
        let lease = run_dhcp(&t, "c", "p1").unwrap().lease.unwrap();
        t.devices[0].ports[0].ip = Some(lease.ip);
        t.devices[0].ports[0].prefix_len = Some(lease.prefix_len);
        let out = run_ping(&t, "c", "10.0.5.1", 1).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.replies.len(), 1);
    }

    #[test]
    fn dhcp_is_only_for_hosts() {
        let err = run_dhcp(&dhcp_topology(true), "sw", "p1").unwrap_err();
        assert!(err.contains("somente PCs e servidores"), "{err}");
    }

    #[test]
    fn exhausted_pool_allocates_nothing() {
        let pool = Pool {
            prefix: 24,
            server_ip: Ipv4Addr::new(10, 0, 5, 1),
            start: u32::from(Ipv4Addr::new(10, 0, 5, 100)),
            end: u32::from(Ipv4Addr::new(10, 0, 5, 100)),
            gateway: Ipv4Addr::new(10, 0, 5, 1),
            leases: HashMap::from([([2, 0, 0, 0, 0, 9], Ipv4Addr::new(10, 0, 5, 100))]),
        };
        assert_eq!(allocate(&pool, [2, 0, 0, 0, 0, 1]), None);
        assert_eq!(allocate(&pool, [2, 0, 0, 0, 0, 9]), Some(Ipv4Addr::new(10, 0, 5, 100)));
    }

    /// Cliente com servidor de nomes 10.0.7.20; servidor com o registro web.lab -> 10.0.7.80.
    fn dns_topology(with_records: bool) -> Topology {
        let mut client = host("c", "10.0.7.10", 24, None);
        client.name_server = Some("10.0.7.20".into());
        let mut server = host("dns", "10.0.7.20", 24, None);
        if with_records {
            server.dns_records = vec![crate::topology::DnsRecord { name: "web.lab".into(), ip: "10.0.7.80".into() }];
        }
        topo(
            vec![client, server, switch("sw")],
            vec![link("l1", ("c", "p1"), ("sw", "p1")), link("l2", ("dns", "p1"), ("sw", "p2"))],
        )
    }

    #[test]
    fn dns_resolves_a_configured_record_through_a_switch() {
        let out = run_dns(&dns_topology(true), "c", "WEB.lab").unwrap();
        assert_eq!(out.address.as_deref(), Some("10.0.7.80"));
        assert!(!out.nxdomain);
        assert!(out.error.is_none());
    }

    #[test]
    fn dns_unknown_name_is_nxdomain() {
        let out = run_dns(&dns_topology(true), "c", "nada.lab").unwrap();
        assert!(out.nxdomain);
        assert!(out.address.is_none());
    }

    #[test]
    fn dns_server_without_records_does_not_answer() {
        let out = run_dns(&dns_topology(false), "c", "web.lab").unwrap();
        assert!(out.address.is_none());
        assert!(out.error.unwrap().contains("sem resposta do servidor de nomes"));
    }

    #[test]
    fn dns_needs_a_name_server() {
        let mut t = dns_topology(true);
        t.devices[0].name_server = None;
        let err = run_dns(&t, "c", "web.lab").unwrap_err();
        assert!(err.contains("nenhum servidor de nomes"), "{err}");
    }

    /// Switch com quatro portas: p1 e p2 na VLAN 10; p3 na VLAN 20; hosts na mesma sub-rede 10.0.9.0/24.
    fn vlan_switch_topology() -> Topology {
        let mut sw = switch("sw");
        sw.ports[0].vlan_mode = Some(crate::topology::VlanMode::Access);
        sw.ports[0].vlan = Some(10);
        sw.ports[1].vlan_mode = Some(crate::topology::VlanMode::Access);
        sw.ports[1].vlan = Some(10);
        sw.ports[2].vlan_mode = Some(crate::topology::VlanMode::Access);
        sw.ports[2].vlan = Some(20);
        topo(
            vec![host("a", "10.0.9.1", 24, None), host("b", "10.0.9.2", 24, None), host("c", "10.0.9.3", 24, None), sw],
            vec![
                link("l1", ("a", "p1"), ("sw", "p1")),
                link("l2", ("b", "p1"), ("sw", "p2")),
                link("l3", ("c", "p1"), ("sw", "p3")),
            ],
        )
    }

    #[test]
    fn hosts_in_the_same_vlan_reach_each_other() {
        let out = run_ping(&vlan_switch_topology(), "a", "10.0.9.2", 1).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.replies.len(), 1);
    }

    #[test]
    fn hosts_in_other_vlans_are_isolated() {
        let out = run_ping(&vlan_switch_topology(), "a", "10.0.9.3", 1).unwrap();
        assert!(out.replies.is_empty());
        assert!(out.error.is_some());
    }

    #[test]
    fn trunk_carries_vlan_tags_between_switches() {
        let mut sw1 = switch("s1");
        sw1.ports[0].vlan_mode = Some(crate::topology::VlanMode::Access);
        sw1.ports[0].vlan = Some(10);
        sw1.ports[7].vlan_mode = Some(crate::topology::VlanMode::Trunk);
        let mut sw2 = switch("s2");
        sw2.ports[0].vlan_mode = Some(crate::topology::VlanMode::Access);
        sw2.ports[0].vlan = Some(10);
        sw2.ports[7].vlan_mode = Some(crate::topology::VlanMode::Trunk);
        let t = topo(
            vec![host("a", "10.0.9.1", 24, None), host("b", "10.0.9.2", 24, None), sw1, sw2],
            vec![
                link("l1", ("a", "p1"), ("s1", "p1")),
                link("l2", ("b", "p1"), ("s2", "p1")),
                link("lt", ("s1", "p8"), ("s2", "p8")),
            ],
        );
        let out = run_ping(&t, "a", "10.0.9.2", 1).unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.replies.len(), 1);
    }

    #[test]
    fn mac_table_is_kept_per_vlan() {
        let out = run_ping(&vlan_switch_topology(), "a", "10.0.9.2", 1).unwrap();
        let sw = out.tables.iter().find(|t| t.device_id == "sw").unwrap();
        assert!(sw.mac_table.iter().all(|m| m.vlan == 10));
    }

    fn tcp_topology(listen: bool) -> Topology {
        let client = host("c", "10.0.6.1", 24, None);
        let mut server = host("s", "10.0.6.2", 24, None);
        if listen {
            server.tcp_listen = vec![7];
        }
        topo(
            vec![client, server, switch("sw")],
            vec![link("l1", ("c", "p1"), ("sw", "p1")), link("l2", ("s", "p1"), ("sw", "p2"))],
        )
    }

    #[test]
    fn tcp_handshake_echo_and_close_through_a_switch() {
        let out = run_tcp(&tcp_topology(true), "c", "10.0.6.2", 7, "hello").unwrap();
        assert!(out.error.is_none(), "{:?}", out.error);
        assert!(out.connected);
        assert_eq!(out.echoed.as_deref(), Some("hello"));
        assert!(out.closed);
    }

    #[test]
    fn tcp_closed_port_is_refused_with_rst() {
        let out = run_tcp(&tcp_topology(false), "c", "10.0.6.2", 7, "hello").unwrap();
        assert!(!out.connected);
        assert!(out.error.unwrap().contains("recusada"));
    }

    #[test]
    fn tcp_segments_appear_in_the_log_with_flags_in_labels() {
        let out = run_tcp(&tcp_topology(true), "c", "10.0.6.2", 7, "x").unwrap();
        assert!(out.log.iter().any(|e| e.frame.as_ref().is_some_and(|f| f.protocol == "TCP")));
    }
}
