# Matriz por elemento MusicXML (começo, ticket 19)

Fase E01. Esta é a **primeira versão** da matriz por elemento prevista em R-E01 e na especificação (importar, renderizar, tocar, editar, exportar). Ela só cobre o que o ticket 19 tocou ou classificou. **"Não suportado" é um valor explícito**, não uma lacuna de preenchimento. **Não há declaração de conformidade integral com o MusicXML**: o corpus é pequeno e amigável (16 fixtures, ver `crates/piano-tutor-fixtures/fixtures/INDICE.md`), e exportações reais de MuseScore, Finale, Sibelius e OMR **não foram testadas** (NÃO VERIFICADO).

Legenda: **sim** = feito e testado; **parcial** = feito com a limitação dita; **não** = não suportado hoje (o elemento continua no arquivo); **n/a** = não se aplica; **preservado** = volta byte a byte ao exportar (A1) e, no compasso editado, volta pelas peças do modelo híbrido (ADR-0017).

Fonte da verdade do que a Oficina lista como "não suportado": a tabela `veredito` de `crates/piano-tutor-core/src/documento/exibicao.rs`. Esta página a resume.

## Elementos que a E01 mostra e preserva (D13: edição é da E03)

| Elemento | Importar | Renderizar | Tocar | Editar | Exportar | Observações |
| --- | --- | --- | --- | --- | --- | --- |
| Letra (`<lyric>`, várias estrofes) | sim: sílaba, estrofe, hífen (`syllabic`), elisão, melisma (`extend`) lidos e ligados à `IdNota` | parcial: uma linha por estrofe abaixo da pauta, com altura reservada; hífen entre sílabas | n/a (não soa) | **não** (E03) | preservado | Sem linha de prolongamento do melisma e sem elisão especial no desenho (`prolongada` é lido, não desenhado). Alinhamento centrado na cabeça. |
| Cifra (`<harmony>`) | sim: raiz, tipo (o atributo `text` manda), graus, baixo | parcial: texto acima da pauta, acima do número do compasso e das casas | n/a | **não** (E03) | preservado | `♯` e `♭` saem como `#` e `b`. `<offset>` ignorado (a cifra vale para a próxima nota). Cifra em compasso sem nota fica no início do compasso. |
| Dedilhado (`<fingering>`) | sim: texto e `substitution` | parcial: glifos SMuFL `fingering1` a `5` da Bravura, do lado oposto da haste; troca `1-2` com traço; outro texto (`p`, `i`...) em texto comum | n/a | **não** (E03) | preservado | Dígitos 6 a 9 não usam glifo (a verificação deles diverge, ver `assets/fontes/bravura/LEIA-ME.md`). Dedo por nota de acorde empilha; não há desvio de linha suplementar. |

## Elementos reconhecidos por nome, preservados, **não desenhados nem editados**

Todos entram na lista "Elementos que o editor ainda não desenha nem edita" da Oficina, com nome e quantidade. **Importar** aqui significa "reconhecido e contado", não "modelado".

| Elemento (nome no MusicXML) | Categoria na lista | Importar | Renderizar | Tocar | Editar | Exportar |
| --- | --- | --- | --- | --- | --- | --- |
| `dynamics` (em `direction-type` e em `notations`) | dinâmica | contado | **não** | **não** (não altera a intensidade) | **não** | preservado |
| `wedge` | crescendo e diminuendo | contado | **não** | **não** | **não** | preservado |
| `words`, `rehearsal`, `metronome`, `bracket`, `dashes`, `symbol`, `other-direction` | texto de direção, marca de ensaio, marca de andamento, outra direção | contado | **não** | `metronome` e `sound tempo` valem para o andamento tocado; o resto **não** | **não** | preservado |
| `pedal`, `octave-shift`, `segno`, `coda` | pedal, 8va e 8vb, segno e coda | contado | **não** | **não** | **não** | preservado |
| `trill-mark`, `turn`, `mordent`, `inverted-mordent`, `tremolo`, `shake`, `wavy-line`, `schleifer`, `haydn`, `accidental-mark` e variantes | ornamento | contado | **não** | **não** (sem execução do ornamento) | **não** | preservado |
| `staccato`, `accent`, `tenuto`, `strong-accent`, `breath-mark`, `caesura` e demais de `articulations` | articulação | contado | **não** | **não** (sem efeito na duração tocada) | **não** | preservado |
| `fermata` (em `notations` e em `barline`) | fermata | contado | **não** | **não** | **não** | preservado |
| `arpeggiate`, `non-arpeggiate`, `glissando`, `slide` | arpejo, glissando | contado | **não** | **não** | **não** | preservado |
| `technical` além de `fingering` (`harmonic`, `up-bow`, `fingernails`, `string`, `fret`...) | técnica | contado | **não** | **não** | **não** | preservado |
| `figured-bass`, `grouping`, `link`, `bookmark`, `print`, `transpose`, `staff-details`, `measure-style`, `notehead`, `cue`, `unpitched` | categoria própria (baixo cifrado, agrupamento, layout de página e sistema...) | contado | **não** | **não** | **não** | preservado |
| Qualquer outro nome (extensões `x-...`, elementos fora da tabela) | desconhecido | contado | **não** | **não** | **não** | preservado |

Um elemento **desconhecido nunca aborta a abertura** (T8): é listado como "desconhecido" e preservado.

## O que a matriz **não** diz (NÃO VERIFICADO)

- Que a tabela de nomes esteja completa: o MusicXML 4.0 tem muito mais elementos; o que não está na tabela cai em "desconhecido" (e é listado, não ignorado). Sem o XSD nesta sessão, a completude da tabela **não foi conferida** contra o esquema.
- Preservação em exportações reais de outros programas (só o corpus CC0 do projeto e variações sintéticas foram exercitados).
- Legibilidade do desenho de letra, cifra e dedilhado: depende do checklist visual do titular (`capturas-19/LEIA-ME.md`).

## Evidência

- Leitura e lista: `crates/piano-tutor-core/src/documento/exibicao.rs` (testes de unidade) e `crates/piano-tutor-fixtures/tests/documento_exibicao.rs` (A3, A15, T8, preservação, corpus inteiro).
- Desenho (A18): `crates/piano-tutor-app/src/partitura_visual/textos_testes.rs`.
- Lista na Oficina e imutabilidade por teclas e cliques: testes `a15_*` e `letra_cifra_e_dedilhado_nao_sao_editaveis_*` em `crates/piano-tutor-app/src/tela_partitura.rs`.
