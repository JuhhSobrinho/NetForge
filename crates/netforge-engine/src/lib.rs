//! Núcleo do simulador NetForge.
//!
//! Fase 1: apenas a fundação. Nenhum protocolo, dispositivo ou evento de rede
//! está implementado ainda (ver docs/PROTOCOLS.md e docs/ROADMAP.md).

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
