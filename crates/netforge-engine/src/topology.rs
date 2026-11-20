//! Modelo de topologia no motor (ADR-014, migrado na Fase 3). Espelha o JSON de
//! apps/desktop/src/topology/model.ts e aplica as mesmas regras de validação.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::Ipv4Addr;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_NAME_LENGTH: usize = 32;
pub const MAX_DEVICES: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Pc,
    Switch,
    Router,
    Server,
}

impl DeviceKind {
    pub fn port_count(self) -> usize {
        match self {
            DeviceKind::Pc | DeviceKind::Server => 1,
            DeviceKind::Switch => 8,
            DeviceKind::Router => 4,
        }
    }

    /// Hosts (PC e servidor) têm uma interface e não encaminham pacotes.
    pub fn is_host(self) -> bool {
        matches!(self, DeviceKind::Pc | DeviceKind::Server)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Port {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix_len: Option<u8>,
    /// Interface administrativamente desligada (`shutdown` na CLI).
    #[serde(default, skip_serializing_if = "is_false")]
    pub shutdown: bool,
    /// Modo VLAN de uma porta de switch (`vlan access` ou `vlan trunk`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan_mode: Option<VlanMode>,
    /// VLAN de acesso (1–4094).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VlanMode {
    Access,
    Trunk,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Registro DNS de um servidor (`ip dns-record`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsRecord {
    pub name: String,
    pub ip: String,
}

/// Pool de um servidor DHCP: concede endereços de `start` a `end` na rede `network`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DhcpPool {
    pub network: String,
    pub prefix_len: u8,
    pub start: String,
    pub end: String,
    pub gateway: String,
}

/// Rota estática de um roteador: `ip route <rede> <prefixo> <próximo salto>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticRoute {
    pub network: String,
    pub prefix_len: u8,
    pub next_hop: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub kind: DeviceKind,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub ports: Vec<Port>,
    /// Gateway padrão de um host (`ip default-gateway`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    /// Rotas estáticas de um roteador.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<StaticRoute>,
    /// Pool DHCP de um servidor (`ip dhcp-pool`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcp_pool: Option<DhcpPool>,
    /// Servidor de nomes de um host (`ip name-server`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_server: Option<String>,
    /// Registros DNS servidos pelo equipamento (`ip dns-record`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns_records: Vec<DnsRecord>,
    /// Portas TCP em escuta (`ip tcp-listen`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tcp_listen: Vec<u16>,
    /// RIPv2 ativo no roteador (`router rip`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub rip: bool,
    /// OSPFv2 ativo no roteador (`router ospf`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub ospf: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortRef {
    pub device_id: String,
    pub port_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub id: String,
    pub from: PortRef,
    pub to: PortRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Topology {
    pub schema_version: u32,
    pub devices: Vec<Device>,
    pub links: Vec<Link>,
}

/// Endereço de uma interface já convertido: (IPv4, comprimento do prefixo).
pub type Address = (Ipv4Addr, u8);

impl Topology {
    /// Valida a topologia com as mesmas regras de `parseTopology` no frontend,
    /// mais as regras de endereçamento e de link desta fase.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!("versão de projeto não suportada: {}", self.schema_version));
        }
        if self.devices.len() > MAX_DEVICES {
            return Err("projeto excede o limite de equipamentos".into());
        }
        let mut device_ids = HashSet::new();
        for d in &self.devices {
            if !device_ids.insert(d.id.as_str()) {
                return Err(format!("equipamento com id duplicado: {}", d.id));
            }
            let name = d.name.trim();
            if name.is_empty() || name.chars().count() > MAX_NAME_LENGTH {
                return Err(format!("nome inválido no equipamento {}", d.id));
            }
            let mut port_ids = HashSet::new();
            for p in &d.ports {
                if !port_ids.insert(p.id.as_str()) {
                    return Err(format!("porta duplicada em {}: {}", d.name, p.id));
                }
                parse_address(p)?;
            }
            if let Some(gw) = &d.gateway {
                if d.kind != DeviceKind::Pc && d.kind != DeviceKind::Server {
                    return Err(format!("gateway padrão só existe em PCs e servidores: {}", d.name));
                }
                gw.parse::<Ipv4Addr>().map_err(|_| format!("gateway inválido: {gw}"))?;
            }
            for p in &d.ports {
                if (p.vlan_mode.is_some() || p.vlan.is_some()) && d.kind != DeviceKind::Switch {
                    return Err(format!("configuração de VLAN só existe em switches: {}", d.name));
                }
                if let Some(v) = p.vlan {
                    if !(1..=4094).contains(&v) {
                        return Err(format!("VLAN fora de 1–4094: {v}"));
                    }
                }
                if p.vlan_mode == Some(VlanMode::Trunk) && p.vlan.is_some() {
                    return Err("porta tronco não tem VLAN de acesso".into());
                }
            }
            if let Some(p) = &d.dhcp_pool {
                if d.kind == DeviceKind::Switch {
                    return Err(format!("pool DHCP não existe em switches: {}", d.name));
                }
                if p.prefix_len > 32 {
                    return Err(format!("prefixo do pool fora de 0–32: {}", p.prefix_len));
                }
                for a in [&p.network, &p.start, &p.end, &p.gateway] {
                    a.parse::<Ipv4Addr>().map_err(|_| format!("endereço inválido no pool DHCP: {a}"))?;
                }
            }
            if !d.tcp_listen.is_empty() && d.kind == DeviceKind::Switch {
                return Err(format!("portas TCP não existem em switches: {}", d.name));
            }
            if d.ospf && d.kind != DeviceKind::Router {
                return Err(format!("OSPF só existe em roteadores: {}", d.name));
            }
            if d.rip && d.kind != DeviceKind::Router {
                return Err(format!("RIP só existe em roteadores: {}", d.name));
            }
            if d.tcp_listen.contains(&0) {
                return Err("porta TCP 0 não é válida".into());
            }
            if let Some(ns) = &d.name_server {
                if !d.kind.is_host() {
                    return Err(format!("servidor de nomes só é configurado em PCs e servidores: {}", d.name));
                }
                ns.parse::<Ipv4Addr>().map_err(|_| format!("servidor de nomes inválido: {ns}"))?;
            }
            if !d.dns_records.is_empty() && d.kind == DeviceKind::Switch {
                return Err(format!("registros DNS não existem em switches: {}", d.name));
            }
            for r in &d.dns_records {
                crate::dns::normalize_name(&r.name)?;
                r.ip.parse::<Ipv4Addr>().map_err(|_| format!("IP inválido em registro DNS: {}", r.ip))?;
            }
            if !d.routes.is_empty() && d.kind != DeviceKind::Router {
                return Err(format!("rotas estáticas só existem em roteadores: {}", d.name));
            }
            for r in &d.routes {
                if r.prefix_len > 32 {
                    return Err(format!("prefixo de rota fora de 0–32: {}", r.prefix_len));
                }
                r.network
                    .parse::<Ipv4Addr>()
                    .map_err(|_| format!("rede de rota inválida: {}", r.network))?;
                r.next_hop
                    .parse::<Ipv4Addr>()
                    .map_err(|_| format!("próximo salto inválido: {}", r.next_hop))?;
            }
        }
        let mut link_ids = HashSet::new();
        let mut used = HashSet::new();
        for l in &self.links {
            if !link_ids.insert(l.id.as_str()) {
                return Err(format!("link com id duplicado: {}", l.id));
            }
            if l.from.device_id == l.to.device_id {
                return Err("link conecta um equipamento a ele mesmo".into());
            }
            for r in [&l.from, &l.to] {
                let device = self
                    .devices
                    .iter()
                    .find(|d| d.id == r.device_id)
                    .ok_or_else(|| format!("link aponta para equipamento inexistente: {}", r.device_id))?;
                if !device.ports.iter().any(|p| p.id == r.port_id) {
                    return Err(format!("link aponta para porta inexistente: {}", r.port_id));
                }
                if !used.insert((r.device_id.as_str(), r.port_id.as_str())) {
                    return Err("uma porta aparece em mais de um link".into());
                }
            }
        }
        Ok(())
    }
}

/// Converte o endereço textual de uma porta. Retorna `None` se a porta não tem endereço.
pub fn parse_address(p: &Port) -> Result<Option<Address>, String> {
    match (&p.ip, p.prefix_len) {
        (None, None) => Ok(None),
        (Some(ip), Some(prefix)) => {
            if prefix > 32 {
                return Err(format!("prefixo fora de 0–32 na porta {}", p.id));
            }
            let addr: Ipv4Addr = ip
                .parse()
                .map_err(|_| format!("endereço IPv4 inválido: {ip}"))?;
            Ok(Some((addr, prefix)))
        }
        _ => Err(format!("endereço e máscara devem vir juntos na porta {}", p.id)),
    }
}

/// Máscara de rede em formato numérico para um prefixo de 0 a 32.
pub fn netmask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix as u32)
    }
}

