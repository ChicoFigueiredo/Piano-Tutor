# E00 / 01 — Diagnóstico do estado atual

Data: 2026-11-27 · Repositório: `/home/user/Piano-Tutor` · Branch `claude/project-thread-w8l1yp` (HEAD `7420691`, 1 commit sobre `main` `222b513`) · Autor: equipe E00 (agente de diagnóstico). Sem commit, sem edição de código. Fonte primária = este repositório (cada afirmação traz `arquivo:linha`) e a execução real dos comandos abaixo. O que não foi executado ou lido está marcado **NÃO VERIFICADO**.

Nota de contexto: durante a execução `git status` mostrou `CONTEXT.md` modificado e `docs/evolucao-musical/e00/` não rastreado. Não toquei em nenhum dos dois além de criar este arquivo.

---

## 1. Inventário

### 1.1 Crates e tamanho (linhas de `.rs`, `find crates/X -name '*.rs' | xargs wc -l`; `tokei` não instalado)

| Crate | LOC `.rs` | Papel | Dependências externas diretas (versão fixada `=`) e licença |
|---|---:|---|---|
| `piano-tutor-app` | 51.218 | GUI egui, telas, professor, curso, SQLite, Luau, mpv, Piper | `bancada-casca` (git), `ureq 3.5.2`, `sha2`, `dirs`, `serde_json`, `toml`, `toml_edit`, `serde`, `png`, `rusqlite 0.43.0 bundled`, `libloading 0.8.9`, `mlua 0.12.3 [luau]` (`crates/piano-tutor-app/Cargo.toml:21-47`) |
| `piano-tutor-core` | 10.318 | Teoria, MIDI puro, partitura, MXL, ritmo, comparador | `roxmltree 0.21.2`, `zip 9.8.1`, `flate2 1.1.11` (`crates/piano-tutor-core/Cargo.toml:14-19`) |
| `piano-tutor-curso` | 4.193 | Pacote de Curso `.ptcurso`: empacotar, verificar, instalar (ed25519) | `ed25519-dalek 2.2.0`, `sha2`, `zip`, `flate2`, `serde`, `toml`, `serde_json`, `dirs`, `getrandom` |
| `piano-tutor-audio` | 4.038 | Relógio mestre, SF2, saída cpal | `cpal 0.18.2`, `rustysynth 1.3.8`, `rtrb 0.4.1` |
| `piano-tutor-alinhamento` | 3.075 | DTW de croma partitura x áudio | `hound 3.5.1`, `serde_json` |
| `piano-tutor-diag` | 2.447 | Diagnóstico de ambiente | `midir 0.11.1`, `cpal`, `tracing`, `tracing-subscriber` |
| `piano-tutor-midi` | 2.108 | Entrada MIDI, NDJSON | `midir 0.11.1` |
| **Total** | **77.397** | 7 crates, edition 2024, rust-version 1.98 (`Cargo.toml:1-19`) | |

Fora dos crates: `fabrica/video/*.py` (Python, dedilhado/calibração, ADR-0013), `scripts/` (bash, 1 `.py`), `assets/partituras/` (5 MusicXML CC0/OpenScore), `assets/licoes/` (TOML), `third_party/mattpocock-skills` (só `LICENSE`).

Arquivos grandes no app (risco de coesão, ver §7): `partitura_visual.rs` 3.359, `gravacao.rs` 2.915, `cursos.rs` 2.636, `exercicios.rs` 2.448, `app.rs` 2.389, `tela_partitura.rs` 2.104, `tela_revisao.rs` 1.996, `revisao.rs` 1.812. No core: `partitura.rs` 2.222, `chamada_resposta.rs` 1.441, `teoria.rs` 1.373.

### 1.2 Dependências e licenças (`cargo metadata --locked`, 516 pacotes no lockfile)

