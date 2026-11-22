# ADR-0008 — Aulas de tutores só para uso pessoal local

**Status:** ACEITA — 2026-11-22 · **Complementa:** ADR-011 da especificação e [Licenças e dados](../seguranca/LICENCAS-E-DADOS.md)

Os vídeos de tutores especializados que alimentam F10–F13 são **apenas para uso pessoal** e ficam **somente no cerebro**. O repositório, a CI e os PRs guardam só hashes, metadados, métricas e Lições sem mídia (sem quadros, áudio ou trechos). O aceite de F10–F13 é feito localmente pelo titular, com evidência sanitizada. Os testes automatizados de CI usam fixtures sintéticas ou mídia gravada pelo próprio titular.

**Por quê:** não existe autorização escrita de redistribuição. Sem esta regra, os critérios de F10–F13 ficariam inverificáveis ou violariam a ADR-011.
