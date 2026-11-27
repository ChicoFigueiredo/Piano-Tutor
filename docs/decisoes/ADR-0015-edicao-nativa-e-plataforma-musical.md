# ADR-0015 — Edição nativa e plataforma musical

**Status:** ACEITA — 2026-11-27 (decisão do titular) · **Substitui:** [ADR-0007](ADR-0007-partitura-somente-leitura.md) · **Detalha:** [ADR-EVOLUCAO](../evolucao-musical/ADR-EVOLUCAO.md) e [00-DECISOES](../evolucao-musical/e00/00-DECISOES.md)

A partitura deixa de ser somente leitura. O Piano Tutor ganha um **editor nativo** em Rust/egui, com layout próprio e a fonte SMuFL Bravura (OFL-1.1). Nenhum editor de terceiros é incorporado, e nenhum código GPL é copiado. O Verovio fica só como oráculo de comparação, em processo separado, nunca linkado. A edição começa pela 1ª fatia (E01): duas pautas de piano e lead sheet (melodia, letra e cifra). Toda edição é salva como **Versão** imutável. O original importado e a Hipótese nunca são sobrescritos.

O acesso por agentes começa com um **servidor MCP** separado (binário próprio), somente leitura, e só depois ganha escrita com dry-run, diff e aprovação (E06). O chat dentro do app vem depois, na fase do professor inteligente (E07). São quatro provedores de IA (Claude, OpenAI, Gemini e Ollama), com teto de US$ 10/mês para a nuvem; o Ollama local não tem limite. O reconhecimento de partitura (OMR) entra como processos opcionais (Audiveris e HOMR, ambos AGPL), com CPU obrigatória e GPU opcional. Plugins Luau só são aceitos se assinados pelo titular, no v1.

Regras que valem para todo o plano: a LLM fica fora do caminho crítico de áudio e MIDI. O app funciona offline, sem Python, Java ou LLM. Linux é a referência de homologação, e macOS é plataforma de teste pessoal conforme [ADR-0014](ADR-0014-macos-como-plataforma-de-teste-pessoal.md).

**Roadmap:**

| Fase | Entrega |
|---|---|
| E00 | Diagnóstico, pesquisas, decisões, spec revisada, esta ADR e backlog |
| E01 | Oficina de correção: modelo editável com IdNota e preservação do XML, render egui + Bravura, PDF ao lado, edição de altura e duração, inserir e remover nota, pausa e acorde, desfazer e refazer, autosave, salvar Versão e praticar a Versão |
| E02 | Acervo: SQLite com migrações, cópia por SHA-256, importação em lote, busca FTS |
| E03 | Editor completo: letra, cifra, dedilhado, ligaduras, ligaduras de expressão, tercinas, vozes, repetições; perfil de atalhos MuseScore |
| E04 | REVAS no editor: step-time, acorde, re-pitch, gravação com quantização não destrutiva |
| E05 | Reconhecimento: Audiveris e HOMR como processos, roteamento vetor/scan, divergências na Oficina, benchmark no corpus público |
| E06 | MCP somente leitura, depois escrita com dry-run, diff e aprovação |
| E07 | Professor inteligente: harness própria, 4 provedores, teto US$ 10/mês |
| E08 | Plugins: Luau assinado e Python isolado |
| E09 | Produção: somente pesquisa e spike |

**Por quê:** a proibição de edição do ADR-0007 foi feita para proteger o prazo de F07, e não para ser um limite permanente. O titular autorizou a edição como requisito prioritário. Um editor próprio, com preservação do XML e Versões imutáveis, mantém a licença do projeto limpa e o original sempre recuperável. Separar MCP, harness e domínio evita que um agente externo ou um provedor de IA sejam necessários para o app funcionar.

**Consequências:**

- O aceite de F07 (leitura, playback, navegação, segurança e exportação) continua como regressão, não como teto funcional.
- Nenhuma edição sobrescreve arquivo original ou Hipótese; toda mudança gera uma Versão nova.
- Audiveris e HOMR vão empacotados junto com o app (decisão 16), cada um como processo externo separado, com avisos e fontes AGPL. O app não depende deles para abrir ou editar.
- Empacotamento de OMR segue adiado para 2027.
- Qualquer nova dependência de editor ou de notação exige checagem de licença; código GPL ou LGPL não é linkado.
- Promover outra plataforma ou mudar o teto de IA exige nova ADR.