- Licenças dominantes: MIT / Apache-2.0 / duplas (≈ 430 dos 516). Nenhuma GPL/AGPL "pura".
- Exceções a revisar (nenhuma é direta do projeto): `option-ext 0.2.0` MPL-2.0 (via `dirs`), `self_cell 1.3.0` Apache-2.0 OR GPL-2.0-only (escolhe Apache), `clipboard-win`/`error-code` BSL-1.0, `webpki-roots` CDLA-Permissive-2.0, `r-efi` (MIT OR Apache-2.0 OR LGPL-2.1-or-later), `epaint_default_fonts` ((MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0; fonte embutida pelo egui).
- Dependência de sistema: ALSA (LGPL-2.1+, ligação dinâmica). `libmpv.so.2` por `dlopen` (`libloading`; `crates/piano-tutor-app/src/video_embutido.rs`), não vinculada. Luau (`luau0-src 0.24.1+luau748`, MIT) compilado dentro do binário (exige compilador C++).
- **Não há `cargo deny` nem `cargo license` na CI** (`.github/workflows/ci.yml`: só fmt, clippy, test e `scripts/verificar-documentacao.py`). `docs/seguranca/LICENCAS-E-DADOS.md` declara auditoria transitiva "NÃO VERIFICADO" para diag/app/audio. A `bancada` tem `deny.toml` próprio (no checkout git), o Piano Tutor não.
- Deriva documental: `LICENCAS-E-DADOS.md` cita a `bancada-casca` na rev `94818fd9…` (seção "Dependências do `piano-tutor-app`"), mas `crates/piano-tutor-app/Cargo.toml:47` e `Cargo.lock:409-411` usam `6d296a78…`. A tabela de licenças está desatualizada (não lista rusqlite, mlua, ureq, ed25519, libloading, png, toml_edit).

### 1.3 Como a `bancada-casca` entra

| Item | Evidência |
|---|---|
| Mecanismo | **git por SHA**, não path nem `[patch]`: `bancada-casca = { git = "https://github.com/ChicoFigueiredo/bancada", rev = "6d296a787eca7f1df8387c47690e9f26ff29b952" }` (`crates/piano-tutor-app/Cargo.toml:47`). Só o app depende dela; o core não importa egui (`piano-tutor-core/Cargo.toml` não lista UI). Não existe `[patch]` nem `.cargo/config` (grep vazio). |
| Compila na nuvem? | **Sim, SEM credencial**: `cargo clippy` e `cargo test` neste container baixaram e compilaram `bancada-casca v0.1.0 (…?rev=6d296a78)` (log: "Checking bancada-casca v0.1.0"). O repositório é público (ADR-0002 ainda diz "privado", `docs/decisoes/ADR-0002-shell.md:5`, desatualizado; `PLANO-DE-MARCOS.md` Premissas já registra a mudança). |
| Licença | `MIT OR Apache-2.0` (metadata do crate). |
| Traz | egui/eframe 0.36.2 (glow, x11, wayland, accesskit), `egui_tiles 0.17.1`, `fluent-bundle 0.16`, serde. 7.669 LOC no crate `casca`. |
| **Achado**: o mesmo repo `bancada` publica 3 crates que o app **não usa** | `bancada-armazem` (SQLite: PRAGMAs, **escada de migrações por `user_version`**, tabela só de acréscimo, backup, preferências), `bancada-segredos` (**keyring do SO + arquivo cifrado Argon2id/XChaCha20**), `bancada-rede` (**portão HTTPS único**: catálogo de endereços, `Authorization` vindo de Segredo, limites/backoff, cache ETag, OAuth PKCE por loopback). `grep bancada_armazem|bancada_segredos|bancada_rede crates` = vazio. Relevante para R-B02, R-A03 (chaves de provider no cofre) e R-N01. Conflito a resolver: `bancada` fixa `rusqlite 0.41` no workspace (`Cargo.toml` do checkout), o app usa `=0.43.0` (`Cargo.toml:41`); como `libsqlite3-sys` tem `links`, as duas versões não coexistem. **NÃO VERIFICADO** se `bancada-armazem` compila com 0.43. |

---

## 2. Baseline (executado, nada consertado)

Ambiente: container Linux 6.18 (root), `/dev/snd` ausente, sem PipeWire. Toolchain `1.98.0` pelo `rust-toolchain.toml` (rustup baixou `rustc 1.98.0 (88d9e12ae 2026-08-18)`, `cargo 1.98.0 (797e8a9bc 2026-08-05)`, rustfmt, clippy).

| Passo | Comando literal | Resultado |
|---|---|---|
| Pré-requisitos | `scripts/bootstrap-linux.sh` (checa) → `scripts/bootstrap-linux.sh --sim` | Faltavam 8 pacotes apt (`libasound2-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev libxcursor-dev libxrandr-dev libxi-dev fluid-soundfont-gm`); instalados, exit 0. Aviso esperado: PipeWire ausente. |
| Formato | `cargo fmt --all --check` | **exit 0**, sem diferenças |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0**, 0 avisos (1 min 14 s após o fetch do git) |
| Testes | `cargo test --workspace` | **exit 0: 1.053 passaram, 0 falharam, 6 ignorados** |

Detalhe por alvo (`test result:`): alinhamento 41 + 12 (integração sintética) + 3 (cli); app 614 (4 ignorados); audio 42 + 1 + 11 + 3 (1 ignorado); core 194 (1 ignorado); curso 1 + 6 + 4 + 7 + 36 + 1 doc-test; diag 43; midi 19 + 15. Soma = 1.053.

Os 6 ignorados são todos dependentes de ambiente externo (`professor_falante.rs:1320` roda o `piper` real; `gravacao.rs:2642` regenera referências; `som.rs:1592` e `audio/tests/sf2_real.rs:70` exigem SF2 real; `video_embutido.rs:953` exige libmpv e vídeo em `/mnt/e2`; `mxl.rs:709` só grava com `PT_MXL_SAIDA`). Portanto **nada do que depende de hardware, áudio real, libmpv, Piper, MuseScore ou REVAS foi exercido aqui** (mensagens `ALSA lib … /dev/snd/seq` nos testes do midi são esperadas; os testes passam sem dispositivo). O gate físico (F01/F02/F03/F14) permanece **NÃO VERIFICADO** por esta execução.

Ressalva de estado documental: `docs/aceite/MATRIZ-DE-ACEITE.md:9-24` marca F00, F01, F02, F03, F05, F06, F07, F10-F14 como "Não iniciado", enquanto o código das mesmas fases existe e passa nos testes (ex.: F07 tem `docs/aceite/F07/evidencia.md`). A fonte de estado real é `docs/planejamento/progresso.json`, não a matriz.

---

## 3. Partitura: modelo, renderização e matriz por elemento MusicXML

### 3.1 Modelo (`crates/piano-tutor-core/src/partitura.rs`, 2.222 linhas)

- Entrada: só `score-partwise` (`partitura.rs:1210`); `de_musicxml` (`:1198`), `.mxl` via `mxl::extrair_musicxml` (`mxl.rs:160`) e `Partitura::de_mxl` (`mxl.rs:201`).
- Estruturas: `Partitura{titulo, compositor, direitos, partes, compassos: Vec<InfoCompasso>, andamentos}` (`:568-575`), `Parte{id, nome, pautas, compassos}` (`:517`), `Compasso{notas, claves}` (`:510`), `Nota` (`:370-401`) com posição/duração racional (`Fracao` i64, `:92`), altura, MIDI, voz, pauta, acorde, adorno, figura, pontos, acidente, ties, vigas, haste, tuplo, slurs.
- **O modelo é derivado e com perda**: depois de `de_musicxml` o XML original só sobrevive como `String` guardada à parte na tela (`tela_partitura.rs:290-293`, `xml: String` "gravado de volta na exportação").
- Não há ID estável de nota, de compasso nem de evento (identidade = índices posicionais; `Alvo{compasso, inicio, fim, mao}` em `partitura_visual.rs:126-132`).
- Info global de compasso (armadura, fórmula, repetição, casas) vem da **primeira parte**, repetições são a união (`partitura.rs:525-527`).
- Limites de segurança: 8 MiB de texto, 2 M de nós, 200 k notas, 20 k compassos, 64 partes, 8 pautas (`:25-35`); XML limitado por `roxmltree` sem resolver entidades externas; MXL: 16 MiB, 128 entradas, taxa 200x, sem `..` (`mxl.rs:23-34`).

### 3.2 Renderização (como desenha)

**Sem fonte musical, sem Bravura, sem SMuFL, sem Verovio.** O layout é próprio e puro em "espaços de pauta" (sp) e o desenho usa primitivas do `Painter` do egui: `Shape::line_segment`, `convex_polygon`, `line` (`partitura_visual.rs:2322-2369`), com cabeças de nota como elipses (`Forma::Cabeca`, `:141-149`), claves como polilinhas (`glifo_clave_sol` `:717`, `_fa` `:771`, `_do` `:803`), pausas por geometria (`:936`), bandeirolas (`:1220`). Texto só para números ("8" de clave, tuplas, numeração de compasso) via `FontId::proportional` (`:2384`). `docs/seguranca/LICENCAS-E-DADOS.md:69` confirma: "Nenhuma fonte musical (Bravura etc.) é embutida". Entrega: `calcular_layout` (`:1675`) e `calcular_layout_fita` (`:1682`, sistema único) produzem `Layout`/`Sistema`/`Item{forma, cor, alvo}`; `pintar_itens` (`:2293`) pinta; hit test só devolve **compasso** (`Layout::compasso_em`, `:291`), não nota.

### 3.3 Spike Verovio / ADR-0007

- ADR-0007 (`docs/decisoes/ADR-0007-partitura-somente-leitura.md:5`) exigia spike egui+Bravura x Verovio (LGPL-3.0) com ≥ 5 MusicXML antes da F07, resultado em ADR de escolha de motor.
- **O spike NÃO foi feito.** `docs/aceite/F07/evidencia.md:3`: "O spike de notação egui × Verovio previsto na ADR-0007 **não foi feito**: aqui só existe a opção A (layout próprio em egui, sem fonte musical)". `MATRIZ-DE-ACEITE.md:17` mantém "Spike de notação … Não iniciado". Não existe ADR de motor. Grep de `verovio|bravura|smufl` em `crates/` = vazio. Portanto a opção A virou o motor de fato sem decisão registrada (**CONFIRMADA** a lacuna).
- `docs/referencias/COMPONENTES-VISUAIS.md:64` registra que o `bravura_metadata.json` não foi baixado ("confirmar antes de fixar") e as constantes de gravura vêm de convenção.

### 3.4 Matriz por elemento (importar / renderizar / tocar / exportar)

Legenda: sim, parcial, não. **Exportar** = hoje a exportação **regrava o XML original byte a byte** (`tela_partitura.rs:956-1000`: `f.write_all(self.xml.as_bytes())` e relê para conferir). Logo "exportar" preserva tudo do arquivo carregado, mas porque não há edição; não prova que o modelo exporta. Marco isso como "cópia" na coluna.

| Elemento | Importar | Renderizar | Tocar | Exportar | Evidência |
|---|---|---|---|---|---|
| Notas (altura, alter, oitava, duração, tipo 128th a breve, pontos) | sim | sim | sim | cópia | `partitura.rs:704-760`, `Figura::de_texto` `:270-290`, `eventos_de_reproducao` `:1469` |
| Pausas (inclusive `measure="yes"`) | sim | sim | n/a | cópia | `:706-707`, `partitura_visual.rs:936` |
| Acordes (`<chord/>`) | sim | sim | sim | cópia | `:705`, evento por nota `:1469-1500` |
| Vozes (`<voice>`, backup/forward) | sim | parcial | sim | cópia | `:845`, `:977-995`; vigas e hastes por voz em `:1106`; cross-voice e colisões **NÃO VERIFICADO** |
| Pautas (`<staves>`, `<staff>`, até 8) | sim | sim | sim | cópia | `:901`, `:848`; MAX_PAUTAS `:35` |
| Múltiplas partes | sim | sim | sim | cópia | `:1248`, `MAX_PARTES` `:33` |
| Cross-staff | parcial (nota carrega a pauta) | **NÃO VERIFICADO** (sem viga cross-staff) | sim | cópia | `Nota.pauta` `:379` |
| Tie (valor) | sim | sim | sim (funde eventos) | cópia | `:783-787`, fusão `:1481-1487` |
| Slur (expressão) | sim | sim | não afeta som | cópia | `:832-838`, `partitura_visual.rs:1181-1196` |
| Tuplet (`time-modification`, `<tuplet>`) | sim | sim (número/colchete, 9 refs) | sim | cópia | `:760-770`, `:830-831` |
| Grace | sim (duração 0) | sim | **não toca** (`if nota.adorno {continue}`) | cópia | `:704`, `:746`, `:1477`; doc `:14` |
| Armadura (`fifths`, modo maior/menor) | sim | sim | n/a | cópia | `:904-915`; modos além de maior/menor ignorados |
| Fórmula de compasso (inclui common/cut, composta) | sim | sim | sim (pulso) | cópia | `:916-940`, `Forma::duracao_do_pulso` `:489` |
| Anacruse (`implicit`, número 0, compasso incompleto) | sim | sim | sim | cópia | `:1065`, `InfoCompasso.incompleto` `:543`; teste `wiegenlied_anacruse…` `:1986` |
| Repetição (`repeat forward/backward`) | sim | sim | sim | cópia | `:1011-1016`, `ordem_de_execucao` `:1419` |
| Volta / casas (`ending`) | sim | sim (numeração `partitura_visual.rs:2050`) | sim | cópia | `:1018-1031` |
| Segno / coda / D.C. / D.S. | **não** | não | **não** | cópia | sem ocorrência em `partitura.rs`; `ordem_de_execucao` só trata repeat+casas |
| Letra (`<lyric>`) | **não** | **não** | n/a | cópia | `partitura.rs:14` ("letra é ignorada"); grep `lyric` em `crates/` = vazio |
| Harmony / cifra (`<harmony>`) | **não** | **não** | n/a | cópia | grep `harmony` = vazio (ver §6: o curso gera `<harmony>`) |
| Fingering (`<technical><fingering>`) | **não** | **não** na partitura | n/a | cópia | grep `fingering` = vazio; o dedilhado do teclado vem do campo TOML `dedilhado` das Lições (`dedos.rs:1`), não do MusicXML |
| Dinâmica (`<dynamics>`, `<wedge>`) | **não** | **não** | **não** (velocity fixa) | cópia | grep `dynamics` = vazio |
| Articulação / ornamento (staccato, trill…) | **não** | **não** | **não** | cópia | grep `ornaments` = vazio |
| Octave-shift (`<octave-shift>`) | **não** | **não** | **não altera MIDI** | cópia | `partitura.rs:13` |
| Transpose (`<transpose>`) | **não** | n/a | **não altera MIDI** | cópia | `partitura.rs:13`; sem distinção escrito x soante |
| Clave (sol/fá/dó, linha, `clef-octave-change`, trocas no compasso) | sim | sim ("8" desenhado) | n/a | cópia | `:649-676`, `:944-960`, `partitura_visual.rs:826-868` |
| Clave percussão / tablatura | parcial (sinal lido; tratada como sol) | **NÃO VERIFICADO** | n/a | cópia | `:649-654`, `Clave::linha_inferior` `:441-448` |
| Andamento (`metronome`, `sound tempo`) | sim | **não desenha marca de tempo** (0 ocorrências de "andamento" no render) | sim (mapa de tempo) | cópia | `:678-690`, `:995-1008`, `mapa_de_andamento` `:1549` |
| Barras (dupla/final/nenhuma) | sim | sim | n/a | cópia | `:1034-1038` |
| Vigas / haste | sim | sim | n/a | cópia | `:794-820` |
| Metadados (título, compositor, direitos) | sim | parcial | n/a | cópia | `:1215-1226` |
| MXL (zip) | sim (leitura segura) | sim | sim | **parcial**: só o app grava `.musicxml`; `mxl::empacotar_mxl` existe (`mxl.rs:208`), mas a tela exporta só `.musicxml/.xml` (`tela_partitura.rs:965-974`) | `mxl.rs:160-208` |
| `score-timewise` | **não** (`RaizNaoSuportada`) | não | não | n/a | `partitura.rs:1210`, `:45` |
| Dinâmica de cursor/duração real vs. escrita | sim (duração real e escrita separadas) | sim | sim | n/a | `Nota.duracao` x `figura/pontos` `:372-390` |

Conclusão: **o modelo cobre ~15 dos ~33 itens**; tudo que a spec nova lista como elementos de edição (letra, cifra, dedilhado, dinâmica, ornamentos, oitava, transposição, segno/coda) **não é lido nem preservado no modelo**; sobrevive apenas porque a exportação atual copia o texto original.

---

## 4. Capacidades já existentes (MCP, LLM, HTTP, SQLite, Luau, Piper, MuseScore)

| Capacidade | Existe? | Onde / evidência |
|---|---|---|
| MCP (servidor ou cliente) | **Não** | grep `mcp`, `rmcp` em `crates/` e em todos os `Cargo.toml` = vazio |
| LLM / provider (OpenAI, Anthropic, Ollama etc.) | **Não** | grep vazio. A IA do projeto é **offline e externa ao app**: transcrição por Claude via Claude Code no cerebro (`CONVERSAO-DE-PARTITURAS.md` §3), Python em `fabrica/video/` |
| Cliente HTTP | Sim, **mínimo** | `ureq 3.5.2` (rustls), só para baixar o SoundFont (`baixar_sf2.rs:84`, `abridor_http`) com SHA-256 e `.part` + rename. Sem portão de rede/catálogo (a `bancada-rede` existe e não é usada) |
| SQLite | Sim, **só preferências/perfis** | `rusqlite 0.43.0 bundled`; `perfis.rs:93-107` cria `meta`, `perfis`, `preferencias` com `CREATE TABLE IF NOT EXISTS` e `meta.esquema = VERSAO_ESQUEMA`. **Não há escada de migrações SQL** (sem `user_version`); migração existe só no nível da pasta de dados (`dados_usuario.rs:29` `VERSAO_ATUAL = 1`, `:43` uma única migração v0→v1 que não altera nada, backup antes de migrar, `MAX_BACKUPS = 5`). **Não há banco de acervo/biblioteca, FTS nem catálogo de partituras.** |
| Luau / mlua | Sim, **sandbox real** | `roteiro_aula.rs:209` `Lua::new_with(libs, …)`, `:211` `set_memory_limit` (16 MiB, `:55`), `:215` `set_interrupt` (5 M de interrupções, `:59`; 50 ms de relógio, `:57`), `:350` `lua.sandbox(true)`; sem `io/os/debug/package/load*` (`:35`), `require` só de módulos do pacote, sem rede/arquivos; API versionada `VERSAO_API = 1` (`:53`). Limite conhecido: chamada nativa pesada (ex.: `string.find` patológico) não é interrompida (`:40-41`). Hoje só conduz o Player de Aula; **não tem API de partitura/patch** |
| Piper (TTS local) | Sim | `professor_falante.rs:236` `comando_piper` (binário no PATH/`~/.local/bin`, modelo `.onnx` em `<dados>/vozes`), instruções `:263`; degrada para texto (`SemPiper` `:268`). Teste real ignorado (`:1320`) |
| MuseScore externo | Sim, **opcional e não exercido** | `revisao.rs:913-917` procura `mscore/musescore/mscore4/musescore4/MuseScore4`; `revisao.rs:942` abre o MusicXML; `revisao.rs:30-31` declara "NÃO VERIFICADO em CI"; caminhos extras de macOS em `professor_falante.rs:150` |
| mpv / libmpv | Sim | `mpv.rs:118` (processo, IPC JSON) e `video_embutido.rs` (dlopen; único módulo com `unsafe`, `Cargo.toml` `[lints.rust] unsafe_code = "deny"`) |
| Processos externos | Só mpv, piper, MuseScore, via `Command::new` sem shell | `mpv.rs:118`, `professor_falante.rs:237,251`, `revisao.rs:942` |
| OMR (Audiveris/HOMR) | **Não no app**; só no pipeline manual | Audiveris 5.11 via flatpak no cerebro (`CONVERSAO-DE-PARTITURAS.md` §3, passo 5). Nenhum código Rust no repo o invoca (grep `audiveris` em `crates/` e `scripts/` = vazio) |
| Cofre de segredos | Não no app | existe `bancada-segredos` (não usada) |
| Telemetria | Não encontrada (grep sem rede além de `ureq` do SF2) | `baixar_sf2.rs` |

---

## 5. Mapa de documentos que conflitam com a nova visão

Regra do titular (ADR-EVOLUCAO.md:2-4): edição nativa autorizada; histórico não se apaga; ADR nova numerada livre. Próximo número livre: **0015** (existentes 0001-0014, com 0009 duplicado). Ações: **manter**, **emendar** (nota/addendum datado, sem apagar), **substituir** (nova ADR marca a antiga como "Substituída por").

| Documento | O que diz (evidência) | Conflito com a nova visão | Ação proposta |
|---|---|---|---|
| **ADR-0007** | F07 só leitura/exportação; edição "para depois de F13"; spike egui+Bravura x Verovio antes (`ADR-0007…:5`) | Proíbe a edição que agora é requisito prioritário; spike nunca feito (`F07/evidencia.md:3`) | **Substituir** (parcial): ADR-0015 revoga a proibição e a ordem "depois de F13"; **manter** a parte "preservar aceites F07 como regressão". Escolha de motor de render passa a ser a matriz de decisão da E00 (exige o spike) |
| **ADR-0003** | "Python em worker opcional… Luau, se adotado, fica restrito a regras pedagógicas" (`ADR-0003…:5`); status PROPOSTA nunca promovida | Luau já usado em Aulas (ADR-0012); nova visão quer plugins Luau de edição, MCP, LLM, OMR Java/Python | **Emendar**: promover a ACEITA com addendum (MCP, harness LLM opcional, Java e Python isolados, Luau para plugins de partitura) |
| **ADR-0005** | Ordem M1..M5; M3 = "spike → F07 → F09"; F07 só leitura (`ADR-0005…:11,17`) | Ordem F00-F14 não contém E00-E09; M3 já executado sem spike | **Emendar**: nota apontando que marcos E00-E09 correm em paralelo ao M5 e que o spike vira entrega E00 |
| **ADR-0008** | Aulas só locais, nada de mídia no repo/CI (`ADR-0008…:5`) | Compatível; **reforça** R-N01 (nenhum curso comprado no git). Risco: fixtures de OMR/PDF precisam ser licenciadas | **Manter**; citar na ADR-0015 |
| **ADR-0009 (duplicado)** | `ADR-0009-cursos-de-tutor-como-pacote.md` (ACEITA, pacote só dados, assinado) e `ADR-0009-licoes-em-arquivos-toml.md` (PROPOSTA, Lua adiado) | Dois arquivos com o mesmo número; o TOML sugere script "para depois", já superado por ADR-0012 | **Emendar**: renumerar o de lições-TOML (sugestão 0009b, sem apagar) e marcar como "parcialmente superada por ADR-0012"; **manter** o de pacotes. Decisão de renumeração é do titular (links entrantes) |
| **ADR-0010** | Preferências em SQLite com perfis (`ADR-0010…`) | Boa base, mas escopo é prefs; acervo/FTS/migrações SQL (R-B02) exigem outro banco ou escada | **Manter** + **emendar** com ponteiro para ADR de armazenamento (acervo no mesmo arquivo ou em outro; `bancada-armazem`) |
| **ADR-0012** | Luau para Roteiros de Aula; sandbox, API versionada, altera ADR-0009 (`ADR-0012…:11-17`) | R-A04 pede reutilizar sem misturar scripts de aula e de edição | **Manter**; ADR-0015 define segunda superfície (plugins de edição) com manifesto/permissões próprios |
| **ADR-0002** | Reuso da `bancada-casca`; "repositório privado" (`ADR-0002…:5`) | Desatualizado (público e compilável sem credencial, §1.3); ignora `bancada-armazem/-segredos/-rede` | **Emendar** |
| **ADR-0004 / ADR-0014** | Linux referência; macOS teste pessoal | Compatível (ADR-EVOLUCAO item 7). Spec ainda fala "Windows/Linux" (ver abaixo) | **Manter** |
| **ADR-0006** | NÃO VERIFICADO bloqueia gates físicos; metas experimentais liberam como beta | Compatível; OMR e agentes entram como "meta experimental" | **Manter** |
| **ADR-0011, ADR-0013** | libmpv e dedilhado por vídeo na fábrica | Sem conflito; ADR-0013 (Python MediaPipe na fábrica) é precedente do worker Python | **Manter** |
| **AGENTS.md** | `AGENTS.md:10` "Python somente worker opcional de ML; Luau somente hipótese de regras pedagógicas **após F13**"; `:6` "Uma fase por vez… começar F00"; `:3` lista de leitura sem a spec evolutiva | Contradiz Luau já em produção e plugins de edição; fluxo de fases F00-F14 não prevê E00-E09; `:11` núcleo isolado de GUI/Python **continua válido** (e vira o requisito do editor) | **Emendar** (após aprovação da ADR-0015): permitir MCP/LLM opcional, Luau de plugins, Java/Python isolados; manter `:11` e `:15-16` (segredos) |
| **CONTEXT.md** (glossário) | Termos Lição, Exercício, Gabarito, Hipótese ("Conteúdo gerado por IA", `:99-103`); estado "só o humano aprova" | Faltam termos Obra, Revisão, Derivação, Patch, Plugin de edição; "Evitar: sugestão da IA" pode colidir com "proposta/patch" da LLM. (O arquivo está modificado no working tree por outro agente; não li o diff) | **Emendar** (outra frente, grill-with-docs) |
| **ESPECIFICACAO-COMPLETA-v1** | F7: "edição mínima de nota/duração/compasso" nos entregáveis e "editor completo estilo MuseScore **não** é requisito" (§Fase 7, `ESPEC…:470-490`); tabela de bibliotecas lista `quick-xml` + `zip`/Bravura; `:5` do cabeçalho remete a ADR-0007; Fase 14 ainda "Windows/Linux"; §10 limita Luau a F13 | Já **auto-conflita** com ADR-0007 (spec admite edição mínima, ADR proíbe); §10 e `:248` limitam Luau; "Gate: editor completo não é requisito" | **Emendar**: addendum de evolução; não reescrever o histórico |
| **MATRIZ-DE-ACEITE** | `:9-24` quase tudo "Não iniciado"; `:17` spike M3 "Não iniciado" | Não reflete o código real (§2) e não tem E00-E09 | **Emendar**: sincronizar com `progresso.json`, acrescentar linhas E00-E09 e marcar o spike como "NÃO FEITO, absorvido pela E00" |
| **PLANO-DE-MARCOS** | M3 "spike → F07 → F09"; `:30` "F07 só leitura e exportação" | Mesma contradição de ADR-0007 | **Emendar** |
| **backlog/fases/F07.md, F13.md** | F07: "A edição de notas fica para depois de F13" e "sem edição de notas (ADR-0007)" (`F07.md:10,18,23`); F13: "Luau opcional" (`F13.md:14`) | Idem | **Emendar** (nota apontando a ADR-0015) |
| **docs/seguranca/LICENCAS-E-DADOS.md** | Sem fonte musical embutida (`:69`); rev da bancada desatualizada; sem cargo-deny | Bravura (OFL-1.1) entrará; novas dependências (rmcp, providers, OMR) exigem inventário | **Emendar** + adicionar `cargo deny` à CI (proposta de ticket) |

---

## 6. `docs/arquitetura/CONVERSAO-DE-PARTITURAS.md` em números (sem conteúdo musical)

| Item | Valor |
|---|---|
| Material | Curso de piano comprado, © 2017 do autor; nada dele no repo público (`:7`). Vive na "oficina" no cerebro: `~/.local/share/piano-tutor/fontes/luciano-alves/` |
| Escopo convertido | 18 Seções, 163 Aulas, **113 Exercícios** (95 transcritos de PDF + 18 gerados da fala), 85 com áudio alinhado, 1.284 falas de Roteiro |
| Pipeline | PDF → PNG e texto/bbox (Poppler `pdftoppm`/`pdftotext`) → **transcrição por Claude Sonnet (subagentes)** a partir da imagem → conferência com fala cronometrada → **Audiveris 5.11** (flatpak) como OMR independente → validação XML → alinhamento Rust (croma + DTW, `piano-tutor-alinhamento`) → pacote `.ptcurso` assinado ed25519 (`piano-tutor-curso`) |
| OMR | ≈ 74 com saída comparável (a maioria 95-100% das alturas iguais), 21 sem saída útil (folhas de leitura sem fórmula de compasso); Audiveris não lê dedilhado nem solfejo (sem idiomas de OCR) |
| Confiança | média 0,89 (**estimativa do próprio agente**, não medida contra gabarito humano) |
| Onde ficam | Oficina por Seção: `partituras/<id>.musicxml`, `.estado.toml`, `.divergencias.md`, `paginas/<id>.png`, `roteiro.toml`; pacote em `~/.local/share/piano-tutor/cursos/<id>/`; chave privada em `~/.config/piano-tutor/chaves/` (fora do git) |
| Estado de revisão | **Tudo Hipótese; zero partituras aprovadas por humano**; fila: 30 alta, 43 média, 40 baixa prioridade; nada testado com o REVAS (`:9`) |
| Ferramentas de edição humana | MuseScore 4.7.5 (flatpak) + tela "Revisão do tutor" (`tela_revisao.rs`, `revisao.rs`) |
| Limitações declaradas | ritmo inventado nas folhas de leitura (Seções 4, 6, 9); repetições removidas para casar com áudio; alinhamento não conferido de ouvido; Seções 14-15 escritas só da fala; uníssonos fundidos (`:7` do §7) |
| Dados reutilizáveis para a E00 | **Nenhum pode entrar no repo ou em CI** (ADR-0008, R-N01). Servem só como corpus privado local de estresse |

Achado de acoplamento: a transcrição gera `<harmony>`, `<fingering>` e `<lyric>` (`CONVERSAO…` §5 passo 3), os três elementos que o leitor Rust **ignora** (§3.4). Ou seja, o conteúdo mais rico do curso só aparece no MuseScore, não no Piano Tutor. Também: os scripts Python citados (`gerar-plano.py`, comparação OMR x transcrição, validação) vivem na oficina fora do repo; **não são reprodutíveis a partir do repositório** (NÃO VERIFICADO se `scripts/atualizar-pacote-curso.sh` cobre só a etapa de pacote).

---

## 7. Riscos técnicos do código atual para a evolução

| # | Risco | Evidência | Impacto na evolução | Mitigação sugerida |
|---:|---|---|---|---|
| 1 | **Modelo de dados não editável e com perda**: sem IDs estáveis, derivado do XML, descarta letra/cifra/dedilhado/dinâmica/ornamento/oitava/transposição/segno | §3.4; `Nota` `partitura.rs:370-401`; `Alvo` por índice `partitura_visual.rs:126` | Editor, undo/redo, seleção estável, MCP/patch e preservação de round-trip exigem novo modelo de documento. A "exportação sem perda" atual é cópia do texto (`tela_partitura.rs:292`, `:991`) e **se quebra no primeiro edit** | Spike de modelo: manter árvore XML original + modelo semântico com IDs, ou modelo próprio completo (decisão R-E01/E03) |
| 2 | **Acoplamento GUI/estado/reprodução**: `EstadoPartitura` mistura fontes, texto XML, modelo, reprodução, prática, exportação e UI numa struct de 2.104 linhas; `desenhar_*` recebem `&mut EstadoPartitura` e `ServicoSom` | `tela_partitura.rs:288-300`, `:1065-1214`; mesma estrutura é usada pelo Player de Aula e revisão (`tela_player_aula.rs`, `tela_revisao.rs`, `cursos.rs` importam `partitura_visual`) | O layout é puro (`calcular_layout`), o que é bom, mas o **modelo não tem serviço de domínio** separado da tela; MCP/CLI/plugins precisariam duplicar lógica | Extrair serviço de documento no core (ou crate novo) antes do editor |
| 3 | **Layout acoplado ao egui**: tipos `Pos2`, `Align2`, `Color32` do egui dentro de `Forma`/`Item`/`Layout` | `partitura_visual.rs:19`, `:136-170` | Layout "puro" vive no crate de UI; um MCP/CLI/exportação PDF sem egui não o reaproveita; sem hit test por nota | Camada geométrica própria (f32) com adaptador egui |
| 4 | **Renderer sem SMuFL**: formas geométricas próprias, sem ornamentos/dinâmica/letra/cifra/texto musical, sem paginação; só 2 modos (sistemas e fita) | `partitura_visual.rs:1-14`, `:1675-1682` | Cada novo símbolo exige desenho manual; qualidade tipográfica não validada por humano (`F07/evidencia.md` "legibilidade NÃO VERIFICADA") | Decidir via spike (Bravura+layout próprio x Verovio) com corpus |
| 5 | **Nenhum teste de round-trip semântico nem corpus de conformidade**: só 5 arquivos (2 gravados pelo projeto, 3 OpenScore), sem XSD, sem fuzzing | `assets/partituras/`, `F07/evidencia.md` (tabela) | Não dá para afirmar suporte a MusicXML em geral; R-E01 exige matriz e corpus | Corpus licenciado + testes de preservação (E00 item 03) |
| 6 | **Sem camada de serviços/IPC/jobs**: app monolítico; não há fila durável, workers, esquema JSON versionado, cancelamento. Os 3 `Command::new` são pontuais | §4; `mpv.rs:118`, `revisao.rs:942` | OMR, MCP e LLM exigirão arquitetura nova; o app é a única fronteira | Definir crate `-servicos`/`-jobs` e contrato JSON antes de OMR |
| 7 | **Banco de acervo inexistente e SQLite sem escada de migrações**; duas estratégias de migração (pasta x banco) | `perfis.rs:93`, `dados_usuario.rs:29-47` | R-B02 (FTS, 10 mil registros, rollback) é trabalho novo; risco de dois bancos | Adotar `bancada-armazem` (verificar conflito rusqlite 0.41 x 0.43) ou escada própria |
| 8 | **Revisão atual edita TOML/arquivos via MuseScore externo**, estado Hipótese→Aprovada sem hash de conteúdo | `revisao.rs:1-33`, `:428` (mtime) | R-B06/E03 pedem aprovação ligada ao hash de revisão; hoje se detecta edição só por data de modificação | Hash SHA-256 por revisão; reaproveitar máquina de estados já testada |
| 9 | **Sem IA/provider/cofre/portão de rede**; `ureq` direto | §4 | Risco de cada feature abrir HTTP por conta própria (viola R-N01). Mas há `bancada-rede`/`-segredos` prontas | Tornar o portão de rede obrigatório (lint/teste que proíba `ureq` fora dele) |
| 10 | **Luau sem API de partitura e com limite conhecido de interrupção em chamada nativa** | `roteiro_aula.rs:40-41` | Plugins de terceiros (R-A04) exigem quota de nativas e isolamento de processo para casos hostis | Spike de segurança (script hostil com `string.rep`/`find`) e decidir thread/processo com timeout |
| 11 | **Escala e desempenho de UI**: arquivos de 2-3 mil linhas, layout calculado de partitura inteira (`calcular_layout` sobre todos os compassos); orçamento "edição ≤ 100 ms em 100 compassos" (R-N02) **sem medição** | `partitura_visual.rs:1692-1712`; sem benchmarks no repo (grep `criterion`/`#[bench]` vazio) | Relayout incremental provavelmente necessário | Benchmark base antes de decidir |
| 12 | **Gates físicos NÃO VERIFICADOS** (F01/F02/F03/F14) e latência nunca medida sob carga; novos workers podem regredir | `MATRIZ-DE-ACEITE.md:10-12`, §2 | R-N02 exige medir com OMR+agente concorrentes | Medição no cerebro; nada neste diagnóstico a substitui |
| 13 | **Deriva documental** (matriz desatualizada, rev da bancada, ADR-0002, ADR-0009 duplicado, tabela de licenças) | §1.2, §5 | Erodem a confiança no que "está aceito" | Sincronizar na E00 (ADR-0015 + matriz) |
| 14 | **Qualidade do corpus do curso é estimada por IA**; "confiança 0,89" sem calibração contra gabarito humano | `CONVERSAO…` §6-7 | Não usar como métrica de OMR da E02 | Corpus de avaliação independente (item 03) |

Pontos positivos verificados (para não perder no desenho novo): núcleo `piano-tutor-core` sem egui/áudio (`piano-tutor-core/Cargo.toml`), `unsafe` proibido no workspace exceto `video_embutido`, limites de segurança em XML/MXL com testes, Luau sandboxado, pacote assinado, 1.053 testes verdes, fmt/clippy limpos, `Fracao` racional exata para tempo.

---

## 8. Conclusões da spec proposta que este diagnóstico toca

| Afirmação (spec `docs/evolucao-musical/`) | Veredito | Evidência |
|---|---|---|
| ADR-EVOLUCAO.md:2: "ADR-0007 proíbe edição pós-F13" | **CONFIRMADA** | `ADR-0007…:5` |
| ADR-EVOLUCAO.md:3: "há numeração duplicada histórica" | **CONFIRMADA** | dois `ADR-0009-*.md` |
| ADR-EVOLUCAO.md:6: "Reutilizar ADR-0012 (mlua/Luau)" (R-A04) | **CONFIRMADA** como base técnica; falta API de partitura | `roteiro_aula.rs:209-350` |
| R-A05 "voz opcional via Piper existente" | **CONFIRMADA** | `professor_falante.rs:236`; teste real só manual (`:1320`) |
| R-B02 "SQLite com migrações" como novidade | **CONFIRMADA** (existe SQLite, não existe acervo nem escada de migrações) | `perfis.rs:93`, `dados_usuario.rs:43` |
| R-E01 "parser Rust … preservar elementos desconhecidos" | **CONFIRMADA** como lacuna: o parser atual não preserva (§3.4) | `partitura.rs:1-15` |
| R-E01 "round-trip" já garantido pelo F07 | **REFUTADA em sentido forte**: F07 prova cópia do texto, não round-trip do modelo | `tela_partitura.rs:956-1000` |
| R-E03 "usar renderizador existente, avaliar melhorias nativas" | **PARCIAL**: o existente não usa SMuFL (§3.2) | `partitura_visual.rs:1-14` |
| Spike Verovio x Bravura teria sido feito | **REFUTADA** | `F07/evidencia.md:3` |
| R-A02 MCP via `rmcp` viável | **NÃO VERIFICADA** (nenhum MCP no repo; fora do escopo deste item) | n/a |
| R-N03 build limpo/lockfile reprodutível | **CONFIRMADA parcialmente**: `Cargo.lock` presente, versões `=`, build limpo na nuvem verde; inventário de licenças desatualizado | §1.2, §2 |

---

## Perguntas ao titular

1. **Destino do modelo musical** (bloqueia E03). A: manter a árvore XML original como fonte da verdade e editar por patch sobre ela (preserva o desconhecido, mais simples de provar round-trip). B: modelo semântico próprio completo com IDs (mais limpo para editor/MCP, exige cobrir todos os elementos). C: híbrido (modelo semântico + "resíduo XML" por nó). **Recomendação: C**, a decidir por spike na E00 com o corpus; só A ou B se o spike mostrar custo menor.
2. **Spike de motor de render** (a lacuna da ADR-0007). A: seguir com o renderer próprio e acrescentar SMuFL/Bravura; B: Verovio (LGPL-3.0, C++/SVG) só como referência ou para PDF; C: spike dos dois com 5 arquivos CC0. **Recomendação: C**, com tempo limitado e critério numérico; Bravura (OFL-1.1) é compatível com o repo.
3. **Reaproveitar `bancada-armazem`, `-segredos` e `-rede`** (já no mesmo repo da casca). A: adotar as três (alinha cofre de chaves, portão de rede e escada SQLite); B: manter `rusqlite`/`ureq` diretos. **Recomendação: A**, condicionada a resolver `rusqlite 0.41` x `0.43` (você controla o repo `bancada`).
4. **Renumeração do ADR-0009 duplicado.** A: renumerar o de lições-TOML para `0009b`; B: manter e só anotar. **Recomendação: A**; só você decide por causa de links entrantes.
5. **Matriz de aceite desatualizada.** Posso propor sincronizá-la com `progresso.json` (marcar F00-F07 etc. com o estado real)? **Recomendação: sim**, em PR separado e sem remover checkboxes (regra `AGENTS.md:14`).
6. **Corpus privado do curso.** Pode ser usado só localmente como estresse não versionado (nunca em CI)? **Recomendação: sim, só local**; o corpus de aceite deve ser CC0/gravado por você.
7. **CI de licenças.** Adicionar `cargo deny` (usar o `deny.toml` da bancada como ponto de partida)? **Recomendação: sim**, antes de entrarem rmcp/providers/OMR.

## Lacunas

- Não li `CONTEXT.md` modificado (alteração de outra frente) nem `docs/evolucao-musical/03-PESQUISA-BENCHMARK.md` em profundidade.
- Não executei o app (GUI), mpv, Piper, MuseScore, Audiveris, nem nenhum teste com hardware/áudio; não medi latência nem desempenho de layout.
- A oficina do curso (`~/.local/share/piano-tutor/fontes/…`) não existe neste ambiente: os números do §6 são os declarados em `CONVERSAO-DE-PARTITURAS.md` (27/11/2026), **NÃO VERIFICADOS** por contagem própria.
- Contagem de `#[test]` por grep estático (1.058) difere da execução (1.053 + 6 ignorados) por testes em módulos condicionais; vale a execução.
- A matriz do §3.4 vem de leitura de código e grep de nomes de elementos; não rodei os 5 arquivos do corpus por um inspetor de elementos, então "renderizar" de vozes complexas, cross-staff e clave de percussão/tablatura ficou como **NÃO VERIFICADO**.
- Compatibilidade `bancada-armazem` (rusqlite 0.41) com o app (0.43): não testada.
- Licenças transitivas: lidas do metadata do Cargo (campo `license`), não dos arquivos de licença; não vale como parecer jurídico.
