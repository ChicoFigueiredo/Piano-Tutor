# Licenças, vídeos e dados

- Repositório do Piano Tutor: não publicar sob licença aberta sem decisão explícita do titular.
- Revisar cada biblioteca, fonte, SoundFont SF2/SFZ, sample, dataset e modelo ML separadamente. O projeto ser local e gratuito não elimina obrigações de redistribuição e atribuição.
- Não subir vídeos de curso comprado, gravações de terceiros, partituras protegidas, samples comerciais nem pesos ML com termos restritos.
- Tratar modelos não comerciais como bloqueados para redistribuição/comercialização até validação jurídica.
- Não armazenar arquivos de vídeo brutos no repositório; salvar caminhos locais e hashes, com consentimento e política de retenção.
- Segredos e tokens em cofre ou variáveis de ambiente, nunca em commits. Logs de diagnóstico devem ser sanitizados.
- Manter rastreabilidade entre vídeo, trecho, algoritmo/versão, hipótese, confiança, correção humana e versão final da lição.

## Dependências do `piano-tutor-diag` (F00)

Versões fixadas com `=` em `crates/piano-tutor-diag/Cargo.toml`; licenças conferidas em `cargo info` (2026-11-22). Dependência de sistema (Linux): `libasound2-dev` (ALSA, LGPL-2.1+, ligação dinâmica, não redistribuída no repositório).

| Crate | Versão | Licença |
| --- | --- | --- |
| `midir` | 0.11.1 | MIT |
| `cpal` | 0.18.2 | Apache-2.0 |
| `tracing` | 0.1.44 | MIT |
| `tracing-subscriber` | 0.3.23 | MIT |

## Dependências do `piano-tutor-app` (F00)

| Dependência | Versão | Licença |
| --- | --- | --- |
| `bancada-casca` (git, `ChicoFigueiredo/bancada`) | rev `94818fd95dc74b5a42a7757b1d258ed17822ed06` | MIT OR Apache-2.0 (declarada no `Cargo.toml` do repositório `bancada`, que é público) |

A `bancada-casca` traz egui/eframe 0.36.2 (glow, X11 e Wayland), `egui_tiles`, `fluent-bundle`, `serde` e suas transitivas; as licenças dessas dependências **não foram auditadas uma a uma: NÃO VERIFICADO** (usar `cargo deny` na CI). Os arquivos `tokens.json` e `temas.json` do app são próprios do Piano Tutor (não copiados da `bancada`). Dependências de sistema (Linux, ligação dinâmica, não redistribuídas): `libx11-dev`, `libxkbcommon-dev`, `libwayland-dev`, `libgl1-mesa-dev`, `libxcursor-dev`, `libxrandr-dev`, `libxi-dev`.

As dependências transitivas do `piano-tutor-diag` (alsa, alsa-sys, libc etc.) também não tiveram a licença auditada uma a uma: **NÃO VERIFICADO**; recomenda-se `cargo deny`/`cargo license` na CI.
