# Roteiro de medição de latência no cerebro (F03-4 e F02-4)

> **Estado: roteiro e ferramenta prontos; nenhuma medição real foi feita.** A lógica de detecção, pareamento e estatística foi testada só com sinais sintéticos (`cargo test -p piano-tutor-diag`). Os comandos de áudio e MIDI abrem dispositivos de verdade e **não foram validados com o REVAS nem com o loopback físico**. Enquanto não houver relatório gerado numa execução real, F03-4 e F02-4 seguem **NÃO VERIFICADO**.

## O que se mede e as metas

| Ficha | Meta | O que o número representa | Subcomando |
|---|---|---|---|
| F03-4 | p95 ≤ 50 ms, equipamento de referência | atraso do NoteOn recebido até o onset do som gravado pela entrada de áudio | `latencia-midi` |
| F03-4 (apoio) | sem meta própria | ida e volta saída → entrada, para saber quanto do atraso é da placa e do buffer | `latencia-loopback` |
| F02-4 | p95 ≤ 60 ms, janela visível, carga normal | atraso da tecla até o destaque na tela, lido em vídeo de câmera rápida | `latencia-quadros` |

Cada relatório traz n, média, desvio, mínimo, p50, p95, p99, máximo e jitter p95. Com menos de 30 pares o relatório diz **NÃO VERIFICADO** e não dá veredito. A ficha manda documentar a limitação e abrir correção se a meta não for atingida.

## Como o programa mede (para saber o que o número significa)

- Todos os instantes vêm do mesmo relógio monotônico do processo (`RelogioMonotonico` do núcleo).
- **NoteOn:** carimbado quando o programa recebe a mensagem da porta MIDI (midir). Não inclui o trajeto USB do teclado (cerca de 1 ms) nem o atraso interno do teclado.
- **Onset:** a gravação da entrada de áudio é mono (média dos canais); o onset é a primeira amostra acima de um limiar de energia (8 vezes a mediana do ruído, no mínimo `--limiar`). O tempo de cada amostra é ligado ao relógio pelo instante do callback de entrada, usando o callback mais pontual de uma janela de 2 s (remove o atraso de despertar e a deriva entre placa e sistema).
- **Viés para menos atraso:** a latência de captura do conversor e do driver não entra na conta.
- **Viés para mais atraso:** som de ataque lento é detectado depois do início perceptível.
- Os dois vieses não se anulam; por isso o relatório os lista e mostra, à parte, a latência que o driver **declara** (informativa, não medida).
- Pareamento: cada onset vai para o NoteOn mais recente que o precede, no máximo `--janela-max-ms` (400 ms) antes. NoteOn com outro a menos de 400 ms (acorde, repique) é descartado e contado. Um NoteOn sem onset conta como "sem onset"; onset sem NoteOn conta como "sem evento".

## Preparação (uma vez)

1. No cerebro, com a árvore do projeto limpa e no commit que será medido:
   ```
   git status --short
   git rev-parse --short HEAD
   cargo build --release -p piano-tutor-diag
   ```
   Anote o commit no rótulo (`--rotulo`).
2. Feche navegador, players e chamadas. Deixe o app Piano Tutor **fechado** nos testes 1 a 3.
3. Veja o que o sistema expõe:
   ```
   cargo run --release -p piano-tutor-diag -- latencia-listar
   ```
   Escolha um trecho do nome da saída, da entrada e da porta do teclado (ex.: `KB-330`). Sem `--saida-dispositivo`/`--entrada-dispositivo`, vale o padrão do sistema. Anote qual é a saída real (placa analógica, USB, HDMI): HDMI e Bluetooth têm latência bem maior e mudam o resultado.
