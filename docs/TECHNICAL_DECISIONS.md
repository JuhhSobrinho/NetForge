# TECHNICAL_DECISIONS.md — Registro de decisões (ADR)

Formato: cada decisão tem contexto, decisão, alternativas e consequências. Status: `proposta` (aguarda aprovação), `aceita`, `substituída`.

Todas as decisões abaixo estão **propostas** até a aprovação do responsável do projeto (Gate 0).

---

## ADR-001 — Aplicação desktop com Tauri 2

- **Status:** aceita (aprovada pelo responsável em 2026-11-18)
- **Contexto:** o produto precisa rodar no Windows 10/11, com UI moderna e acesso a recursos do sistema, sem um navegador inteiro embutido por aplicativo.
- **Decisão:** usar Tauri 2 com WebView2.
- **Alternativas:** Electron (maior consumo de memória, runtime Node completo); Qt/C++ (mais controle, mais custo de UI web); WinUI/WPF (acoplado a Windows, menos reuso de frontend).
- **Consequências:** binário menor e Rust no backend. Exige cuidado com capabilities e com o instalador (ver R6). Empacotamento NSIS/MSI é o caminho padrão.

## ADR-002 — Motor de simulação em Rust, fora da UI

- **Status:** aceita (aprovada pelo responsável em 2026-11-18)
- **Contexto:** o briefing exige separação entre motor e interface e determinismo.
- **Decisão:** o motor é uma biblioteca Rust (`netforge-engine`) sem dependências de Tauri ou UI. A camada Tauri apenas o expõe.
- **Alternativas:** motor em TypeScript (mais simples de integrar, mas pior para desempenho e para testes de tempo); motor em C++ (mais trabalho de integração).
- **Consequências:** o motor pode ser testado com `cargo test` sem abrir a janela. Exige contrato de tipos entre Rust e TS (ver ADR-007).

## ADR-003 — Canvas: React Flow como candidata, não como decisão final

- **Status:** proposta (decisão final na Fase 2)
- **Contexto:** o React Flow (`@xyflow/react`) tem licença MIT e suporte documentado a nós e arestas customizados com múltiplos handles. Não temos benchmark próprio com topologias grandes.
- **Decisão:** adotar React Flow **atrás de uma camada de adaptação** (`apps/desktop/src/canvas/`), de modo que a troca por canvas próprio não reescreva o resto da UI.
- **Alternativas:** canvas próprio em SVG/Canvas 2D (controle total, custo alto de zoom, pan, seleção e acessibilidade); Konva/Pixi (bom desempenho, menos recursos de grafo prontos).
- **Critério de saída da Fase 2:** 200 nós e 300 arestas com interação fluida (alvo a definir em teste). Se não atingir, avaliar canvas próprio ou Pixi.
- **Consequências:** dependência de terceiro; monitorar atualizações e a API de handles.

## ADR-004 — Modelo de tempo discreto e virtual

- **Status:** aceita (aprovada pelo responsável em 2026-11-18)
- **Contexto:** o briefing pede modo Realtime, modo Simulation e passo-a-passo, com latência e perda.
- **Decisão:** fila de eventos ordenada por tempo virtual. Realtime sincroniza o relógio virtual com o relógio de parede; Simulation só avança por comando.
- **Alternativas:** simulação por ticks fixos (simples, mas imprecisa para latência e ruim para inspeção de eventos); modelo de threads reais por dispositivo (realista, não determinístico).
- **Consequências:** determinismo e testes reprodutíveis. Latências são valores virtuais, não medidas reais.

## ADR-005 — Step backward por snapshot + replay

- **Status:** aceita (aprovada pelo responsável em 2026-11-18)
- **Contexto:** o briefing pede step backward "quando suportado pelo histórico de eventos".
- **Decisão:** snapshots periódicos do estado + log de eventos. Para voltar, restaura o snapshot anterior e reexecuta até o ponto desejado.
- **Alternativas:** reversão de cada evento (cada operação precisa de inverso — frágil); guardar todo estado a cada evento (memória alta).
- **Consequências:** exige que o motor seja determinístico (ADR-004). Custo de memória controlado por intervalo de snapshot. Recurso oferecido só quando o histórico estiver íntegro.

