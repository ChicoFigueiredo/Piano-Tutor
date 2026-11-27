# E00 — Decisões do titular (grill-with-docs, 2026-11-27)

Decisões tomadas por Chico em cards. Prevalecem sobre a proposta em `docs/evolucao-musical/` e sobre recomendações das pesquisas.

| # | Tema | Decisão |
|---|---|---|
| 1 | Primeira fatia após E00 | Corrigir as partituras já convertidas: PDF e partitura lado a lado, editar, salvar como Versão, praticar. Editor antes de OMR novo e antes do catálogo completo. |
| 2 | Escopo do editor v1 | Piano em duas pautas + lead sheet (melodia, letra, cifra). Multi-partes depois. |
| 3 | IA na nuvem com material comprado | Só com consentimento por lote; padrão local; registrar o que saiu. |
| 4 | Importação no Acervo | Copia o arquivo para o Acervo, imutável, identificado por SHA-256. |
| 5 | Gabarito do benchmark | Corpus público com MusicXML (ex.: OpenScore Lieder CC0), PDFs e scans degradados gerados dele; Chico confere só uma amostra. |
| 6 | Provedores de IA | Claude, OpenAI, Gemini e Ollama desde a fase do professor inteligente. |
| 7 | Termo do catálogo | Acervo (não Biblioteca). |
| 8 | Estado salvo | Versão (não "revisão"). |
| 9 | Distribuição | Código aberto gratuito. |
| 10 | Licença do código | MIT OR Apache-2.0. |
| 11 | Chat de IA | MCP primeiro (Chico usa Claude Code ou outro cliente); chat dentro do app na fase seguinte. |
| 12 | Hardware de reconhecimento | CPU obrigatório; GPU só acelera. |
| 13 | Letra, cifra, dedilhado na 1ª fatia | Mostrar e preservar; editar na fatia seguinte. |
| 14 | Seta ↑/↓ | Semitom (perfil MuseScore); Alt+Shift+seta pela escala. Atalhos remapeáveis. |
| 15 | Plugins Luau v1 | Só assinados pelo titular. |
| 16 | Audiveris e HOMR (AGPL) | Empacotados junto, cada um como processo separado, com avisos e fontes AGPL. App funciona sem eles. Empacotamento segue adiado para 2027. |
| 17 | PDFtoMusic Pro | Não usar; benchmark só com ferramentas abertas. |
| 18 | Teto de IA na nuvem | US$ 10/mês, parada obrigatória; Ollama local sem limite. |

Decisões de engenharia do supervisor (reversíveis) estão em [SUPERVISAO.md](SUPERVISAO.md).

## Roadmap reordenado (proposta para to-spec)

1. **E00** — esta fase: diagnóstico, pesquisas, decisões, spec revisada, ADR-0015, backlog.
2. **E01 Oficina de correção** — modelo editável com IdNota e preservação do XML, render egui + Bravura (fecha o spike da ADR-0007), PDF ao lado, alterar altura/duração, inserir/remover nota/pausa/acorde, desfazer/refazer, autosave, salvar Versão, praticar a Versão. Letra/cifra/dedo exibidos e preservados.
3. **E02 Acervo** — SQLite com migrações, cópia por hash, importação em lote, busca FTS.
4. **E03 Editor completo** — letra, cifra, dedilhado, ties, slurs, tercinas, vozes, repetições; perfil de atalhos MuseScore.
5. **E04 REVAS no editor** — step-time, acorde, re-pitch, gravação com quantização não destrutiva.
6. **E05 Reconhecimento** — Audiveris + copista/HOMR como processos, roteamento vetor/scan, divergências na Oficina; benchmark no corpus público.
7. **E06 MCP somente leitura → escrita com dry-run/diff/aprovação.**
8. **E07 Professor inteligente** — harness própria, 4 provedores, teto US$ 10/mês.
9. **E08 Plugins** (Luau assinado, Python isolado). **E09 Produção** (só pesquisa/spike).
