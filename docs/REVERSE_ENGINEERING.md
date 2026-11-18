# REVERSE_ENGINEERING.md — Metodologia de engenharia reversa comportamental

## 1. Escopo e limites

- **Objetivo:** entender o comportamento observável do Cisco Packet Tracer (e de referências como GNS3/EVE-NG) e reimplementar **comportamentos e protocolos** de forma independente.
- **Não faremos:** desmontagem, depuração ou instrumentação do binário do PT; cópia de código, assets, textos ou saídas literais de comandos; contorno de licenciamento; uso de recursos internos não públicos.
- **Fontes válidas:** observação em uma instalação legítima, documentação pública, RFCs, capturas de pacotes feitas por nós.

## 2. Ferramentas

| Ferramenta | Uso no projeto | Quando usar |
|---|---|---|
| Wireshark | Captura e análise de quadros/pacotes; validação do motor via PCAP | Sempre que um protocolo for validado (ver §5). |
| Accessibility Insights | Inspeção da árvore de controles | Para testar a acessibilidade do NetForge. Para o PT, só se houver dúvida de organização de janelas. |
| WinApp CLI | Automação de UI para testes repetíveis | Avaliar na Fase 2+ para E2E do NetForge. Não é necessária para a pesquisa. |
| Ghidra | — | Não usar. Análise de binário do PT está fora do escopo. |
| x64dbg | — | Não usar. |
| Frida | — | Não usar. Instrumentação de software proprietário não está no escopo. |

Nenhuma dessas ferramentas foi instalada nesta fase. A instalação de Wireshark (e de qualquer outra) requer aprovação prévia com a lista e a justificativa.

## 3. Modelo de registro de evidências

Cada observação registrada deve conter:

- **Cenário:** topologia, versão do software, sistema operacional, data.
- **Ação:** o que foi feito, passo a passo.
- **Resultado observado:** saída exata (capturas de tela ou texto).
- **Rótulo:** `[OBSERVADO]`, `[DOCUMENTADO]` ou `[HIPÓTESE]`.
- **Evidência anexada:** arquivo PCAP ou captura, quando houver.

## 4. Processo para cada funcionalidade

A metodologia do briefing tem 11 passos. Abaixo, o exemplo "comunicação entre dois computadores", com o que cada passo produz.

| # | Passo | Saída esperada |
|---|---|---|
| 1 | Criar topologia simples (2 PCs + 1 switch) | Diagrama e arquivo do projeto |
| 2 | Configurar IPs (ex.: 192.168.1.10/24 e .20/24) | Configuração registrada |
| 3 | Executar ping | Resultado com tempos e perdas |
| 4 | Observar comportamento (Realtime e Simulation) | Sequência de eventos visíveis |
| 5 | Registrar etapas da comunicação | Lista ordenada de eventos |
| 6 | Capturar pacotes | PCAP exportado ou captura de tela da simulação |
| 7 | Documentar resultados | Entrada na documentação da funcionalidade |
| 8 | Identificar protocolos | Lista: ARP (resolução), ICMP (echo), Ethernet (quadro) |
| 9 | Implementar de forma independente | Código do NetForge a partir das RFCs |
| 10 | Reproduzir o cenário no NetForge | Log de eventos + PCAP |
| 11 | Comparar resultados | Tabela de diferenças com justificativa |

### Documentação por funcionalidade

Cada funcionalidade estudada terá um arquivo com as seções: nome; objetivo; comportamento observado; regras identificadas; protocolos envolvidos; entradas e saídas; dependências; casos de teste; limitações; possíveis melhorias.

## 5. Validação com Wireshark

- Comparar quadros ARP, ICMP echo request/reply e Ethernet (MAC de origem/destino, ordem, tamanho).
- Critério: campos de cabeçalho idênticos em valor, quando determinísticos (ex.: tipo EtherType, protocolo IP, tipo ICMP). Campos não determinísticos (ex.: identificador ICMP, sequência) são registrados como "variável" e não comparados.
- O PCAP exportado pelo NetForge é a entrada da comparação.

## 6. Exemplo de registro (modelo, não observado)

> **Funcionalidade:** resolução ARP antes do primeiro ping.
> **Cenário:** 2 PCs + switch, primeiro ping, sem tabela ARP.
> **Comportamento observado:** [PENDENTE — a ser preenchido após observação.]
> **Hipótese:** o host de origem envia ARP request em broadcast antes do ICMP echo; o primeiro echo pode ser descartado ou enfileirado enquanto aguarda o ARP reply.
> **Protocolos:** Ethernet, ARP, IPv4, ICMP.
> **Caso de teste (proposto):** primeiro ping gera ARP request + reply antes do echo; segundo ping não gera ARP.

A hipótese deve ser confirmada ou refutada antes de virar regra.

## 7. Regras de qualidade

1. Nenhuma saída de comando do PT é copiada como texto de referência do NetForge.
2. Toda regra tem fonte (observação, documentação ou RFC).
3. Hipóteses não viram requisito sem evidência.
4. Limitações são registradas, inclusive as que tornam o NetForge diferente do PT.

Ver também: [RESEARCH.md](RESEARCH.md), [PROTOCOLS.md](PROTOCOLS.md), [TESTING_STRATEGY.md](TESTING_STRATEGY.md).
