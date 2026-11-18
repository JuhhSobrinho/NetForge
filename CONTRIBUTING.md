# Contribuindo com o NetForge

## Princípios

1. **Originalidade.** Não copiar código, textos, assets ou saídas de comando do Cisco Packet Tracer ou de outro software proprietário.
2. **Comportamento, não cópia.** Estudo é feito por observação e documentação pública; a implementação segue RFCs e especificações abertas.
3. **Sem representação falsa.** Um recurso só é marcado como implementado quando há código, testes e validação. Ver [docs/PROTOCOLS.md](docs/PROTOCOLS.md).
4. **Motor independente da UI.** Regras de rede não entram em componentes React.
5. **Documentação acompanha o código.** Mudança de comportamento exige atualização em `docs/` e no `CHANGELOG.md`.

## Antes de começar

- Leia [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) e [docs/TECHNICAL_DECISIONS.md](docs/TECHNICAL_DECISIONS.md).
- Para mudanças de arquitetura, abra uma proposta e registre um novo ADR.

## Fluxo de trabalho

1. Crie uma branch a partir de `main`.
2. Escreva ou atualize testes antes da implementação quando possível.
3. Garanta que os testes passam localmente.
4. Atualize a documentação afetada.
5. Abra um pull request descrevendo o motivo e os testes executados.

## Padrões

- **Rust:** `cargo fmt` e `cargo clippy` sem avisos novos.
- **TypeScript:** tipagem explícita em fronteiras (IPC, props públicas). Sem `any` sem justificativa.
- **Commits:** em inglês, no imperativo, curtos (`add ARP request handling`).
- **Idioma da documentação:** português.

## Pesquisa e engenharia reversa

- Registre cada observação com cenário, resultado e rótulo (`[OBSERVADO]`, `[DOCUMENTADO]`, `[HIPÓTESE]`). Ver [docs/REVERSE_ENGINEERING.md](docs/REVERSE_ENGINEERING.md).
- Não contorne licenciamento nem analise binários de software de terceiros.
- Não use recursos internos não públicos.

## Dependências

Antes de adicionar uma dependência, justifique no pull request: o que resolve, alternativas consideradas e licença. Evite dependências desnecessárias.