4. Taxa de amostragem: confira no PipeWire (`pw-metadata -n settings`, campo `clock.rate`) ou no KDE; anote. Nada de efeitos de áudio ligados.
5. Teclado: fone no teclado, volume local no mínimo ou Local Control desligado. Som do teclado que sai pelo ar entra na gravação como onset **sem** atraso do PC e falseia o resultado.
6. Os relatórios vão para `docs/aceite/F03/`. Os nomes sugeridos abaixo já separam por buffer.

## Teste 1: loopback de áudio (ida e volta da placa)

Ligue a saída de linha na entrada de linha com um cabo (ou, sem cabo, o microfone perto do alto-falante, com volume moderado). Não fale durante a gravação.

```
cargo run --release -p piano-tutor-diag -- latencia-loopback \
  --duracao 40 --buffer 256 \
  --rotulo "cerebro, <placa>, buffer 256, commit <hash>" \
  --saida docs/aceite/F03/latencia-loopback-buffer256.md
```

Repita com `--buffer 128` e `--buffer 512`, mudando o nome do arquivo. A gravação toca cerca de 80 cliques em intervalos irregulares de 350 a 650 ms.

Leitura:

- Bom sinal: "Pares" perto de "Eventos de referência", poucos "sem onset" e "sem evento", jitter p95 de poucos ms.
- O atraso do loopback é saída mais entrada; **não** é o MIDI→áudio e não tem meta própria. Serve para achar o buffer em que a placa fica estável (sem estalo, sem pares perdidos) e como piso.
- Nenhum onset: aumente o volume, aproxime o microfone ou use `--limiar 0.005`. Onsets demais "sem evento": ruído de ambiente; reduza o barulho externo ou suba `--limiar`.

## Teste 2: MIDI → áudio, piso do caminho (o programa toca o som)

Mede o menor atraso que a máquina permite: NoteOn recebido, tom gerado no callback de saída, captado pela entrada. Não passa pelo motor do app.

```
cargo run --release -p piano-tutor-diag -- latencia-midi --gerar-som \
  --duracao 90 --buffer 256 --porta-midi KB-330 \
  --rotulo "cerebro, KB-330 USB, <placa>, buffer 256, commit <hash>" \
  --saida docs/aceite/F03/latencia-midi-piso-buffer256.md
```

Toque **uma nota por vez**, firme, com cerca de 1,5 s entre elas, até passar de 40 notas. O programa mostra a contagem de notas a cada segundo em que ela muda. Microfone (ou cabo) como no teste 1.

## Teste 3: MIDI → áudio do app (é o que a ficha F03-4 pede)

1. Terminal A: abra o app e deixe o SoundFont carregado (tela Timbres com status "pronto"). Ajuste o volume do app para o som chegar bem ao microfone, sem saturar.
   ```
   cargo run --release -p piano-tutor-app
   ```
2. Terminal B (o app continua aberto e com o teclado selecionado):
   ```
   cargo run --release -p piano-tutor-diag -- latencia-midi \
     --duracao 90 --buffer 256 --porta-midi KB-330 \
     --rotulo "cerebro, KB-330 USB, app commit <hash>, buffer do app 256" \
     --saida docs/aceite/F03/latencia-midi-app-buffer256.md
   ```
   `--buffer` aqui é só o buffer da **gravação**. O buffer do app é o da configuração dele; anote-o no rótulo.
3. Toque uma nota por vez, com 1,5 s de intervalo, mais de 40 notas, na região média do teclado.
4. Se a porta MIDI não puder ser aberta por dois programas ao mesmo tempo, o diag diz o erro: anote e use outro caminho (porta virtual do ALSA com `aconnect`), sem inventar resultado.
5. Repita 3 vezes (e com outro buffer do app, se for ajustável), para ver a variação entre execuções.

Interpretação: o número do teste 3 inclui o app inteiro, o buffer de saída, o ar até o microfone e a gravação. Compare o p95 com 50 ms. Se passar, registre "p95 de X ms no cerebro, método e vieses acima". Se não passar, registre o valor, a configuração de buffer e abra a correção, como manda a ficha. Se o piso do teste 2 já estiver perto de 50 ms, o problema é a placa ou o buffer, não o app.

