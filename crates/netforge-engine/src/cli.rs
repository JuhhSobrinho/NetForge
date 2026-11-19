//! CLI por equipamento (Fase 4). Parser próprio: a saída vem do estado real da topologia,
//! não de respostas fixas. A sintaxe é do NetForge (inspirada em CLIs de roteadores), não
//! uma cópia da saída de nenhum equipamento real. Ver docs/FEATURES.md, seção 5.
//!
//! O motor não guarda sessão: o frontend envia a topologia atual, o modo e a linha digitada,
//! e recebe a saída, o novo modo e, se houve configuração, a topologia alterada.

use crate::sim::run_ping;
use crate::topology::{netmask, Device, DeviceKind, StaticRoute, Topology};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

const PING_COUNT: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    User,
    Privileged,
    Config,
    Interface,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliSession {
    pub mode: Mode,
    #[serde(default)]
    pub interface: Option<String>,
}

impl Default for CliSession {
    fn default() -> Self {
        CliSession { mode: Mode::User, interface: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliOutcome {
    pub output: Vec<String>,
    pub session: CliSession,
    pub prompt: String,
    /// Presente somente quando o comando alterou a configuração.
    pub topology: Option<Topology>,
    /// Verdadeiro quando a linha foi rejeitada (saída começa com "%").
    pub error: bool,
}

/// Executa uma linha da CLI no equipamento `device_id`.
pub fn execute(t: &Topology, device_id: &str, session: CliSession, line: &str) -> Result<CliOutcome, String> {
    let index = t
        .devices
        .iter()
        .position(|d| d.id == device_id)
        .ok_or_else(|| "equipamento não existe".to_string())?;
    let dev = &t.devices[index];
    let mut ctx = Ctx { topology: t, index, session, output: Vec::new(), changed: None, error: false };
    let line = line.trim();
    if !line.is_empty() {
        if let Err(e) = ctx.dispatch(line) {
            ctx.output.push(format!("% {e}"));
            ctx.error = true;
        }
    }
    let prompt = prompt_for(dev, &ctx.session);
    Ok(CliOutcome {
        output: ctx.output,
        session: ctx.session,
        prompt,
        topology: ctx.changed,
        error: ctx.error,
    })
}

fn hostname(d: &Device) -> String {
    d.name.split_whitespace().collect::<Vec<_>>().join("-")
}

pub fn prompt_for(d: &Device, s: &CliSession) -> String {
    let h = hostname(d);
    match s.mode {
        Mode::User => format!("{h}>"),
        Mode::Privileged => format!("{h}#"),
        Mode::Config => format!("{h}(config)#"),
        Mode::Interface => format!("{h}(config-if-{})#", s.interface.as_deref().unwrap_or("?")),
    }
}

struct Ctx<'a> {
    topology: &'a Topology,
    index: usize,
    session: CliSession,
    output: Vec<String>,
    changed: Option<Topology>,
    error: bool,
}

impl Ctx<'_> {
    fn device(&self) -> &Device {
        &self.topology.devices[self.index]
    }

    /// Topologia de trabalho: a já alterada nesta linha, ou a original.
    fn working(&self) -> Topology {
        self.changed.clone().unwrap_or_else(|| self.topology.clone())
    }

    fn save(&mut self, t: Topology) -> Result<(), String> {
        t.validate()?;
        self.changed = Some(t);
        Ok(())
    }

    fn dispatch(&mut self, line: &str) -> Result<(), String> {
        let words: Vec<String> = line.split_whitespace().map(|w| w.to_lowercase()).collect();
        let args: Vec<&str> = line.split_whitespace().collect();
        let w: Vec<&str> = words.iter().map(|s| s.as_str()).collect();

        // Comandos de navegação valem em qualquer modo.
        match w.as_slice() {
            ["exit"] => return self.exit(),
            ["end"] => {
                if matches!(self.session.mode, Mode::Config | Mode::Interface) {
                    self.session = CliSession { mode: Mode::Privileged, interface: None };
                }
                return Ok(());
            }
            ["show", "ip", "interface", "brief"] => return self.show_interfaces(),
            ["show", "ip", "route"] => return self.show_routes(),
            ["ping", ..] => return self.ping(&args),
            _ => {}
        }

        match self.session.mode {
            Mode::User => match w.as_slice() {
                ["enable"] => {
                    self.session = CliSession { mode: Mode::Privileged, interface: None };
                    Ok(())
                }
                _ => Err(unknown(line)),
            },
            Mode::Privileged => match w.as_slice() {
                ["disable"] => {
                    self.session = CliSession { mode: Mode::User, interface: None };
                    Ok(())
                }
                ["configure", "terminal"] | ["conf", "t"] => {
                    self.session = CliSession { mode: Mode::Config, interface: None };
                    Ok(())
                }
                _ => Err(unknown(line)),
            },
            Mode::Config => self.config_command(&w, &args, line),
            Mode::Interface => self.interface_command(&w, &args, line),
        }
    }

    fn exit(&mut self) -> Result<(), String> {
        self.session = match self.session.mode {
            Mode::Interface => CliSession { mode: Mode::Config, interface: None },
            Mode::Config => CliSession { mode: Mode::Privileged, interface: None },
            Mode::Privileged => CliSession { mode: Mode::User, interface: None },
            Mode::User => CliSession::default(),
        };
        Ok(())
    }

    fn config_command(&mut self, w: &[&str], args: &[&str], line: &str) -> Result<(), String> {
        match w {
            ["interface", id] => {
                let port = self.find_port(id)?;
                self.session = CliSession { mode: Mode::Interface, interface: Some(port) };
                Ok(())
            }
            ["ip", "route", ..] => self.add_route(args, line),
            ["no", "ip", "route", ..] => self.remove_route(args, line),
            ["ip", "default-gateway", ip] => self.set_gateway(Some(*ip)),
            ["no", "ip", "default-gateway"] => self.set_gateway(None),
            _ => Err(unknown(line)),
        }
    }

    fn interface_command(&mut self, w: &[&str], args: &[&str], line: &str) -> Result<(), String> {
        let port_id = self.session.interface.clone().ok_or("nenhuma interface selecionada")?;
        match w {
            ["ip", "address", ip, mask] => {
                let addr: Ipv4Addr = ip.parse().map_err(|_| format!("endereço inválido: {}", args[2]))?;
                let prefix = parse_mask(*mask)?;
                self.edit_port(&port_id, |p| {
                    p.ip = Some(addr.to_string());
                    p.prefix_len = Some(prefix);
                })
            }
            ["no", "ip", "address"] => self.edit_port(&port_id, |p| {
                p.ip = None;
                p.prefix_len = None;
            }),
            ["shutdown"] => self.edit_port(&port_id, |p| p.shutdown = true),
            ["no", "shutdown"] => self.edit_port(&port_id, |p| p.shutdown = false),
            _ => Err(unknown(line)),
        }
    }

    /// Localiza a porta pelo id (`p1`) ou pelo nome da interface (`porta 1`, sem espaço: `porta1`).
    fn find_port(&self, id: &str) -> Result<String, String> {
        let dev = self.device();
        if dev.kind == DeviceKind::Switch {
            return Err("switches não têm configuração de interface nesta versão".into());
        }
        let wanted = id.to_lowercase();
        let found = dev.ports.iter().find(|p| {
            p.id.to_lowercase() == wanted || p.name.to_lowercase().replace(' ', "") == wanted
        });
        found
            .map(|p| p.id.clone())
            .ok_or_else(|| format!("interface inexistente: {id}"))
    }

    fn edit_port(&mut self, port_id: &str, f: impl FnOnce(&mut crate::topology::Port)) -> Result<(), String> {
        let mut t = self.working();
        let d = &mut t.devices[self.index];
        let p = d
            .ports
            .iter_mut()
            .find(|p| p.id == port_id)
            .ok_or("interface inexistente")?;
        f(p);
        self.save(t)
    }

    fn set_gateway(&mut self, ip: Option<&str>) -> Result<(), String> {
        if !self.device().kind.is_host() {
            return Err("ip default-gateway só existe em PCs e servidores".into());
        }
        let mut t = self.working();
        let value = match ip {
            Some(s) => {
                s.parse::<Ipv4Addr>().map_err(|_| format!("gateway inválido: {s}"))?;
                Some(s.to_string())
            }
            None => None,
        };
        t.devices[self.index].gateway = value;
        self.save(t)
    }

    fn add_route(&mut self, args: &[&str], line: &str) -> Result<(), String> {
        if self.device().kind != DeviceKind::Router {
            return Err("rotas estáticas só existem em roteadores".into());
        }
        if args.len() != 5 {
            return Err(unknown(line));
        }
        let network: Ipv4Addr = args[2].parse().map_err(|_| format!("rede inválida: {}", args[2]))?;
        let prefix = parse_mask(args[3])?;
        let next_hop: Ipv4Addr = args[4].parse().map_err(|_| format!("próximo salto inválido: {}", args[4]))?;
        let mut t = self.working();
        let route = StaticRoute {
            network: (u32::from(network) & netmask(prefix)).to_string_ip(),
            prefix_len: prefix,
            next_hop: next_hop.to_string(),
        };
        let routes = &mut t.devices[self.index].routes;
        if !routes.contains(&route) {
            routes.push(route);
        }
        self.save(t)
    }

    fn remove_route(&mut self, args: &[&str], line: &str) -> Result<(), String> {
        if self.device().kind != DeviceKind::Router {
            return Err("rotas estáticas só existem em roteadores".into());
        }
        // no ip route <rede> <máscara> [próximo salto]
        if args.len() < 5 || args.len() > 6 {
            return Err(unknown(line));
        }
        let network: Ipv4Addr = args[3].parse().map_err(|_| format!("rede inválida: {}", args[3]))?;
        let prefix = parse_mask(args[4])?;
        let target = (u32::from(network) & netmask(prefix)).to_string_ip();
        let mut t = self.working();
        t.devices[self.index]
            .routes
            .retain(|r| !(r.network == target && r.prefix_len == prefix));
        self.save(t)
    }

    fn show_interfaces(&mut self) -> Result<(), String> {
        let dev = self.device().clone();
        self.output.push(format!("{:<12} {:<17} {}", "Interface", "IP-Address", "Status"));
        for p in &dev.ports {
            let ip = match (&p.ip, p.prefix_len) {
                (Some(ip), Some(_)) => ip.clone(),
                _ => "unassigned".into(),
            };
            let linked = self.topology.links.iter().any(|l| {
                (l.from.device_id == dev.id && l.from.port_id == p.id) || (l.to.device_id == dev.id && l.to.port_id == p.id)
            });
            let status = if p.shutdown {
                "administratively down"
            } else if linked {
                "up"
            } else {
                "down"
            };
            self.output.push(format!("{:<12} {:<17} {}", p.id, ip, status));
        }
        Ok(())
    }

    fn show_routes(&mut self) -> Result<(), String> {
        let dev = self.device().clone();
        let has_gateway = dev.gateway.is_some();
        let has_static = !dev.routes.is_empty();
        self.output.push("Códigos: C - conectada, S - estática, S* - gateway padrão".into());
        if !has_gateway && !has_static && !dev.ports.iter().any(|p| p.ip.is_some()) {
            self.output.push("(tabela vazia)".into());
            return Ok(());
        }
        for p in &dev.ports {
            let linked = self.topology.links.iter().any(|l| {
                (l.from.device_id == dev.id && l.from.port_id == p.id) || (l.to.device_id == dev.id && l.to.port_id == p.id)
            });
            if let (Some(ip), Some(prefix)) = (&p.ip, p.prefix_len) {
                if linked && !p.shutdown {
                    if let Ok(addr) = ip.parse::<Ipv4Addr>() {
                        let net = Ipv4Addr::from(u32::from(addr) & netmask(prefix));
                        self.output.push(format!("C    {net}/{prefix} is directly connected, {}", p.id));
                    }
                }
            }
        }
        if let Some(gw) = &dev.gateway {
            self.output.push(format!("S*   0.0.0.0/0 via {gw}"));
        }
        for r in &dev.routes {
            self.output.push(format!("S    {}/{} via {}", r.network, r.prefix_len, r.next_hop));
        }
        Ok(())
    }

    fn ping(&mut self, args: &[&str]) -> Result<(), String> {
        if args.len() != 2 {
            return Err(unknown(&args.join(" ")));
        }
        let target = args[1];
        let outcome = run_ping(self.topology, &self.device().id, target, PING_COUNT)?;
        for r in &outcome.replies {
            self.output.push(format!(
                "Resposta de {}: icmp_seq={} tempo={:.2} ms",
                outcome.target,
                r.seq,
                r.rtt_us as f64 / 1000.0
            ));
        }
        if let Some(e) = &outcome.error {
            self.output.push(format!("% {e}"));
            self.error = true;
        }
        self.output.push(format!(
            "Taxa de sucesso: {} de {} respostas",
            outcome.replies.len(),
            outcome.transmitted
        ));
        Ok(())
    }
}

fn unknown(line: &str) -> String {
    format!("comando desconhecido ou incompleto: {line}")
}

/// Aceita máscara decimal pontuada (255.255.255.0) ou comprimento de prefixo (24).
pub fn parse_mask(s: &str) -> Result<u8, String> {
    if s.contains('.') {
        let bits: u32 = s
            .parse::<Ipv4Addr>()
            .map(u32::from)
            .map_err(|_| format!("máscara inválida: {s}"))?;
        let prefix = bits.leading_ones();
        if prefix < 32 && (bits << prefix) != 0 {
            return Err(format!("máscara não contígua: {s}"));
        }
        return Ok(prefix as u8);
    }
    let prefix: u8 = s.parse().map_err(|_| format!("máscara inválida: {s}"))?;
    if prefix > 32 {
        return Err(format!("prefixo fora de 0–32: {s}"));
    }
    Ok(prefix)
}

trait IpText {
    fn to_string_ip(self) -> String;
}

impl IpText for u32 {
    fn to_string_ip(self) -> String {
        Ipv4Addr::from(self).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{sample_pc, Device, Link, Port, PortRef, SCHEMA_VERSION};

    fn router(id: &str) -> Device {
        Device {
            id: id.into(),
            kind: DeviceKind::Router,
            name: format!("Roteador {id}"),
            x: 0.0,
            y: 0.0,
            ports: (1..=4).map(|i| Port { id: format!("p{i}"), name: format!("Porta {i}"), ip: None, prefix_len: None, shutdown: false }).collect(),
            gateway: None,
            routes: Vec::new(),
        }
    }

    fn topo() -> Topology {
        let mut a = sample_pc("a", None);
        a.ports[0].ip = None;
        let r = router("r");
        Topology {
            schema_version: SCHEMA_VERSION,
            devices: vec![a, r],
            links: vec![Link {
                id: "l1".into(),
                from: PortRef { device_id: "a".into(), port_id: "p1".into() },
                to: PortRef { device_id: "r".into(), port_id: "p1".into() },
            }],
        }
    }

    fn run(t: &Topology, dev: &str, s: CliSession, line: &str) -> CliOutcome {
        execute(t, dev, s, line).unwrap()
    }

    fn enter_interface(t: &Topology, dev: &str, port: &str) -> CliSession {
        let s = run(t, dev, CliSession::default(), "enable");
        let s = run(t, dev, s.session, "configure terminal");
        let s = run(t, dev, s.session, &format!("interface {port}"));
        s.session
    }

    #[test]
    fn prompts_follow_the_mode() {
        let t = topo();
        let s = run(&t, "r", CliSession::default(), "");
        assert_eq!(s.prompt, "Roteador-r>");
        let s = run(&t, "r", s.session, "enable");
        assert_eq!(s.prompt, "Roteador-r#");
        let s = run(&t, "r", s.session, "conf t");
        assert_eq!(s.prompt, "Roteador-r(config)#");
        let s = run(&t, "r", s.session, "interface p2");
        assert_eq!(s.prompt, "Roteador-r(config-if-p2)#");
    }

    #[test]
    fn ip_address_and_no_shutdown_change_the_topology() {
        let t = topo();
        let s = enter_interface(&t, "r", "p1");
        let out = run(&t, "r", s, "ip address 10.0.1.1 255.255.255.0");
        let changed = out.topology.expect("configuração deve alterar a topologia");
        let p = &changed.devices[1].ports[0];
        assert_eq!(p.ip.as_deref(), Some("10.0.1.1"));
        assert_eq!(p.prefix_len, Some(24));
    }

    #[test]
    fn show_ip_interface_brief_reflects_real_state() {
        let mut t = topo();
        t.devices[1].ports[0].ip = Some("10.0.1.1".into());
        t.devices[1].ports[0].prefix_len = Some(24);
        let out = run(&t, "r", CliSession { mode: Mode::Privileged, interface: None }, "show ip interface brief");
        let joined = out.output.join("\n");
        assert!(joined.contains("p1") && joined.contains("10.0.1.1") && joined.contains("up"), "{joined}");
        assert!(joined.contains("p2") && joined.contains("unassigned") && joined.contains("down"), "{joined}");
    }

    #[test]
    fn shutdown_is_reported_as_administratively_down() {
        let mut t = topo();
        t.devices[1].ports[0].shutdown = true;
        let out = run(&t, "r", CliSession::default(), "show ip interface brief");
        assert!(out.output.join("\n").contains("administratively down"));
    }

    #[test]
    fn static_route_is_added_and_shown() {
        let t = topo();
        let s = run(&t, "r", CliSession { mode: Mode::Config, interface: None }, "ip route 10.0.9.0 255.255.255.0 10.0.1.2");
        let changed = s.topology.expect("rota deve alterar a topologia");
        assert_eq!(changed.devices[1].routes.len(), 1);
        let out = run(&changed, "r", CliSession { mode: Mode::Privileged, interface: None }, "show ip route");
        assert!(out.output.join("\n").contains("S    10.0.9.0/24 via 10.0.1.2"));
    }

    #[test]
    fn route_commands_are_rejected_on_hosts() {
        let t = topo();
        let out = run(&t, "a", CliSession { mode: Mode::Config, interface: None }, "ip route 10.0.9.0 24 10.0.1.2");
        assert!(out.error);
        assert!(out.topology.is_none());
    }

    #[test]
    fn default_gateway_is_set_on_hosts_and_shown_in_routes() {
        let t = topo();
        let s = run(&t, "a", CliSession { mode: Mode::Config, interface: None }, "ip default-gateway 10.0.1.1");
        let changed = s.topology.unwrap();
        let out = run(&changed, "a", CliSession { mode: Mode::Privileged, interface: None }, "show ip route");
        assert!(out.output.join("\n").contains("S*   0.0.0.0/0 via 10.0.1.1"));
    }

    #[test]
    fn unknown_commands_are_errors_not_silent() {
        let t = topo();
        let out = run(&t, "r", CliSession::default(), "frobnicate");
        assert!(out.error);
        assert!(out.output[0].starts_with("% "));
    }

    #[test]
    fn parse_mask_accepts_dotted_and_prefix_and_rejects_noncontiguous() {
        assert_eq!(parse_mask("255.255.255.0").unwrap(), 24);
        assert_eq!(parse_mask("255.255.0.0").unwrap(), 16);
        assert_eq!(parse_mask("0.0.0.0").unwrap(), 0);
        assert_eq!(parse_mask("24").unwrap(), 24);
        assert!(parse_mask("255.0.255.0").is_err());
        assert!(parse_mask("33").is_err());
    }

    #[test]
    fn invalid_configuration_is_not_applied() {
        let t = topo();
        let s = enter_interface(&t, "r", "p1");
        let out = run(&t, "r", s, "ip address 10.0.1.1 255.0.255.0");
        assert!(out.error);
        assert!(out.topology.is_none());
    }

    #[test]
    fn ping_from_the_cli_uses_the_engine() {
        let mut t = topo();
        t.devices[0].ports[0].ip = Some("10.0.1.10".into());
        t.devices[0].ports[0].prefix_len = Some(24);
        t.devices[0].gateway = Some("10.0.1.1".into());
        t.devices[1].ports[0].ip = Some("10.0.1.1".into());
        t.devices[1].ports[0].prefix_len = Some(24);
        let out = run(&t, "a", CliSession::default(), "ping 10.0.1.1");
        assert!(out.output.iter().any(|l| l == "Taxa de sucesso: 4 de 4 respostas"), "{:?}", out.output);
    }
}
