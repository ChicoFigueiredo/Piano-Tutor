# E00 / 03 — Pesquisa do editor nativo de partituras (R-E01..R-E05)

Data de acesso das fontes: **2026-11-27**. Legenda de evidência: **[V]** verificado nesta sessão (URL + o que foi lido); **[R]** lido no repositório local; **[M]** conhecimento prévio, **NÃO VERIFICADO** (não use como fato). Limites desta sessão: o orçamento de WebFetch esgotou (100/h) e `api.github.com` ficou bloqueado (sem acesso ao repositório); por isso o código-fonte de atalhos do MuseScore (`notationactions.cpp`) **não foi lido** e os atalhos vêm do handbook oficial. Nenhum código de terceiros foi copiado.

## 0. Estado real do repositório (ponto de partida) [R]

| Fato | Evidência |
|---|---|
| Modelo de leitura `Partitura` (partes, compassos, `Nota` com `posicao`/`duracao` em `Fracao` i64/i64, em semínimas) | `crates/piano-tutor-core/src/partitura.rs:92,370,568` |
| Sem IDs estáveis nas notas; sem referência ao nó XML de origem; parser com `roxmltree` 0.21.2 (somente leitura) | `partitura.rs:1198`, `core/Cargo.toml:14` |
| Exportar = regravar o **texto XML carregado, sem alterar** (hoje round-trip é perfeito porque nada muda) | `tela_partitura.rs:955` |
| Render: layout próprio em egui, **sem fonte musical** (glifos por curvas). Spike egui × Verovio da ADR-0007 **não foi feito** | `partitura_visual.rs:1`, `docs/aceite/F07/evidencia.md:3` |
| Hit test só até **compasso** (`compasso_em`) | `partitura_visual.rs:291` |
| Revisão (M4) já abre o MusicXML no MuseScore externo (`LancadorMuseScore`), não exercido com o MuseScore real | `revisao.rs:930-950` |
| Bravura/OFL-1.1 já aprovada em doc do projeto, mas **não embutida** | `docs/seguranca/LICENCAS-E-DADOS.md:69` |

Consequência: o editor depende do spike de ADR-0007 (motor de render) e da decisão de modelo (seção 5). ADR-0007 diz "edição só depois de F13"; esta pesquisa serve para preparar essa fase, não a antecipa.

## 1. Referências

