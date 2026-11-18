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
| Ethernet II | IEEE 802.3 / formato EtherType | 3 | não implementado | Quadro com MAC origem/destino, EtherType, FCS simulado (sem CRC real obrigatório) | Auto-negociação, jumbo frames |
| ARP | RFC 826 | 3 | não implementado | Request/reply, cache com timeout | Gratuitous ARP, proxy ARP |
| Switch MAC learning | Comportamento de switch (didático) | 4 | não implementado | Aprendizado, encaminhamento, flood em desconhecido | Aging detalhado, port security |
| VLAN (802.1Q) | IEEE 802.1Q | 6 | não implementado | Tag, access/trunk básicos | Q-in-Q, GVRP |
| STP | IEEE 802.1D | 6+ | não implementado | — | Versão completa, RSTP/MSTP |

## Camada de rede

| Protocolo | Referência | Fase | Status | Escopo inicial | Fora do escopo inicial |
|---|---|---|---|---|---|
| IPv4 | RFC 791 | 3 | não implementado | Cabeçalho, TTL, checksum, roteamento de host e router | Fragmentação completa (avaliar), opções |
| ICMP (IPv4) | RFC 792 | 3 | não implementado | Echo request/reply, Time Exceeded, Destination Unreachable | Redirect, Router Advertisement |
| Roteamento estático | — | 4 | não implementado | Tabela, longest prefix match | Rotas default múltiplas com métricas |
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

## Protocolos no cenário inicial (Fase 3)

Ethernet → ARP → IPv4 → ICMP. Esse é o conjunto para o primeiro gate (ping entre dois PCs via switch).

Ver também: [REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md), [TESTING_STRATEGY.md](TESTING_STRATEGY.md).
