# COMPETITOR_ANALYSIS.md — Análise comparativa

Fontes: documentação pública e materiais de fornecedores. **Nenhum dado abaixo foi verificado por uso direto nesta fase.** Onde fontes divergem, isso é indicado.

## 1. Visão geral

| Critério | Cisco Packet Tracer | GNS3 | EVE-NG | NetForge (proposta) |
|---|---|---|---|---|
| Foco principal | Ensino/aprendizado, topologias simuladas | Emulação de equipamentos reais (imagens) e integração | Laboratório virtual multi-fornecedor | Ensino com motor próprio e análise |
| Motor de simulação | Modelos próprios de protocolo (proprietário) | Executa imagens reais via emuladores | Executa imagens reais (QEMU/Docker em Pro) | Motor próprio, em Rust, de protocolos reimplementados |
| Realismo | Comportamento simplificado, didático | Alto (quando usa imagem real) | Alto (imagem real) | Didático, por especificação, sem imagens |
| Requisitos de máquina | Leve | Pesado (depende das imagens) | Pesado (servidor/VM) | Leve (alvo) |
| Licença | Gratuito com conta NetAcad (proprietário) | Open source (GPL/Apache, conferir por componente) | Community descontinuada; Pro pago | A definir (proposta: open source) |
| Plataforma | Windows, Linux, macOS (conferir versão) | Windows, Linux, macOS | Servidor Linux, acesso por navegador | Windows (inicial) |
| Interface | Desktop, canvas + modos | Desktop (Qt) + Web UI | Web | Desktop moderno (Tauri + React) |
| Captura de pacotes | Sim (modo Simulation, inspeção de PDU) | Sim (captura Wireshark integrada) | Sim (captura integrada) | Planejado (export PCAP) |
| Extensibilidade | Limitada/fechada | Alta (API REST/controller) | Média | Planejada (plugins, Fase 7) |

Observação: as capacidades de GNS3 e EVE-NG dependem de imagens de terceiros que o usuário deve obter por conta própria. Não as tratamos como parte do produto.

## 2. Cisco Packet Tracer

**Pontos fortes (documentados):**
- Duas formas de operação: Realtime e Simulation (com relógio controlável e PDUs).
- Inspeção de PDUs por camada, didática para iniciantes.
- Configuração por CLI e GUI nos equipamentos.
- Materiais de curso integrados (NetAcad).

**Limitações (documentadas ou de uso comum):**
- Modelos de protocolo simplificados; nem tudo corresponde ao equipamento real.
- Software fechado; extensão limitada.
- Exige conta para algumas funções.

**Pontos a estudar (sem cópia):** organização da tela, fluxo de criação de PDUs, uso do relógio de simulação.

## 3. GNS3

**Pontos fortes (documentados):** arquitetura separada (GUI, controller, compute); API HTTP/JSON; projetos com estado em disco; integração com máquinas virtuais e Docker; computes distribuídos.

**Lições para o NetForge:**
- A separação de controller e compute é um bom modelo para o futuro (Fase 7+), mesmo que no MVP tudo rode no mesmo processo.
- Estado do projeto em disco, separado da interface, facilita recuperação.
- API HTTP/JSON bem definida é um bom contrato para integrações e para testes.

**Limitações:** complexidade de instalação; dependência de imagens externas.

## 4. EVE-NG

**Pontos fortes (documentados):** organização de laboratórios com muitos nós; suporte a QEMU e (na edição Pro) Docker; acesso por navegador.

**Situação das edições (fontes conflitantes):**
- Uma fonte indica que a edição Community de 63 nós está descontinuada, sem atualizações.
- Outra fonte, do próprio site do fornecedor, ainda lista a Community como gratuita com 63 nós.
- Fontes de terceiros mencionam uma edição com 7 nós e a edição Pro com 1.024 nós.

**Conclusão:** os limites e preços exatos não são confiáveis a partir dessas fontes. Devem ser verificados em eve-ng.net antes de qualquer afirmação no produto.

**Lições para o NetForge:** gerenciar muitos nós exige organização de projeto clara; a camada de laboratório deve ser separada da de emulação.

## 5. React Flow vs canvas próprio (decisão de ferramenta)

Detalhe em [TECHNICAL_DECISIONS.md, ADR-003](TECHNICAL_DECISIONS.md#adr-003--canvas-react-flow-como-candidata-não-como-decisão-final).

## 6. Diferenciais

Hipóteses de como o NetForge pode ir além do Packet Tracer. Cada uma é avaliada em FEATURES.md §10.

| Diferencial | Por que pode ser superior | Risco | Viabilidade |
|---|---|---|---|
| Caminho do pacote com alterações de cabeçalho por salto | Torna o encaminhamento visível de forma didática | Custo de UI | Alta |
| Diagnóstico de ping com causas reais | Ajuda o aluno a entender falhas | Só cobre o que o motor modela | Média |
| Histórico com recuperação | Experimentar sem medo | Custo de snapshot | Média |
| Modo aprendizagem | Liga teoria à simulação | Conteúdo a escrever | Média |
| Motor determinístico e testável | Reprodutibilidade de aulas e avaliações | Disciplina de engenharia | Alta |
| Exportação PCAP | Integra com Wireshark, ferramenta que o usuário já conhece | Fidelidade do PCAP | Alta |

## 7. Conclusões

1. O espaço do NetForge não é "mais um clone do PT". A vantagem plausível está em **motor determinístico, rastreável e testável**, com **exportação e validação com Wireshark**.
2. GNS3 e EVE-NG mostram que separar interface, controller e compute é um caminho sólido — mas a emulação de imagens reais está fora do MVP.
3. Diferenciais como diagnóstico e aprendizagem só são viáveis depois que o motor modelar os protocolos corretamente.
