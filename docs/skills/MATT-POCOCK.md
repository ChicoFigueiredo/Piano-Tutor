# Skills Matt Pocock — adoção controlada

**Origem:** [mattpocock/skills](https://github.com/mattpocock/skills) — commit revisado `abeeaf95660ba9e8b7c753eb38f441ca707a8823` (2026-11-19). Skills são instruções de agentes, não dependências de execução Rust e não devem ser usadas sem inspeção das alterações upstream.

## Skills incorporadas ao repositório

Oito skills do commit de origem foram copiadas com recursos e metadados para `.agents/skills/` (Codex) e `.claude/skills/` (Claude Code): `setup-matt-pocock-skills`, `grill-with-docs`, `to-spec`, `to-tickets`, `implement-spec`, `tdd`, `code-review`, `research`. O texto da licença está em `third_party/mattpocock-skills/LICENSE`.

A instalação foi **por incorporação de arquivos via integração GitHub**, e não por execução de `npx`. As demais skills permanecem sugestões opcionais até serem instaladas.

## Fluxo recomendado

1. `grill-with-docs` e `research`: perguntas, evidência e arquitetura.
2. `to-spec`, `to-tickets`: especificação e recorte da fase.
3. `prototype` e `tdd`: experimento controlado e testes.
4. `implement-spec` ou `implement`: entrega da fase.
5. `code-review`, `diagnosing-bugs`, `retro`, `pr`: auditoria, correção e documentação.
6. `domain-modeling`: quando houver modelagem complexa de domínio musical.

Instalação via `scripts/instalar-skills.sh` ou `scripts/instalar-skills.ps1`, requer Node/npm, internet e execução local. O comando é oferecido como provisionamento e NÃO foi executado no ambiente desta conversa. Validar a sintaxe do CLI na máquina antes de depender dele. Para as oito listadas acima, os arquivos foram versionados na `main`; executar o `npx` é opcional apenas para instalação/atualização adicional, precedida de revisão.
