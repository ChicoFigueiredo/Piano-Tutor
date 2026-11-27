# Bravura (SMuFL): fonte de notação do spike de render

Ticket evolucao/09 e [ADR-0018](../../../docs/decisoes/ADR-0018-motor-de-render-egui-bravura.md).

A Bravura é a fonte musical de referência do SMuFL (Standard Music Font Layout), da Steinberg
Media Technologies GmbH. Licença: **SIL Open Font License 1.1 (OFL-1.1)**, texto integral em
[`OFL.txt`](OFL.txt), com o aviso de copyright e o Reserved Font Name "Bravura".

## Origem e versão

| Item | Valor |
| --- | --- |
| Origem oficial | repositório `steinbergmedia/bravura`, ramo `master`, pasta `redist/` (baixado em 2026-11-27 por `raw.githubusercontent.com`) |
| Versão | **1.482** (campo `head.fontRevision` do OTF = 1,482; `name` ID 5 = "Version 1.482"; `fontVersion` do JSON = 1.482; mais recente no FONTLOG de 24/08/2026) |
| Commit exato | **não determinado** (a API do GitHub não estava acessível no ambiente; só os arquivos brutos) |
| SMuFL | a metadata segue a especificação SMuFL 1.x (o FONTLOG cita 1.2 como base) |

## Arquivos versionados (só o necessário)

| Arquivo | Origem | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `Bravura.otf` | `redist/otf/Bravura.otf` | 889.228 | `cdf0f893ee1fdb64b7f6713d71ee0dcfc349c0ac01429a8e451b01a9e79f5f3b` |
| `Bravura.json` | `redist/Bravura.json` (a metadata SMuFL: `glyphBBoxes`, `glyphAdvanceWidths`, `glyphsWithAnchors`, `engravingDefaults`) | 1.256.995 | `489fb4d734fc0b72a3cc601b50ebdf0bf8c57802ab9893e52ce030b86bf2ce11` |
| `OFL.txt` | `LICENSE.txt` na raiz do repositório de origem (copyright 2026) | 4.420 | `bdca252186865f3b2c0e76da4a4e335c14ba1d37f80194fca254ddfa88e84fbf` |

**Não** incluídos de propósito: `BravuraText.otf` (3 MB), `Bravura.woff2`, `FONTLOG.txt` e
`OFL-FAQ.txt`. Para conferir, recalcule o SHA-256 dos arquivos aqui e compare com a tabela.

## Como o spike usa

- Só testes: `crates/piano-tutor-app/src/partitura_visual/spike_bravura.rs` embute o OTF e o JSON
  por `include_bytes!`/`include_str!` **dentro de `#[cfg(test)]`**. O app e o binário de produção
  **não** carregam a fonte (nada muda no render atual).
- Os pontos de código SMuFL usados (claves, cabeças, pausas, acidentes, bandeirolas, algarismos de
  fôrma e de tercina, dedilhado, pontos de repetição, chave) foram conferidos contra o OTF (mapa
  de caracteres) e contra os `glyphBBoxes` da metadata: a caixa de cada glifo no OTF bate com a
  metadata (tolerância de 6 unidades de 1.000 por em). Isso foi feito por script local; o teste
  `todo_glifo_usado_existe_na_metadata_da_bravura` cobre a parte da metadata na CI.

## Obrigações da OFL-1.1 para quem adotar a fonte no app

1. Toda cópia distribuída (inclusive embutida no binário ou no `.deb`) leva o aviso de copyright e
   o texto da licença (`OFL.txt`); numa tela de licenças do app, ou no pacote, de forma visível.
2. A fonte não pode ser vendida isoladamente (vender o app com ela é permitido).
3. **Reserved Font Name:** qualquer versão modificada (inclusive um *subset* para emagrecer o
   binário) não pode usar o nome "Bravura"; teria de ser renomeada. Embutir o OTF **inteiro e
   sem alteração** evita o problema (889 KB).
4. A licença da fonte não contamina o código (MIT OR Apache-2.0) nem as partituras desenhadas
   com ela.
