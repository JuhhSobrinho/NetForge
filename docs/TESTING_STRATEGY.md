# TESTING_STRATEGY.md — Estratégia de testes

## 1. Pirâmide de testes

| Nível | Ferramenta | O que cobre | Quando roda |
|---|---|---|---|
| Unitário (Rust) | `cargo test` | Cada protocolo, parser de CLI, tabelas (ARP, MAC, rotas) | A cada commit |
| Unitário (TS) | `vitest` | Componentes puros, adaptadores, tipos de IPC | A cada commit |
| Integração de motor | `cargo test` com cenários | Topologias completas executadas sem UI | A cada commit |
| Cenários de referência | `tests/scenarios/*.json` + resultado esperado | Logs de eventos e tabelas esperadas | A cada commit |
| Comparação com PCAP | Script + Wireshark (`tshark`, se aprovado) | Quadros gerados vs referência | Por protocolo, em marcos |
| E2E de UI | A escolher (ex.: WinApp CLI — a avaliar) | Fluxos de usuário no app | Antes de cada release |
| Acessibilidade | Accessibility Insights | Árvore de controles, contraste, foco | Por release |
| Desempenho | Benchmark em Rust + medição de frame da UI | Eventos/s, tempo de render com N nós | Por marco (Fase 2, 5) |

## 2. Testes do motor (mais importantes)

- **Determinismo:** mesma entrada → mesmo log, byte a byte. Teste de regressão compara log serializado.
- **Propriedades** (proptest ou equivalente, a confirmar): um quadro enviado é entregue ao destino ou descartado com motivo; TTL nunca fica negativo; tabela MAC não tem duplicatas por VLAN.
- **Limites:** topologia vazia; dispositivo sem interface; endereço duplicado; máscara inválida; loop de camada 2 (sem STP — deve ser detectado e reportado, não travar).
- **Tempo:** modo Simulation não avança sem comando; step avança exatamente um evento.

## 3. Testes de protocolo

Para cada protocolo, um conjunto de casos com:

- entrada (quadro/pacote em bytes ou topologia);
- saída esperada (quadros gerados, descartes com motivo);
- referência (RFC, seção).

Testes de cabeçalho usam vetores de teste extraídos das RFCs quando existirem.

## 4. Testes de projeto e compatibilidade

- Round-trip JSON: salvar → carregar → salvar produz o mesmo arquivo.
- Migração: projetos de `schemaVersion` antigo abrem na versão nova com o resultado esperado.
- Arquivo corrompido: erro tratado, sem perda silenciosa.

## 5. Testes de UI

- Componentes de canvas testados com dados fixos.
- Teste de que a UI não contém lógica de rede (revisão de código + lint de dependências: o frontend não importa módulos de protocolo).
- Verificação de responsividade do layout em resoluções comuns (1366×768, 1920×1080).

## 6. Critérios de aceitação por fase

Cada fase do [ROADMAP.md](ROADMAP.md) tem um gate. O gate só passa se:

1. Todos os testes automatizados passam.
2. Cenários de referência da fase produzem o resultado esperado.
3. Documentação atualizada (FEATURES/PROTOCOLS).
4. Nenhum botão decorativo, nenhum comando com resposta fixa.

## 7. Como validar contra o Packet Tracer

- Mesmo cenário em PT e NetForge; registrar capturas e logs.
- Comparação em dois níveis: **comportamento** (quem responde, em que ordem) e **quadros** (campos determinísticos). Diferenças são listadas e justificadas — nunca "corrigidas" silenciosamente para bater.

## 8. Medição de desempenho

Métricas iniciais: eventos processados por segundo (motor isolado); tempo de render com 100/200/500 nós; uso de memória após 10 mil eventos. As metas numéricas serão definidas após o protótipo da Fase 2 — **não as inventamos agora.**

## 9. Ferramentas de teste fora do escopo

Nenhuma ferramenta é instalada nesta fase. Cada uma que for proposta para adoção deve ser apresentada com justificativa antes da instalação.
