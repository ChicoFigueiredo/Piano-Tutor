# ADR-0006 — Regras de gate: NÃO VERIFICADO, metas experimentais e critérios subjetivos

**Status:** ACEITA — 2026-11-22 · **Complementa:** §13.1/§13.2 da especificação

1. **NÃO VERIFICADO bloqueia** sempre nos gates de hardware (F01, F02, F03, F14). Nas outras fases, um critério NÃO VERIFICADO só passa com justificativa escrita e aprovação explícita do titular.
2. **Meta experimental** (F11 F1 de onsets ≥ 0,75; F12 ≥ 80% mão+dedo+tecla): se não for atingida, a fase é liberada como **beta**. O conteúdo gerado continua sendo Hipótese e só vira Lição ou Gabarito depois de Aprovado por revisão humana.
3. **Bug crítico** = crash, nota presa, perda de dados ou latência acima da meta da fase. Uma fase não fecha com bug crítico aberto.
4. **Critérios auditivos** (timbre "reconhecivelmente diferente", estalos, clipping) são comprovados por um **checklist de escuta** assinado pelo titular e anexado à evidência da fase, e não por métrica automática.

**Por quê:** a spec exigia PASS/FAIL/UNVERIFIED sem dizer o que bloqueia, e vários critérios eram subjetivos. O aceite já é humano; esta ADR só o torna rastreável sem criar instrumentação que não ensina piano.
