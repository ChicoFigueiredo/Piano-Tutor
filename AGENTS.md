# Instruções de agentes — Piano Tutor

Leia antes de começar: `README.md`, `CONTEXT.md`, `docs/planejamento/PLANO-DE-MARCOS.md`, `docs/decisoes/ADR-0004..0008`, `docs/specs/ESPECIFICACAO-COMPLETA-v1.md`, `docs/aceite/MATRIZ-DE-ACEITE.md`, `docs/equipe/WORKFLOW.md` e a ficha `backlog/fases/Fxx.md` atual.

## Mandatos

- **Sempre PT-BR** em documentação, UX, critérios e relatório. Identificadores de código preferencialmente em português sem acentos, mantendo padrões de interoperabilidade (MIDI, MusicXML) e nomes das crates de terceiros.
- **Uma fase por vez**: começar F00 (provar ambiente, `bancada-casca` e dispositivo) antes de F01. A **ordem de execução segue os marcos** (M1–M5) do `PLANO-DE-MARCOS.md`, não a numeração. Aceite humano obrigatório.
- **Nenhum falso positivo**: testes não executados = NÃO VERIFICADO; sem REVAS físico = não homologar MIDI real; sem áudio medido = não homologar latência.
- **Não substituir arquitetura:** Rust para app/áudio/MIDI; `bancada-casca` para shell; Python somente worker opcional de ML; Luau somente hipótese de regras pedagógicas após F13.
- **Núcleo musical isolado** de GUI, sistema operacional, Python e hardware, testado por unidade.
- **Não copiar** código GPL, modelos, vídeos, fonts e SoundFonts de terceiros sem controle de licença e permissão. Não commitar mídia de curso.
- **Não inventar** disponibilidade MIDI ou recursos de velocity/sustain; calibrar KB-330 real.
- **Política de mudança:** specs e gates só mudam com PR/ADR, justificativa e aprovação humana. Não remover checkboxes para obter PASS.
- **Processo:** teste antes de implementar, spike quando a dependência for incerta; toda PR descreve comportamento demonstrável, evidência, regressão e riscos.
- **Segredos:** sem chaves, credenciais, dados pessoais, modelos protegidos ou mídia não licenciada em commits.

## Fluxo Matt Pocock quando as skills estiverem instaladas

`grill-with-docs` → `to-spec` (somente alterações novas) → `to-tickets` → `implement` → `tdd` → `code-review` → `retro`; `prototype` e `research` para spikes. Use `implement-spec` só quando houver tickets bloqueantes mapeados e o gate da fase permitir paralelismo. O README e as fichas atuais **já são specs**, não gere outra especificação conflitante.

## Definição de pronto

Comportamento demonstrável + testes automatizados adequados + ensaio real quando requerido + evidência + licenças + documentação + revisão humana. A CI verde não equivale a aceite de fase.