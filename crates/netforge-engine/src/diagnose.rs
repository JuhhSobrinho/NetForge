//! Diagnóstico determinístico de uma execução de ping (ADR-010). Cada conclusão vem de um
//! evento real do log, indicado em `evidence`. Nada é inferido sem evidência, e a sugestão
//! aponta um comando da CLI do próprio NetForge.

use crate::sim::{LogEntry, LogKind};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: Severity,
    pub title: String,
    /// Índices no log da execução que sustentam a conclusão.
    pub evidence: Vec<usize>,
    pub suggestion: String,
}

/// Analisa o resultado de um ping. `error` é a falha final, se houver.
pub fn diagnose(target: &str, transmitted: u8, replies: usize, error: Option<&str>, log: &[LogEntry]) -> Vec<Finding> {
    if error.is_none() && replies == transmitted as usize {
        return vec![Finding {
            severity: Severity::Info,
            title: format!("Conectividade confirmada com {target}"),
            evidence: Vec::new(),
            suggestion: String::new(),
        }];
    }
    let mut out = Vec::new();

    // Erro ICMP recebido de um equipamento intermediário.
    if let Some(e) = error {
        if let Some(from) = after(e, "informado por ") {
            let idx = last_index(log, |l| l.kind == LogKind::Rx && l.detail.contains("ICMP"));
            if e.contains("inalcançável") {
                out.push(Finding {
                    severity: Severity::Error,
                    title: format!("{from} informou que o destino {target} é inalcançável"),
                    evidence: idx.into_iter().collect(),
                    suggestion: format!("No equipamento {from}: show ip route; ip route <rede> <máscara> <próximo salto>"),
                });
            } else if e.contains("tempo excedido") {
                out.push(Finding {
                    severity: Severity::Warning,
                    title: format!("Pacote descartado por TTL em {from}"),
                    evidence: idx.into_iter().collect(),
                    suggestion: "Verifique se há laço de roteamento entre os roteadores do caminho".into(),
                });
            }
        }
        if e.contains("possível laço") {
            out.push(Finding {
                severity: Severity::Error,
                title: "Tempestade de broadcast: laço de camada 2".into(),
                evidence: Vec::new(),
                suggestion: "Remova o laço entre switches; o STP ainda não está implementado".into(),
            });
        }
    }

    // Quadros descartados, com o equipamento onde ocorreu.
    for (i, l) in log.iter().enumerate() {
        if l.kind != LogKind::Drop {
            continue;
        }
        let d = &l.detail;
        if d.contains("porta sem link") {
            out.push(finding(Severity::Error, format!("Interface sem cabo em {}", l.device), i, "Ligue a interface a outro equipamento"));
        } else if d.contains("interface desligada") {
            out.push(finding(Severity::Error, format!("Interface desligada (shutdown) em {}", l.device), i, "Na interface: no shutdown"));
        } else if d.contains("destino fora da VLAN") {
            out.push(finding(
                Severity::Error,
                format!("Quadro bloqueado pela VLAN no switch {}", l.device),
                i,
                "Coloque as portas de origem e destino na mesma VLAN (vlan access <id>)",
            ));
        } else if d.contains("quadro marcado em porta de acesso") {
            out.push(finding(
                Severity::Warning,
                format!("Quadro marcado chegou a porta de acesso em {}", l.device),
                i,
                "Configure a porta do outro lado como tronco, ou ajuste a VLAN",
            ));
        } else if let Some(dst) = after(d, "sem rota para ") {
            out.push(finding(
                Severity::Error,
                format!("{} não tem rota para {dst}", l.device),
                i,
                "Adicione uma rota estática (ip route) ou o gateway padrão (ip default-gateway)",
            ));
        } else if d.contains("TTL expirado") {
            out.push(finding(Severity::Warning, format!("TTL expirado em {}", l.device), i, "Verifique laços de roteamento"));
        }
    }

    // Ninguém respondeu ao ARP do destino e não há outra causa explicada.
    if out.is_empty() && replies == 0 {
        let asked = last_index(log, |l| l.kind == LogKind::Tx && l.detail.contains(&format!("ARP quem tem {target} ")));
        let answered = log.iter().any(|l| l.detail.contains(&format!("ARP {target} está em")));
        if let (Some(i), false) = (asked, answered) {
            out.push(finding(
                Severity::Error,
                format!("Ninguém respondeu ao ARP de {target}"),
                i,
                "Confirme que o endereço existe na mesma sub-rede, que o equipamento está ligado e na mesma VLAN",
            ));
        }
    }

    out
}

