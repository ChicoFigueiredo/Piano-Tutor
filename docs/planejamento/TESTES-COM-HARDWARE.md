# Testes com hardware pendentes (REVAS KB-330, áudio real, janela no KDE)

Lista consolidada depois da rodada noturna de 22–23/11/2026. Tudo abaixo segue **NÃO VERIFICADO** até
ser executado no cerebro, com o titular. Cada item diz o critério, o que fazer e onde registrar.
Ordem sugerida: do mais rápido ao mais longo.

## 1. Janela e visual (10 min, sem teclado)

| # | Critério | O que fazer | Registrar em |
|---|---|---|---|
| 1.1 | F00-2 | Abrir o app: tela de abertura aparece e some; Ribbon com ícones; temas Grafite, Executivo, Noite, Claro, Ébano legíveis; Ctrl+F1 recolhe | `docs/aceite/F00/app-cerebro.md` + captura |
| 1.2 | UI | Seletor de instrumento no Ribbon: buscar "piano", "cordas", "41"; trocar de família; escolher um timbre e ouvir | mesma página |
| 1.3 | UI | Ícone da janela e do menu do KDE (só aparece no menu com o `.deb` instalado) | mesma página |
| 1.4 | F07-3 | Partitura a 100% e 150%: pautas, nota-alvo e cursor legíveis; abrir as 5 partituras e um `.mxl` de verdade (MuseScore/OpenScore) | `docs/aceite/F07/evidencia.md` |
| 1.5 | F07-1 | Conferir as 2 partituras novas (Ode à Alegria, Brilha brilha estrelinha) contra uma edição de referência | idem |

## 2. Teclado REVAS conectado (30–40 min)

| # | Critério | O que fazer | Registrar em |
|---|---|---|---|
| 2.1 | F00-3, F01-1 | Monitor MIDI com o REVAS desligado (não trava) e ligado; calibrar as 61 teclas, velocity e pedal | `docs/aceite/F01/` (relatório da calibração é gerado pelo app) |
| 2.2 | F01-2 | 100 notas e 20 acordes conferidos no Monitor | `docs/aceite/F01/conferencia-manual.md` |
| 2.3 | F01-4 | Tirar o cabo USB com notas seguradas: status muda em até 3 s, nada fica preso, reconecta sozinho | `docs/aceite/F01/` |
| 2.4 | F01-5 | O Monitor mostra "não suportado pelo dispositivo" onde o REVAS não manda dado | idem |
| 2.5 | F02-1, F02-2 | Espelho: 61 teclas no lugar certo; 10 notas juntas; soltar apaga só a solta | `docs/aceite/F02/` |
| 2.6 | F04-2 | Acordes e escalas: tocar nota isolada, acorde, arpejo; conferir que um par de notas não vira "acorde"; trocar tonalidade e ver a grafia (Fá♯/Sol♭) | `docs/aceite/F04/evidencia.md` |
| 2.7 | F05-5 | Exercício com erro proposital: "Onde errei" mostra compasso/tempo; "Ouvir o que toquei" toca a tentativa | `docs/aceite/F05/evidencia.md` |
| 2.8 | F06-1/2 | Gravar com pedal e acordes, tocar de volta em piano e acordeon, salvar `.mid` e abrir em outro programa | `docs/aceite/F06/evidencia.md` |
| 2.9 | F06-3 | Fechar o app gravando: o rascunho aparece em `gravacoes/recuperacao/` | idem |
| 2.10 | F07-4 | Partitura → Praticar (espera e tempo): notas certas/erradas coloridas e resumo no fim | `docs/aceite/F07/evidencia.md` |
| 2.11 | F08-3 | Ritmos: exercício de pulso com tolerância mudada | `docs/aceite/F08/evidencia.md` |
| 2.12 | F09-1 | Os 8 pads soam distintos no FluidR3 | `docs/aceite/F09/evidencia.md` |

## 3. Medições (40–60 min) — roteiro em `docs/aceite/F03/roteiro-medicao-latencia.md`

| # | Critério | Comando |
|---|---|---|
| 3.1 | F03-4 (apoio) | `cargo run --release -p piano-tutor-diag -- latencia-loopback ...` com buffers 128/256/512 |
| 3.2 | F03-4 | `latencia-midi` tocando o REVAS (app fechado; som do teclado no fone) |
| 3.3 | F02-4 | `latencia-quadros` + vídeo de celular a 240 fps filmando tecla e tela |
| 3.4 | F03-7, F08-1, F09-4 | Metrônomo e bateria 10–20 min junto com o piano, ouvir deriva (as métricas offline já passam) |
| 3.5 | F03-1 | Escuta assinada: piano e segundo timbre distintos |

