# F01 — Calibração do REVAS KB-330 no cerebro (2026-11-24)

Execução real, com o titular tocando o teclado. Ambiente: cerebro (Kubuntu 26.04), USB nativo,
app compilado da branch `claude/project-thread-wel6ol` (`cargo build --release`).

## Como o teclado aparece no Linux

- `lsusb`: `0a67:2313 Medeli Electronics Co., Ltd USB AudioDevice` (o KB-330 é OEM Medeli).
- `amidi -l`: `hw:2,0,0  USB AudioDevice MIDI 1`.
- `aconnect -l`: cliente 24 `USB AudioDevice`, porta 0 `USB AudioDevice MIDI 1`.
- O nome da porta não contém "REVAS" nem "KB-330".

## Ocorrência durante o teste

Ao abrir o Monitor MIDI, a porta pré-selecionada era `Midi Through` (virtual, 14:0), a primeira da
lista; conectar nela não recebe nada do teclado. Com a troca manual para `USB AudioDevice MIDI 1`
a conexão funcionou (`aconnect` mostrou `piano-tutor-monitor` conectado de 24:0). Corrigido em
seguida: a sugestão inicial passa a pular `Midi Through` (`monitor.rs`, `porta_sugerida`).

## Relatório gerado pelo app (`~/.local/share/piano-tutor/rev-as-calibracao.md`)

### Calibração do teclado

- Data: 2026-11-24
- Dispositivo: USB AudioDevice:USB AudioDevice MIDI 1 24:0
- Faixa medida: MIDI 36 a 96 (61 teclas)
- Teclas conferidas: 61 de 61
- Liberação (NoteOff) recebida: 61 teclas
- Velocity: suportado (observado 13 a 110)
- Pedal de sustain (CC64): suportado

| Tecla | MIDI | Nota | Conferida | Liberação |
|---:|---:|---|---|---|
| 1 | 36 | Dó2 | sim | sim |
| 2 | 37 | Dó♯2 | sim | sim |
| 3 | 38 | Ré2 | sim | sim |
| 4 | 39 | Ré♯2 | sim | sim |
| 5 | 40 | Mi2 | sim | sim |
| 6 | 41 | Fá2 | sim | sim |
| 7 | 42 | Fá♯2 | sim | sim |
| 8 | 43 | Sol2 | sim | sim |
| 9 | 44 | Sol♯2 | sim | sim |
| 10 | 45 | Lá2 | sim | sim |
| 11 | 46 | Lá♯2 | sim | sim |
| 12 | 47 | Si2 | sim | sim |
| 13 | 48 | Dó3 | sim | sim |
| 14 | 49 | Dó♯3 | sim | sim |
| 15 | 50 | Ré3 | sim | sim |
| 16 | 51 | Ré♯3 | sim | sim |
| 17 | 52 | Mi3 | sim | sim |
| 18 | 53 | Fá3 | sim | sim |
| 19 | 54 | Fá♯3 | sim | sim |
| 20 | 55 | Sol3 | sim | sim |
| 21 | 56 | Sol♯3 | sim | sim |
| 22 | 57 | Lá3 | sim | sim |
| 23 | 58 | Lá♯3 | sim | sim |
| 24 | 59 | Si3 | sim | sim |
| 25 | 60 | Dó4 | sim | sim |
| 26 | 61 | Dó♯4 | sim | sim |
| 27 | 62 | Ré4 | sim | sim |
| 28 | 63 | Ré♯4 | sim | sim |
| 29 | 64 | Mi4 | sim | sim |
| 30 | 65 | Fá4 | sim | sim |
| 31 | 66 | Fá♯4 | sim | sim |
| 32 | 67 | Sol4 | sim | sim |
| 33 | 68 | Sol♯4 | sim | sim |
| 34 | 69 | Lá4 | sim | sim |
| 35 | 70 | Lá♯4 | sim | sim |
| 36 | 71 | Si4 | sim | sim |
| 37 | 72 | Dó5 | sim | sim |
| 38 | 73 | Dó♯5 | sim | sim |
| 39 | 74 | Ré5 | sim | sim |
| 40 | 75 | Ré♯5 | sim | sim |
| 41 | 76 | Mi5 | sim | sim |
| 42 | 77 | Fá5 | sim | sim |
| 43 | 78 | Fá♯5 | sim | sim |
| 44 | 79 | Sol5 | sim | sim |
| 45 | 80 | Sol♯5 | sim | sim |
| 46 | 81 | Lá5 | sim | sim |
| 47 | 82 | Lá♯5 | sim | sim |
| 48 | 83 | Si5 | sim | sim |
| 49 | 84 | Dó6 | sim | sim |
| 50 | 85 | Dó♯6 | sim | sim |
| 51 | 86 | Ré6 | sim | sim |
| 52 | 87 | Ré♯6 | sim | sim |
| 53 | 88 | Mi6 | sim | sim |
| 54 | 89 | Fá6 | sim | sim |
| 55 | 90 | Fá♯6 | sim | sim |
| 56 | 91 | Sol6 | sim | sim |
| 57 | 92 | Sol♯6 | sim | sim |
| 58 | 93 | Lá6 | sim | sim |
| 59 | 94 | Lá♯6 | sim | sim |
| 60 | 95 | Si6 | sim | sim |
| 61 | 96 | Dó7 | sim | sim |

## Perfil salvo (`teclado.txt`)

```
# Piano Tutor: perfil do teclado v1
dispositivo=USB AudioDevice:USB AudioDevice MIDI 1 24:0
grave=36
velocity=sim
velocity_faixa=13-110
pedal=sim
conferidas=61
liberacoes=61
```

Pendente de assinatura do titular: conferência visual de que cada tecla física acendeu a posição certa (F02-1) e a conferência manual de 100 notas e 20 acordes (F01-2).

## Recalibração com a roda de pitch bend (mesmo dia)

Depois da etapa nova da roda de bend, o titular recalibrou. Relatório do app:

- Velocity: suportado (observado 1 a 119)
- Pedal de sustain (CC64): suportado
- Roda de pitch bend: suportado (repouso 373, curso -2944 a 2048, zona morta ±1302)

Observações: o REVAS manda pitch bend no canal 1 com resolução de 128 unidades (7 bits). A roda
não chega ao fundo de curso MIDI (±8192) e chia em repouso (cerca de ±650); a correção do app
estica cada lado até ±8192 e trata o chiado como repouso. Uma captura anterior com `aseqdump`
(3 min) mostrou o valor parado perto de -2700, provavelmente com a roda segurada.
