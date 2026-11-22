# Fluxo de trabalho com agentes

1. Leia `AGENTS.md`, especificação integral e a ficha da fase no backlog.
2. Use as skills de Matt Pocock para questionamento, especificação, decomposição, TDD, implementação e revisão quando estiverem instaladas.
3. Crie testes reproduzíveis antes da lógica. Não altere requisitos sem ADR ou decisão documentada.
4. Envie PR com critério → evidência; teste hardware somente na máquina onde o dispositivo está disponível.
5. Coordenação valida especificação, arquitetura, licenças e compatibilidade; QA barra avanço sem prova.
6. Trabalhe sequencialmente. Atividade em paralelo só dentro da fase atual com contratos explícitos e testes independentes.

Ver [matriz](../aceite/MATRIZ-DE-ACEITE.md), [skills](../skills/MATT-POCOCK.md) e [especificação](../specs/ESPECIFICACAO-COMPLETA-v1.md).

## Orquestração no Claude

Decisão do titular (2026-11-22): **Opus orquestra e decide modelo e effort por ticket**.

1. **Opus (orquestrador)** lê a ficha da fase atual, fatia em tickets (`to-tickets`) e, para cada ticket, escolhe **modelo** (Opus, Sonnet ou Haiku) e **effort** (baixo, médio, alto) conforme o risco. A escolha e o motivo ficam registrados no próprio ticket.
   - Referência de calibração, não regra fixa: Haiku/baixo para formatação, docs, issues e checklists; Sonnet/médio–alto para núcleo Rust, MIDI, áudio, notação e worker de ML; Opus para áudio em tempo real, relógio mestre, contratos entre módulos e qualquer ticket que já falhou uma vez.
2. **Subagentes** (`.claude/agents/`) entregam cada ticket em worktree própria, test-first (`tdd`), e abrem um PR por ticket ou por grupo coeso.
3. **qa-aceite** audita critério → evidência e classifica PASS/FAIL/NÃO VERIFICADO ([ADR-0006](../decisoes/ADR-0006-regras-de-gate.md)). O Opus revisa o PR (`code-review`).
4. **Ensaios de hardware** (REVAS, latência, escuta) rodam no **cerebro**, nunca na nuvem. O titular assina o checklist de escuta e dá o aceite da fase.
