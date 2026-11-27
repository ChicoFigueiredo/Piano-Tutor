# ADR-0016 — Licença do código: MIT OR Apache-2.0

**Status:** ACEITA — 2026-11-27 (decisão do titular) · **Complementa:** [ADR-0002](ADR-0002-shell.md) e [ADR-0015](ADR-0015-edicao-nativa-e-plataforma-musical.md)

O código do Piano Tutor é distribuído sob **MIT OR Apache-2.0**, a mesma licença da bancada-casca. Os textos estão em `LICENSE-MIT` e `LICENSE-APACHE` na raiz, e o campo `license` está em `[workspace.package]` do `Cargo.toml`. A distribuição é gratuita e de código aberto.

Dependências com licença forte de cópia não entram no binário do app. Audiveris e HOMR (AGPL-3.0) e Piper (GPL-3.0 na linha atual `piper1-gpl`; a linha antiga `rhasspy/piper` era MIT) só rodam como **processos externos**, invocados pelo app por linha de comando ou protocolo local, com aviso e fonte publicadas. O Verovio (LGPL-3.0) nunca é linkado: fica só como oráculo de comparação em processo separado, conforme [ADR-0015](ADR-0015-edicao-nativa-e-plataforma-musical.md).

**Por quê:** a bancada já é MIT OR Apache-2.0, e manter a mesma licença evita conflito entre o shell e o app e permite reuso pelos dois lados. Limitar as dependências copyleft a processos separados preserva a liberdade de licenciar o código próprio sem exigir que o app inteiro seja AGPL ou GPL.

**Consequências:**

- Cada contribuição de código fica sob MIT OR Apache-2.0.
- Toda nova dependência com licença AGPL, GPL ou LGPL exige ADR ou registro explícito antes de entrar, e o `cargo deny` na CI verifica as licenças antes de novas dependências de providers, MCP e OMR.
- Não se copia código GPL para o projeto, mesmo traduzido. Referências funcionais são permitidas.
- A licença de cada modelo, fonte e voz de terceiros (por exemplo, Bravura OFL-1.1 e a voz Piper) é conferida à parte e registrada na documentação de fontes.
- Trocar a licença do projeto exige nova ADR e aprovação do titular.