## F02-4: MIDI → tela por vídeo de câmera rápida (240 quadros/s)

O app não tem, hoje, medição interna de MIDI→tela. O método abaixo mede o que a pessoa vê, de ponta a ponta:

1. Celular em câmera lenta a **240 fps** (confirme a taxa real filmando um cronômetro de tela ou o relógio do celular, e anote). Tripé fixo, boa luz (a 240 fps o vídeo escurece).
2. Enquadre juntos o teclado (a tecla que será tocada, de lado, para ver o curso) e a tela do app, com a tela do app que destaca a tecla tocada visível (confirme qual tela faz isso; anote no rótulo). Janela do app visível, em primeiro plano, sem outros programas pesados (carga normal).
3. Toque a mesma tecla de forma seca, 40 vezes ou mais, com 2 s entre elas.
4. No vídeo, passando quadro a quadro (por exemplo, `mpv --no-audio` e as teclas `,` e `.`; ou o editor que preferir), para cada toque anote:
   - quadro A: a tecla chega ao fundo do curso (referência do contato do teclado, onde o teclado costuma gerar o NoteOn);
   - quadro B: primeiro quadro em que o destaque aparece na tela.
5. Grave um número por linha em um arquivo, n = B − A (quadros). Comentários com `#` são aceitos. Exemplo de arquivo: `contagens-tela.txt`.
6. Gere o relatório:
   ```
   cargo run --release -p piano-tutor-diag -- latencia-quadros contagens-tela.txt \
     --fps 240 --rotulo "cerebro, tela <modelo e Hz>, app commit <hash>, janela visível" \
     --saida docs/aceite/F02/latencia-tela.md
   ```
7. Limites do método (também escritos no relatório):
   - cada marca tem ±1 quadro (±4,17 ms a 240 fps), então cada medida pode errar até ±2 quadros;
   - a taxa de atualização da tela entra (60 Hz = 16,7 ms por quadro de tela) e o atraso do próprio monitor também; isso é o que o usuário vê, e é a leitura conservadora;
   - o ponto A (tecla no fundo) é uma referência de pessoa, não o instante exato do NoteOn; mantenha o mesmo critério em todas as medidas;
   - uma pessoa só contando 40 vezes se cansa: peça a uma segunda pessoa para contar uma amostra de 10 vídeos e compare.
8. Medição de software (carimbos internos de evento recebido e quadro enviado) seria complemento útil, mas **não existe** no app hoje: **NÃO IMPLEMENTADO**.

## Depois de medir

1. Confira que os arquivos `.md` têm nomes de dispositivo sem número de série (o programa sanitiza, mas leia).
2. `git add docs/aceite/F03/latencia-*.md docs/aceite/F02/latencia-tela.md` e commit com o resultado real, o commit medido e os buffers.
3. A matriz de aceite e o `progresso.json` só mudam com revisão humana (QA): este roteiro não altera nenhum dos dois.
4. Se algum relatório disser **NÃO VERIFICADO** ou "acima da meta", o status continua assim; não arredondar nem repetir até passar sem registrar as execuções ruins.

## Tabela para preencher (só com números dos relatórios reais)

| Medição | Buffer | n pares | p50 | p95 | p99 | Veredito | Arquivo |
|---|---|---|---|---|---|---|---|
| Loopback | 128 | | | | | (sem meta) | |
| Loopback | 256 | | | | | (sem meta) | |
| Loopback | 512 | | | | | (sem meta) | |
| MIDI→áudio, piso (`--gerar-som`) | 256 | | | | | (sem meta) | |
| MIDI→áudio, app (F03-4) | | | | | | meta 50 ms | |
| MIDI→tela, vídeo 240 fps (F02-4) | n/a | | | | | meta 60 ms | |
