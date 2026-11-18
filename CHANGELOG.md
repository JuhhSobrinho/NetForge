# Changelog

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/). Versionamento ainda não definido.

## [Não lançado]

### Adicionado
- Fase 1: workspace Rust (`Cargo.toml` na raiz) com o crate `crates/netforge-engine`; app Tauri 2 em `apps/desktop` (React 19, TypeScript, Vite 8); comando IPC `engine_status` validado por testes Rust e Vitest; CSP restritiva e capability mínima (`core:default`).
- Crates de projeto, dispositivos, protocolos, persistência e plugins ainda não foram criados: entram nas fases em que forem necessários.
- Fase 0: documentação inicial em `docs/` (pesquisa, arquitetura, funcionalidades, roadmap, decisões técnicas, metodologia de engenharia reversa, análise de concorrentes, protocolos, estratégia de testes).
- `README.md`, `CONTRIBUTING.md` e este `CHANGELOG.md`.

### Observações
- Nenhuma funcionalidade de simulação foi implementada ainda.
- Decisões de arquitetura em `docs/TECHNICAL_DECISIONS.md` aprovadas em 2026-11-18 (ADR-003, ADR-009 e ADR-010 seguem abertos).
