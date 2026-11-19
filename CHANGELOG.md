# Changelog

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/). Versionamento ainda não definido.

## [Não lançado]

### Adicionado
- Fase 5: quadros decodificados por camada e enlace por transmissão no log do motor; linha do tempo com play, pausa, avançar, voltar, velocidade, filtro por protocolo e cursor; enlace animado no canvas durante a transmissão; inspetor com Ethernet, ARP, IPv4 e ICMP. ADR-018.
- Fase 4: roteadores com encaminhamento, rotas estáticas, TTL e ICMP de erro; gateway padrão em PCs e servidores; interfaces desligáveis (shutdown); CLI por equipamento com modos e comandos show ip interface brief, show ip route, show arp, show mac address-table, ping, traceroute, ip address, ip route, ip default-gateway; ADR-017.
- Fase 3: motor de simulação em Rust com modelo de topologia e validação; Ethernet II, ARP, IPv4 e ICMP echo (escopo mínimo); switch com aprendizado de MAC; ping com tempo virtual (1000 µs por link); captura PCAP da interface de origem; comandos `run_ping` e `export_pcap`; endereço IPv4 e prefixo por porta; painel de teste de ping. Ver ADR-016.
- Fase 2: canvas com React Flow 13; equipamentos PC, switch, roteador e servidor com portas; ligações entre portas com validação (porta única, sem laço no mesmo equipamento); renomear, remover equipamento e link; undo/redo (100 passos); salvar e abrir projeto JSON com `schemaVersion` 1 via diálogo nativo; ADR-014 e ADR-015.
- Fase 1: workspace Rust (`Cargo.toml` na raiz) com o crate `crates/netforge-engine`; app Tauri 2 em `apps/desktop` (React 19, TypeScript, Vite 8); comando IPC `engine_status` validado por testes Rust e Vitest; CSP restritiva e capability mínima (`core:default`).
- Crates de projeto, dispositivos, protocolos, persistência e plugins ainda não foram criados: entram nas fases em que forem necessários.
- Fase 0: documentação inicial em `docs/` (pesquisa, arquitetura, funcionalidades, roadmap, decisões técnicas, metodologia de engenharia reversa, análise de concorrentes, protocolos, estratégia de testes).
- `README.md`, `CONTRIBUTING.md` e este `CHANGELOG.md`.

### Corrigido
- Canvas com 200 equipamentos: a seleção recriava todos os nós e arestas, gerando um quadro de ~300 ms. Agora só os objetos que mudam são atualizados (ADR-003).

### Observações
- Nenhuma funcionalidade de simulação foi implementada ainda.
- Decisões de arquitetura em `docs/TECHNICAL_DECISIONS.md` aprovadas em 2026-11-18 (ADR-003, ADR-009 e ADR-010 seguem abertos).
