# F03 — Jitter do relógio mestre (render offline)

> **Ambiente:** RENDER OFFLINE NA NUVEM, sem dispositivo de áudio. Os atrasos de despertar do callback são **modelados** (pseudoaleatórios, sementes fixas), não medidos no PC de referência. Este relatório **não** comprova a latência MIDI→áudio real (meta p95 ≤ 50 ms, ticket #39): **NÃO VERIFICADO**.

Gerado por `cargo test -p piano-tutor-audio --test jitter_offline` (com `PT_PUBLICAR_JITTER=1` para regravar). O teste falha se este arquivo divergir do código ou se algum cenário dentro da especificação errar mais de 1 bloco.

## Metodologia

1. Dispositivo simulado consome frames a `taxa × (1 + deriva)`; o callback `k` acorda em `t0 + k·bloco/taxa_real + atraso`, com atraso uniforme em `[0, J]` e, quando indicado, picos de +3 ms em 1% dos callbacks.
2. 2.000 notas com timestamps monotônicos conhecidos (intervalo ≈ 21,3 ms + deslocamento pseudoaleatório) chegam à fila com atraso de entrega uniforme em `[0, D]` e seguem o caminho real: `ControleAudio` → fila `rtrb` → `MotorAudio` → sintetizador.
3. O sintetizador falso emite um impulso no frame em que recebe a nota; a saída é varrida e cada impulso é comparado ao frame ideal `round((e − t0)·taxa_real) + latência alvo`. Erro negativo = soou antes do ideal.
4. *Erro* é contra o alvo ideal (inclui o viés constante da âncora); *jitter* é `|erro − erro médio|` (variação, sem o viés). Folga = latência − (bloco + J + pico + D).

## Resultados do agendador (sintetizador falso)

| Cenário | Taxa | Bloco | Latência alvo | J / D | Folga | Detectados | Erro médio | \|Erro\| p95 | \|Erro\| máx | Jitter p95 | Jitter máx | Atrasados | ≤ 1 bloco |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A: bloco 64, âncora crua | 48000 Hz | 64 | 256 fr (5.33 ms) | 0.5 / 0.2 ms | 158 fr | 2000/2000 | -12.68 fr (-0.264 ms) | 23 fr (0.479 ms) | 24 fr (0.500 ms) | 11.32 fr (0.236 ms) | 12.68 fr (0.264 ms) | 0 | sim |
| B: bloco 128, âncora crua | 48000 Hz | 128 | 480 fr (10.00 ms) | 1.0 / 0.5 ms | 280 fr | 2000/2000 | -25.47 fr (-0.531 ms) | 46 fr (0.958 ms) | 48 fr (1.000 ms) | 22.53 fr (0.469 ms) | 25.47 fr (0.531 ms) | 0 | sim |
| C: bloco 256, âncora crua, picos | 48000 Hz | 256 | 720 fr (15.00 ms) | 1.0 / 0.5 ms | 248 fr | 2000/2000 | -27.45 fr (-0.572 ms) | 46 fr (0.958 ms) | 192 fr (4.000 ms) | 25.45 fr (0.530 ms) | 164.55 fr (3.428 ms) | 0 | sim |
| D: bloco 256, âncora suavizada, picos | 48000 Hz | 256 | 720 fr (15.00 ms) | 1.0 / 0.5 ms | 248 fr | 2000/2000 | -25.59 fr (-0.533 ms) | 31 fr (0.646 ms) | 36 fr (0.750 ms) | 5.41 fr (0.113 ms) | 10.41 fr (0.217 ms) | 0 | sim |
| E: bloco 512, suavizada, deriva +100 ppm | 48000 Hz | 512 | 1440 fr (30.00 ms) | 2.0 / 1.0 ms | 784 fr | 2000/2000 | -50.37 fr (-1.049 ms) | 56 fr (1.167 ms) | 61 fr (1.271 ms) | 7.37 fr (0.154 ms) | 17.37 fr (0.362 ms) | 0 | sim |
| F: 44,1 kHz, bloco 256, suavizada, deriva −50 ppm | 44100 Hz | 256 | 441 fr (10.00 ms) | 1.0 / 0.5 ms | 119 fr | 2000/2000 | -21.97 fr (-0.498 ms) | 24 fr (0.544 ms) | 27 fr (0.612 ms) | 3.03 fr (0.069 ms) | 5.03 fr (0.114 ms) | 0 | sim |
| G: bloco 256 sem agendamento (imediato) | 48000 Hz | 256 | — (imediato) | 1.0 / 0.5 ms | — | 2000/2000 | 111.97 fr (2.333 ms) | 232 fr (4.833 ms) | 273 fr (5.688 ms) | 128.03 fr (2.667 ms) | 161.03 fr (3.355 ms) | 0 | sim (jitter) |
| H: bloco 256, latência alvo curta demais (128) | 48000 Hz | 256 | 128 fr (2.67 ms) | 1.0 / 0.5 ms | -200 fr | 2000/2000 | 15.19 fr (0.317 ms) | 104 fr (2.167 ms) | 141 fr (2.938 ms) | 88.81 fr (1.850 ms) | 125.81 fr (2.621 ms) | 2219 | sim |

Leitura:

- Com folga ≥ 0 (A–F), todo evento soa a menos de 1 bloco do alvo e nenhum chega atrasado. O erro residual é o atraso de despertar do callback que a âncora não remove.
- A âncora suavizada (D) reduz o jitter em relação à crua (C) com os mesmos atrasos (mesma semente), ao custo de um viés constante, que é só latência fixa.
- Sem agendamento (G), a latência é menor, mas varia em até ~1 bloco + J (jitter alto).
- Com latência alvo menor que o bloco (H), parte dos eventos chega atrasada e é tocada no início do bloco seguinte (contados em `comandos_atrasados`). Configuração não recomendada; publicada para mostrar o limite.

## Quantização interna do `rustysynth` (SF2 sintético)

O `rustysynth` renderiza em blocos internos; um evento aplicado no meio de um bloco interno só soa no início do bloco interno seguinte. Medido com um SF2 **sintético gerado no teste** (onda quadrada própria, sem amostras de terceiros, sem reverb), callbacks ideais de 256 frames e notas em frames absolutos conhecidos. Atraso = primeiro frame audível − frame agendado (faixa esperada: 0 a bloco interno, pela quantização mais até 1 frame da rampa de ganho da voz). O envelope do SF2 sintético tem atraso e ataque mínimos; num SF2 real o envelope do preset soma-se a isso.

| Bloco interno | Taxa | Detectados | Atraso médio | Atraso p95 | Atraso máx |
|---|---|---|---|---|---|
| 64 | 48000 Hz | 200/200 | 32.08 fr (0.668 ms) | 59 fr (1.229 ms) | 63 fr (1.312 ms) |
| 16 | 48000 Hz | 200/200 | 7.59 fr (0.158 ms) | 15 fr (0.312 ms) | 15 fr (0.312 ms) |

## Limites e pendências

- Atrasos de despertar, deriva e atraso de entrega são **modelos**. O jitter real do callback ALSA/PipeWire no cerebro: **NÃO VERIFICADO**.
- Latência MIDI→áudio por captura/loopback (p95 ≤ 50 ms): **NÃO VERIFICADO** (ticket #39, exige REVAS e PC de referência).
- Contrato: o timestamp dos eventos MIDI deve vir do mesmo `RelogioMonotonico` usado no callback; o timestamp próprio do `midir` tem outra origem.