fn finding(severity: Severity, title: String, index: usize, suggestion: &str) -> Finding {
    Finding { severity, title, evidence: vec![index], suggestion: suggestion.to_string() }
}

fn after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find(|c: char| c == ' ' || c == ')' || c == ',').unwrap_or(rest.len());
    Some(&rest[..end]).filter(|s| !s.is_empty())
}

fn last_index(log: &[LogEntry], pred: impl Fn(&LogEntry) -> bool) -> Option<usize> {
    log.iter().rposition(pred)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: LogKind, device: &str, detail: &str) -> LogEntry {
        LogEntry {
            time_us: 0,
            device: device.into(),
            device_id: device.into(),
            kind,
            detail: detail.into(),
            frame: None,
            link: None,
        }
    }

    #[test]
    fn success_is_reported_as_confirmed() {
        let f = diagnose("10.0.0.2", 4, 4, None, &[]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Info);
    }

    #[test]
    fn unanswered_arp_points_to_the_request() {
        let log = vec![
            entry(LogKind::Tx, "PC 1", "02:4e:46:00:00:00 > ff:ff:ff:ff:ff:ff: ARP quem tem 10.0.0.9 pergunta por 10.0.0.1"),
            entry(LogKind::Flood, "Switch 1", "02:4e:46:00:00:00 > ff:ff:ff:ff:ff:ff: ARP quem tem 10.0.0.9 pergunta por 10.0.0.1"),
        ];
        let f = diagnose("10.0.0.9", 1, 0, Some("sem resposta para icmp_seq=1"), &log);
        assert_eq!(f[0].title, "Ninguém respondeu ao ARP de 10.0.0.9");
        assert_eq!(f[0].evidence, vec![0]);
    }

    #[test]
    fn vlan_block_is_explained_with_the_switch_name() {
        let log = vec![entry(LogKind::Drop, "Switch 1", "destino fora da VLAN 20")];
        let f = diagnose("10.0.9.3", 1, 0, Some("sem resposta para icmp_seq=1"), &log);
        assert!(f.iter().any(|x| x.title.contains("VLAN") && x.title.contains("Switch 1")));
    }

    #[test]
    fn icmp_unreachable_names_the_reporting_router() {
        let log = vec![entry(LogKind::Rx, "PC 1", "IPv4 10.0.1.1 > 10.0.1.10 ICMP destino inalcançável")];
        let err = "destino inalcançável (código 0) informado por 10.0.1.1";
        let f = diagnose("172.16.0.1", 1, 0, Some(err), &log);
        assert!(f[0].title.contains("10.0.1.1"));
        assert_eq!(f[0].severity, Severity::Error);
    }

    #[test]
    fn missing_route_and_shutdown_are_explained() {
        let log = vec![
            entry(LogKind::Drop, "Roteador 1", "sem rota para 192.168.9.9"),
            entry(LogKind::Drop, "Switch 1", "interface desligada; quadro descartado"),
        ];
        let f = diagnose("192.168.9.9", 1, 0, Some("sem resposta para icmp_seq=1"), &log);
        assert!(f.iter().any(|x| x.title.contains("não tem rota para 192.168.9.9")));
        assert!(f.iter().any(|x| x.title.contains("desligada")));
    }

    #[test]
    fn loop_is_reported() {
        let f = diagnose("10.0.0.2", 1, 0, Some("limite de 20000 eventos atingido: possível laço de camada 2 (STP não implementado)"), &[]);
        assert!(f.iter().any(|x| x.title.contains("laço")));
    }
}