## ADR-006 — Protocolos implementados por especificação (RFC), não por engenharia reversa

- **Status:** aceita (aprovada pelo responsável em 2026-11-18)
- **Contexto:** o briefing autoriza estudo comportamental, mas proíbe cópia de código e invenção de detalhes internos.
- **Decisão:** cada protocolo é implementado a partir das RFCs e de capturas de referência. O comportamento observado no Packet Tracer serve como hipótese a comparar, não como especificação.
- **Consequências:** algumas escolhas de compatibilidade didática (ex.: sintaxe de comandos) ficam explícitas no código. Ver [PROTOCOLS.md](PROTOCOLS.md).

## ADR-007 — Comunicação tipada entre frontend e backend

- **Status:** aceita (aprovada pelo responsável em 2026-11-18)
- **Contexto:** o briefing pede mensagens tipadas.
- **Decisão:** tipos de comando/evento definidos em Rust (serde) e gerados para TypeScript (ex.: `ts-rs` ou `specta` — **a escolher na Fase 1**, após checar a documentação de cada uma).
- **Alternativas:** tipos escritos à mão nos dois lados (risco de divergência).
- **Consequências:** uma etapa de geração no build; testes que falham se os tipos divergirem.

## ADR-008 — Persistência: JSON para projetos, SQLite para histórico

- **Status:** aceita (aprovada pelo responsável em 2026-11-18)
- **Contexto:** o briefing pede JSON para intercâmbio e SQLite para o restante; também exige preservação de compatibilidade de projetos.
- **Decisão:** projetos são JSON com `schemaVersion` e migrações versionadas. Histórico, recuperação e índice de laboratórios em SQLite.
- **Alternativas:** só JSON (simples, mas ruim para histórico grande); só SQLite (projeto não compartilhável como arquivo).
- **Consequências:** dois formatos a manter; testes de migração obrigatórios.

## ADR-009 — Plugins: adiar o mecanismo, definir o contrato depois

- **Status:** proposta (aberta)
- **Contexto:** plugins são desejáveis, mas um mecanismo prematuro trava a API.
- **Decisão:** a Fase 7 decide entre plugins em Rust (compilados, mais rápidos, mais complexos), WebAssembly (isolamento, custo de integração) ou scripts (menos seguros). Até lá, só o crate `netforge-plugin-api` existe, vazio.
- **Consequências:** nenhum plugin é prometido nas fases anteriores.

## ADR-010 — Assistente de IA: consultas ao estado real, sem invenção

- **Status:** proposta (aberta)
- **Contexto:** o briefing exige que a IA consulte o simulador e não invente diagnósticos.
- **Decisão:** antes de qualquer LLM, implementar diagnósticos determinísticos baseados em eventos de descarte (ex.: "ARP sem resposta; host de destino fora da sub-rede"). Um LLM, se usado, só recebe esse estado como contexto e tem a saída validada contra o motor.
- **Consequências:** o assistente é limitado pelo que o motor modela; a interface deve deixar isso claro.

## ADR-011 — Ferramentas de engenharia reversa: apenas as necessárias

- **Status:** aceita (preliminarmente)
- **Decisão:** Wireshark é obrigatório para validação. Accessibility Insights é usado para acessibilidade do próprio NetForge. Ghidra, x64dbg e Frida ficam fora do plano.
- **Motivo:** o objetivo é comportamento e protocolo, não análise do binário do Packet Tracer. Ver [REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md).

## ADR-012 — Idioma

- **Status:** aceita (aprovada pelo responsável em 2026-11-18)
- **Decisão:** documentação em português (briefing). Código, identificadores e mensagens de commit em inglês, para facilitar contribuições externas no futuro.

## ADR-013 — Não há repositório git próprio ainda

- **Status:** observação
- **Contexto:** o diretório `net-forge/` está vazio e dentro de um repositório git maior do usuário (raiz em `C:\Users\julia`). Isso cria risco de commits acidentais no repositório errado.
- **Recomendação:** criar um repositório próprio em `net-forge/` antes da Fase 1 (decisão do responsável).
