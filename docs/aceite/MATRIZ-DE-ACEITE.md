# Matriz de aceite — Piano Tutor

**Regra:** fase só conclui com evidência executável e aprovação humana. Simulações não comprovam o REVAS físico.

Ordem de execução por marco ([plano](../planejamento/PLANO-DE-MARCOS.md), [ADR-0005](../decisoes/ADR-0005-marcos-e-ordem.md)). Regras de gate: [ADR-0006](../decisoes/ADR-0006-regras-de-gate.md) — NÃO VERIFICADO bloqueia F01, F02, F03 e F14.

| Marco | Fase | Entrega | Estado | Gate |
|---|---|---|---|---|
| M1 | [F00](../../backlog/fases/F00.md) | Preparação, inventário e laboratório reproduzível | Não iniciado | `bancada-casca` compila nativamente no Linux; CI compila a `bancada` pública |
| M1 | [F01](../../backlog/fases/F01.md) | Experimento MIDI físico e calibração REVAS | Não iniciado | Hardware — NÃO VERIFICADO bloqueia: 61 teclas medidas, 0 pares perdidos em 10.000, desconexão ≤ 3 s |
| M1 | [F02](../../backlog/fases/F02.md) | Espelho virtual das 61 teclas e assistência básica | Não iniciado | Hardware — NÃO VERIFICADO bloqueia: MIDI→tela p95 ≤ 60 ms, 10 notas simultâneas |
| M1 | [F03](../../backlog/fases/F03.md) | Motor sonoro SoundFont, relógio mestre e `.deb` mínimo | Não iniciado | Hardware — NÃO VERIFICADO bloqueia: MIDI→áudio p95 ≤ 50 ms, jitter do relógio medido, checklist de escuta assinado, `.deb` instala no cerebro |
| M2 | [F04](../../backlog/fases/F04.md) | Núcleo de teoria musical e identificação | Não iniciado | Testes de unidade: 128 notas MIDI, duração sem deriva em 1.000 compassos |
| M2 | [F08](../../backlog/fases/F08.md) | Metrônomo, compassos e sincronização pedagógica | Não iniciado | Jitter p95 ≤ 10 ms e erro médio ≤ 2 ms (render offline); 20 min sem drift |
| M2 | [F06](../../backlog/fases/F06.md) | Gravar, importar e reproduzir MIDI | Não iniciado | Round-trip `.mid` sem perda em 3 arquivos de referência; malformados sem crash |
| M2 | [F05](../../backlog/fases/F05.md) | Professor assistido: exercícios, feedback e progressão | Não iniciado | 100% de acerto do classificador nos casos determinísticos; replay (F06) e compasso (F08) |
| M3 | — | Spike de notação: egui + Bravura × Verovio ([ADR-0007](../decisoes/ADR-0007-partitura-somente-leitura.md)) | Não iniciado | ≥ 5 MusicXML licenciados; ADR de escolha de motor |
| M3 | [F07](../../backlog/fases/F07.md) | Partituras MusicXML/MXL, duas claves, somente leitura e exportação | Não iniciado | ≥ 5 arquivos abertos corretamente; legível a 100%/150%; zip-bomb rejeitado |
| M3 | [F09](../../backlog/fases/F09.md) | Pads, drum machine, samples e looper | Não iniciado | Jitter p95 ≤ 10 ms offline; mesmo relógio de áudio; 10 min sincronizados |
| M4 | [F10](../../backlog/fases/F10.md) | Catálogo de Aulas em vídeo e marcação manual (somente local, [ADR-0008](../decisoes/ADR-0008-aulas-somente-locais.md)) | Não iniciado | Aceite local; loop A–B ≤ 100 ms; CI com fixtures sintéticas |
| M4 | [F11](../../backlog/fases/F11.md) | IA de fala e transcrição de áudio musical | Não iniciado | Meta experimental F1 de onsets ≥ 0,75; se falhar, libera como beta ([ADR-0006](../decisoes/ADR-0006-regras-de-gate.md)) |
| M4 | [F12](../../backlog/fases/F12.md) | Visão superior das mãos, teclado e fusão multimodal | Não iniciado | Meta experimental ≥ 80% mão+dedo+tecla; se falhar, libera como beta ([ADR-0006](../decisoes/ADR-0006-regras-de-gate.md)) |
| M4 | [F13](../../backlog/fases/F13.md) | Fábrica de Lições, revisão e botão “Praticar este trecho” | Não iniciado | Só conteúdo Aprovado vira Lição/Gabarito; 5 Lições; aceite local |
| M5 | [F14](../../backlog/fases/F14.md) | Consolidação, Linux, distribuição e pedagogia | Não iniciado | Hardware — NÃO VERIFICADO bloqueia: 60 min sem crash, `.deb` em instalação limpa sem Python, sem bug crítico |

## Modelo mínimo de evidência

1. Link do commit e identificação do SO/hardware.
2. Comandos de testes e resultado (PASS/FAIL/UNVERIFIED).
3. Capturas e logs sanitizados; vídeo curto quando houver demonstração.
4. Mapeamento de cada critério para evidência reproduzível.
5. Pendências, riscos, regressões, licenças e aprovação explícita.

Não declarar latência, precisão de IA ou compatibilidade de plataforma sem medição. Hipóteses de IA são revisadas antes de virar gabarito.
