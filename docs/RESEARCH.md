# RESEARCH.md — Pesquisa da Fase 0

Status: **Fase 0 — em andamento.** Este documento registra o que foi pesquisado, o que foi confirmado em fontes e o que ainda é hipótese.

Convenção de rótulos usada em todo o `docs/`:

- **[OBSERVADO]** — comportamento visto diretamente em uma instalação real. *Nenhum item está nesta categoria ainda*: não instalamos nem executamos o Packet Tracer nesta fase.
- **[DOCUMENTADO]** — afirmado em documentação oficial ou em página de projeto, consultada nesta fase.
- **[HIPÓTESE]** — suposição de engenharia que precisa de validação.

---

## 1. Escopo da pesquisa desta fase

1. Ferramentas de investigação (Ghidra, x64dbg, Frida, Accessibility Insights, WinApp CLI, Wireshark).
2. Referências de produto (Cisco Packet Tracer, GNS3, EVE-NG).
3. Bibliotecas e tecnologias candidatas (React Flow, Tauri, Rust).
4. Protocolos-alvo (ver [PROTOCOLS.md](PROTOCOLS.md)).

Fora de escopo nesta fase: qualquer código de aplicação, instalação de software no sistema e análise de binários de terceiros.

## 2. Resumo das descobertas

| Tema | Achado | Rótulo |
|---|---|---|
| Packet Tracer — modos | Possui dois modos: *Realtime* (tempo contínuo, limitado pelos modelos de protocolo) e *Simulation* (relógio pausável, controle do fluxo de PDUs, inspeção por camada). | [DOCUMENTADO] — materiais oficiais Cisco/NetAcad e ajuda do PT |
| Packet Tracer — PDU | PDUs são o artefato central da visualização de pacotes; no modo Simulation, eventos podem ser enfileirados e avançados passo a passo. | [DOCUMENTADO] |
| GNS3 — arquitetura | Separa GUI, controller e compute. A GUI conversa com o controller por HTTP/JSON; o controller guarda o estado do projeto em disco; o compute executa os emuladores. | [DOCUMENTADO] — docs.gns3.com e documentação do gns3-server |
| GNS3 — variante de topologia | Um controller e um ou mais computes; o compute local pode rodar no mesmo processo do controller. | [DOCUMENTADO] |
| EVE-NG — edições | Community está descontinuada (sem atualizações); Professional oferece Docker e mais nós. Fontes divergem sobre limites atuais. | [DOCUMENTADO] com ressalva — fontes de terceiros conflitam |
| React Flow (`@xyflow/react`) | Licença MIT; suporta nós e arestas customizados e múltiplos handles. | [DOCUMENTADO] |
| Tauri 2 | Capabilities por label de janela; comandos só são liberados se permitidos; sidecars exigem permissão `shell:allow-execute`. | [DOCUMENTADO] — parte por snippets de guias de terceiros |
| Tauri + instalador NSIS | Há relato (issue #15134) de sidecar não atualizado em reinstalação e cópia falhando se o processo estiver rodando. | [HIPÓTESE] até validação com a doc oficial |

## 3. Fontes consultadas

- Cisco Packet Tracer — materiais de modos Realtime/Simulation (NetAcad e ajuda do PT).
- GNS3 — documentação de arquitetura (docs.gns3.com) e referência do gns3-server.
- EVE-NG — página de comparação de features e artigos de fornecedores (ver [COMPETITOR_ANALYSIS.md](COMPETITOR_ANALYSIS.md) sobre a confiabilidade).
- React Flow — pacote npm `@xyflow/react` e README do repositório xyflow.
- Tauri — documentação v2 (sidecar, capabilities) e issue #15134 do repositório tauri-apps/tauri.

**Limitação:** a pesquisa foi feita com buscas gerais. Antes da Fase 2, as afirmações sobre React Flow e Tauri devem ser conferidas diretamente nas páginas oficiais (API de handles, guia do instalador Windows).

## 4. Ferramentas investigadas

Análise detalhada em [REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md#2-ferramentas). Resumo:

| Ferramenta | Recomendação | Motivo |
|---|---|---|
| Wireshark | **Usar** | Validação do motor (PCAP) — essencial. |
| Accessibility Insights | **Usar (secundário)** | Testar acessibilidade do próprio NetForge; observar UI do PT só se necessário. |
| WinApp CLI | **Avaliar mais tarde** | Útil para testes E2E do NetForge; maturidade e escopo a confirmar antes de adotar. |
| Ghidra | **Dispensável** | Análise de binário do PT não é necessária e fica fora da ética do projeto. |
| x64dbg | **Dispensável** | Idem; não há necessidade de depuração do PT. |
| Frida | **Dispensável** | Instrumentação de software proprietário não está no escopo. |

## 5. Pontos de atenção legais e éticos

- O Packet Tracer é software proprietário da Cisco. Nosso método é **comportamental**: observar entradas, saídas e documentação, sem copiar código, assets proprietários ou detalhes internos.
- Não contornar licenciamento nem qualquer mecanismo de proteção.
- Não afirmar que um comportamento é "igual ao PT" sem registrar o cenário de comparação (ver [TESTING_STRATEGY.md](TESTING_STRATEGY.md)).
- Padrões da IETF (RFCs) são a fonte primária para protocolos; o NetForge se baseia neles, não em implementações de terceiros.

## 6. Perguntas em aberto

1. Qual versão do Packet Tracer será usada como referência comportamental? (decidir antes da Fase 3)
2. O React Flow atende aos requisitos de performance com centenas de nós? (teste na Fase 2)
3. Qual formato de plugin será adotado? (ver [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md), ADR-009)
4. Haverá suporte oficial a Linux/macOS? (fora do escopo inicial)

## 7. Próximos passos da pesquisa

- Instalar o Packet Tracer em uma máquina de teste **somente após aprovação** (ver regra de instalação no briefing).
- Executar os 11 passos de [REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md) no cenário "dois PCs e um switch".
- Validar as afirmações marcadas como [HIPÓTESE].

Ver também: [ARCHITECTURE.md](ARCHITECTURE.md), [COMPETITOR_ANALYSIS.md](COMPETITOR_ANALYSIS.md), [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md).