#[cfg(test)]
pub(crate) fn sample_pc(id: &str, ip: Option<&str>) -> Device {
    Device {
        id: id.into(),
        kind: DeviceKind::Pc,
        name: format!("PC {id}"),
        x: 0.0,
        y: 0.0,
        ports: vec![Port {
            id: "p1".into(),
            name: "Porta 1".into(),
            ip: ip.map(String::from),
            prefix_len: ip.map(|_| 24),
            shutdown: false, vlan_mode: None, vlan: None,
        }],
        gateway: None,
        routes: Vec::new(),
        dhcp_pool: None,
        name_server: None,
        dns_records: Vec::new(),
            tcp_listen: Vec::new(),
            rip: false,
            ospf: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Topology {
        Topology {
            schema_version: SCHEMA_VERSION,
            devices: vec![
                sample_pc("a", Some("10.0.0.1")),
                sample_pc("b", Some("10.0.0.2")),
                Device {
                    id: "sw".into(),
                    kind: DeviceKind::Switch,
                    name: "Switch 1".into(),
                    x: 0.0,
                    y: 0.0,
                    ports: (1..=8).map(|i| Port { id: format!("p{i}"), name: format!("Porta {i}"), ip: None, prefix_len: None, shutdown: false, vlan_mode: None, vlan: None }).collect(),
                    gateway: None,
                    routes: Vec::new(),
                    dhcp_pool: None,
                    name_server: None,
                    dns_records: Vec::new(),
            tcp_listen: Vec::new(),
            rip: false,
            ospf: false,
                },
            ],
            links: vec![
                Link {
                    id: "l1".into(),
                    from: PortRef { device_id: "a".into(), port_id: "p1".into() },
                    to: PortRef { device_id: "sw".into(), port_id: "p1".into() },
                },
                Link {
                    id: "l2".into(),
                    from: PortRef { device_id: "b".into(), port_id: "p1".into() },
                    to: PortRef { device_id: "sw".into(), port_id: "p2".into() },
                },
            ],
        }
    }

    #[test]
    fn valid_topology_passes() {
        assert!(base().validate().is_ok());
    }

    #[test]
    fn rejects_bad_schema_version() {
        let mut t = base();
        t.schema_version = 9;
        assert!(t.validate().is_err());
    }

    #[test]
    fn rejects_port_used_twice() {
        let mut t = base();
        t.links[1].from = PortRef { device_id: "a".into(), port_id: "p1".into() };
        assert!(t.validate().is_err());
    }

    #[test]
    fn rejects_self_link_and_missing_ports() {
        let mut t = base();
        t.links[0].to = PortRef { device_id: "a".into(), port_id: "p1".into() };
        assert!(t.validate().is_err());
        let mut t = base();
        t.links[0].to.port_id = "p9".into();
        assert!(t.validate().is_err());
    }

    #[test]
    fn rejects_invalid_address_and_prefix() {
        let mut t = base();
        t.devices[0].ports[0].ip = Some("300.1.1.1".into());
        assert!(t.validate().is_err());
        let mut t = base();
        t.devices[0].ports[0].prefix_len = Some(33);
        assert!(t.validate().is_err());
        let mut t = base();
        t.devices[0].ports[0].prefix_len = None;
        assert!(t.validate().is_err());
    }

    #[test]
    fn netmask_values() {
        assert_eq!(netmask(0), 0);
        assert_eq!(netmask(24), 0xffff_ff00);
        assert_eq!(netmask(32), u32::MAX);
    }

    #[test]
    fn deserializes_typescript_json_shape() {
        let json = r#"{"schemaVersion":1,"devices":[{"id":"a","kind":"pc","name":"PC 1","x":10,"y":20,"ports":[{"id":"p1","name":"Porta 1","ip":"192.168.1.10","prefixLen":24}]}],"links":[]}"#;
        let t: Topology = serde_json::from_str(json).unwrap();
        assert!(t.validate().is_ok());
    }
}
