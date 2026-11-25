#!/usr/bin/env bash
# Refaz, assina e instala um Pacote de Curso a partir da oficina do tutor (ADR-0009).
# Rode depois de aprovar partituras/falas na tela "Revisão do tutor".
#
# Uso: scripts/atualizar-pacote-curso.sh [tutor] [plano]
#   tutor  pasta em ~/.local/share/piano-tutor/fontes/ (padrão: luciano-alves)
#   plano  plano de importação (padrão: <fontes>/<tutor>/plano-*.toml, o primeiro)
# Variáveis: CHAVE (padrão ~/.config/piano-tutor/chaves/curso.chave),
#            SOMENTE_APROVADAS=1 para deixar fora do pacote tudo que não foi aprovado.
#
# Nada aqui entra no git: oficina, pacote e chave privada ficam fora do repositório.
set -euo pipefail

TUTOR="${1:-luciano-alves}"
FONTES="${XDG_DATA_HOME:-$HOME/.local/share}/piano-tutor/fontes/$TUTOR"
PLANO="${2:-$(ls "$FONTES"/plano-*.toml 2>/dev/null | head -n1)}"
CHAVE="${CHAVE:-$HOME/.config/piano-tutor/chaves/curso.chave}"
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"

[ -f "$PLANO" ] || { echo "Plano de importação não encontrado em $FONTES" >&2; exit 1; }
[ -f "$CHAVE" ] || { echo "Chave privada não encontrada: $CHAVE (gere com: piano-tutor-curso chave-gerar ~/.config/piano-tutor/chaves)" >&2; exit 1; }

cargo build --release --offline -p piano-tutor-curso --manifest-path "$RAIZ/Cargo.toml" >/dev/null
BIN="$RAIZ/target/release/piano-tutor-curso"

ORIGEM="$FONTES/pacote-origem"
rm -rf "$ORIGEM"
EXTRA=()
[ "${SOMENTE_APROVADAS:-0}" = "1" ] && EXTRA+=(--somente-aprovadas)
"$BIN" importar --plano "$PLANO" --oficina "$FONTES/oficina" -o "$ORIGEM" "${EXTRA[@]}"

ID="$(sed -n 's/^id *= *"\(.*\)"/\1/p' "$ORIGEM/manifesto.toml" | head -n1)"
PACOTE="$FONTES/$ID.ptcurso"
"$BIN" empacotar "$ORIGEM" --chave "$CHAVE" -o "$PACOTE"
"$BIN" verificar "$PACOTE"
"$BIN" instalar "$PACOTE"
echo "Pronto: reabra a tela Cursos no Piano Tutor."
