#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
command -v gh >/dev/null || { echo "Instale/autentique GitHub CLI"; exit 1; }
for f in backlog/fases/F*.md; do
  title="$(head -n 1 "$f" | sed 's/^# //')"
  if gh issue list --state all --search "$title in:title" --json title --jq '.[].title' | grep -Fxq "$title"; then
    echo "Já existe: $title"; continue
  fi
  gh issue create --title "$title" --body-file "$f"
done
