# ADR-0011 — Player de Aula: vídeo embutido com libmpv e sincronia por Âncoras

**Status:** PROPOSTA — 2026-11-25 · **Pedido do titular:** "não era para ter vídeo, partitura em tempo real e teclado com dedo e dedilhado no tempo certo? ... controles em modo de editor para mover tempo+ tempo− acelerar+ acelerar−, tal como editor de vídeo".

## Contexto

A tela do Curso era uma árvore de Seções com "Abrir" e "Ver vídeo", que abria o mpv numa janela à parte, sem ligação com a partitura nem com o teclado. O titular quer viver cada Aula como num player: vídeo, partitura e teclado com o dedilhado andando juntos, e um editor para acertar a sincronia quando o professor tocar.

## Decisão

1. **Player de Aula** é a tela de uma Lição derivada de uma Aula. Layout padrão: vídeo à esquerda e partitura à direita em cima, linha do tempo (trechos dos Exercícios e Âncoras) no meio, teclado com dedos acesos embaixo; os painéis se redimensionam e mudam de posição por alças.
2. **Vídeo embutido com libmpv** (libmpv2, a mesma do "Ver vídeo"), renderizado numa textura do egui. Um botão **destaca** o vídeo numa janela própria, que pode ser maximizada, e o devolve ao painel. O **relógio do vídeo** comanda o cursor da partitura e o teclado.
3. **Sincronia por Âncoras.** O Alinhamento (nota ↔ instante) continua sendo a verdade. O **modo editor** do próprio player mostra a linha do tempo com a forma de onda e as Âncoras arrastáveis; tempo+/tempo− deslocam e acelerar+/acelerar− esticam as Âncoras do trecho selecionado, como num editor de vídeo. Salvar grava uma nova **Hipótese** do Alinhamento, que segue o fluxo de revisão (Em revisão → Aprovada) e volta ao pacote pela fábrica.
4. **Modo de acompanhamento** escolhido pelo aluno e trocável durante a Aula: Espera, Segue no tempo, Chamada e resposta.
5. **Atalhos padrão** (configuráveis): Espaço pausa e continua; Backspace ou o botão "voltar" do mouse voltam ao início do trecho (duplo: trecho anterior); Tab ou o botão "avançar" do mouse trocam o Modo de acompanhamento.
6. **Primeira fatia:** Aula 14 "Música (Cinco notas no piano)", que já tem partitura e Alinhamento.

## Alternativas descartadas

- **mpv em janela separada sincronizado por IPC:** já existia e não dá a sensação de player; não permite a linha do tempo junto do vídeo.
- **Decodificar com ffmpeg:** mais controle por quadro, porém mais código próprio (áudio, seek, relógio) para manter.
- **Só deslocamento e velocidade globais:** o professor fala entre uma tocada e outra, então um ajuste único não segura a Aula toda.
- **Ajuste só local, por cima do pacote:** não vira conteúdo revisado e se perde para quem receber o pacote.

## Consequências

- O app passa a ligar à libmpv (dlopen de `libmpv.so.2`). Sem ela, o player mostra a partitura e o teclado e oferece o "Ver vídeo" externo. O empacotamento `.deb` precisa declarar `libmpv2` quando for retomado.
- O Alinhamento ganha Âncoras manuais com proveniência ("editor", autor, data), distintas das notas alinhadas pelo DTW.
- O dlopen da libmpv exige `unsafe`. O crate do app troca `[lints] workspace = true` por `unsafe_code = "deny"` (mais os lints clippy do workspace, copiados), e só `video_embutido.rs` tem `#![allow(unsafe_code)]`. O resto do app continua sem `unsafe`.
- Render da libmpv em modo software (quadro RGB0 → textura do egui); render por OpenGL fica para quando o custo de CPU pesar.
- Para testar sem clicar: `PIANO_TUTOR_ROTA=cursos PIANO_TUTOR_PLAYER=<curso>/<lição>/<exercício>` abre o player direto.
