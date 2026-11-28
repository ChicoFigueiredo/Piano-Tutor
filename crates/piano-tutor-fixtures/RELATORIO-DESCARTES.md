# Relatório de descartes do modelo atual

Gerado pelo teste `leitura` de `piano-tutor-fixtures` (modelo `Partitura::de_musicxml`).
"sem campo" = o modelo atual não tem onde guardar a categoria: tudo é descartado.

## anacruse

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 3 | 3 | 0 |
| notas e pausas | 5 | 5 | 0 |

## duas-vozes

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 2 | 2 | 0 |
| notas e pausas | 6 | 6 | 0 |

## tercina

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 1 | 1 | 0 |
| notas e pausas | 5 | 5 | 0 |
| tercinas e grupos irregulares (time-modification) | 3 | 3 | 0 |

## repeticao

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 3 | 3 | 0 |
| notas e pausas | 3 | 3 | 0 |
| marcas de repetição (repeat; o modelo une as partes, então 2 partes com a mesma barra contam 2 no arquivo e 1 no modelo) | 2 | 2 | 0 |
| compassos com casa (ending) | 2 | 2 | 0 |

## letra

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 2 | 2 | 0 |
| notas e pausas | 6 | 6 | 0 |
| letra (lyric) | 6 | sem campo | 6 |

## cifra

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 2 | 2 | 0 |
| notas e pausas | 3 | 3 | 0 |
| cifra (harmony) | 3 | sem campo | 3 |

## dedilhado

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 2 | 2 | 0 |
| notas e pausas | 6 | 6 | 0 |
| dedilhado (fingering) | 6 | sem campo | 6 |

## dinamica

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 2 | 2 | 0 |
| notas e pausas | 4 | 4 | 0 |
| dinâmica (dynamics, wedge) | 4 | sem campo | 4 |

## ornamento

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 2 | 2 | 0 |
| notas e pausas | 4 | 4 | 0 |
| ornamentos e fermata | 4 | sem campo | 4 |

## desconhecido

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 2 | 2 | 0 |
| notas e pausas | 3 | 3 | 0 |
| elementos desconhecidos (x-*, grouping, figured-bass, ...) | 6 | sem campo | 6 |

## cem-compassos

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 100 | 100 | 0 |
| notas e pausas | 400 | 400 | 0 |

## brahms-wiegenlied

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 19 | 19 | 0 |
| notas e pausas | 267 | 267 | 0 |
| notas de adorno (grace) | 3 | 3 | 0 |
| ligaduras de valor (tie start) | 26 | 26 | 0 |
| marcas de repetição (repeat; o modelo une as partes, então 2 partes com a mesma barra contam 2 no arquivo e 1 no modelo) | 2 | 1 | 1 |
| andamento (sound tempo) | 1 | 1 | 0 |
| letra (lyric) | 90 | sem campo | 90 |
| dinâmica (dynamics, wedge) | 9 | sem campo | 9 |

## schubert-heidenroeslein

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 16 | 16 | 0 |
| notas e pausas | 250 | 250 | 0 |
| notas de adorno (grace) | 4 | 4 | 0 |
| marcas de repetição (repeat; o modelo une as partes, então 2 partes com a mesma barra contam 2 no arquivo e 1 no modelo) | 2 | 1 | 1 |
| andamento (sound tempo) | 1 | 1 | 0 |
| letra (lyric) | 138 | sem campo | 138 |
| dinâmica (dynamics, wedge) | 2 | sem campo | 2 |
| ornamentos e fermata | 6 | sem campo | 6 |

## faure-barcarolle

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 42 | 42 | 0 |
| notas e pausas | 682 | 682 | 0 |
| notas de adorno (grace) | 18 | 18 | 0 |
| tercinas e grupos irregulares (time-modification) | 78 | 78 | 0 |
| ligaduras de valor (tie start) | 88 | 88 | 0 |
| andamento (sound tempo) | 1 | 1 | 0 |
| letra (lyric) | 120 | sem campo | 120 |
| dinâmica (dynamics, wedge) | 23 | sem campo | 23 |

## beethoven-ode

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 16 | 16 | 0 |
| notas e pausas | 94 | 94 | 0 |
| andamento (sound tempo) | 1 | 1 | 0 |

## brilha-brilha

| categoria | no arquivo | no modelo | descartado |
|---|---|---|---|
| compassos (1ª parte) | 12 | 12 | 0 |
| notas e pausas | 66 | 66 | 0 |
| andamento (sound tempo) | 1 | 1 | 0 |

