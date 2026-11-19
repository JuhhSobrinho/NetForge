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
        }],
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
                    ports: (1..=8).map(|i| Port { id: format!("p{i}"), name: format!("Porta {i}"), ip: None, prefix_len: None }).collect(),
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
