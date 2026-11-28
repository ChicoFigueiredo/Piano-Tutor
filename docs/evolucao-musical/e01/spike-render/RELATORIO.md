# Spike de render (ticket 09): egui + Bravura contra o Verovio como oráculo

Fase E01. Fecha a pendência de render da [ADR-0007](../../../decisoes/ADR-0007-partitura-somente-leitura.md) com a [ADR-0018](../../../decisoes/ADR-0018-motor-de-render-egui-bravura.md). Data: 2026-11-27.

**Estado de verificação, em uma linha:** o protótipo roda, desenha o corpus inteiro sem pânico e as contagens batem com o oráculo na maioria dos elementos; **a legibilidade é NÃO VERIFICADA até o titular assinar o [checklist visual](CHECKLIST-VISUAL.md)**. Tudo o que depende de GPU real, de tela HiDPI, de Windows ou macOS também é NÃO VERIFICADO (ver seção 8).

## 1. O que foi construído

- **Protótipo** em `crates/piano-tutor-app/src/partitura_visual/spike_bravura.rs` (só em `#[cfg(test)]`; ~1.900 linhas com a tabela de glifos, mais ~580 de testes e captura). **Reaproveita** do layout próprio atual o que não depende de fonte: quebra em sistemas, espaçamento horizontal e colunas por tempo, análise de vozes, vigas, colunas de acidentes e empilhamento vertical das pautas. **Redesenha com a Bravura** o que hoje é forma própria: claves (sol, fá, dó, 8va/8vb), armadura, fôrma de compasso (algarismos e C), cabeças (breve, semibreve, mínima, preta), pausas (breve a quasifusa), acidentes (os cinco), bandeirolas, pontos de aumento, número de tercina, pontos de repetição e a chave. Hastes, vigas, linhas, barras e arcos continuam geometria (como no Verovio e em qualquer gravura). Posições e espessuras vêm da **metadata oficial** (`Bravura.json`: caixas, avanços, âncoras de haste, `engravingDefaults`).
- Acrescenta o que o modelo atual descarta (ticket 08 trata do modelo): **letra, cifra e dedilhado**, lidos direto do MusicXML (`extrair_extras`) e desenhados como texto (letra, cifra) ou glifo SMuFL (dedilhado 1 a 5) junto das notas.
- **Cena neutra** (`Cena`: glifos, linhas, polígonos, caminhos e textos em sp, com `IdNota` por cabeça) pintada por `pintar_cena` no egui (família de fonte `Bravura` registrada em `FontDefinitions`; a linha de base de cada glifo vem do `Glyph::pos` do próprio epaint).
- **Capturas = o que o egui produz**: o teste pinta num `Context` sem janela, tessela e rasteriza em software os triângulos, incluindo o atlas de fonte. Um teste confere que a tinta da cabeça da Bravura cai na caixa da metadata com folga de 1,5 px (a Bravura chega ao egui e na posição certa).
- **Oráculo**: `scripts/spike-render/comparar_verovio.py` roda o Verovio (pacote PyPI 6.4.1) num processo Python separado, gera SVG por peça e zoom, conta elementos, mede posições e monta PNG lado a lado. **Nunca linkado, fora do Cargo, fora da CI** (teste `verovio_nao_e_dependencia_do_cargo`).
- **Corpus**: as 15 fixtures do ticket 07 (10 próprias CC0, `cem-compassos` gerada, 3 lieder OpenScore CC0, 2 peças do projeto CC0). A comparação visual e numérica usa as 5 peças reais (`brilha-brilha`, `beethoven-ode`, `brahms-wiegenlied`, `schubert-heidenroeslein`, `faure-barcarolle`) mais `cem-compassos`, a 100% e 150%; as 10 próprias exercitam um caso cada (letra, cifra, dedilhado, tercina, anacruse, repetição, duas vozes, dinâmica, ornamento, elementos desconhecidos).

## 2. Resultados objetivos

Fonte: `RELATORIO-OBJETIVO.md` (gerado pelo script; todas as peças e os dois zooms). Números a 100%, protótipo / Verovio.

