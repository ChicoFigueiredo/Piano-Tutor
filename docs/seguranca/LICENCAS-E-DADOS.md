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

As dependências transitivas (alsa, alsa-sys, libc etc.) não tiveram a licença auditada uma a uma: **NÃO VERIFICADO**; recomenda-se `cargo deny`/`cargo license` na CI.

## Dependências do `piano-tutor-midi` (F01, ticket #32)

Versões fixadas com `=` em `crates/piano-tutor-midi/Cargo.toml`; licenças conferidas na API do crates.io (2026-11-22).

| Crate | Versão | Licença | Uso |
| --- | --- | --- | --- |
| `midir` | 0.11.1 | MIT | entrada MIDI (ALSA) |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | **somente dev-dependency**: validar o NDJSON nos testes |

O log NDJSON de sessão (`sessao-midi-<segundos-unix>.ndjson`) grava apenas timestamp monotônico, tipo, canal, nota, velocidade/valor e nomes de porta sanitizados (`piano_tutor_core::sanitizar`); bytes de SysEx e de mensagens desconhecidas não são gravados. Transitivas de `serde_json` não auditadas uma a uma: **NÃO VERIFICADO**.
