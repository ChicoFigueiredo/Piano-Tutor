# Matriz de aceite — Piano Tutor

**Regra:** fase só conclui com evidência executável e aprovação humana. Simulações não comprovam o REVAS físico.

Ordem de execução por marco ([plano](../planejamento/PLANO-DE-MARCOS.md), [ADR-0005](../decisoes/ADR-0005-marcos-e-ordem.md)). Regras de gate: [ADR-0006](../decisoes/ADR-0006-regras-de-gate.md) — NÃO VERIFICADO bloqueia F01, F02, F03 e F14.

| Marco | Fase | Entrega | Estado | Gate |
|---|---|---|---|---|
| M1 | [F00](../../backlog/fases/F00.md) | Preparação, inventário e laboratório reproduzível | Em andamento — evidência de nuvem (aceite pendente) | `bancada-casca` compila nativamente no Linux; CI compila a `bancada` pública |
| M1 | [F01](../../backlog/fases/F01.md) | Experimento MIDI físico e calibração REVAS | Em andamento — evidência de hardware no cerebro 2026-11-24, 61/61 teclas (assinatura pendente). Evidência offline: [calibracao-tela](F01/calibracao-tela.md) | Hardware — NÃO VERIFICADO bloqueia: 61 teclas medidas, 0 pares perdidos em 10.000, desconexão ≤ 3 s |
| M1 | [F02](../../backlog/fases/F02.md) | Espelho virtual das 61 teclas e assistência básica | Em andamento — evidência de nuvem (aceite pendente) | Hardware — NÃO VERIFICADO bloqueia: MIDI→tela p95 ≤ 60 ms, 10 notas simultâneas |
| M1 | [F03](../../backlog/fases/F03.md) | Motor sonoro SoundFont, relógio mestre e `.deb` mínimo | Em andamento — evidência de nuvem (aceite pendente) | Hardware — NÃO VERIFICADO bloqueia: MIDI→áudio p95 ≤ 50 ms, jitter do relógio medido, checklist de escuta assinado, `.deb` instala no cerebro |
| M2 | [F04](../../backlog/fases/F04.md) | Núcleo de teoria musical e identificação | Em andamento — evidência de nuvem (aceite pendente). Evidência offline: [evidência](F04/evidencia.md); itens 1d, 2b, 3c e 5 NÃO VERIFICADOS | Testes de unidade: 128 notas MIDI, duração sem deriva em 1.000 compassos |
| M2 | [F08](../../backlog/fases/F08.md) | Metrônomo, compassos e sincronização pedagógica | Em andamento — evidência de nuvem (aceite pendente). Evidência offline: [evidência](F08/evidencia.md); render offline e medição real NÃO VERIFICADOS | Jitter p95 ≤ 10 ms e erro médio ≤ 2 ms (render offline); 20 min sem drift |
| M2 | [F06](../../backlog/fases/F06.md) | Gravar, importar e reproduzir MIDI | Em andamento — evidência de nuvem (aceite pendente) | Round-trip `.mid` sem perda em 3 arquivos de referência; malformados sem crash |
| M2 | [F05](../../backlog/fases/F05.md) | Professor assistido: exercícios, feedback e progressão | Em andamento — evidência de nuvem (aceite pendente) | 100% de acerto do classificador nos casos determinísticos; replay (F06) e compasso (F08) |
| M3 | — | Spike de notação: egui + Bravura × Verovio ([ADR-0007](../decisoes/ADR-0007-partitura-somente-leitura.md)) | Não iniciado | ≥ 5 MusicXML licenciados; ADR de escolha de motor |
| M3 | [F07](../../backlog/fases/F07.md) | Partituras MusicXML/MXL, duas claves, somente leitura e exportação | Em andamento — evidência de nuvem (aceite pendente) | ≥ 5 arquivos abertos corretamente; legível a 100%/150%; zip-bomb rejeitado |
| M3 | [F09](../../backlog/fases/F09.md) | Pads, drum machine, samples e looper | Em andamento — evidência de nuvem (aceite pendente). Evidência offline: [evidência](F09/evidencia.md); kit, samples, looper e choke não implementados; demais NÃO VERIFICADOS | Jitter p95 ≤ 10 ms offline; mesmo relógio de áudio; 10 min sincronizados |
| M4 | [F10](../../backlog/fases/F10.md) | Catálogo de Aulas em vídeo e marcação manual (somente local, [ADR-0008](../decisoes/ADR-0008-aulas-somente-locais.md)) | Não iniciado | Aceite local; loop A–B ≤ 100 ms; CI com fixtures sintéticas |
| M4 | [F11](../../backlog/fases/F11.md) | IA de fala e transcrição de áudio musical | Não iniciado | Meta experimental F1 de onsets ≥ 0,75; se falhar, libera como beta ([ADR-0006](../decisoes/ADR-0006-regras-de-gate.md)) |
| M4 | [F12](../../backlog/fases/F12.md) | Visão superior das mãos, teclado e fusão multimodal | Não iniciado | Meta experimental ≥ 80% mão+dedo+tecla; se falhar, libera como beta ([ADR-0006](../decisoes/ADR-0006-regras-de-gate.md)) |
| M4 | [F13](../../backlog/fases/F13.md) | Fábrica de Lições, revisão e botão “Praticar este trecho” | Não iniciado | Só conteúdo Aprovado vira Lição/Gabarito; 5 Lições; aceite local |
| M5 | [F14](../../backlog/fases/F14.md) | Consolidação, Linux, distribuição e pedagogia | Em andamento — código (aceite pendente) | Hardware — NÃO VERIFICADO bloqueia: 60 min sem crash, `.deb` em instalação limpa sem Python, sem bug crítico |

