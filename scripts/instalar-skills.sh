#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
command -v npx >/dev/null || { echo 'Node/npm com npx requerido.' >&2; exit 1; }
UPSTREAM='mattpocock/skills@abeeaf95660ba9e8b7c753eb38f441ca707a8823'
echo "ATENÇÃO: confirme a origem e a licença das skills antes de executar: $UPSTREAM"
npx --yes skills@latest add "$UPSTREAM" -a codex -a claude-code --copy -y --skill setup-matt-pocock-skills grill-with-docs to-spec to-tickets implement implement-spec tdd code-review prototype research diagnosing-bugs domain-modeling retro pr
echo 'Revise e versione as skills adicionadas: git status'