| elemento | brilha-brilha | beethoven-ode | brahms-wiegenlied | schubert-heidenroeslein | faure-barcarolle | cem-compassos |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cabeças | 66 / 66 | 94 / 94 | 253 / 253 | 196 / 196 | 663 / 663 | 400 / 400 |
| hastes | 66 / 66 | 94 / 94 | 179 / 179 | 148 / 148 | 493 / 493 | 400 / 400 |
| vigas (polígonos) | 0 / 0 | 2 / 2 | 15 / 15 | 37 / 45 | 120 / 136 | 0 / 0 |
| bandeirolas | 0 / 0 | 3 / 3 | 41 / 41 | 90 / 82 | 115 / 115 | 0 / 0 |
| pausas | 0 / 0 | 0 / 0 | 14 / 14 | 54 / 54 | 19 / 19 | 0 / 0 |
| acidentes | 0 / 0 | 0 / 0 | 2 / 2 | 10 / 10 | 67 / 67 | 0 / 0 |
| pontos de aumento | 0 / 0 | 3 / 3 | 10 / 10 | 3 / 3 | 334 / 334 | 0 / 0 |
| ligaduras (valor e expressão) | 0 / 0 | 0 / 0 | 74 / 67 | 22 / 16 | 226 / 222 | 0 / 0 |
| sílabas de letra | 0 / 0 | 0 / 0 | 90 / 94 | 138 / 138 | 120 / 123 | 0 / 0 |
| dinâmicas | 0 / 0 | 0 / 0 | 0 / 1 | 0 / 3 | 0 / 15 | 0 / 0 |
| ornamentos e fermata | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 6 | 0 / 0 | 0 / 0 |
| linhas suplementares | 0 / 0 | 15 / 15 | 30 / 28 | 8 / 9 | 150 / 129 | 98 / 98 |

Cabeças e hastes batem em tudo (inclusive nas fixtures próprias). Letra, cifra e dedilhado das fixtures próprias batem (6, 3 e 6 elementos; Verovio idem). As diferenças restantes vêm de: dinâmica, ornamento e fermata (o modelo atual descarta e o protótipo não desenha); agrupamento de vigas (regra do modelo e do layout herdado, ver lacuna 5); contagem de ligaduras e de sílabas (Verovio conta de outro jeito ou parte em duas; **causa não investigada**); linhas suplementares (o Verovio agrupa por nota/acorde).

**Posição relativa** (fração da largura do compasso, centro de cada cabeça; compassos com a mesma contagem de cabeças nos dois lados):

| peça | compassos comparados | desvio médio | desvio máximo |
| --- | ---: | ---: | ---: |
| brilha-brilha, 100% | 12 de 12 | 0,059 | 0,168 |
| beethoven-ode, 100% | 16 de 16 | 0,053 | 0,133 |
| brahms-wiegenlied, 100% | 19 de 19 | 0,094 | 0,389 |
| schubert-heidenroeslein, 100% | 16 de 16 | 0,069 | 0,208 |
| faure-barcarolle, 100% | 42 de 42 | 0,072 | 0,232 |
| cem-compassos, 100% | 100 de 100 | 0,043 | 0,118 |

Em média as cabeças ficam a 4% a 9% da largura do compasso do lugar onde o Verovio as põe: é diferença de espaçamento (estilo), não erro. **Quebra de sistemas:** o protótipo (herdando o layout atual) põe menos compassos por sistema que o Verovio (por exemplo, 13 sistemas contra 10 no Faure, a 100%), então a posição das barras por sistema só coincide em poucos sistemas (o desvio máximo da barra, onde a quebra coincide, ficou em 0,001 a 0,053 da largura do sistema; ver o relatório objetivo).

**Colisões no protótipo** (sobreposição de caixas de símbolos e textos; ver limites na seção 8): 0 em `brilha-brilha` e `beethoven-ode`; 2 em `cem-compassos` (cabeça contra número de compasso); 23 em Brahms, 98 em Schubert, 77 em Faure, a 100%. Por tipo de par: bandeirola contra cabeça domina (9 em Brahms, 54 em Schubert, 49 em Faure; **provável falso positivo da medição por caixa**, pois a caixa da bandeirola inclui a haste); envolvendo letra há 14 em Brahms, 42 em Schubert (bandeirola 31, cabeça 8, acidente 2, letra 1) e 7 em Faure (letra×tercina 6, letra×letra 1), reflexo da falta de reserva de espaço vertical (lacuna 1); ponto de aumento contra cabeça: 20 em Faure (lacuna 12).

