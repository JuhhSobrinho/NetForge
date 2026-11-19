# NetForge

Simulador de redes para Windows, com motor próprio, interface moderna e foco educacional.

> **Status: Fase 3 concluída (com ressalvas).** O app monta topologia com endereços IPv4 e roda ping entre dois PCs ligados por switch, com log de eventos e exportação PCAP. Implementados com escopo mínimo: Ethernet, ARP, IPv4 (sem roteamento) e ICMP echo. Roteadores ainda não encaminham pacotes (Fase 4). Ver [docs/PROTOCOLS.md](docs/PROTOCOLS.md).

## Objetivo

Permitir criar topologias, configurar dispositivos, executar comandos, simular protocolos, observar pacotes e investigar problemas de conectividade — com um motor independente e testável.

## O que existe hoje

- [docs/RESEARCH.md](docs/RESEARCH.md) — pesquisa e ferramentas
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — arquitetura proposta
- [docs/FEATURES.md](docs/FEATURES.md) — funcionalidades e prioridades
- [docs/ROADMAP.md](docs/ROADMAP.md) — fases e gates
- [docs/TECHNICAL_DECISIONS.md](docs/TECHNICAL_DECISIONS.md) — decisões (ADR)
- [docs/REVERSE_ENGINEERING.md](docs/REVERSE_ENGINEERING.md) — metodologia comportamental
- [docs/COMPETITOR_ANALYSIS.md](docs/COMPETITOR_ANALYSIS.md) — Packet Tracer, GNS3, EVE-NG
- [docs/PROTOCOLS.md](docs/PROTOCOLS.md) — status de cada protocolo
- [docs/TESTING_STRATEGY.md](docs/TESTING_STRATEGY.md) — estratégia de testes

## O que ainda não existe

Nenhum protocolo, dispositivo ou comando está implementado. Veja o status real em [docs/PROTOCOLS.md](docs/PROTOCOLS.md) e [docs/FEATURES.md](docs/FEATURES.md).

## Stack proposta

React + TypeScript + Vite (interface), Tauri 2 (desktop), Rust (motor), SQLite e JSON (persistência). Detalhes e justificativas em [docs/TECHNICAL_DECISIONS.md](docs/TECHNICAL_DECISIONS.md).

## Contribuindo

Veja [CONTRIBUTING.md](CONTRIBUTING.md).

## Licença

A definir (decisão pendente do responsável do projeto).
