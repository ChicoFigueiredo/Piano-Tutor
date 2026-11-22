# ADR-0002 — Reutilização da bancada-casca

**Status:** ACEITA — 2026-11-21

UI em Rust com [`bancada-casca`](https://github.com/ChicoFigueiredo/bancada) (egui/eframe). Adotar API pública `AppCasca`, `Casca`, `Aba`, `Acao`, `Grupo` e `PainelDock`; usar dependência git revisionada e acesso autorizado ao repositório privado na CI. Não duplicar Ribbon do Chess Studio. Integrar após laboratório F00, conforme dependências disponíveis.
