# 31: Épico E05: Reconhecimento (OMR em processos separados)

**Fase:** E05

**R-IDs/critérios:** R-B03, R-B04, R-B05, T3, T6, T7, T10, D12, D16, D17

**Seam:** S3 (contrato de worker); S2

**Modelo sugerido:** Opus / alto para o to-tickets do épico; os tickets filhos recebem modelo e effort próprios quando forem criados

**What to build:** **Épico (não quebrar agora).** Objetivo demonstrável: um PDF vetorial ou escaneado vira uma Hipótese com proveniência, as divergências aparecem na Oficina, e existe um benchmark público reproduzível.

**Decisões já tomadas na especificação:** contrato de worker JSON com versão de esquema (Job, entradas, opções, progresso, candidatos, diagnósticos, proveniência); timeouts, cancelamento, limites, diretório por Job, sem shell; Audiveris (CPU) e um segundo motor candidato (só se replicado localmente) como processos separados; roteamento vetor × scan por inspeção de fontes do PDF; fusão só detecta discordância; fila durável e retomável; sem rede por padrão; nada vira Gabarito automático; PDFtoMusic Pro fora (D17); primeiro passo: PoC e baseline medido, depois escolha do motor; cargo deny (ticket 01) antes de qualquer adaptador.

Este ticket é um marcador de fase: quando seus bloqueios fecharem, o orquestrador roda `to-tickets` sobre ele e gera as fatias verticais (prefactors primeiro) antes de qualquer implementação.

**Blocked by:** 27 (Gate de encerramento da E01: matriz de rastreabilidade, ADRs e demonstração), 28 (Épico E02: Acervo), 01 (Auditoria de dependências e licenças com cargo deny na CI), 04 (Preparo do corpus CC0 para o Reconhecimento (OpenScore Lieder + PDFs e scans degradados))

**Status:** ready-for-agent

- [ ] Dado scan ambíguo e dois motores divergentes, mostrar candidatos com página, região e confiança sem substituir o original.
- [ ] Dada falha de um motor, o outro segue.
- [ ] Dado cancelamento e reinício, sem duplicar nem aprovar arquivo parcial.
- [ ] Dado PDF com instrução maliciosa, ele é tratado como dado, nunca como instrução.
- [ ] Métricas por estrato publicadas, com metas 'a calibrar após baseline' e sem promessa de precisão.
- [ ] Gate da fase: contrato de worker testado com motor falso; benchmark rodado e registrado; gates de latência medidos com OMR concorrente; aceite do titular.
- [ ] NÃO VERIFICADO sem hardware: latência de MIDI/áudio sob OMR; uso de CPU/GPU no PC de referência; desempenho em macOS; qualidade em material comprado (só local, nunca em CI).
- [ ] Este épico só é considerado pronto quando seus tickets filhos (a criar) estiverem aceitos e o titular registrar o aceite humano da fase.

**Notas:** Usa o corpus preparado no ticket 04. Empacotamento de Audiveris/HOMR permanece adiado para 2027 (D16).
