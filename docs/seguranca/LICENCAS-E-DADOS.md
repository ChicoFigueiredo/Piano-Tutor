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

## Dependências do `piano-tutor-audio` (F03, ticket #37)

Versões fixadas com `=` em `crates/piano-tutor-audio/Cargo.toml`; licenças conferidas em `cargo info` (2026-11-22).

| Crate | Versão | Licença | Uso |
| --- | --- | --- | --- |
| `cpal` | 0.18.2 | Apache-2.0 | Saída de áudio (mesma versão do `piano-tutor-diag`) |
| `rustysynth` | 1.3.8 | MIT | Sintetizador SF2, Rust puro, sem dependências |
| `rtrb` | 0.4.1 | MIT OR Apache-2.0 | Fila SPSC sem lock entre controle e callback, sem dependências |

Transitivas novas: nenhuma além das do `cpal` (alsa, alsa-sys, libc, dasp_sample, bitflags, cfg-if), já listadas como **NÃO VERIFICADO** acima.

## SoundFonts

- **Nenhum SF2 é versionado nem embutido.** Testes na nuvem usam um SF2 **sintético** gerado em memória pelo próprio código (`crates/piano-tutor-audio/src/sf2_sintetico.rs`: onda quadrada e senoide calculadas, sem amostras de terceiros).
- O teste de integração com SF2 real só roda com `PT_SF2` apontando para um arquivo instalado no sistema.
- **FluidR3_GM** (`/usr/share/sounds/sf2/FluidR3_GM.sf2`, pacote Debian main `fluid-soundfont-gm` 3.1-5.3): licença MIT, © 2000-2002, 2008 Frank Wen e © 2008 Toby Smithe (conforme `/usr/share/doc/fluid-soundfont-gm/copyright`). Decisão de empacotamento: o `.deb` declara `Depends: fluid-soundfont-gm` e **não** embute o SF2. Escolha final do SF2 e auditoria: ticket #36.
- `docs/aceite/F03/f03-gm000-piano-c4.wav` e `f03-gm021-acordeon-c4.wav` são renders curtos (2 s, mono, 16 bits, 44,1 kHz) feitos a partir do FluidR3_GM, só para escuta e checklist; atribuição MIT acima, aplicável a essas cópias.
