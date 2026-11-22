#!/usr/bin/env bash
# Gera o pacote .deb mínimo do app Olá Piano Tutor (F03, ticket #40).
# Uso: scripts/empacotar-deb.sh   (requer cargo-deb: cargo install cargo-deb --locked)
set -euo pipefail

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$raiz"

if ! command -v cargo-deb >/dev/null 2>&1; then
    echo "cargo-deb não encontrado. Instale com: cargo install cargo-deb --locked" >&2
    exit 1
fi

cargo deb -p piano-tutor-app

# cargo-deb grava o pacote em target/debian/.
pacote="$(ls -t "$raiz"/target/debian/piano-tutor_*.deb 2>/dev/null | head -n 1 || true)"
if [ -z "$pacote" ]; then
    echo "Nenhum .deb encontrado em target/debian/." >&2
    exit 1
fi
echo "$pacote"