| Projeto | Licença | Evidência | O que ensina (resumo; sem copiar código) |
|---|---|---|---|
| MuseScore 4 (Studio, última 4.7.6 na página de releases) | GPL-3.0 (repo local `COMPONENTES-VISUAIS.md:39`; leitura direta do LICENSE falhou) | [V] releases; licença [R]/**NÃO VERIFICADO** direto | Modos de entrada explícitos (passo, ritmo, re-pitch, inserir, tempo real); "duração primeiro, altura depois"; comandos por ação nomeada com atalho reconfigurável; **somente referência de UX** |
| Verovio 6.4.1 (npm, 2026-11-03) | LGPL-3.0 (`COPYING` = texto GPLv3 + `COPYING.LESSER`; npm `LGPL-3.0-only`) | [V] | Renderizador (MusicXML/MEI→SVG), espaçamento/vigas/ligaduras de nível editorial, WASM oficial. Modelo interno é MEI com `xml:id` por elemento → ponte natural SVG↔modelo para hit test [M] |
| VexFlow 5.11.0 (2026-11-24) | MIT | [V] npm | Biblioteca de **desenho** (não de layout completo): você posiciona; mostra o custo de fazer o layout por conta própria [M] |
| OpenSheetMusicDisplay 3.0.0 (2026-11-21) | BSD-3-Clause | [V] npm | MusicXML→VexFlow; exemplo de camada "importa → modelo gráfico → desenha" e do que quebra em partituras reais (colisões, tuplets) [M] |
| Smoosic 2.0.4 | MIT | [V] npm | Editor web puro: modelo próprio + SMuFL + undo por snapshots/comandos; é o único editor aberto permissivo para **ler** como arquitetura de referência [M] |
| Denemo | GPL-3.0 (`COPYING`) | [V] raw | Editor sobre LilyPond (render externo + edição por texto); prova a abordagem "render externo, edição semântica" [M]. Somente referência |
| Dorico (Steinberg) | proprietário | — | UX: modos Write, popovers de texto para entrada (`Shift+N`), entrada por altura-antes-da-duração opcional [V parcial: ref. card + índice de ajuda] |
| Sibelius (Avid) | proprietário | — | Teclado numérico para durações; Flexi-time (seção 3) |
| Finale (MakeMusic) | proprietário | — | Speedy Entry (seção 2/3) |
| Flat.io, Noteflight | proprietários (SaaS) | **NÃO VERIFICADO** | Não pesquisados (orçamento). Servem só de benchmark de UX web |

Lições transversais [M, NÃO VERIFICADO como fato]: (a) modelo de documento separado da vista; (b) toda edição vira comando reversível (padrão Command + pilha undo agrupada por transação); (c) entrada de nota é um **modo** com cursor de entrada próprio; (d) o hit test moderno usa "mapa de caixas por elemento" gerado pelo layout (não geometria refeita); (e) relayout incremental por compasso/sistema, não da peça toda.

## 2. Atalhos — R-E04

### 2.1 Fonte e versão

- Fonte [V]: handbook oficial do MuseScore 4, páginas `handbook.musescore.org/basics/entering-notes-and-rests`, `/appendix/all-keyboard-shortcuts.md`, `/basics/default-keyboard-shortcuts.md` e `/basics/alternative-note-input-methods`. **O handbook não declara o número de versão** nas páginas; só marca "a partir da 4.4" para multimeasure rest. A página de releases do GitHub mostra 4.7.6 como "Latest". Portanto: "MuseScore 4.x (handbook sem versão explícita; última release 4.7.6)".
- Pendência: confirmar no código (`notationactions.cpp`, não lido). Nenhuma verificação por versão menor (4.0, 4.1…) foi possível: **NÃO VERIFICADO por versão**, exceto o que o handbook diz.
- MuseScore 3.3.4 (handbook 3) confirma modos e `Ctrl+Shift+I` re-pitch, `1–9` durações [V]; o mapeamento exato 3–7 no MS3 **NÃO VERIFICADO**.

### 2.2 Tabela real de entrada de nota (MuseScore 4, Win/Linux; macOS troca Ctrl→Cmd e Alt→Option) [V]

| Ação | Atalho |
|---|---|
| Entrar/sair do modo de entrada | `N` (repetir `N` sai); `Esc` também sai |
| Notas | `A`–`G` |
| Acrescentar ao acorde | `Shift+A`..`Shift+G` (nota nova **acima**; entre a mais grave primeiro) |
| Intervalo acima da nota | `Alt+1..9` (`Alt+8` = oitava) |
| Duração | `1`=64ª, `2`=32ª, `3`=16ª, `4`=colcheia, `5`=semínima, `6`=mínima, `7`=semibreve, `8`=breve, `9`=longa |
| Ponto de aumento | `.` (alterna) |
| Pausa | `0` (com a duração já escolhida) |
| Alterar altura | `↑`/`↓` = **semitom**; `Ctrl+↑`/`Ctrl+↓` = oitava |
| Acidentes | `+` sustenido, `-` bemol, `=` bequadro; `J` regrafa enarmônico, `Ctrl+J` só altura atual |
| Ligadura de valor (tie) | `T` (cria a 2ª nota junto) |
| Ligadura de expressão (slur) | `S` |
| Quiáltera | `Ctrl+2`..`Ctrl+9` (`Ctrl+3` tercina) |
| Letra (lyrics) | `Ctrl+L` (próx. sílaba `Space`, hífen `-`, melisma `_`) |
| Cifra | `Ctrl+K` (próx. elemento `Space`) |
| Repetir | `R`; dobrar/dividir duração `Shift+W`/`Shift+Q` |
| Voz 1–4 | `Ctrl+Alt+1..4` |
| Desfazer / refazer | `Ctrl+Z` / `Ctrl+Shift+Z` (`Cmd+Shift+Z` no macOS) |
| Modo inserir / re-pitch | `Ctrl+I` / `Ctrl+Shift+I` |
| Teclado virtual | `P` |
| Tempo real, avanço manual | `Enter` do teclado numérico (Fn+Return no Mac); recomendado mapear tecla/pedal MIDI |

### 2.3 Veredito item a item de R-E04

| Item da spec | Veredito | Nota |
|---|---|---|
| `N` entrada | CONFIRMADO | MS4; alterna |
| `Esc` saída | CONFIRMADO | |
| `A–G` notas | CONFIRMADO | |
| `0` pausa | CONFIRMADO | duração antes, depois `0` |
| "3–7 semicolcheia→semibreve" | CONFIRMADO no MS4, **incompleto** | 3=16ª ... 7=semibreve; mas `1,2` = 64ª/32ª e `8,9` = breve/longa. Perfil PT deve decidir se expõe 1–9 |
| Ponto de aumento | CONFIRMADO | `.` |
| "Setas altura" | CONFIRMADO, com ressalva | `↑↓` = semitom no MuseScore; Sibelius usa **diatônico** (seção 2.4). Escolha de produto |
| `Ctrl+setas` oitava | CONFIRMADO | |
| `Shift+nota` acorde | CONFIRMADO | |
| `T` tie | CONFIRMADO | |
| `S` slur | CONFIRMADO | |
| `Ctrl+3` tercina | CONFIRMADO | |
| `Ctrl+L` letra | CONFIRMADO | |
| `Ctrl+K` cifra | CONFIRMADO no MS4; **CONFLITA com o PT** | `Ctrl+K` já abre a paleta de ações (`estado.rs:248`) |
| `Ctrl+Z` | CONFIRMADO | |
| Redo "conforme plataforma" | PARCIAL | handbook lista só `Ctrl+Shift+Z` (`Cmd+Shift+Z`); `Ctrl+Y` **NÃO VERIFICADO** |
| "Não são defaults universais" | CONFIRMADO | Sibelius, Dorico e Finale divergem (abaixo) |

### 2.4 Comparação com outros programas

| Ação | MuseScore 4 [V] | Dorico [V ref. card 2018, sem versão] | Sibelius [V guia ND, sem versão] | Finale 2014 Win [V manual] |
|---|---|---|---|---|
| Iniciar entrada | `N` | `Shift+N` | `N` (`N`/`Esc` sai) | ferramenta Speedy Entry (clique no compasso) |
| Durações | `1–9` linha numérica | teclas `1–9` (mapa completo **NÃO VERIFICADO**) | **teclado numérico** (1=menor, 4=semínima, 6=semibreve) | teclado numérico |
| Pausa | `0` | `,` | `0` alterna nota/pausa | número sem tecla de sintetizador pressionada |
| Ponto | `.` | `.` (`Alt+.` cicla pontos) | `.` do keypad | `.` (até 10 pontos) |
| Acorde | `Shift+letra` | `Q` | `1–9` linha numérica = intervalo acima; `Shift+1–9` abaixo | segurar teclas MIDI + `Enter` |
| Altura | `↑↓` semitom | — (não mapeado) | `↑↓` **diatônico**; `Shift+PgUp/PgDn` semitom | `+`/`-` semitom |
| Oitava | `Ctrl+↑↓` | `Ctrl+Alt+↑↓` | `Ctrl+↑↓` | — |
| Tie | `T` | `T` | NÃO VERIFICADO | `T` ou `=` |
| Slur | `S` | `S` / `Shift+S` fim | NÃO VERIFICADO | — |
| Tercina | `Ctrl+3` | `;` início, `:` fim | `Ctrl+3` | — |
| Letra / cifra | `Ctrl+L` / `Ctrl+K` | `Shift+L` / `Shift+Q` | NÃO VERIFICADO | — |

Leitura: o núcleo `N`, `A–G`, `.`, `T`, `S`, `Ctrl+3`, `Ctrl+↑↓` é compartilhado por MuseScore, Dorico (parcial) e Sibelius (parcial); durações e pausa divergem. Laptops sem teclado numérico favorecem o esquema `1–9` do MuseScore.

### 2.5 Perfil padrão proposto ("MuseScore-like", reconfigurável) e mapa de conflitos

Princípios: (1) teclas de edição valem **só com o editor em foco e sem campo de texto focado** (padrão já usado: `egui_wants_keyboard_input` em `tela_partitura.rs:1348` e `tela_ritmos.rs:36`); (2) camada global do app (Esc, Ctrl+K, Ctrl+., Ctrl+T, Ctrl+F1) tem prioridade; (3) um único registro `Acao → Atalho` reconfigurável, com detecção de colisão.

Atalhos já existentes no PT [R]:

| Atalho PT | Dono | Conflito com perfil MuseScore | Resolução proposta |
|---|---|---|---|
| `Ctrl+.` Pânico | global (`acoes.rs:17`, dispara com campo de texto focado) | nenhum (`.` do MS é sem modificador) | **reservado, intocável**; teste automatizado: nenhuma ação do editor pode ocupar `Ctrl+.` |
| `Esc` | casca: fecha paleta, bastidores, KeyTips | **conflito de prioridade** (Esc sai do modo de entrada) | Esc fecha primeiro a camada da casca; se nada aberto, sai do modo de entrada; segundo Esc limpa seleção |
| `Ctrl+K` | paleta de ações (`estado.rs:248`) | **conflito direto** (cifra) | no editor, cifra = `Ctrl+Shift+K` ou `Alt+K`; manter `Ctrl+K` global |
| `Ctrl+T` tema | global | MS usa `T` sem Ctrl (tie) | sem conflito; evitar `Ctrl+T` no editor |
| `Ctrl+F1` Ribbon | global | nenhum | — |
| `Space` toca/para (partitura e ritmos) | telas | MS também usa `Space` p/ tocar; na letra, `Space`=próxima sílaba | em modo de edição de texto, `Space` pertence ao texto |
| `1–8` pads (tela Ritmos, `tela_ritmos.rs:22`) | só a tela Ritmos | `1–9` durações | telas distintas; **proibido** compartilhar a tela; exclusão por foco |
| Teclas `A S D F G H J K L ;` + `W E T…` tocam o piano (espelho, `tela_espelho.rs:631`) | só tela Teclado | `A–G`, `J`, `T`, `S` | roteamento exclusivo por tela/foco (R-E05 já exige editor × treino mutuamente exclusivos) |
| `Alt` / KeyTips | casca | `Alt+1..9` intervalos | **NÃO VERIFICADO** como o KeyTips é acionado; testar antes de adotar `Alt+N` |
| `Ctrl+Z` | não usado hoje (grep sem ocorrência) | — | livre |

Perfil mínimo da primeira fatia: `N`, `Esc`, `A–G`, `Shift+A–G`, `0`, `3–7` (e opcionalmente `1,2,8,9`), `.`, `↑↓`, `Ctrl+↑↓`, `Delete`/`Backspace`, `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+Y` (alias PT, documentado como nosso). `T`, `S`, `Ctrl+3`, `Ctrl+L`, `Ctrl+K→Ctrl+Shift+K` ficam para a fatia 2. Mouse e seleção (`Shift`=intervalo, `Ctrl`=soma) e foco MIDI: a documentar na ADR do editor (não pesquisados).

## 3. Entrada MIDI step-time e tempo real — para R-E05

### 3.1 O que as fontes dizem

| Programa | Comportamento [fonte] |
|---|---|
| MuseScore 4 passo-a-passo | Teclas em sequência entram uma a uma "desde que cada tecla seja totalmente solta antes da próxima"; **teclas simultâneas viram acorde** (sem janela configurável documentada: o limite é o soltar das teclas) [V handbook]. Acidente do botão da barra influencia a grafia; `J` regrafa [V]. Duração pode ser mapeada a teclas MIDI em Preferências→Mapeamentos MIDI [V] |
| MuseScore **Re-pitch** | `Ctrl+Shift+I`: troca alturas mantendo durações; aceita teclado do PC, MIDI ou piano virtual, **não** mouse [V MS3 e MS4 alt. methods] |
| MuseScore **Inserir** | insere antes do cursor e desloca o resto do compasso; indicador de compasso incompleto [V MS3/MS4] |
| MuseScore **só ritmo** | `1–9` entram durações; altura depois via re-pitch [V MS3] |
| MuseScore **tempo real automático** | metrônomo; andamento = intervalo entre cliques (Preferências→Entrada de notas); segurar a tecla estende a nota a cada clique; soltar interrompe o avanço; pausas exigem iniciar o metrônomo com "Real-time Advance" [V] |
| MuseScore **tempo real manual** | "toque no pedal/tecla" marca o andamento; liberar a tecla fixa a duração [V] |
| Sibelius Flexi-time | opções de "Flexibility of tempo" (None/rubato), "minimum note value" (padrão 32ª), "record into multiple voices"; limpeza posterior com plug-ins (Renotate Performance, Combine Tied Notes, Remove Rests) [V, mas artigo de 2008 sobre Sibelius 5; **versões atuais NÃO VERIFICADAS**] |
| Finale Speedy Entry (2014) | com MIDI: segurar teclas e apertar número de duração; **sem tecla pressionada, o número cria pausa**; acorde = segurar + `Enter` [V] |
| Dorico | existem tópicos "Inputting notes using MIDI recording" e "Optimization for MIDI recording", "MIDI Quantize Options" (aparecem em busca) — **conteúdo NÃO VERIFICADO** (WebFetch esgotado) |
| Quantização / metrônomo no MuseScore 4 | **NÃO VERIFICADO** (a página do handbook não cobre) |

### 3.2 Implicações para R-E05 (propostas, derivadas das fontes)

| Requisito | Proposta | Base |
|---|---|---|
| Acorde por janela | janela configurável (padrão sugerido 30–60 ms; **valor é hipótese**, calibrar no KB-330) agrupando NoteOn; duração = a selecionada; alternativa "soltar tudo" como MuseScore. Oferecer os dois critérios | MuseScore (soltar) × Finale (segurar + confirmar) |
| Re-pitch | modo próprio: cada NoteOn (ou acorde fechado pela janela) substitui a altura da nota/acorde sob o cursor e avança; preserva duração/voz/pauta; sem nota-alvo → termina | MuseScore re-pitch |
| Pausas | tecla `0`/botão em step-time; no tempo real, só quando o metrônomo avança sem tecla | MuseScore RT automático, Finale |
| Tempo real | metrônomo do PT (já existe em Ritmos) + quantização **não destrutiva**: guardar eventos MIDI brutos (tempo ns, nota, velocity) na proposta e derivar notação por grade/duração mínima configurável (padrão sugerido: semicolcheia, não 32ª, como lição do Flexi-time) | Sibelius (mín. de nota, flex. de tempo) |
| Pedal/avanço | tratar como opcional e só após calibração: o PT hoje marca `pedal_sustain` = `NaoObservado` (`piano-tutor-midi/src/capacidades.rs:108`) | repo [R] |
| Robustez | NoteOn vel 0 já vira NoteOff no `core::midi` (`midi.rs:50`); falta cobrir all-notes-off, desconexão e 61 teclas no roteador do editor | [R] |

## 4. Rust, fontes e motor de render — R-E01/R-E03

### 4.1 Crates (consulta crates.io API em 2026-11-27) [V]

| Crate | Versão / data | Licença | Uso / avaliação |
|---|---|---|---|
| `roxmltree` | 0.21.2 / 2026-10-24 (116 M downloads) | MIT OR Apache-2.0 | **já usado**; árvore somente leitura; `Node::range()` (lib.rs:1453) dá faixa de bytes no texto original → permite patch textual (seção 5) |
| `quick-xml` | 0.42.0 / 2026-08-22 | MIT | leitor/escritor de eventos; bom para **regravar** trechos |
| `musicxml` | 1.1.2 / 2024-11-05 (23 k) | MIT | SDK tipado de MusicXML (parse+export). Cobertura do schema, versão (3.1/4.0) e round-trip **NÃO VERIFICADOS**; sem atualização há 12 meses; risco de perder desconhecidos |
| `mei` | 0.3.1 | Apache-2.0 | **REFUTADO como MEI**: descrição "Cargo build subsystem", não é Music Encoding |
| `smufl` | 0.2.1 / 2023-03 | MIT | parseia metadados SMuFL; versão antiga; trivial reescrever com `serde` |
| `verovio` / `verovio-sys` / `verovio-data` | 0.3.6 / 2026-06 | LGPL-3.0-or-later | bindings `cxx` com fontes C++ do Verovio **vendoradas** (README: Verovio 6.2.1, exige C++20); "read+play"; **edição MEI é não-objetivo declarado**; expõe timemap e bbox map [V README] |
| `verovioxide`(-sys) | 0.3.0 / 2026-02 | Apache-2.0 (wrapper; o Verovio embutido continua LGPL) | alternativa de bindings; manutenção **NÃO VERIFICADA** |
| `acorde` (core/io/layout/render-svg) | 2.0.1 / 2026-07-26 (4 k) | MIT OR Apache-2.0 | "layout sem pixels" + render SVG + parsers MusicXML/MIDI/ABC/MSCZ **em Rust puro**; projeto jovem, qualidade de gravura **NÃO VERIFICADA**; candidato a **estudo no spike** (ler, não depender) |
| `cecilia-engrave` | 0.51.0 / criado 2026-11-12 | MIT OR Apache-2.0 | motor de layout→lista de desenho; muito novo; só observar |
| `cadenzier` | 0.1.0-dev | **AGPL-3.0** | editor por teclado; **não copiar**, só referência |
| `mscz`, `dolce-model` | 0.1.0 / 0.9.0 | MIT/Apache | (MSCZ; ScoreDoc com edição, em chinês) imaturos; só observação |

Não achei (nas buscas feitas) crate maduro de **layout musical editorial** em Rust. "notation-rs" **NÃO EXISTE** no crates.io (`notation` é um app de notas, não musical).

### 4.2 Fontes SMuFL [V parcial]

| Fonte | Licença | Evidência |
|---|---|---|
| Bravura (Steinberg) | SIL OFL-1.1, *Reserved Font Name "Bravura"*; copyright 2026 | [V] `LICENSE.txt` no repositório oficial; `redist/otf/Bravura.otf` existe |
| `bravura_metadata.json` | — | caminho `redist/(json/)bravura_metadata.json` deu 404 em `raw.githubusercontent` → **caminho exato NÃO VERIFICADO**; conferir no release do SMuFL/Bravura antes de embutir |
| Leland (MuseScore), Petaluma (Steinberg) | OFL-1.1 declarado em `COMPONENTES-VISUAIS.md:10` | **NÃO VERIFICADO direto** nesta sessão (404 nos caminhos testados) |

OFL-1.1: pode embutir e redistribuir com o texto da licença; proibido vender a fonte isoladamente; **RFN**: se modificar a fonte (subset/conversão), o nome derivado não pode ser "Bravura". Subsetting é "modificação" para fins de RFN [M] — usar o arquivo original inteiro evita a questão.

### 4.3 Verovio e LGPL-3.0 em Rust (análise técnica, **não é parecer jurídico**)

- `verovio-sys` compila o C++ **dentro** do binário (linkagem estática por padrão do cargo). LGPL-3.0 exige que o usuário possa **recombinar/relinkar** com versão modificada da biblioteca [M — texto LGPL §4, não relido nesta sessão]; com binário Rust único, isso significa entregar objetos/arquivos para relink ou fonte do app inteira.
- O projeto hoje tem repositório público **sem licença aberta atribuída** (`README.md:141`): "público sem licença" não é "fonte disponibilizada" → a ponte LGPL não fica resolvida sozinha.
- Rotas que evitam o problema: (a) Verovio como **processo separado** (binário/worker que recebe MusicXML e devolve SVG + JSON de caixas; padrão já usado no projeto para Python/ML); (b) `libverovio.so` **dinâmica** substituível; (c) WASM oficial do npm (`verovio` 6.4.1, LGPL-3.0-only) executado num runtime embarcado — maior custo e **NÃO VERIFICADO** em Rust.
- Pergunta ao titular: tolerância a obrigações LGPL (aviso, fonte do Verovio, relink) vs. custo de manter layout próprio.

### 4.4 Matriz de opções de motor

| Critério | A) Layout próprio egui + SMuFL | B) Verovio renderiza + egui interage | C) Híbrido |
|---|---|---|---|
| Descrição | `calcular_layout` do repo evolui; glifos Bravura via `Painter::text`; mapa de caixas próprio | Verovio gera SVG/bboxes por elemento; egui mostra e captura clique; edição altera o modelo e re-renderiza | Verovio para leitura/PDF/QA e comparação; layout próprio para o editor |
| Qualidade de gravura | depende de nós; já há regras (`partitura_visual.rs:1-10`), glifos aproximados | alta (nível editorial) [M] | alta só na vista Verovio |
| Hit test / seleção | trivial: temos geometria própria | via `xml:id` ↔ SVG/bbox [M] (README do wrapper confirma "bbox map") | dois sistemas |
| Edição interativa ao vivo | natural (relayout parcial por compasso) | re-render do sistema/peça a cada comando (latência, **NÃO MEDIDA**); wrapper declara edição não-objetivo [V] | complexo |
| Licença | tudo nosso + OFL | **LGPL-3.0** (seção 4.3) | LGPL no lado Verovio |
| Dependências | só Rust | C++20, tempo de build, 6.2.1 no wrapper vs 6.4.1 no npm | ambas |
| Esforço relativo (A=100) | 100 (mas **incerteza alta**) | 40–60 só leitura; 70–90 com edição por re-render | 140+ |
| Risco principal | engraving: espaçamento, beams, colisões, ligaduras | acoplamento ao ciclo do Verovio; SVG→egui (rasterizar? `egui_extras`/resvg **NÃO VERIFICADO**) | manter dois renderizadores coerentes |

Riscos de gravura (para A): (1) **espaçamento horizontal** polifônico/duas claves com alinhamento vertical de tempos; (2) **vigas** com 2 pautas, cross-staff, direção de haste e vigas parciais; (3) **colisões** de acidentes, ponto, ligaduras, letra e cifras; (4) **ligaduras de expressão/tie** (curvas que cruzam sistemas, quebra de linha); (5) quiálteras aninhadas; (6) fator de zoom 100/150/200% sem fio de linha fino sumir; (7) acessibilidade/contraste. Esforço relativo por item (A): espaçamento e colisões = alto; vigas = médio; ligaduras = alto; tuplets = médio; glifos SMuFL = baixo (fonte pronta).

**Recomendação provisória (a confirmar pelo spike da ADR-0007):** A para o **editor** (hit test e latência decidem), com Verovio apenas como **oráculo de comparação visual** no spike (processo separado, sem linkar). Se o spike mostrar que A não atinge legibilidade em partituras de piano reais, passar a B em processo separado.

## 5. Modelo de documento: round-trip de MusicXML

Requisitos [R-E01/E02]: preservar desconhecidos, IDs estáveis, durações racionais, "sem perdas silenciosas".

| Abordagem | Como funciona | Prós | Contras |
|---|---|---|---|
| **A. Árvore XML original + overlay de edições** | DOM é a verdade; edições mutam/overlay nós | Preservação máxima; desconhecidos grátis | Invariantes musicais dispersos no DOM (`<backup>`, `<forward>`, `divisions`, `voice`/`staff`, beams recalculados); undo = diff de DOM; modelo musical sem tipos fortes |
| **B. Modelo próprio + "unknown passthrough"** | modelo tipado (como `Partitura`); cada entidade guarda `extras: Vec<Fragmento>` com nós/atributos não entendidos, com índice de posição | Boa edição/undo; testável sem XML | Ordem dos filhos (schema impõe sequência) e referências cruzadas (tie/slur/tuplet) fácil de corromper; **perda silenciosa** em qualquer campo esquecido |
| **C. (recomendada) Modelo próprio + origem por faixa de bytes + patch textual** | Mantém o **texto original** e, por entidade (`parte`, `compasso`, `nota`), a faixa (`Node::range`, verificada em roxmltree 0.21.2) e um bit `sujo`. Exportação: percorre o original; entidade **não suja** é copiada **byte a byte**; entidade suja é regenerada (campos conhecidos, em ordem do schema) **reaproveitando os filhos desconhecidos** lidos dela; nós fora dos compassos (`credit`, `defaults`, `part-list`, `identification`, comentários) ficam intactos | Round-trip perfeito do que não foi editado (igual ao `exportar` atual); perda restringida ao compasso editado; auditável (diff textual) | Compasso editado perde formatação/comentários internos; exige regenerar `<note>` com todos os filhos do schema (priorizar lista fechada + "desconhecido preservado"); índices de faixa só valem para o texto original (aplicar patches **uma vez**, de trás para frente) |

Decisões complementares:

| Tema | Recomendação |
|---|---|
| **Durações** | `Fracao` i64/i64 em semínimas (já existe). Na exportação: `divisions` = MMC dos denominadores usados (ou manter o original se todo valor couber); tuplets: valor real = nominal × normal/atual; **erro explícito** se não representável, nunca arredondar. `NÃO VERIFICADO`: limites de overflow do `Fracao` em peças longas (testar) |
| **IDs estáveis** | `IdNota(u64)` interno, monotônico por documento, não reutilizado, atribuído na leitura; **não** usar índice. Persistir no revisão: gravar o `id` do MusicXML só se já existir; senão `<nome>.ids.toml` ao lado (evita poluir o XML). Se o atributo `id` por elemento existe no MusicXML 4.0 é **NÃO VERIFICADO** nesta sessão |
| **Seleção/range** | por `(IdNota, IdNota)` + pauta/voz, não por posição |
| **Comando** | enum tipado + transação; cada comando devolve seu inverso (ou snapshot do compasso afetado: mais simples e seguro na 1ª fatia); validação antes de aplicar (soma de durações da voz = compasso, ou sinal de compasso incompleto como o MuseScore) |
| **Teste de aceite do round-trip** | (1) corpus `assets/partituras/` (OpenScore Lieder, CC0): `exportar(importar(x)) == x` byte a byte sem edição; (2) edição única: `diff` textual limitado ao compasso; (3) reimportar e comparar modelos; (4) matriz R-E01 por elemento com "não suportado" visível; **não declarar conformidade integral sem corpus** |

## 6. Primeira fatia mínima (corrige uma transcrição OMR de piano)

Objetivo: o revisor abre uma Hipótese, corrige erros típicos de OMR e salva como **revisão** sem tocar a Hipótese original. Fora: letras, cifras, tuplets novos, arrastar, múltiplos instrumentos, entrada MIDI.

| # | Item | Critério demonstrável |
|---|---|---|
| 1 | Modelo editável com `IdNota` + origem por faixa + `sujo` (seção 5-C), dentro de `piano-tutor-core`, sem egui | testes de unidade: round-trip byte a byte do corpus; só 1 compasso muda no diff |
| 2 | Comandos: `AlterarAltura(id, ±semitom/±oitava)`, `AlterarDuracao(id, figura, pontos)`, `InserirNota/Pausa(compasso, pos)`, `RemoverNota(id)` (vira pausa, mantendo a duração), `AdicionarAoAcorde(id, altura)` | validação de duração do compasso; rejeita comando inválido sem alterar nada |
| 3 | Transação + undo/redo (pilha com snapshot do compasso afetado) | `Ctrl+Z`/`Ctrl+Shift+Z`; N desfazeres restauram exatamente |
| 4 | Seleção de **nota** no render (hoje só compasso) e cursor de entrada | clique seleciona; setas ←→ navegam; `Tab` muda voz/pauta (decisão) |
| 5 | Atalhos do perfil mínimo (seção 2.5) | testes de roteamento: `Ctrl+.` continua disparando o Pânico; `Esc` respeita a casca |
| 6 | Salvar como revisão: grava `<id>.revisao-N.musicxml` na oficina (sem sobrescrever a Hipótese) e registra no `[[historico]]` do `.estado.toml` (`toml_edit` já usado) | a Hipótese segue byte-idêntica; a aprovação continua humana (CONTEXT.md) |
| 7 | Playback da edição (já existe) com a nota nova soando | tocar o compasso editado |

Ficam para a fatia 2: `T`, `S`, `Ctrl+3`, MIDI step-time (R-E05 parcial: acorde e re-pitch), letras/cifras (R-E06). Dependências bloqueantes: **spike ADR-0007** (motor de render), ADR de modelo de documento (C acima), decisão sobre ordem F13.

## Perguntas ao titular

| # | Pergunta | Opções | Recomendação |
|---|---|---|---|
| 1 | Motor de render do editor | A) próprio egui+Bravura; B) Verovio em processo separado; C) Verovio linkado | A para editar, Verovio só como oráculo no spike |
| 2 | Aceita obrigações LGPL do Verovio (aviso, fonte, relink) | sim/não; processo separado/`.so` | processo separado se usar |
| 3 | Licença aberta para o código do PT (afeta LGPL e reuso) | atribuir/manter sem licença | decidir antes de qualquer linkagem LGPL |
| 4 | Setas de altura | semitom (MuseScore) ou diatônico (Sibelius) | semitom + `Shift+↑↓` diatônico? ou o inverso — **escolha do titular**, usuário-alvo é pianista (semitom reflete o teclado) |
| 5 | `Ctrl+K` | manter paleta global e usar `Ctrl+Shift+K` p/ cifra; ou trocar a paleta | manter paleta |
| 6 | Durações: expor `1–9` ou só `3–7` | 1–9 completo (MS4); 3–7 | 1–9, pois "MS4" é o modelo declarado |
| 7 | Salvar edição | revisão separada (recomendado) ou sobrescrever a Hipótese | revisão separada |
| 8 | Ordem: editor antes ou depois de F13 | ADR-0007 diz depois | manter depois; usar a fatia 1 como spike de modelo antes |

## Lacunas (o que não foi possível verificar)

1. `notationactions.cpp` do MuseScore e a versão exata do handbook (WebFetch/GitHub API bloqueados; orçamento esgotado).
2. Mapeamento completo de durações do Dorico e do Sibelius; atalhos de tie/slur/lyrics no Sibelius; Dorico/Sibelius atuais (cards de 2018, guia sem versão).
3. Páginas Dorico "MIDI recording"/"Quantize", Sibelius Reference PDF (só títulos vistos); quantização e metrônomo no MuseScore 4.
4. Flat.io e Noteflight (nada lido).
5. Licença direta de MuseScore, Leland, Petaluma; caminho de `bravura_metadata.json`.
6. Qualidade de `musicxml` 1.1.2, `acorde`, `cecilia-engrave`, `verovioxide`; round-trip real de cada um.
7. Latência de re-render do Verovio e carga de CPU; renderização SVG→egui.
8. Existência do atributo `id` por elemento em MusicXML 4.0; comportamento do KeyTips com `Alt+dígito`.
9. Nada foi testado com hardware (KB-330/REVAS) ou com o MuseScore real; janela de acorde de 30–60 ms é hipótese.
