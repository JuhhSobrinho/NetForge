# PROTOCOLS.md — Protocolos: plano de implementação

Regra central do briefing: **não apresentar um protocolo como implementado enquanto existir apenas uma representação visual.** Abaixo, o status de cada protocolo é o status real do código — hoje, todos são **não implementados**.

Fonte primária de comportamento: RFCs do IETF. O comportamento observado no Packet Tracer é usado como hipótese a comparar, não como especificação.

## Legenda de status

- `não implementado` — nenhum código.
- `planejado` — fase definida.
- `em implementação` — código em andamento, com testes incompletos.
- `implementado` — testes de referência passando, validado com PCAP quando aplicável.

## Camada de enlace

| Protocolo | Referência | Fase | Status | Escopo inicial | Fora do escopo inicial |
|---|---|---|---|---|---|
| Ethernet II | IEEE 802.3 / formato EtherType | 3 | implementado (escopo mínimo) | Quadro com MAC origem/destino e EtherType; padding até 60 bytes; sem FCS | Auto-negociação, jumbo frames, FCS/CRC |
| ARP | RFC 826 | 3 | implementado (escopo mínimo) | Request/reply para Ethernet/IPv4; cache sem envelhecimento (timeout não implementado) | Gratuitous ARP, proxy ARP, timeout de cache |
| Switch MAC learning | Comportamento de switch (didático) | 3 (antecipado) | implementado (escopo mínimo) | Aprendizado pelo MAC de origem, encaminhamento, inundação para destinos desconhecidos e broadcast | Envelhecimento da tabela (aging), port security, STP (laço gera erro explícito) |
| VLAN (802.1Q) | IEEE 802.1Q | 6 | não implementado | Tag, access/trunk básicos | Q-in-Q, GVRP |
| STP | IEEE 802.1D | 6+ | não implementado | — | Versão completa, RSTP/MSTP |

## Camada de rede

| Protocolo | Referência | Fase | Status | Escopo inicial | Fora do escopo inicial |
|---|---|---|---|---|---|
| IPv4 | RFC 791 | 3 | implementado | Cabeçalho de 20 bytes com checksum, TTL 64 e DF; encaminhamento por roteadores com TTL (descarte com ICMP tempo excedido); rotas conectadas, estáticas e gateway padrão | Opções, fragmentação, protocolos de roteamento dinâmico (RIP/OSPF, fases 7+) |
| ICMP (IPv4) | RFC 792 | 3 | implementado (escopo mínimo) | Echo request/reply (tipos 0 e 8); destino inalcançável (tipo 3, código 0) e tempo excedido (tipo 11), recebidos pelo ping | Redirect, códigos de erro além de 0 |
| Roteamento estático | — | 4 | implementado (Fase 4) | Tabela com maior prefixo; rota conectada vence rota estática de mesmo prefixo; próximo salto precisa ser alcançável por interface conectada | Rotas com métricas, rotas flutuantes, balanceamento |
| IPv6 | RFC 8200 | 6+ | não implementado | Cabeçalho, ICMPv6 echo | NDP completo, SLAAC (planejado depois) |
| RIP | RFC 2453 (RIPv2) | 7+ | não implementado | — | Tudo (etapa posterior) |
| OSPF | RFC 2328 (OSPFv2) | 7+ | não implementado | — | Tudo (etapa posterior) |

## Camada de transporte

| Protocolo | Referência | Fase | Status | Escopo inicial | Fora do escopo inicial |
|---|---|---|---|---|---|
| UDP | RFC 768 | 6 | não implementado | Datagramas, portas, checksum | — |
| TCP | RFC 9293 (consolida RFC 793 e atualizações) | 6 | não implementado | Handshake, dados, ACK, encerramento, retransmissão simples | Controle de congestionamento completo, SACK, janelas dinâmicas avançadas |

Observação: TCP completo é grande. Proposta: começar com handshake e transferência confiável simples, e declarar explicitamente o que não é implementado.

## Camada de aplicação

| Protocolo | Referência | Fase | Status | Escopo inicial | Fora do escopo inicial |
|---|---|---|---|---|---|
| DHCP | RFC 2131 | 6 | não implementado | DORA (Discover/Offer/Request/Ack) com pool simples | Relay, opções avançadas |
| DNS | RFC 1034 / 1035 | 6 | não implementado | Consulta A simples contra zona local | Recursão, DNSSEC, TCP fallback |
| HTTP | RFC 9110 | 6+ | não implementado | GET/resposta simples | Cache, keep-alive complexo |
| HTTPS | RFC 8446 (TLS 1.3) | 6+ | **não implementado — apenas abstração didática planejada** | Representação visual e explicação do conceito | Implementação TLS real: não planejada para o MVP |

**Atenção:** HTTPS, conforme o briefing, será uma **abstração didática**. Não haverá criptografia real nesse caminho, e isso deve aparecer na interface.

## Política de status

- Um protocolo só recebe o status `implementado` quando: (a) testes de referência passam, (b) há cenário de regressão em `tests/scenarios/`, e (c) quando aplicável, o PCAP gerado é comparado com captura de referência no Wireshark.
- Representações visuais sem motor não aparecem na lista de protocolos suportados.
- Cada item "fora do escopo" deve constar na interface ou na ajuda quando o usuário tentar usá-lo.

## Status após a Fase 4

Roteamento estático e encaminhamento com TTL implementados, além de gateway padrão em hosts e mensagens ICMP de erro. Validados por testes (roteamento entre duas sub-redes, duas rotas estáticas em sentido oposto, ICMP inalcançável, interface desligada) e por execução no app: PC 1 - roteador - PC 2 configurados só pela CLI, com 4 de 4 respostas. Ainda sem validação no Wireshark.

## Status após a Fase 3

Implementados com escopo mínimo: Ethernet II, ARP (request/reply), switch com aprendizado de MAC, IPv4 (sem roteamento) e ICMP echo. Validados por testes de vetores (checksum RFC 1071, layouts de ARP e IPv4) e por cenário ponta a ponta (PC - switch - PC, RTT de 8 ms no primeiro ping e 4 ms nos seguintes). **Ainda não validado no Wireshark**: exige instalação aprovada.

## Protocolos no cenário inicial (Fase 3)

Ethernet → ARP → IPv4 → ICMP. Esse é o conjunto para o primeiro gate (ping entre dois PCs via switch).

Ver também: [REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md), [TESTING_STRATEGY.md](TESTING_STRATEGY.md).


## Status após o início da Fase 6

- **UDP (RFC 768):** implementado. Cabeçalho completo; checksum enviado como zero (permitido em IPv4). Serve de transporte para o DHCP.
- **DHCP (RFC 2131):** implementado com escopo mínimo. Formato BOOTP com opções 1 (máscara), 3 (roteador), 50, 51, 53, 54. Fluxo DORA (Discover, Offer, Request, Ack) executado pela simulação; a concessão é gravada na interface como endereço estático. Limitações: só no mesmo domínio de broadcast (sem relay); o servidor precisa ter interface na sub-rede do pool; sem renovação de concessão.
- **DNS, TCP reduzido e VLAN:** planejados, ainda não implementados.
