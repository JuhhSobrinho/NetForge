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

- **Status:** aceita (2026-11-18). Medição com 200 equipamentos e 300 links (modo dev, Chromium headless via Playwright, 1280×800):
  - Pan e zoom: 60 FPS médios, p95 de 16,7 ms.
  - Arrastar equipamento: 60 FPS médios, pior quadro de 16,8 ms.
  - Carregamento até todos os nós visíveis: cerca de 1,5 s (modo dev; build de produção não medido).
  - Primeira medição tinha um quadro de 300 ms ao selecionar. A causa era recriar os 200 nós e 300 arestas a cada seleção. Corrigido com mesclagem preservando objetos inalterados (src/canvas/merge.ts) e atualização separada de seleção.
  - Critério de saída (interação fluida com 200/300) atendido. React Flow mantido.
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

## ADR-014 — Topologia editada no frontend na Fase 2; regras de rede migram para o Rust na Fase 3

- **Status:** parcialmente substituída por ADR-016 (2026-11-18)
- **Contexto:** a Fase 2 é de canvas e edição. O motor Rust ainda não tem modelo de dispositivos, e mover cada edição por IPC antes disso acrescentaria latência e código sem benefício de simulação.
- **Decisão:** o modelo de topologia (dispositivos, portas, links), as regras de conexão e a validação de JSON ficam em `apps/desktop/src/topology/model.ts`, como funções puras testadas com Vitest. Essa lógica **não contém comportamento de rede**. Na Fase 3, quando o motor precisar do grafo, o modelo passa para `crates/netforge-engine` e o frontend passa a consumi-lo via IPC.
- **Alternativas:** modelo no Rust desde já (mais fiel ao ADR-002, porém mais trabalho sem ganho imediato); modelo só no React (rejeitado: mistura estado com componentes).
- **Consequências:** há duas implementações temporárias da validação de JSON (TS e, na Fase 3, Rust). Os cenários de teste em `tests/scenarios/` devem valer para as duas.

## ADR-015 — Abrir e salvar arquivos: diálogo nativo + comandos Rust com validação de caminho

- **Status:** aceita (2026-11-18)
- **Decisão:** o caminho é escolhido com o plugin `tauri-plugin-dialog` (diálogos nativos). A leitura e a escrita passam por dois comandos Rust (`open_project_file`, `save_project_file`) que exigem caminho absoluto, extensão `.json` e tamanho máximo de 5 MiB. Não há plugin `fs`: a permissão de arquivo fica restrita aos comandos do app e às duas permissões do diálogo (`dialog:allow-open`, `dialog:allow-save`).
- **Motivo:** menor superfície de permissão (princípio já aplicado na Fase 1). O frontend não consegue ler caminhos arbitrários do disco.
- **Consequências:** o caminho vindo do diálogo é confiável por ser escolhido pelo usuário, mas a validação continua no Rust.

## ADR-016 — Motor recebe um snapshot da topologia; edição continua no frontend (substitui parcialmente ADR-014)

- **Status:** aceita (2026-11-18)
- **Contexto:** ADR-014 previa migrar o modelo para o Rust na Fase 3. Na implementação, o motor precisa de um modelo próprio para simular, então ele ganhou um modelo e um validador. A edição (undo/redo, sincronização com o React Flow) continua no frontend.
- **Decisão:** cada ping envia a topologia atual ao motor (`run_ping`). O motor valida o snapshot com as mesmas regras do frontend e simula. O motor não guarda estado entre chamadas: cada execução parte de uma simulação nova, então o ARP e a tabela MAC não persistem.
- **Consequências:**
  - As regras de validação existem em duas implementações (TypeScript e Rust). Qualquer divergência aparece em testes; a migração completa da edição para o Rust fica para a Fase 4, quando o CLI precisar alterar a topologia.
  - Latência de link fixa de 1000 µs e sem perda de pacotes. Esses parâmetros são constantes do motor, não configuráveis por enquanto.
  - Captura PCAP feita na interface da origem (quadros transmitidos e recebidos). Não há captura por link, como um sniffer de rede faria.
  - Laço de camada 2 sem STP é interrompido por limite de eventos (20 000) e reportado como erro.

## ADR-017 — CLI sem estado no motor; modo de sessão guardado no frontend

- **Status:** aceita (2026-11-18)
- **Contexto:** a CLI precisa de modos (usuário, privilegiado, configuração, interface), mas o motor só guarda a topologia e executa pings sob demanda.
- **Decisão:** o motor (`netforge_engine::cli::execute`) recebe topologia, equipamento, modo e linha; devolve saída, novo modo e, se houve configuração, a topologia alterada. O frontend guarda o modo por equipamento e aplica a topologia alterada como uma única etapa de undo.
- **Consequências:**
  - Cada linha envia a topologia inteira. Aceitável com topologias pequenas; se virar gargalo, o motor passa a guardar a topologia entre chamadas.
  - O modo se perde quando o terminal é recriado (ao trocar de equipamento). Isso é uma limitação conhecida, não um bug.
  - `show arp` e `show mac address-table` ficam de fora até o motor manter tabelas entre execuções; a decisão é deliberadamente adiada.
  - A sintaxe de comandos é do NetForge (inspirada em CLIs de roteadores), e as interfaces são nomeadas `p1`, `p2`... para não copiar nomes de equipamentos reais.

### Atualização (mesma fase)

As tabelas ARP e MAC passaram a ser devolvidas pelo motor ao final de cada ping ou traceroute (`tables`). O frontend guarda a última execução e a envia de volta aos comandos `show arp` e `show mac address-table`. O motor continua sem estado entre chamadas. Consequência: as tabelas mostram a última execução, não um estado contínuo.

## ADR-018 — Linha do tempo sobre o log completo (refina ADR-005 para a visualização)

- **Status:** aceita (2026-11-18)
- **Contexto:** a Fase 5 pede voltar e avançar na simulação. ADR-005 previa snapshot + replay.
- **Decisão:** o motor devolve o log completo de cada execução (com quadro decodificado e enlace por transmissão). O "voltar" move o cursor dentro desse log; não há restauração de estado de rede no frontend.
- **Motivo:** a execução é determinística e de tamanho limitado (MAX_LOG_ENTRIES), então o log completo já contém tudo que a visualização precisa. Snapshots só seriam necessários se o motor passasse a rodar sessões longas.
- **Consequências:** o voltar mostra o que já aconteceu, não reexecuta nada. Se o motor ganhar simulação contínua (fora do escopo atual), a decisão deve ser revista.