## 3. Desempenho (release, mesma máquina, 100%)

Build `--release`; uma execução, números **só registrados** (T9: sem alvo). Layout + cena = `calcular_layout` + `montar_cena`; pintura = construir todos os `Shape` no egui (sem recorte, página inteira); tesselação = `Context::tessellate`.

| peça | compassos | layout + cena (ms) | pintura egui (ms) | tesselação (ms) | hit test (µs/consulta) | Verovio carga + render SVG (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| brilha-brilha | 12 | 0,49 | 0,84 | 0,07 | 0,49 | 5,5 + 3,9 |
| beethoven-ode | 16 | 0,77 | 1,05 | 0,06 | 0,88 | 6,9 + 5,0 |
| brahms-wiegenlied | 19 | 2,37 | 3,34 | 0,17 | 1,58 | 24,7 + 20,7 |
| schubert-heidenroeslein | 16 | 2,17 | 3,30 | 0,14 | 1,85 | 21,4 + 18,8 |
| faure-barcarolle | 42 | 6,82 | 4,62 | 0,56 | 1,25 | 79,6 + 102,4 |
| **cem-compassos** | **100** | **1,16** | **0,98** | **0,12** | **0,55** | **20,4 + 21,2** |

- **Hit test por nota** na peça de 100 compassos: 0,55 µs por consulta (release; 2.000 consultas espalhadas; só registrado). Em build de depuração: 5,08 µs. A varredura é linear nas cabeças dos sistemas cuja faixa vertical contém o ponto; sem índice espacial.
- O Verovio rodou **dentro** do processo Python (carga e render SVG medidos separados); a partida de um processo externo e a transferência do SVG **não foram medidas** e se somariam a cada edição. Para a edição interativa, o ciclo "editar, reler, relayout" do Verovio (dezenas a ~200 ms nas peças maiores) fica acima do orçamento de um quadro; o protótipo fica entre 1 e 12 ms por página inteira nas peças medidas.
- Relayout incremental **não foi necessário** neste corpus; a decisão fica adiada para a baseline do ticket 26 (como pede o ticket).

## 4. Lista de lacunas do render próprio contra o oráculo

Severidade: **A** (alta: atrapalha a leitura do que a E01 promete), **M** (média: legível, mas visivelmente aquém), **B** (baixa). Custo: **P** (até 1 dia), **M** (2 a 4 dias), **G** (5 a 10 dias), em dias de trabalho de agente com revisão. **Os custos são estimativas do agente, não medidas.** "Evidência" aponta para as capturas em `capturas/` ou para o relatório objetivo.

| # | Elemento | Lacuna (evidência) | Sev. | Custo |
| --- | --- | --- | :-: | :-: |
| 1 | **Letra** | Sem reserva de espaço vertical: com 2 ou 3 versos a letra invade a pauta de baixo (`schubert-heidenroeslein-100.png`, `brahms-wiegenlied-100.png`; em `letra-100.png` o verso 2 fica cortado pela borda inferior da página do protótipo). Largura da sílaba não entra no espaçamento horizontal (sílabas se tocam). Sem linha de prolongamento de melisma, hífen só colado à sílaba, sem elisão, alinhamento centrado (não à esquerda da cabeça). O Verovio resolve tudo isso. | **A** | **M a G** (4 a 6 dias: passe de layout vertical por verso e largura mínima por sílaba em `planejar_compasso`) |
| 2 | **Cifra** | Texto acima da pauta sem reserva: encosta no número do compasso (`cifra-100.png`) e nas casas de repetição. ♯ e ♭ saem como `#` e `b` no texto (a Bravura tem os glifos), sem sobrescrito de extensões. | M | M (2 a 3 dias) |
| 3 | **Dedilhado** | Dígitos 1 a 5 em glifo Bravura (`fingering1` a `5`), do lado oposto da haste (`dedilhado-100.png`). Faltam substituição (5-1), dedo por nota de acorde e desvio de linha suplementar e de ligadura. Dígitos 6 a 9 não usados (verificação deles diverge, ver `LEIA-ME` da fonte). | B | P (1 a 2 dias) |
| 4 | **Ligaduras** | Arcos do layout atual (espessura constante, Bézier simples). O Verovio usa espessura variável (a metadata traz `slurEndpointThickness` 0,1 e `slurMidpointThickness` 0,22) e evita hastes e cabeças; no Faure alguns arcos da mão esquerda cruzam cabeças. | M | M (3 a 4 dias: polígono em lente e desvio vertical simples) |
| 5 | **Vigas** | Algoritmo herdado (desnível máximo de 1 sp). As contagens diferem do oráculo em Schubert (37 contra 45) e Faure (120 contra 136): vigas parciais (ganchos) e quebra de viga por pulso. Na fixture `tercina` o agrupamento automático do **modelo** divide o grupo em 2+1 (o arquivo não traz `<beam>`; o Verovio não vigia). Sem viga entre pautas. Problema de regra de agrupamento (modelo e layout), não da fonte. | M | M (3 a 5 dias) |
| 6 | **Tercinas** | Número em glifo (`tuplet3`) e colchete simples; o colchete não inclina com a viga, só o número (sem razão 3:2). Em Faure o número encosta na letra (6 caixas). | B | P (1 a 2 dias) |
| 7 | **Acidentes** | Os cinco tipos agora são glifos Bravura (acabou o desenho manual). Faltam acidente de cortesia e entre parênteses; as colunas de acidentes são a regra herdada. | B | P |
| 8 | **Repetições** | Barras (fina e grossa pela metadata), pontos em glifo `repeatDot` e casas 1/2 funcionam (`repeticao-100.png`). Faltam segno, coda, D.C./D.S. e repetição de compasso (glifos existem na Bravura; fora do corpus da E01). | B | P a M |
| 9 | **Dinâmica, crescendo, pedal, ornamento, fermata, andamento, nome das partes** | O modelo atual descarta e o protótipo não desenha. O Verovio desenha (dinâmicas: 1, 3 e 15 em Brahms, Schubert e Faure; ornamentos e fermata: 6 em Schubert). Depende do modelo (ticket 08) e do E03. | M | G (por grupo, 3 a 5 dias cada) |
| 10 | **Baixo cifrado e extensões `x-`** | Sem suporte (o Verovio desenha o `figured-bass` de `desconhecido`). | B | fora do v1 |
| 11 | **Pausas e vozes** | Glifos de pausa corretos, deslocamento por voz herdado (±1 sp), sem desvio fino contra notas da outra voz (`duas-vozes`). | B | P a M |
| 12 | **Acordes com segunda e pontos** | No Faure há 20 pares ponto contra cabeça por caixa; possível colisão real em acorde com cabeça deslocada (regra de ponto herdada do layout). Falta conferir olhando. | B | P |
| 13 | **Espaçamento horizontal** | Herdado: menos compassos por sistema que o Verovio; cabeças a 4% a 9% da largura do compasso de onde o oráculo as põe. Estilo, não defeito. | B | n/a |
| 14 | **Apojaturas** | Cabeça em 70% sem barra de acciaccatura. | B | P |

Resumo: nenhum item exige trocar de motor. A lacuna **A** (letra) é de **layout vertical**, que o Verovio resolve e qualquer layout próprio, inclusive o atual, teria de resolver; não é limitação do egui nem da Bravura. Soma estimada para chegar ao que o oráculo faz nesse corpus (itens 1 a 9): da ordem de 20 a 30 dias de agente com revisão (**estimativa, não medida**).

## 5. Esforço comparado

| Caminho | Esforço (estimado) | Observação |
| --- | --- | --- |
| **Layout próprio em egui + Bravura (proposto)** | Protótipo: uma sessão de agente (~1.900 linhas, reaproveitando o layout existente e a geometria do ticket 06). Para fechar as lacunas: ~20 a 30 dias (seção 4). | Identidade de nota (`IdNota`) e hit test nativos; relayout em milissegundos; licença limpa; sem processo externo. |
| Verovio como motor (processo separado) | Estimativa do agente, **sem protótipo**: integrar processo, SVG e `id`s; desenhar SVG no egui exigiria um rasterizador de SVG (dependência nova) ou converter para primitivas; a cada edição reenviar o MusicXML inteiro. | LGPL-3.0 (processo separado evita o linkamento, mas há empacotamento e aviso); latência por edição de dezenas a ~200 ms; sem layout incremental; o hit test dependeria do mapa de `id`s do SVG. |
| Verovio linkado (C++/FFI) | **Descartado** pela política (T6 e ADR-0015: nada LGPL linkado). | |

## 6. Hit test, identidade de nota e ticket 06

- `Cena::nota_em` usa a caixa do glifo da cabeça (metadata) alargada em `tol`, escolhe a mais próxima do centro e só varre sistemas cuja faixa vertical contém o ponto. Teste: o centro de cada cabeça devolve o próprio `IdNota`; ponto longe devolve `None`. As cabeças do protótipo recebem `IdNota` sequenciais **provisórios** (só o protótipo); a identidade estável é do ticket 10.
- O **golden de primitivas do ticket 06** (`golden_de_primitivas_das_partituras_embutidas`) continua passando **sem alteração**: o protótipo não muda nenhum código de produção (a única linha de produção tocada é a declaração `#[cfg(test)] mod spike_bravura;`). O render atual do app segue exatamente como estava.

## 7. Como reproduzir

```
PT_SPIKE_SAIDA=/tmp/spike cargo test --release -p piano-tutor-app --lib exportar_corpus -- --ignored
python3 -m venv /tmp/spike-venv && /tmp/spike-venv/bin/pip install verovio cairosvg pillow
/tmp/spike-venv/bin/python scripts/spike-render/comparar_verovio.py /tmp/spike --recorte 560
python3 scripts/spike-render/verificar_glifos.py assets/fontes/bravura/Bravura.otf assets/fontes/bravura/Bravura.json   # precisa de fonttools
```

As capturas completas e o SVG do Verovio por peça ficam em `/tmp/spike/comparacao/` (fora do git). Em `capturas/` estão só 11 recortes pequenos (topo de cada lado; protótipo em cima, Verovio embaixo), em tons de cinza. O rótulo "NO GLYPH" no Verovio é a nota do andamento que a fonte de texto do pacote não tem: limitação do oráculo neste ambiente, não do protótipo.

## 8. O que NÃO foi verificado

- **Legibilidade** (100% e 150%, duas claves, letra, cifra, dedilhado): julgamento humano pendente (ver `CHECKLIST-VISUAL.md`). A impressão do agente ao olhar as capturas (cabeças, claves, armadura, fôrma, pausas e acidentes legíveis; letra sobreposta em peças com vários versos) **não vale como aceite**.
- **GPU real**: as capturas são o que o egui tesselou, rasterizado em software no teste; não rodou com a janela e o backend gráfico do app. Hinting e antisserrilhado de glifo em GPU podem diferir.
- **HiDPI** (`pixels_per_point` diferente de 1), **Windows e macOS**: não testados.
- **Colisões**: medidas por **caixa** (estimativa de 0,55 da altura por caractere para texto), não pela tinta: superestimam (principalmente bandeirola contra cabeça) e ignoram sobreposição de haste, viga e arco. Servem para apontar onde olhar.
- **Verovio**: só a versão 6.4.1 do pacote PyPI, em processo Python; o binário de linha de comando não foi usado. O oráculo não foi consultado sobre as próprias colisões. A rodada na CI **não existe** (e não deve existir).
- Diferenças de contagem de ligaduras, sílabas e vigas: causa **não investigada** além do que está na seção 4.
- Licença da Bravura conferida pelo texto da OFL e pelo aviso nos arquivos; nenhuma revisão jurídica. Commit exato da origem não determinado.
