# FEATURES.md — Mapa de funcionalidades

Cada funcionalidade tem uma fase-alvo (ver [ROADMAP.md](ROADMAP.md)) e um status. Status possíveis: `planejado`, `pesquisa`, `em implementação`, `pronto`. **Hoje, todas estão em `planejado` ou `pesquisa`.**

Prioridades: **P0** = MVP; **P1** = depois do MVP; **P2** = ideias em avaliação.

## 1. Network Designer (canvas)

| Funcionalidade | Prioridade | Fase | Status | Notas |
|---|---|---|---|---|
| Canvas com zoom e pan | P0 | 2 | pesquisa | Avaliar React Flow contra canvas próprio. |
| Drag and drop de dispositivos | P0 | 2 | planejado | |
| Conexões entre portas (handles) | P0 | 2 | pesquisa | Depende da API de handles do React Flow. |
| Identificação de interfaces (rótulos) | P0 | 2 | planejado | |
| Seleção múltipla | P1 | 2 | planejado | |
| Organização automática (layout) | P2 | 5+ | pesquisa | Avaliar biblioteca de layout (ex.: dagre/elk) — não decidido. |
| Grupos e áreas | P2 | 5+ | pesquisa | |
| Duplicação de equipamentos | P1 | 2 | planejado | |
| Atalhos de teclado | P1 | 2 | planejado | |
| Undo / redo | P0 | 2 | planejado | Baseado em comandos do motor, não em snapshots de UI. |

## 2. Device Manager

| Dispositivo | Prioridade | Fase | Observação |
|---|---|---|---|
| PC | P0 | 4 | Terminal + IP. |
| Laptop | P1 | 4 | Mesmo modelo de PC, com outro ícone/portas. |
| Switch | P0 | 4 | Tabela MAC, VLAN na Fase 6. |
| Router | P0 | 4 | Tabela de rotas estática. |
| Server | P1 | 6 | DHCP/DNS quando implementados. |
| Hub | P2 | 4 | Didático; domínio de colisão simplificado. |
| Access Point | P2 | 6+ | Depende de modelo Wi-Fi, ainda não definido. |
| Firewall | P2 | 7+ | Escopo a definir; pode ser ACL simples. |

Cada dispositivo terá: ID único, nome, modelo, interfaces, endereços IP, configurações, estado operacional e tabelas internas (MAC, ARP, rotas).

## 3. Connection Manager

| Funcionalidade | Prioridade | Fase |
|---|---|---|
| Cabo Ethernet (cobre) | P0 | 2/3 |
| Estado de porta (up/down, administrativo) | P0 | 3 |
| Velocidade e duplex (informativo no MVP) | P1 | 4 |
| Falhas físicas simuladas (desconexão, cabo cortado) | P1 | 5 |

Velocidade e duplex **não** alteram o desempenho no MVP; aparecem apenas como atributo. Modelar isso exige decisão própria (ADR futura).

## 4. Simulation Engine

Ver [ARCHITECTURE.md §3](ARCHITECTURE.md#3-motor-de-simulação). Funcionalidades P0: fila de eventos, relógio virtual, encaminhamento de quadros, ARP, IPv4, ICMP. P1: latência e perda configuráveis, tabelas exibidas. P2: STP (Fase 6+).

## 5. Virtual CLI

Requisito do briefing: parser próprio e respostas baseadas no estado real.

| Comando | Prioridade | Fase |
|---|---|---|
| `show ip interface brief` | P0 | 4 (implementado) |
| `show ip route` | P0 | 4 (implementado) |
| `show arp` | P0 | 3/4 (implementado: tabela da última execução de ping ou traceroute) |
| `show mac address-table` | P0 | 4 (implementado para switches: tabela da última execução) |
| `ping` | P0 | 3 (implementado) |
| `traceroute` | P1 | 4 (implementado: TTL crescente, até 16 saltos) |
| `configure terminal` | P0 | 4 (implementado) |
| `interface` | P0 | 4 (implementado; nomes `p1`, `p2`... ou `porta 1`) |
| `ip address` | P0 | 4 (implementado) |
| `no shutdown` | P0 | 4 (implementado; também `shutdown`) |

Regra: **a sintaxe dos comandos é nossa**. Comandos com o nome de IOS são uma escolha de compatibilidade didática; não copiamos a saída textual do IOS. Respostas devem ser geradas pelo estado do motor. Nenhum comando retorna texto fixo.

## 6. Packet Inspector

Campos: origem, destino, protocolo, MAC, IP, portas TCP/UDP, cabeçalhos por camada, payload (quando disponível), dispositivo atual, histórico de encaminhamento. Visualização em camadas (ex.: Ethernet → IPv4 → ICMP).

Prioridade P0 para cabeçalhos e histórico de encaminhamento (Fase 5).

## 7. Simulation Timeline

Play, pause, step forward (P0); step backward (P1, depende de histórico completo); velocidade (P0); filtros por protocolo (P1); histórico de eventos (P0); animação de pacotes (P0); seleção de evento (P0). Cada evento tem ID rastreável.

## 8. Project Manager

| Funcionalidade | Prioridade | Fase |
|---|---|---|
| Criar, salvar, abrir projetos (JSON) | P0 | 2 |
| Importar/exportar topologia JSON | P0 | 2 |
| Pastas de organização | P2 | 7 |
| Modelos de laboratório | P1 | 7 |
| Histórico de alterações | P1 | 5 |
| Recuperação após falha | P1 | 5 |

## 9. Intelligent Assistant

Propósito: explicar, nunca inventar. Só responde com base no estado real do simulador (consultas tipadas ao motor). Fase 7. Antes de implementar, avaliar se um assistente com LLM é necessário ou se regras determinísticas de diagnóstico atendem melhor. Ver [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md), ADR-010.

## 10. Diferenciais (ideias em avaliação)

| Ideia | Viabilidade inicial | Decisão |
|---|---|---|
| Visualização do caminho do pacote com alterações de cabeçalho | Alta — depende do Packet Inspector | Fase 5 |
| Diagnóstico de ping falho com causas reais | Média — exige eventos com motivo de descarte | Fase 5/7 |
| Modo aprendizagem | Média — conteúdo educacional | Fase 7 |
| Histórico com recuperação de estados | Média — snapshot+replay | Fase 5 |
| Laboratórios automáticos | Baixa/média — exige gerador validado | Fase 7 |
| Plugins | Média — exige API estável | Fase 7 |
| Temas, painéis e atalhos | Alta | Fase 2+ |

Ver a análise de cada ideia em [COMPETITOR_ANALYSIS.md](COMPETITOR_ANALYSIS.md#diferenciais).
