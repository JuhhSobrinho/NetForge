# ROADMAP.md — Roadmap realista

Estimativas são de esforço de **uma pessoa desenvolvedora em tempo parcial**. Elas devem ser revisadas ao fim de cada fase. Não há datas comprometidas.

## Visão geral

| Fase | Nome | Entrega verificável | Estimativa |
|---|---|---|---|
| 0 | Pesquisa e descoberta | docs/ completa, arquitetura aprovada | 2–3 semanas |
| 1 | Fundação | App Tauri abre, IPC ida-e-volta, CI com testes | 2–3 semanas |
| 2 | Interface e topologia | Canvas, dispositivos, portas, conexões, JSON | 4–6 semanas |
| 3 | Motor de simulação | Ethernet + ARP + IPv4 + ICMP ping entre 2 PCs via switch | 4–6 semanas |
| 4 | Equipamentos e CLI | Switch, router, PCs, CLI com os comandos P0 | 6–8 semanas |
| 5 | Simulação visual | Animação, timeline, inspector, histórico | 4–6 semanas |
| 6 | Protocolos adicionais | DHCP, DNS, UDP, TCP básico, VLAN | 8–12 semanas |
| 7 | Recursos avançados | Plugins, IA, laboratórios, integrações | 8+ semanas (aberto) |
| 8 | Estabilização e distribuição | Instalador, updates, segurança | 3–4 semanas |

Total estimado até a Fase 6: **~7–9 meses** em tempo parcial. Fases 7 e 8 não são estimáveis sem resultado da Fase 6.

## Marcos de validação (gates)

Nenhuma fase começa antes do gate da anterior ser cumprido.

- **Gate 0:** arquitetura e decisões aprovadas pelo responsável do projeto (este documento).
- **Gate 1:** `cargo test` e `vitest` passam; app sobe no Windows; comando de exemplo ida-e-volta.
- **Gate 2:** topologia de 10 nós salva/carrega sem perda; teste de round-trip JSON.
- **Gate 3:** cenário "2 PCs + switch, ping" produz log de eventos igual ao esperado e PCAP abre no Wireshark com os mesmos quadros.
- **Gate 4:** cenário "2 redes com router" funciona; todos os comandos P0 respondem com estado real.
- **Gate 5:** timeline e step backward validados em cenários de regressão.
- **Gate 6:** cada protocolo tem casos de teste de referência com captura comparada.

## Detalhe por fase

### Fase 0 — Pesquisa e descoberta
- [x] Mapear ferramentas e referências (ver RESEARCH.md)
- [x] Mapear funcionalidades (FEATURES.md)
- [x] Propor arquitetura (ARCHITECTURE.md, TECHNICAL_DECISIONS.md)
- [x] **Aprovação do responsável** (aprovada em 2026-11-18)
- [ ] Observação comportamental do Packet Tracer (cenário da metodologia) — requer instalação aprovada

### Fase 1 — Fundação (concluída em 2026-11-18)
- Estrutura de pastas e workspace Rust (`crates/`) + app Tauri 2 + React/TS/Vite.
- Comando `ping_core` de ida-e-volta (frontend → Rust → frontend) com tipos compartilhados.
- Configuração de lint, formatação e testes (`cargo test`, `vitest`).
- Capabilities do Tauri mínimas (negar tudo, liberar só o necessário).

### Fase 2 — Interface e topologia (concluída em 2026-11-18)
- Canvas, dispositivos, portas, conexões, propriedades, salvamento JSON.
- Decisão final sobre React Flow: **mantido** (ADR-003). Medição com 200 nós e 300 links: pan, zoom e arraste a 60 FPS.

### Fase 3 — Motor de simulação (concluída em 2026-11-18, com ressalvas: validação no Wireshark pendente)
- Fila de eventos, relógio virtual, interfaces, Ethernet, ARP, IPv4, ICMP.
- Primeiro cenário de referência com PCAP.

### Fase 4 — Equipamentos e CLI (concluída em 2026-11-18, com limitações: histórico do terminal se perde ao trocar de equipamento; tabelas refletem a última execução)
- Switch (MAC learning), router (rotas estáticas), PCs, parser de CLI.
- Os 10 comandos P0 de FEATURES.md §5.

### Fase 5 — Simulação visual
- Animação de pacotes, timeline, Packet Inspector, histórico.

### Fase 6 — Protocolos adicionais
- DHCP, DNS, UDP, TCP (versão reduzida), VLAN. TCP completo é provavelmente fora do escopo; ver PROTOCOLS.md.

### Fase 7 — Recursos avançados
- Plugins, IA, laboratórios, integrações (Docker/QEMU avaliados aqui).

### Fase 8 — Estabilização
- Instalador NSIS/MSI, assinatura de código, updates, revisão de segurança.

## Riscos

| # | Risco | Impacto | Probabilidade | Mitigação |
|---|---|---|---|---|
| R1 | Escopo de protocolos cresce sem limite | Alto | Alta | Fases fechadas; "não implementado" explícito |
| R2 | Lógica de rede vaza para a UI | Alto | Média | Revisão de arquitetura; testes no motor |
| R3 | Não-determinismo na simulação | Alto | Média | Seeds, relógio virtual, testes de log |
| R4 | React Flow não escala para topologias grandes | Médio | Média | Protótipo na Fase 2; canvas atrás de interface |
| R5 | Comportamento do PT mal interpretado | Médio | Média | Registro de evidências; marcar hipóteses |
| R6 | Instalador Windows com sidecar desatualizado | Médio | Baixa/média | Evitar sidecar; testar reinstalação |
| R7 | Pressão para copiar saída/interface do PT | Médio | Média | Política de originalidade (CONTRIBUTING.md) |
| R8 | Custo de step backward | Médio | Média | Snapshot+replay; limitar a cenários pequenos |
| R9 | Fontes de terceiros sobre EVE-NG/Tauri desatualizadas | Baixo | Média | Conferir na documentação oficial |

## Dependências externas

- Node.js/npm (frontend), Rust toolchain (`rustup`), Tauri CLI, WebView2 (Windows).
- Wireshark (validação; não é dependência de runtime do app).

**Nenhuma dessas dependências foi instalada.** Antes de instalar, será apresentada a lista com justificativa, conforme o briefing.

Ver também: [TESTING_STRATEGY.md](TESTING_STRATEGY.md), [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md).
