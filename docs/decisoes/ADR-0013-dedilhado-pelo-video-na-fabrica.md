# ADR-0013 — Dedilhado detectado no vídeo de cima, offline na fábrica

**Status:** PROPOSTA — 2026-11-25 · **Pedido do titular:** "aplique/crie também heurística para quando ele estiver com essa visão (câmera de cima) e aplicar ao software, indicando dedo, nota na partitura".

## Contexto

Muitas Aulas do Luciano mostram o teclado filmado de cima, com câmera fixa. Daí dá para saber que dedo toca cada nota, o que falta hoje no conteúdo do Curso. Cada vídeo tem enquadramento próprio.

## Decisão

1. A detecção roda **offline na fábrica**, em Python, com GPU (RTX 4080 do cerebro), nunca ao vivo no app.
2. **Calibração de vídeo automática e conferida:** a fábrica acha as teclas pretas e o Dó central, propõe o mapa de teclas sobre um quadro, e o revisor aprova ou arrasta os cantos. Fica salva por Aula.
3. **Mãos:** pontos dos dedos por quadro (MediaPipe Hands). Para cada nota do Alinhamento, o dedo é o que está sobre a tecla no instante em que ela soa.
4. A saída é uma **Hipótese de dedilhado** por nota, com confiança e proveniência, que passa pela revisão e, aprovada, aparece na partitura e nas bolinhas do teclado.
5. A detecção **não** gera Âncoras de sincronia nem corrige notas da partitura (decisão do titular).

## Alternativas descartadas

- **Ao vivo no app:** pesado e sem ganho, já que o vídeo é fixo.
- **Calibração manual de cada vídeo:** 163 Aulas tornam o trabalho grande demais.

## Consequências

- Nova dependência só na fábrica (MediaPipe, OpenCV), fora do app e do pacote.
- Vídeos sem a câmera de cima ficam sem Hipótese de dedilhado; a calibração diz isso.
