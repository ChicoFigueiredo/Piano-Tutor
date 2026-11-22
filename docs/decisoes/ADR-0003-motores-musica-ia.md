# ADR-0003 — Música, áudio e inteligência artificial

**Status:** PROPOSTA PARA VALIDAÇÃO POR EXPERIMENTO — 2026-11-21

Rust (midir, rustysynth, cpal; avaliar Kira) é responsável pelos eventos e áudio interativo. MusicXML é canônico para notação; MIDI documenta execução. Python é executado em worker opcional e isolado para ML multimodal. Luau, se adotado, fica restrito a regras pedagógicas. Toda biblioteca e modelo exige teste, revisão de licença, versões fixadas e justificativa antes de virar dependência permanente.
