# Matriz de aceite — Piano Tutor

**Regra:** fase só conclui com evidência executável e aprovação humana. Simulações não comprovam o REVAS físico.

Ordem de execução por marco ([plano](../planejamento/PLANO-DE-MARCOS.md), [ADR-0005](../decisoes/ADR-0005-marcos-e-ordem.md)). Regras de gate: [ADR-0006](../decisoes/ADR-0006-regras-de-gate.md) — NÃO VERIFICADO bloqueia F01, F02, F03 e F14.

| Marco | Fase | Entrega | Estado | Gate |
|---|---|---|---|---|
| M1 | [F00](../../backlog/fases/F00.md) | Preparação, inventário e laboratório reproduzível | Não iniciado | `bancada-casca` compila nativamente no Linux; CI compila a `bancada` pública |
| M1 | [F01](../../backlog/fases/F01.md) | Experimento MIDI físico e calibração REVAS | Evidência offline: [calibracao-tela](F01/calibracao-tela.md); aceite pendente | Hardware — NÃO VERIFICADO bloqueia: 61 teclas medidas, 0 pares perdidos em 10.000, desconexão ≤ 3 s |
| M1 | [F02](../../backlog/fases/F02.md) | Espelho virtual das 61 teclas e assistência básica | Não iniciado | Hardware — NÃO VERIFICADO bloqueia: MIDI→tela p95 ≤ 60 ms, 10 notas simultâneas |
| M1 | [F03](../../backlog/fases/F03.md) | Motor sonoro SoundFont, relógio mestre e `.deb` mínimo | Não iniciado | Hardware — NÃO VERIFICADO bloqueia: MIDI→áudio p95 ≤ 50 ms, jitter do relógio medido, checklist de escuta assinado, `.deb` instala no cerebro |
| M2 | [F04](../../backlog/fases/F04.md) | Núcleo de teoria musical e identificação | Evidência offline: [evidência](F04/evidencia.md); itens 1d, 2b, 3c e 5 NÃO VERIFICADOS | Testes de unidade: 128 notas MIDI, duração sem deriva em 1.000 compassos |
| M2 | [F08](../../backlog/fases/F08.md) | Metrônomo, compassos e sincronização pedagógica | Evidência offline: [evidência](F08/evidencia.md); render offline e medição real NÃO VERIFICADOS | Jitter p95 ≤ 10 ms e erro médio ≤ 2 ms (render offline); 20 min sem drift |
| M2 | [F06](../../backlog/fases/F06.md) | Gravar, importar e reproduzir MIDI | Não iniciado | Round-trip `.mid` sem perda em 3 arquivos de referência; malformados sem crash |
| M2 | [F05](../../backlog/fases/F05.md) | Professor assistido: exercícios, feedback e progressão | Não iniciado | 100% de acerto do classificador nos casos determinísticos; replay (F06) e compasso (F08) |
| M3 | — | Spike de notação: egui + Bravura × Verovio ([ADR-0007](../decisoes/ADR-0007-partitura-somente-leitura.md)) | Não iniciado | ≥ 5 MusicXML licenciados; ADR de escolha de motor |
| M3 | [F07](../../backlog/fases/F07.md) | Partituras MusicXML/MXL, duas claves, somente leitura e exportação | Não iniciado | ≥ 5 arquivos abertos corretamente; legível a 100%/150%; zip-bomb rejeitado |
| M3 | [F09](../../backlog/fases/F09.md) | Pads, drum machine, samples e looper | Evidência offline: [evidência](F09/evidencia.md); kit, samples, looper e choke não implementados; demais NÃO VERIFICADOS | Jitter p95 ≤ 10 ms offline; mesmo relógio de áudio; 10 min sincronizados |
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

## Conciliação com progresso.json

Fonte do estado real: [`docs/planejamento/progresso.json`](../planejamento/progresso.json) (131 itens). Ele registra estado por **item** (`nao_iniciado`, `parcial`, `codigo`, `evidencia_nuvem`, `evidencia_hardware`), sem rótulo por fase. Por isso a tabela compara a matriz com a distribuição dos itens de cada fase. Nenhum estado foi alterado nesta conciliação e nenhuma fase está aceita: o aceite humano não está registrado em nenhum dos dois arquivos.

