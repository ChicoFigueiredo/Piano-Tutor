# ADR-0004 — Somente Linux na v1

**Status:** ACEITA — 2026-11-22 · **Substitui:** ADR-002 da especificação (§12, "Windows nativo primeiro; Linux desktop depois")

A v1 é desenvolvida, homologada e distribuída **somente em Linux**. O PC de referência é o computador **cerebro** (Kubuntu 26.04, kernel 7.0, Ryzen 7 3700X, 128 GB, RTX 4080 SUPER 16 GB), com o REVAS KB-330 em USB nativo (ALSA/PipeWire) e saída de áudio **onboard (Realtek da ASUS B550) via PipeWire**. O Windows sai do escopo: nenhum gate de F00, F01, F03 ou F14 exige Windows. O motivo é que o cerebro é a máquina de uso diário do titular, onde o teclado já está ligado. Validar dois sistemas operacionais em cada gate de hardware dobraria o custo do caminho crítico sem trazer valor pedagógico na v1.

**Consequências:** o gate de F00 passa a ser "a `bancada-casca` compila e abre nativamente no Linux (X11/Wayland do KDE)". A distribuição é feita por pacote `.deb` (ver [ADR-0005](ADR-0005-marcos-e-ordem.md)). Suporte a Windows volta só por ADR nova.
