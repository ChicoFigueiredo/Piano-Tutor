#!/usr/bin/env bash
# Gera "Piano Tutor.app" universal (x86_64 + arm64), com assinatura ad-hoc e sem notarização
# (ADR-0014: macOS como plataforma de teste pessoal). Rode no próprio Mac.
#
# Uso: scripts/empacotar-app-macos.sh [pasta-de-saida]   (padrão: target/macos)
# Requer: Xcode ou Command Line Tools (clang, lipo, iconutil, sips, codesign) e rustup.
# Dependências de execução, fora do .app: brew install mpv (libmpv + mpv); piper em
# ~/.local/bin; MuseScore em /Applications. O SoundFont é baixado na primeira execução.
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
    echo "Este script só roda no macOS." >&2
    exit 1
fi

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$raiz"
saida="${1:-$raiz/target/macos}"
alvos=(x86_64-apple-darwin aarch64-apple-darwin)
nome_bin="piano-tutor-app"
app="$saida/Piano Tutor.app"
versao="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"

# Mínimo comum às duas arquiteturas; o Mac Home (Intel 2016) roda Sequoia via OCLP.
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

for alvo in "${alvos[@]}"; do
    rustup target add "$alvo" >/dev/null
    echo "==> cargo build --release --target $alvo"
    cargo build --release -p piano-tutor-app --target "$alvo"
done

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Frameworks"

lipo -create \
    "target/x86_64-apple-darwin/release/$nome_bin" \
    "target/aarch64-apple-darwin/release/$nome_bin" \
    -output "$app/Contents/MacOS/$nome_bin"
lipo -info "$app/Contents/MacOS/$nome_bin"

# Ícone: PNG 1024 versionado em pacote-macos/ -> .icns.
icones="$(mktemp -d)/piano-tutor.iconset"
mkdir -p "$icones"
png="pacote-macos/piano-tutor-1024.png"
for t in 16 32 128 256 512; do
    sips -z "$t" "$t" "$png" --out "$icones/icon_${t}x${t}.png" >/dev/null
    d=$((t * 2))
    sips -z "$d" "$d" "$png" --out "$icones/icon_${t}x${t}@2x.png" >/dev/null
done
iconutil -c icns "$icones" -o "$app/Contents/Resources/piano-tutor.icns"

sed "s/@VERSAO@/$versao/g" pacote-macos/Info.plist > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"

# Assinatura ad-hoc (sem conta de desenvolvedor). Na primeira abertura: botão direito > Abrir.
codesign --force --deep --sign - "$app"
codesign --verify --verbose=2 "$app"

echo "$app"