| Fase | Estado na matriz | Estado no progresso (itens) | Situação | Explicação |
|---|---|---|---|---|
| F00 | Não iniciado | codigo 2, evidencia_nuvem 3, parcial 1 | Divergente, aguarda decisão | Progresso registra código e evidência de nuvem; a matriz diz não iniciado. |
| F01 | Evidência offline; aceite pendente | evidencia_hardware 1, evidencia_nuvem 2, codigo 1, nao_iniciado 1 | Divergente, aguarda decisão | Progresso registra calibração no cerebro (2026-11-24, 61/61 teclas) como evidência de hardware, pendente de assinatura do titular; a matriz não menciona essa evidência. Hardware segue NÃO VERIFICADO. |
| F02 | Não iniciado | evidencia_nuvem 3, parcial 2 | Divergente, aguarda decisão | Progresso registra evidência de nuvem; a matriz diz não iniciado. Medição de latência segue sem medição real. |
| F03 | Não iniciado | evidencia_nuvem 6, codigo 1, parcial 1 | Divergente, aguarda decisão | Progresso registra evidência de nuvem em 6 itens; a matriz diz não iniciado. Latência e escuta seguem NÃO VERIFICADO. |
| F04 | Evidência offline | evidencia_nuvem 8, codigo 1, nao_iniciado 1 | Compatível | O item não iniciado é a revisão e aprovação humana, ainda não registrada. |
| F05 | Não iniciado | codigo 7, evidencia_nuvem 3, nao_iniciado 1 | Divergente, aguarda decisão | Progresso registra código e evidência de nuvem; existe `docs/aceite/F05/evidencia.md` com testes automatizados, não citado na matriz. |
| F06 | Não iniciado | codigo 5, evidencia_nuvem 4, nao_iniciado 1 | Divergente, aguarda decisão | Progresso registra código e evidência de nuvem; a matriz diz não iniciado. |
| F07 | Não iniciado | parcial 3, evidencia_nuvem 2, nao_iniciado 1 | Divergente, aguarda decisão | Progresso registra trabalho parcial e evidência de nuvem; a matriz diz não iniciado. |
| F08 | Evidência offline | evidencia_nuvem 5, codigo 4, nao_iniciado 1 | Compatível | Render offline e medição real seguem NÃO VERIFICADO, como declara a matriz. |
| F09 | Evidência offline; kit, samples, looper e choke não implementados | codigo 3, evidencia_nuvem 2, parcial 1 | Divergente em conteúdo, aguarda confirmação | Progresso marca como código os itens de pads, sequenciador e relógio comum; a matriz diz que kit, samples, looper e choke não estão implementados. Falta esclarecer o que é "looper" e "choke" no progresso. |
| F10 | Não iniciado | nao_iniciado 11 | Compatível | Sem trabalho registrado. |
| F11 | Não iniciado | nao_iniciado 8 | Compatível | Sem trabalho registrado. |
| F12 | Não iniciado | nao_iniciado 7 | Compatível | Sem trabalho registrado. |
| F13 | Não iniciado | nao_iniciado 14 | Compatível | Sem trabalho registrado. |
| F14 | Não iniciado | parcial 1, codigo 1, nao_iniciado 12 | Divergente, aguarda decisão | Progresso registra migrações e `.deb` em andamento; a matriz diz não iniciado. |
| Spike de notação (M3) | Não iniciado | fora do progresso.json | Não comparável | O progresso não registra o spike; a matriz é a única fonte para essa linha. |

**Resumo:** 6 fases compatíveis (F04, F08, F10 a F13), 8 divergentes (F00, F01, F02, F03, F05, F06, F07, F14) e 1 a confirmar (F09). Nenhuma divergência está sem explicação. Para resolver, o titular precisa definir: (1) como um estado por fase é derivado dos estados por item do progresso; (2) se a evidência de hardware de F01 altera o rótulo da fase antes da assinatura; (3) o que F09 entende por "looper" e "choke". Até essa decisão, a coluna Estado da matriz não foi alterada.
