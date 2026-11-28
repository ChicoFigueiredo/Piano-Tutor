# Regras para agentes implementando tickets da evolução musical

- PT-BR. Leia AGENTS.md, CONTEXT.md, docs/specs/ESPECIFICACAO-EVOLUCAO-MUSICAL.md (só as seções do seu ticket) e o SEU ticket em backlog/evolucao/. Siga .claude/skills/tdd/SKILL.md: teste que falha primeiro (registre o red), depois green, depois refactor.
- Escopo = só o seu ticket. Nada de reescritas amplas. Não mude comportamento visível do app salvo o que o ticket pede; as 1053 testes atuais continuam passando.
- Antes de terminar: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (ou só os crates afetados se o ticket for só docs/CI). Se o ambiente não tiver deps: `scripts/bootstrap-linux.sh --sim`.
- Commit local no seu worktree, mensagem `feat(eNN): ...` ou `docs(...)`, terminando com:
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_013zAMcZP8dxwjbQkUaUSgWr
  NÃO faça push. Não mexa em backlog/evolucao/README.md nem em outros tickets; marque no SEU ticket os critérios cumpridos ([x]) e o status ("feito, aguarda revisão").
- Nunca marque PASS o que não rodou; hardware (REVAS/áudio/macOS) = NÃO VERIFICADO. Nada de material comprado do titular.
- Resposta final (≤10 linhas): branch/commit, critérios cumpridos, comandos e resultado, riscos.
