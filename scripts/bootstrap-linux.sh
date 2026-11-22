#!/usr/bin/env bash
# Bootstrap do Piano Tutor para Linux (Kubuntu 26.04 / Ubuntu-like com apt).
# Padrão: apenas checa. Com --sim: instala os pacotes apt ausentes.
set -euo pipefail

PACOTES=(build-essential pkg-config libasound2-dev)
INSTALAR=0

if [[ "${1:-}" == "--sim" ]]; then
    INSTALAR=1
elif [[ $# -gt 0 ]]; then
    echo "Uso: $0 [--sim]" >&2
    echo "  (sem argumento) apenas checa; --sim instala os pacotes via apt." >&2
    exit 2
fi

# 1. Plataforma
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "ERRO: este bootstrap é somente para Linux (detectado: $(uname -s))." >&2
    exit 1
fi
echo "OK: sistema Linux detectado."

# 2. Pacotes apt
if ! command -v apt-get >/dev/null 2>&1 || ! command -v dpkg >/dev/null 2>&1; then
    echo "AVISO: apt/dpkg não encontrado; instale manualmente: ${PACOTES[*]}"
else
    ausentes=()
    echo "Pacotes apt necessários:"
    for pacote in "${PACOTES[@]}"; do
        if dpkg -s "$pacote" >/dev/null 2>&1; then
            echo "  [ok]       $pacote"
        else
            echo "  [ausente]  $pacote"
            ausentes+=("$pacote")
        fi
    done

    if [[ ${#ausentes[@]} -eq 0 ]]; then
        echo "OK: todos os pacotes apt estão instalados."
    elif [[ $INSTALAR -eq 1 ]]; then
        echo "Instalando via apt: ${ausentes[*]}"
        sudo apt-get update
        sudo apt-get install -y "${ausentes[@]}"
        echo "OK: pacotes instalados."
    else
        echo "PENDENTE: faltam ${#ausentes[@]} pacote(s). Para instalar, rode: $0 --sim"
    fi
fi

# 3. Rust: rustup, cargo e versão exigida pelo rust-toolchain.toml
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if command -v rustup >/dev/null 2>&1; then
    echo "OK: rustup encontrado ($(rustup --version 2>/dev/null | head -n1))."
else
    echo "AVISO: rustup não encontrado. Instale em https://rustup.rs"
fi

if command -v cargo >/dev/null 2>&1; then
    echo "OK: cargo encontrado ($(cargo --version))."
else
    echo "AVISO: cargo não encontrado."
fi

if [[ -f "$RAIZ/rust-toolchain.toml" ]]; then
    canal="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$RAIZ/rust-toolchain.toml" | head -n1)"
    echo "Toolchain exigida (rust-toolchain.toml): ${canal:-não identificada}"
else
    echo "AVISO: rust-toolchain.toml não encontrado em $RAIZ."
fi

# 4. PipeWire (opcional; nunca falha o script)
if command -v pactl >/dev/null 2>&1 && pactl info >/dev/null 2>&1; then
    echo "OK: PipeWire/PulseAudio ativo (pactl info respondeu)."
elif command -v pw-cli >/dev/null 2>&1 && pw-cli info 0 >/dev/null 2>&1; then
    echo "OK: PipeWire ativo (pw-cli info 0 respondeu)."
else
    echo "AVISO: PipeWire não detectado (pactl/pw-cli indisponíveis ou inativos). Opcional nesta etapa."
fi

echo "Concluído."
