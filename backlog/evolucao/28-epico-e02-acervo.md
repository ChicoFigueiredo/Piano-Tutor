# 28: Épico E02: Acervo

**Fase:** E02

**R-IDs/critérios:** R-B01, R-B02, R-N01, T2, D4, D7

**Seam:** S2

**Modelo sugerido:** Opus / alto para o to-tickets do épico; os tickets filhos recebem modelo e effort próprios quando forem criados

**What to build:** **Épico (não quebrar agora).** Objetivo demonstrável: importar uma pasta mista (PDF, MusicXML, MXL) com deduplicação por hash, achar uma partitura por título e letra com acento, reiniciar o app e ver tudo persistido, restaurar um backup e conferir hashes iguais.

**Decisões já tomadas na especificação:** SQLite com escada de migrações e rollback; um único serviço dono da escrita; cópia imutável por SHA-256 (D4); metadados desconhecidos nulos explícitos; busca de texto completo com normalização de acentos; Versões ligadas à partitura (o armazenamento de Versões da E01 migra para cá); backup com manifesto; verificar conflito de versões do driver SQLite com o armazém existente antes de adotar; uma única estratégia de migração (sem dois bancos); importação em lote com fila simples em processo (a fila durável é da E05).

Este ticket é um marcador de fase: quando seus bloqueios fecharem, o orquestrador roda `to-tickets` sobre ele e gera as fatias verticais (prefactors primeiro) antes de qualquer implementação.

**Blocked by:** 27 (Gate de encerramento da E01: matriz de rastreabilidade, ADRs e demonstração)

**Status:** ready-for-agent

- [ ] Dado um lote de 100 arquivos mistos com duplicatas, importar deixa uma cópia por hash e rejeita os malformados com motivo, sem travar.
- [ ] Dada a busca 'coração' sobre texto 'coracao', o resultado aparece (normalização de acentos).
- [ ] Dado um backup, restaurar em Acervo vazio dá hashes coincidentes; dada uma migração, o rollback restaura o esquema anterior.
- [ ] Com 10 mil metadados sintéticos, o p95 de busca é registrado e comparado à meta 'a calibrar' de 250 ms (sem rebaixar em silêncio).
- [ ] Gate da fase: critérios em CI, aceite do titular, sem material comprado em fixtures.
- [ ] NÃO VERIFICADO sem hardware: desempenho no PC de referência com disco real e Acervo grande; macOS.
- [ ] Este épico só é considerado pronto quando seus tickets filhos (a criar) estiverem aceitos e o titular registrar o aceite humano da fase.