## Modelo mínimo de evidência

1. Link do commit e identificação do SO/hardware.
2. Comandos de testes e resultado (PASS/FAIL/UNVERIFIED).
3. Capturas e logs sanitizados; vídeo curto quando houver demonstração.
4. Mapeamento de cada critério para evidência reproduzível.
5. Pendências, riscos, regressões, licenças e aprovação explícita.

Não declarar latência, precisão de IA ou compatibilidade de plataforma sem medição. Hipóteses de IA são revisadas antes de virar gabarito.

## Conciliação com progresso.json

Fonte do estado real: [`docs/planejamento/progresso.json`](../planejamento/progresso.json) (131 itens, cada um com `estado` e `pct`). O progresso não tem rótulo por fase; o rótulo da coluna Estado é derivado dos itens pela regra abaixo (convenção do supervisor do ticket 02, reversível).

**Regra de rótulo por fase:**
1. "Não iniciado" só se todos os itens da fase estão em `nao_iniciado` com `pct` 0.
2. "Aceita" só com aceite humano registrado. Hoje nenhuma fase tem aceite registrado.
3. Nos demais casos: "Em andamento — <maior evidência existente> (aceite pendente)". A ordem de evidência é: evidência de hardware > evidência de nuvem > código > parcial.
4. F01 usa o texto específico: evidência de hardware no cerebro em 2026-11-24, 61/61 teclas, com assinatura pendente.
5. Looper e choke: nenhum item do progresso os registra como implementados. F09-1 cita choke groups só como critério, e não há item de looper. Também foram procurados em `backlog/fases/F09.md` e na spec v1, sem implementação registrada. A nota "não implementados" permanece na matriz.

Nenhum estado de hardware foi promovido: F01, F02, F03 e F14 seguem NÃO VERIFICADO nos gates. Nenhuma fase está aceita.

| Fase | Estado na matriz | Estado no progresso (itens) | Situação |
|---|---|---|---|
| F00 | Em andamento — evidência de nuvem (aceite pendente) | codigo 2, evidencia_nuvem 3, parcial 1 | Compatível pela regra |
| F01 | Em andamento — evidência de hardware no cerebro 2026-11-24, 61/61 teclas (assinatura pendente) | evidencia_hardware 1, evidencia_nuvem 2, codigo 1, nao_iniciado 1 | Compatível pela regra |
| F02 | Em andamento — evidência de nuvem (aceite pendente) | evidencia_nuvem 3, parcial 2 | Compatível pela regra |
| F03 | Em andamento — evidência de nuvem (aceite pendente) | evidencia_nuvem 6, codigo 1, parcial 1 | Compatível pela regra |
| F04 | Em andamento — evidência de nuvem (aceite pendente) | evidencia_nuvem 8, codigo 1, nao_iniciado 1 | Compatível pela regra |
| F05 | Em andamento — evidência de nuvem (aceite pendente) | codigo 7, evidencia_nuvem 3, nao_iniciado 1 | Compatível pela regra |
| F06 | Em andamento — evidência de nuvem (aceite pendente) | codigo 5, evidencia_nuvem 4, nao_iniciado 1 | Compatível pela regra |
| F07 | Em andamento — evidência de nuvem (aceite pendente) | parcial 3, evidencia_nuvem 2, nao_iniciado 1 | Compatível pela regra |
| F08 | Em andamento — evidência de nuvem (aceite pendente) | evidencia_nuvem 5, codigo 4, nao_iniciado 1 | Compatível pela regra |
| F09 | Em andamento — evidência de nuvem (aceite pendente) | codigo 3, evidencia_nuvem 2, parcial 1 | Compatível pela regra; nota de looper e choke mantida |
| F10 | Não iniciado | nao_iniciado 11 | Compatível |
| F11 | Não iniciado | nao_iniciado 8 | Compatível |
| F12 | Não iniciado | nao_iniciado 7 | Compatível |
| F13 | Não iniciado | nao_iniciado 14 | Compatível |
| F14 | Em andamento — código (aceite pendente) | parcial 1, codigo 1, nao_iniciado 12 | Compatível pela regra |
| Spike de notação (M3) | Não iniciado | fora do progresso.json | Não comparável; linha só da matriz |

**Resumo:** as 15 fases F00 a F14 são compatíveis com a regra, e não há divergência sem explicação. A linha do spike de notação não tem equivalente no progresso.
