# fabrica/video — Hipótese de dedilhado pelo vídeo de cima (protótipo)

Implementa a decisão do `docs/decisoes/ADR-0013`: offline, só na fábrica, nunca no app. Tudo que sai daqui
é **Hipótese** (`estado = "hipotese"`) e só vira conteúdo depois da revisão humana. Não gera Âncoras nem corrige notas.

## Ambiente

```sh
cd fabrica/video
uv venv --python 3.12 .venv && uv pip install --python .venv/bin/python -r requirements.txt
mkdir -p modelos && curl -fsSL -o modelos/hand_landmarker.task \
  https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task
# sha256 do modelo usado no protótipo: fbc2a30080c3c557093b5ddfc334698132eb341044ccee322ccf8bcf3607cde1
```

`.venv/` e `modelos/` estão no `.gitignore`. CPU basta (MediaPipe roda em XNNPACK; ~6 s para as 16 notas da Aula 14).

## 1. Calibração de vídeo

```sh
.venv/bin/python calibrar.py "<aula>.mp4" --saida calibracao.json [--quadro-s T] [--janela-s A,B] [--faixa auto|superior|inferior]
```

Mediana de quadros (tira as mãos), faixa de pretas por "pente" de corridas escuras, padrão 2-3 para achar os Dós,
ajuste por mínimos quadrados da grade de brancas. Grava `calibracao.json` e `calibracao.png` (mapa sobreposto, **o
revisor confere este PNG**). O Dó central é a **suposição** "Dó mais próximo do centro horizontal", marcada em
`suposicoes`. Quando o quadro tem um teclado gráfico sobreposto além do filmado (Aula 14), a faixa certa é a mais
próxima das pontas dos dedos detectadas pelo MediaPipe.

## 2. Dedilhado

```sh
.venv/bin/python dedilhado.py "<aula>.mp4" --calibracao calibracao.json --alinhamento cinco-notas.json \
    [--trecho-inicio S] --saida dedilhado-cinco-notas.json [--mao auto|direita|esquerda] [--depurar PASTA]
```

Instante da nota no vídeo = `offset_aula_s` do Alinhamento (ou `--trecho-inicio`) + `t_audio_s`. Em volta dele
(−80 ms a +40 ms) o MediaPipe Hands acha as mãos; o lado (direita/esquerda) vem da geometria do dorso visto de cima,
porque o rótulo do MediaPipe não vale nessa vista. Cada par (mão, dedo) soma uma gaussiana da distância horizontal da
ponta (landmarks 4, 8, 12, 16, 20 = dedos 1..5) ao centro da tecla; vence o maior. Saída por nota:
`{indice, midi, dedo, mao, confianca, ...}` mais `segundo_colocado` para o revisor.

## Limites conhecidos

- Só vale para vídeos com câmera fixa de cima. A detecção de mão precisa do recorte do teclado com margem (as mãos
  encostam na borda inferior do quadro; sem isso o MediaPipe não as vê).
- A heurística é "dedo sobre a tecla" (x da ponta), não "dedo que afunda a tecla": com a mão parada em cinco dedos
  basta, mas travessias de polegar, saltos e dedos sobre teclas vizinhas precisam de revisão (ver `segundo_colocado`).
- Dó central por suposição; teclas pretas e quadros com oclusão forte reduzem a confiança.
