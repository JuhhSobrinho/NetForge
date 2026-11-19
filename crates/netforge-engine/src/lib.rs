//! Núcleo do simulador NetForge (Fase 3).
//!
//! Módulos: `topology` (modelo e validação), `ethernet`, `arp`, `ipv4`, `icmp`, `checksum`
//! (protocolos), `sim` (motor de eventos e ping), `pcap` (exportação para o Wireshark).
//! Ver docs/PROTOCOLS.md para o status de cada protocolo.

pub mod arp;
pub mod checksum;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod pcap;
pub mod sim;
pub mod topology;

/// Versão do motor, vinda do `Cargo.toml` do workspace.
pub fn engine_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_version_matches_workspace_version() {
        assert_eq!(engine_version(), "0.1.0");
    }
}
