# 03: Anotar a ADR-0009 duplicada sem renomear

**Fase:** Infra (E00)

**R-IDs/critérios:** Decisão do supervisor (ADR-0009 duplicado: anotar, não renomear)

**Seam:** N/A (só docs)

**Modelo sugerido:** Haiku / baixo

**What to build:** Quem lê as ADRs encontra, em cada uma das duas ADR-0009, uma nota que explica a duplicidade e aponta para a outra, sem que nenhum arquivo ou link existente seja renomeado. A próxima numeração livre é registrada (ADR-0017, já que a 0015 e a 0016 existem).

**Blocked by:** None (can start immediately)

**Status:** feito na E00 (notas já adicionadas às duas ADR-0009; nada a implementar)

- [ ] Cada ADR-0009 tem uma nota de duplicidade com link para a irmã; nenhum arquivo foi renomeado ou movido.
- [ ] A verificação de documentação existente passa (links íntegros).
- [ ] O índice de decisões, se existir, lista as duas e indica a próxima numeração livre.

**Notas:** Pode ser feito junto do ticket 02 se o supervisor preferir um único PR de documentação, mas a matriz exige PR separado; manter separados por padrão.
