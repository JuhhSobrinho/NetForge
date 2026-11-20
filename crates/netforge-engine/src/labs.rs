//! Laboratórios prontos (Fase 7). Cada laboratório traz uma topologia inicial sem endereços,
//! um enunciado com os endereços a usar e objetivos verificáveis com ping. A verificação usa o
//! motor, não respostas fixas. Ver docs/FEATURES.md.

use crate::sim::run_ping;
use crate::topology::{Device, DeviceKind, Link, Port, PortRef, Topology, SCHEMA_VERSION};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Objective {
    pub description: String,
    pub source_id: String,
    pub target: String,
    /// Verdadeiro: o ping deve ter resposta. Falso: o ping deve falhar (isolamento).
    pub expect_success: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabSpec {
    pub id: String,
    pub title: String,
    pub statement: String,
    pub topology: Topology,
    pub objectives: Vec<Objective>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabSummary {
    pub id: String,
    pub title: String,
    pub objectives: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveResult {
    pub description: String,
    pub passed: bool,
    pub detail: String,
}

fn port(id: &str) -> Port {
    Port { id: id.into(), name: format!("Porta {}", &id[1..]), ip: None, prefix_len: None, shutdown: false, vlan_mode: None, vlan: None }
}

fn device(id: &str, kind: DeviceKind, name: &str, ports: usize) -> Device {
    Device {
        id: id.into(),
        kind,
        name: name.into(),
        x: 0.0,
        y: 0.0,
        ports: (1..=ports).map(|i| port(&format!("p{i}"))).collect(),
        gateway: None,
        routes: Vec::new(),
        dhcp_pool: None,
        name_server: None,
        dns_records: Vec::new(),
        tcp_listen: Vec::new(),
        rip: false,
    }
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

fn objective(description: &str, source: &str, target: &str, expect_success: bool) -> Objective {
    Objective { description: description.into(), source_id: source.into(), target: target.into(), expect_success }
}

/// Todos os laboratórios prontos, na ordem de dificuldade.
pub fn all_labs() -> Vec<LabSpec> {
    let mut labs = vec![
        LabSpec {
            id: "primeira-comunicacao".into(),
            title: "1 · Primeira comunicação".into(),
            statement: "Dois PCs ligados a um switch. Configure PC 1 com 10.0.0.1/24 e PC 2 com 10.0.0.2/24 (interface p1 de cada um). Objetivo: PC 1 alcança PC 2.".into(),
            topology: topo(
                vec![device("pc1", DeviceKind::Pc, "PC 1", 1), device("pc2", DeviceKind::Pc, "PC 2", 1), device("sw", DeviceKind::Switch, "Switch 1", 8)],
                vec![link("l1", ("pc1", "p1"), ("sw", "p1")), link("l2", ("pc2", "p1"), ("sw", "p2"))],
            ),
            objectives: vec![objective("PC 1 alcança 10.0.0.2", "pc1", "10.0.0.2", true)],
        },
        LabSpec {
            id: "duas-redes".into(),
            title: "2 · Duas redes com roteador".into(),
            statement: "PC 1 em 10.0.1.0/24 e PC 2 em 10.0.2.0/24, ligados a um roteador. Use 10.0.1.10 (PC 1, gateway 10.0.1.1), 10.0.2.10 (PC 2, gateway 10.0.2.1), e 10.0.1.1 e 10.0.2.1 nas interfaces p1 e p2 do roteador. Objetivo: PC 1 alcança PC 2 sem rotas estáticas.".into(),
            topology: topo(
                vec![device("pc1", DeviceKind::Pc, "PC 1", 1), device("pc2", DeviceKind::Pc, "PC 2", 1), device("r1", DeviceKind::Router, "Roteador 1", 4)],
                vec![link("l1", ("pc1", "p1"), ("r1", "p1")), link("l2", ("pc2", "p1"), ("r1", "p2"))],
            ),
            objectives: vec![
                objective("PC 1 alcança o roteador", "pc1", "10.0.1.1", true),
                objective("PC 1 alcança PC 2", "pc1", "10.0.2.10", true),
            ],
        },
        LabSpec {
            id: "isolamento-vlan".into(),
            title: "3 · Isolamento por VLAN".into(),
            statement: "Três PCs num switch. Use 10.0.9.1, 10.0.9.2 e 10.0.9.3 (/24) em PC 1, PC 2 e PC 3. Coloque as portas de PC 1 e PC 2 na VLAN 10 e a de PC 3 na VLAN 20 (vlan access). Objetivo: PC 1 alcança PC 2, e PC 1 não alcança PC 3.".into(),
            topology: topo(
                vec![
                    device("pc1", DeviceKind::Pc, "PC 1", 1),
                    device("pc2", DeviceKind::Pc, "PC 2", 1),
                    device("pc3", DeviceKind::Pc, "PC 3", 1),
                    device("sw", DeviceKind::Switch, "Switch 1", 8),
                ],
                vec![
                    link("l1", ("pc1", "p1"), ("sw", "p1")),
                    link("l2", ("pc2", "p1"), ("sw", "p2")),
                    link("l3", ("pc3", "p1"), ("sw", "p3")),
                ],
            ),
            objectives: vec![
                objective("PC 1 alcança PC 2 (mesma VLAN)", "pc1", "10.0.9.2", true),
                objective("PC 1 não alcança PC 3 (outra VLAN)", "pc1", "10.0.9.3", false),
            ],
        },
    ];
    for lab in labs.iter_mut() {
        for (i, d) in lab.topology.devices.iter_mut().enumerate() {
            d.x = 80.0 + (i as f64) * 230.0;
            d.y = 120.0;
        }
    }
    labs
}

pub fn list_labs() -> Vec<LabSummary> {
    all_labs()
        .into_iter()
        .map(|l| LabSummary { id: l.id, title: l.title, objectives: l.objectives.len() })
        .collect()
}

pub fn get_lab(id: &str) -> Option<LabSpec> {
    all_labs().into_iter().find(|l| l.id == id)
}

/// Verifica os objetivos de um laboratório contra a topologia atual do aluno.
pub fn check_lab(topology: &Topology, objectives: &[Objective]) -> Vec<ObjectiveResult> {
    objectives
        .iter()
        .map(|o| match run_ping(topology, &o.source_id, &o.target, 1) {
            Ok(out) => {
                let ok = !out.replies.is_empty();
                let passed = ok == o.expect_success;
                let detail = match (ok, &out.error) {
                    (true, _) => format!("resposta de {}", o.target),
                    (false, Some(e)) => e.clone(),
                    (false, None) => "sem resposta".into(),
                };
                ObjectiveResult { description: o.description.clone(), passed, detail }
            }
            Err(e) => ObjectiveResult { description: o.description.clone(), passed: !o.expect_success, detail: e },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lab_has_a_unique_id_and_objectives() {
        let labs = all_labs();
        let mut ids: Vec<&str> = labs.iter().map(|l| l.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), labs.len());
        assert!(labs.iter().all(|l| !l.objectives.is_empty()));
    }

    #[test]
    fn every_lab_topology_is_valid() {
        for l in all_labs() {
            assert!(l.topology.validate().is_ok(), "{}", l.id);
        }
    }

    #[test]
    fn unconfigured_lab_fails_its_objectives() {
        let lab = get_lab("primeira-comunicacao").unwrap();
        let results = check_lab(&lab.topology, &lab.objectives);
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed);
    }

    #[test]
    fn solved_first_lab_passes() {
        let mut lab = get_lab("primeira-comunicacao").unwrap();
        lab.topology.devices[0].ports[0].ip = Some("10.0.0.1".into());
        lab.topology.devices[0].ports[0].prefix_len = Some(24);
        lab.topology.devices[1].ports[0].ip = Some("10.0.0.2".into());
        lab.topology.devices[1].ports[0].prefix_len = Some(24);
        let results = check_lab(&lab.topology, &lab.objectives);
        assert!(results.iter().all(|r| r.passed), "{:?}", results);
    }

    #[test]
    fn solved_vlan_lab_passes_both_objectives() {
        let mut lab = get_lab("isolamento-vlan").unwrap();
        let ips = ["10.0.9.1", "10.0.9.2", "10.0.9.3"];
        for (i, ip) in ips.iter().enumerate() {
            lab.topology.devices[i].ports[0].ip = Some(ip.to_string());
            lab.topology.devices[i].ports[0].prefix_len = Some(24);
        }
        let sw = 3;
        lab.topology.devices[sw].ports[0].vlan_mode = Some(crate::topology::VlanMode::Access);
        lab.topology.devices[sw].ports[0].vlan = Some(10);
        lab.topology.devices[sw].ports[1].vlan_mode = Some(crate::topology::VlanMode::Access);
        lab.topology.devices[sw].ports[1].vlan = Some(10);
        lab.topology.devices[sw].ports[2].vlan_mode = Some(crate::topology::VlanMode::Access);
        lab.topology.devices[sw].ports[2].vlan = Some(20);
        let results = check_lab(&lab.topology, &lab.objectives);
        assert!(results.iter().all(|r| r.passed), "{:?}", results);
    }
}
