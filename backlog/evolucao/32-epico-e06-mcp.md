# 32: Épico E06: MCP

**Fase:** E06

**R-IDs/critérios:** R-A01, R-A02, T3

**Seam:** S3 (protocolo MCP em stdio); S1; S2

**Modelo sugerido:** Opus / alto para o to-tickets do épico; os tickets filhos recebem modelo e effort próprios quando forem criados

**What to build:** **Épico (não quebrar agora).** Objetivo demonstrável: um cliente MCP independente lista partituras e Versões, lê compasso, busca e dispara um Job local; depois escreve por patch.

**Decisões já tomadas na especificação:** binário separado com runtime assíncrono confinado a ele; rmcp oficial com versão fixada (3.x.y); transporte stdio; resources (Acervo, partitura, Versão) e tools (busca, compasso, análise, comparação, status de Job); erros estáveis; descoberta de capacidades; escrita só numa segunda etapa: patch tipado, dry-run, diff, aprovação humana e bloqueio otimista por hash da Versão; a escrita sempre cria nova Versão; verificação de dependências (ticket 01) antes de adotar o SDK.

Este ticket é um marcador de fase: quando seus bloqueios fecharem, o orquestrador roda `to-tickets` sobre ele e gera as fatias verticais (prefactors primeiro) antes de qualquer implementação.

**Blocked by:** 27 (Gate de encerramento da E01: matriz de rastreabilidade, ADRs e demonstração), 28 (Épico E02: Acervo), 01 (Auditoria de dependências e licenças com cargo deny na CI)

**Status:** ready-for-agent

- [ ] Dado cliente independente, listar e consultar funciona.
- [ ] Dado patch sobre Versão desatualizada, ele é recusado.
- [ ] Dado caminho externo ou argumento inválido, é recusado.
- [ ] Dado app fechado ou MCP encerrado, o outro lado segue.
- [ ] Gate da fase: E2E com cliente independente; auditoria registrada; aceite do titular.
- [ ] NÃO VERIFICADO sem hardware: o fluxo com o Claude Code do titular até ele testar; macOS.
- [ ] Este épico só é considerado pronto quando seus tickets filhos (a criar) estiverem aceitos e o titular registrar o aceite humano da fase.
