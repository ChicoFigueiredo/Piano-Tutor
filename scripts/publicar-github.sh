#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
command -v git >/dev/null && command -v gh >/dev/null || { echo 'Git e gh necessários'; exit 1; }
gh auth status
if ! gh repo view ChicoFigueiredo/Piano-Tutor >/dev/null 2>&1; then
 echo 'Repositório não existe; crie-o na conta autorizada antes de continuar.' >&2; exit 1
fi
if [ ! -d .git ]; then git init -b main; fi
if ! git remote get-url origin >/dev/null 2>&1; then git remote add origin https://github.com/ChicoFigueiredo/Piano-Tutor.git; fi
git fetch origin main
git pull --ff-only origin main
printf '%s\n' 'Revise git status e use git add/commit/push conscientemente. Este script não força push.'
git status --short