## 4. Longos (deixar rodando)

| # | Critério | O que fazer |
|---|---|---|
| 4.1 | F02-3 | 30 min tocando com reconexão no meio, sem nota presa |
| 4.2 | F14-4 | 60 min de sessão real sem crash nem underrun |
| 4.3 | F03-8, F14-1 | Instalar o `.deb` no cerebro (`scripts/empacotar-deb.sh`), abrir pelo menu e tocar |
| 4.4 | F14-5 | Abrir o app duas vezes e conferir `~/.local/share/piano-tutor/versao-dados.txt` e `backups/` |

## 5. Piloto M4 — Curso do Luciano, Seção 3 (60–90 min, com teclado e fone)

Especificação: `docs/planejamento/PILOTO-CURSO-LUCIANO.md`. Tudo do Luciano fica fora do git, em
`~/.local/share/piano-tutor/fontes/luciano-alves/` (oficina) e `~/.local/share/piano-tutor/cursos/` (pacote instalado).
Até aqui só há testes automáticos com fixtures sintéticas; **nada abaixo foi feito com o REVAS, som ou voz reais**.

**Revisão do tutor (sem teclado)**

| # | O que fazer | Esperado |
|---|---|---|
| 5.1 | Abrir **Tutor › Revisão do tutor**, aba Partituras | 6 partituras em `[?] Hipótese`, página do PDF à esquerda e partitura renderizada à direita |
| 5.2 | Em cada uma: **Ouvir**, comparar com o PDF e ler `Divergências` | Notas, figuras, dedos e solfejo iguais ao PDF |
| 5.3 | **Abrir no MuseScore**, corrigir se preciso, salvar o `.musicxml`, **Recarregar** | A tela avisa a mudança e mostra a versão nova |
| 5.4 | **Em revisão** → **Aprovar** (ou **Rejeitar** com nota) | `partituras/<id>.estado.toml` ganha `[[historico]]` com data e revisor |
| 5.5 | Aba Roteiro: **Ouvir** algumas falas (voz Piper), editar, aprovar | Voz em pt-BR inteligível; só aprovadas são ditas no Curso |
| 5.6 | Ouvir `oficina/audio/<id>.opus` (`mpv <arquivo>`) e conferir que é a execução inteira, sem fala no início | Início e fim limpos (Dolphin, Mary e Jingle têm o Dó final possivelmente truncado) |
| 5.7 | `scripts/atualizar-pacote-curso.sh` | Importa, assina, verifica ("confiável") e instala |

**Aluno (com o REVAS)**

| # | O que fazer | Esperado |
|---|---|---|
| 5.8 | **Cursos** › Curso do Luciano | Autor, licenciado e selo `[OK] confiável`; Lições da Seção 3; só Exercícios aprovados abrem |
| 5.9 | Abrir um Exercício aprovado e tocar no modo Praticar | Notas verdes/vermelhas, progresso gravado em `cursos/progresso/luciano-alves.piano-1.txt` |
| 5.10 | **Ouvir o professor** | Toca o trecho do Luciano e o cursor anda junto com as notas (Alinhamento) |
| 5.11 | **Ver vídeo**: indicar a pasta `/mnt/e2/Musica/Curso-Piano-1/03 - Mão direita` | mpv abre a Aula no instante em que ele toca |
| 5.12 | Professor falante ligado | Diz as falas aprovadas (antes, passos, acerto/erro, depois) sem travar a tela |
| 5.13 | **Chamada e resposta** num Exercício de Curso e no exercício 7 (Tríade de Dó) da fábrica | "Ouça…" → "Sua vez" → `[OK] Acertou` ou `[X] Tente de novo (2/3)`; acorde tocado junto é aceito |
| 5.14 | Tolerâncias da Chamada e resposta | Conferir se 250 ms de tolerância e 1,5 s de silêncio para fechar a resposta são confortáveis no REVAS |
| 5.15 | Apagar `~/.local/share/piano-tutor/cursos/luciano-alves.piano-1` e abrir o app | App funciona igual, só com o Curso de fábrica |

## O que a noite já deixou provado só em software

Testes automáticos verdes no workspace inteiro (núcleo, MIDI, áudio, diagnóstico e app), métricas
offline do metrônomo e da bateria, leitor `.mxl` com defesa contra zip-bomb, MIDIs de referência
com ida e volta, backup e rollback dos dados. Nada disso substitui os itens acima.
