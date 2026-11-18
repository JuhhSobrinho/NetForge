# ARCHITECTURE.md — Arquitetura proposta

Status: **proposta para aprovação.** Nada aqui está implementado.

## 1. Princípios

1. **Motor independente da interface.** O motor de simulação não conhece React, DOM ou Tauri. A UI envia comandos e recebe estado/eventos.
2. **Estado é a fonte da verdade no motor.** A UI é uma projeção do estado; nunca guarda a lógica de rede.
3. **Determinismo.** Dada a mesma topologia, configuração, seed e sequência de comandos, a simulação produz o mesmo log de eventos. Isso é o que torna testes, replay e o modo passo-a-passo confiáveis.
4. **Protocolos como módulos.** Cada protocolo implementa um contrato comum; adicionar um protocolo não exige reescrever o núcleo.
5. **Sem representação falsa.** Um recurso só aparece na UI/CLI quando o motor o implementa de fato.

## 2. Camadas (visão geral)

```
┌──────────────────────────────────────────────────────────────┐
│  Interface (React + TypeScript)                              │
│  Canvas · Device panel · CLI terminal · Timeline · Inspector │
└───────────────▲───────────────────────────┬──────────────────┘
                │ eventos tipados           │ comandos tipados
┌───────────────┴───────────────────────────▼──────────────────┐
│  Camada Tauri (IPC)  — commands, events, validação de input  │
└───────────────▲───────────────────────────┬──────────────────┘
┌───────────────┴───────────────────────────▼──────────────────┐
│  Application services (Rust)                                 │
│  Project manager · Device manager · Session/history          │
├──────────────────────────────────────────────────────────────┤
│  Simulation engine (Rust, sem I/O de UI)                     │
│  Event queue · Clock · Scheduler · Packet forwarding         │
├──────────────────────────────────────────────────────────────┤
│  Protocols (Rust): Ethernet · ARP · IPv4 · ICMP · ...        │
├──────────────────────────────────────────────────────────────┤
│  Persistence: JSON (projetos) · SQLite (histórico/recovery)  │
│  Integrações externas (futuro): Docker · QEMU · PCAP export  │
└──────────────────────────────────────────────────────────────┘
```

Os nove módulos pedidos no briefing mapeiam assim:

| # | Módulo do briefing | Onde vive | Observação |
|---|---|---|---|
| 1 | Interface visual | `apps/desktop/src` (React) | Nunca contém regra de rede. |
| 2 | Gerenciamento de projetos | `crates/netforge-project` | Lê/grava JSON versionado. |
| 3 | Gerenciamento de dispositivos | `crates/netforge-devices` | Modelos de PC, switch, router etc. |
| 4 | Motor de simulação | `crates/netforge-engine` | Fila de eventos, relógio, encaminhamento. |
| 5 | Protocolos | `crates/netforge-protocols` | Um módulo por protocolo. |
| 6 | Sistema de eventos | `crates/netforge-engine` (tipos) | Eventos internos e eventos exportados à UI. |
| 7 | Persistência | `crates/netforge-store` | SQLite e serialização. |
| 8 | Integrações externas | `crates/netforge-integrations` | Vazio até a Fase 7. |
| 9 | Sistema de plugins | `crates/netforge-plugin-api` | Só o contrato; implementação na Fase 7. |

## 3. Motor de simulação

### 3.1 Modelo de tempo e eventos

- Modelo **discreto por eventos**: uma fila ordenada por `(timestamp_virtual, sequência)`.
- O relógio é **virtual**. O modo *Realtime* faz o relógio virtual acompanhar o tempo de parede; o modo *Simulation* só avança por comando (step, play, pause).
- Latência e perda são parâmetros de link aplicados quando um evento de entrega é agendado.

### 3.2 Tipos de evento (inicial)

- `FrameTransmit`, `FrameDeliver` (camada 2)
- `TimerFired` (ARP timeout, ICMP timeout, futuramente TCP retransmit)
- `ConfigChanged` (comandos de CLI, mudanças de interface)
- `LinkStateChanged`

### 3.3 Encaminhamento

- Cada dispositivo recebe `receive(frame, ingress_interface)` e devolve uma lista de ações (`transmit`, `drop(reason)`, `log`).
- Ações viram eventos novos na fila; o dispositivo nunca escreve diretamente em outro dispositivo. Isso preserva a rastreabilidade.

### 3.4 Step backward

Não usaremos "desfazer" reverso de estado (caro e propenso a erro). A abordagem é **snapshot + replay**: guardar snapshots periódicos e reproduzir a partir do mais próximo. Só é oferecido quando o histórico estiver completo (ver [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md), ADR-005).

## 4. Interface e comunicação

- Frontend envia **comandos** (`add_device`, `connect_ports`, `run_cli_line`, `sim_step`) via Tauri commands com tipos gerados ou espelhados.
- Backend emite **eventos** (`sim://event`, `topology://changed`) para a UI.
- Payloads grandes (log completo) são paginados; a UI não recebe o estado inteiro a cada tick.
- Validação de entrada ocorre no backend. A UI não é confiável para regras de rede.

## 5. Dados e compatibilidade

- Projeto em JSON com campo `schemaVersion`. Migrações são funções versionadas.
- Histórico e recuperação em SQLite (um arquivo por projeto ou banco único da aplicação — a decidir na Fase 2).
- Exportação de captura em **PCAP** para validação no Wireshark.

## 6. Riscos arquiteturais (resumo)

Ver lista completa em [ROADMAP.md](ROADMAP.md#riscos). Os principais:

- Vazamento de lógica de rede para a UI.
- Complexidade de determinismo com concorrência em Rust.
- Custo de step backward com topologias grandes.

## 7. Estrutura de pastas proposta

```
net-forge/
├── apps/
│   └── desktop/            # Tauri + React + TypeScript + Vite
│       ├── src/            # UI
│       └── src-tauri/      # Camada IPC (sem regra de rede)
├── crates/
│   ├── netforge-engine/
│   ├── netforge-protocols/
│   ├── netforge-devices/
│   ├── netforge-project/
│   ├── netforge-store/
│   ├── netforge-integrations/
│   └── netforge-plugin-api/
├── docs/
├── tests/
│   ├── scenarios/          # topologias JSON + resultados esperados
│   └── pcap-reference/     # capturas de referência
├── README.md
├── CONTRIBUTING.md
└── CHANGELOG.md
```

Essa estrutura só é criada na Fase 1, após aprovação.

Ver também: [FEATURES.md](FEATURES.md), [PROTOCOLS.md](PROTOCOLS.md), [TESTING_STRATEGY.md](TESTING_STRATEGY.md).
