#!/usr/bin/env python3
"""Verifica a integridade mínima do bootstrap sem instalar dependências."""
from pathlib import Path
import sys

root = Path(__file__).resolve().parent.parent
expected = ["README.md", "AGENTS.md", "CLAUDE.md",
            "docs/specs/ESPECIFICACAO-COMPLETA-v1.md",
            "docs/aceite/MATRIZ-DE-ACEITE.md",
            "Cargo.toml", "crates/piano-tutor-core/src/lib.rs"]
expected += [f"backlog/fases/F{i:02}.md" for i in range(15)]
missing = [p for p in expected if not (root/p).is_file()]
full = (root/"docs/specs/ESPECIFICACAO-COMPLETA-v1.md").read_text(encoding="utf-8") if not missing else ""
for i in range(15):
    if f"### Fase {i} —" not in full:
        missing.append(f"especificacao: Fase {i}")
if missing:
    print("FALHA: arquivos/fases ausentes: " + ", ".join(missing), file=sys.stderr)
    sys.exit(1)
print("OK: documentação, fases F00-F14 e bootstrap presentes")
