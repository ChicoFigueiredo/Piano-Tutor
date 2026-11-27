# Piano Tutor — evolução musical agentiva
**Proposta de evolução autorizada pelo titular em 27/11/2026.** Baseline main: 222b5136fafbdb0d72b1c6eae666ab63d5022a88.
O produto evolui de professor assistido para estação de trabalho musical local-first: importar, catalogar, editar, estudar, compor e produzir. O titular determinou editor nativo Rust, não incorporar um editor web ou copiar código incompatível. Referências abertas servem para pesquisa de padrões, interação e algoritmos.
A restrição de somente leitura do ADR-0007 deve ser revista e substituída no escopo da evolução; AGENTS.md e specs/gates conflitantes devem ser reconciliados em PR. A decisão de produto está tomada; alternativas técnicas e metas propostas devem ser contestadas com evidência. Não cancelar gates físicos anteriores, nem chamar conteúdo não revisado de gabarito.
## Navegação
- [01 — requisitos e arquitetura](01-REQUISITOS.md)
- [02 — histórias, aceite e sequência](02-ROADMAP-ACEITE.md)
- [03 — pesquisa independente e benchmark](03-PESQUISA-BENCHMARK.md)
- [04 — definição de pronto e governança](04-DEFINICAO-DE-PRONTO.md)
- [ADR proposta de evolução](ADR-EVOLUCAO.md)
- [Prompt para a equipe](PROMPT-EQUIPE.md)
## Evidência de partida
Na baseline, verificar Cargo.toml, crates/piano-tutor-core/src/partitura.rs, crates/piano-tutor-app/src/{partitura_visual,tela_partitura,professor,professor_falante,revisao,roteiro_aula}.rs e docs/arquitetura/CONVERSAO-DE-PARTITURAS.md. O último relata 95 PDFs transcritos com Claude Sonnet, Poppler, Audiveris e Python, aproximadamente 74 com saída comparável, 21 sem saída útil; nenhuma revisão humana. Concordância de altura não equivale a precisão global. O leitor Rust documenta que ignora letras e tem outras limitações (ornamentos, transposição, octave-shift); não confundir suporte do formato com suporte do app. O professor atual é roteiro/Piper e há Luau via mlua; não foi encontrado MCP/LLM no workspace inspecionado. Revalidar tudo no HEAD antes de editar.
