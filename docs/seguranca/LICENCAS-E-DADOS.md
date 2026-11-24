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
As dependências transitivas (alsa, alsa-sys, libc etc.) não tiveram a licença auditada uma a uma: **NÃO VERIFICADO**; recomenda-se `cargo deny`/`cargo license` na CI.

## Dependências do `piano-tutor-midi` (F01, ticket #32)

Versões fixadas com `=` em `crates/piano-tutor-midi/Cargo.toml`; licenças conferidas na API do crates.io (2026-11-22).

| Crate | Versão | Licença | Uso |
| --- | --- | --- | --- |
| `midir` | 0.11.1 | MIT | entrada MIDI (ALSA) |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | **somente dev-dependency**: validar o NDJSON nos testes |

O log NDJSON de sessão (`sessao-midi-<segundos-unix>.ndjson`) grava apenas timestamp monotônico, tipo, canal, nota, velocidade/valor e nomes de porta sanitizados (`piano_tutor_core::sanitizar`); bytes de SysEx e de mensagens desconhecidas não são gravados. Transitivas de `serde_json` não auditadas uma a uma: **NÃO VERIFICADO**.
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

## Dependências do `piano-tutor-core` e partituras (F07)

| Item | Versão | Licença | Uso |
| --- | --- | --- | --- |
| `roxmltree` | 0.21.2 (fixada com `=`) | MIT OR Apache-2.0 (conferida em `cargo info`, 2026-11-22) | leitura de MusicXML; DTD permitida (os arquivos MusicXML trazem `DOCTYPE`), sem resolver entidades externas; limite de nós e de tamanho aplicado pelo próprio `piano-tutor-core` |
| `memchr` (transitiva) | já presente no `Cargo.lock` | MIT OR Unlicense | **NÃO VERIFICADO** uma a uma |

Partituras empacotadas em `assets/partituras/`: três lieder do **OpenScore Lieder Corpus** (CC0 1.0, obras em domínio público), com origem, commit e SHA-256 em `assets/partituras/LEIA-ME.md`. Nenhuma fonte musical (Bravura etc.) é embutida: a notação é desenhada com formas próprias no painter do egui.

## Dependências do `piano-tutor-core` para `.mxl` (F07)

Versões fixadas com `=` em `crates/piano-tutor-core/Cargo.toml`; licenças lidas do campo `license` de cada `Cargo.toml` baixado (2026-11-22). Todas Rust puro, sem ligação a biblioteca C. `zip` entra com `default-features = false` e só `deflate-flate2` (sem AES, bzip2, zstd, lzma, ppmd, time nem zopfli); o `flate2` é fixado diretamente só para ligar o backend `rust_backend` (miniz_oxide).

| Crate | Versão | Licença | Observação |
| --- | --- | --- | --- |
| `roxmltree` | 0.21.2 | MIT OR Apache-2.0 | (já existia) leitor XML |
| `zip` | 9.8.1 | MIT | leitura e escrita do contêiner |
| `flate2` | 1.1.11 | MIT OR Apache-2.0 | deflate |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 | transitiva |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | transitiva |
| `simd-adler32` | 0.3.10 | MIT | transitiva |
| `crc32fast` | 1.5.5 | MIT OR Apache-2.0 | transitiva |
| `typed-path` | 0.13.0 | MIT OR Apache-2.0 | transitiva do `zip` |
| `constant_time_eq` | 0.6.1 | CC0-1.0 OR MIT-0 OR Apache-2.0 | transitiva do `zip` |
| `indexmap`, `hashbrown`, `equivalent` | 2.14.2, 0.17.2, 1.0.3 | MIT OR Apache-2.0 | transitivas do `zip` |
| `getrandom`, `libc`, `cfg-if`, `memchr` | 0.4.3, 0.191, 1.0.5, 2.8.3 | MIT OR Apache-2.0 (memchr: Unlicense OR MIT) | transitivas do `zip` |
| `zeroize`, `zeroize_derive` | 1.9.2, 1.5.0 | Apache-2.0 OR MIT | transitivas do `zip` |
| `syn`, `quote`, `proc-macro2`, `unicode-ident` | 2.0.119, 1.0.47, 1.0.107, 1.0.27 | MIT OR Apache-2.0 (`unicode-ident` também Unicode-3.0) | só em compilação (derive) |

`cargo deny`/`cargo license` **não foram executados**: a conferência acima é do campo `license` de cada crate, não de uma auditoria jurídica.

## Dependências do `piano-tutor-curso` (M4, Pacote de Curso)

Versões fixadas com `=` em `crates/piano-tutor-curso/Cargo.toml`; licenças conferidas nos `Cargo.toml` dos crates no registro local (2026-11-24).

| Crate | Versão | Licença | Uso |
| --- | --- | --- | --- |
| `ed25519-dalek` | 2.2.0 | BSD-3-Clause | assinatura e verificação do manifesto (traz `curve25519-dalek` 4.1.3, BSD-3-Clause; `subtle` 2.6.1, BSD-3-Clause; `fiat-crypto` 0.2.9, MIT OR Apache-2.0 OR BSD-1-Clause; `ed25519` 2.2.3, `signature` 2.2.0, `zeroize`, `sha2` 0.10.9 e `rand_core` 0.6.4, MIT OR Apache-2.0) |
| `sha2` | 0.11.1 | MIT OR Apache-2.0 | SHA-256 dos arquivos (mesma versão do app) |
| `zip` | 9.8.1 | MIT | contêiner `.ptcurso` (mesma configuração do core) |
| `flate2` | 1.1.11 | MIT OR Apache-2.0 | deflate (backend `miniz_oxide`, Rust puro) |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | modelo de dados |
| `toml` | 1.2.0 | MIT OR Apache-2.0 | manifesto e Roteiro (traz `toml_writer`, `serde_spanned`, `toml_datetime`, `toml_parser`, MIT OR Apache-2.0) |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | Alinhamento |
| `dirs` | 7.0.0 | MIT OR Apache-2.0 | pasta de dados padrão da CLI |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 | semente aleatória das chaves geradas |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | **somente dev-dependency**: pastas temporárias nos testes |

O crate não faz rede e as fixtures dos testes são sintéticas (nenhum conteúdo de curso real no repositório). Transitivas não listadas não foram auditadas uma a uma: **NÃO VERIFICADO** (usar `cargo deny` na CI). Chaves privadas de autor nunca entram no repositório.

## Dependências do `piano-tutor-alinhamento` (M4, etapa 4)

Versões fixadas com `=` em `crates/piano-tutor-alinhamento/Cargo.toml`; licenças conferidas em `cargo info` (2026-11-24).

| Crate | Versão | Licença | Uso |
| --- | --- | --- | --- |
| `hound` | 3.5.1 | Apache-2.0 | leitura de WAV PCM/float (sem dependências) |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | saída `alinhamentos/<id>.json` (mesma versão do `piano-tutor-midi`) |

A FFT (radix-2) é código próprio, conferido contra a DFT direta, para não trazer `rustfft` e suas transitivas (`num-complex`, `primal-check`, `strength_reduce`, `transpose` etc.). Transitivas de `serde_json` (`itoa`, `memchr`, `zmij`, `serde_core`, `indexmap` etc.) não auditadas uma a uma: **NÃO VERIFICADO**.

Os testes usam apenas áudio **sintético** gerado em memória (senoides, harmônicos e ruído); nenhum áudio de curso é lido ou versionado. O áudio real da Aula só existe no cerebro, fora do repositório.
