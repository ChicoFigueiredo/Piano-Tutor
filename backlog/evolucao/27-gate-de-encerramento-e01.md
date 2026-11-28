# 27: Gate de encerramento da E01: matriz de rastreabilidade, ADRs e demonstração

**Fase:** E01

**R-IDs/critérios:** Gate da E01 (itens 1–6); A1–A18; matriz de rastreabilidade por ticket

**Seam:** S1, S4 (verificação)

**Modelo sugerido:** Opus / alto para a revisão e o relatório (Haiku para preencher a matriz)

**What to build:** A E01 termina com evidência: uma matriz de rastreabilidade (R-ID, história, critério, teste, fixture e licença, evidência, risco, dono; PASS, FAIL, NÃO VERIFICADO ou N/A), as ADRs de modelo de documento e de render atualizadas com a evidência dos spikes, a revisão de código independente sem P0/P1 e a demonstração ao titular com o roteiro do objetivo da fase. O aceite humano é do titular; o agente não o infere de CI verde.

**Blocked by:** 11 (Segurança de XML e MXL na abertura da Oficina (A13)), 19 (Letra, cifra e dedilhado exibidos e preservados, com lista de elementos não suportados), 20 (PDF original ao lado, com zoom e rolagem sincronizados por compasso (A17)), 22 (Diff entre duas Versões, por compasso, com antes e depois (A10)), 24 (Autosave em intervalos curtos e recuperação após queda (A7)), 25 (Praticar a Versão com os Exercícios existentes, e ouvir o compasso editado (A14)), 26 (Baseline de desempenho dos comandos numa peça de 100 compassos (A16))

**Status:** ACEITA pelo titular (Chico, 2026-11-28 13:42 UTC, cartão de decisão "Aceito a E01"). O critério 1 (CI no Linux de referência) e os itens NÃO VERIFICADOS da matriz continuam registrados como estavam.

- [ ] A1–A13 e A15 passam em CI no Linux de referência; A17–A18 por teste de modelo e captura de tela; A14 por simulação; A16 registrada como baseline.
- [x] fmt, clippy com avisos como erro, testes do workspace e build release verdes, sem regressão sobre a linha de base de 1053 testes verdes e 6 ignorados (a diferença é só de testes acrescentados).
- [x] Revisão de código independente sem achados P0/P1 (qa-aceite audita critério → evidência).
- [x] O ADR do modelo de documento e o ADR de render estão atualizados com evidência dos spikes; a ADR-0015 referencia ambos.
- [x] A matriz de rastreabilidade está preenchida; itens NÃO VERIFICADOS (legibilidade visual, som em áudio real, REVAS 61 teclas e latências, estabilidade prolongada, macOS) permanecem NÃO VERIFICADO e listados, sem conversão por inferência.
- [x] Roteiro de demonstração executado (ou preparado para o titular): abrir com PDF ao lado, corrigir altura e duração, remover nota falsa, inserir pausa e acorde, desfazer e refazer, fechar sem salvar e recuperar, salvar Versão, conferir que a Hipótese permanece byte a byte igual, aprovar e praticar.
- [x] Aceite humano explícito do titular registrado (nome e data), ou o estado 'aguardando aceite'.

**Notas:** Desbloqueia os épicos E02, E03 (e, com eles, E05, E06). Spike opcional de MCP somente leitura (ganchos) não está incluído; ver README.

**Entrega do gate (2026-11-28):** [matriz](../../docs/evolucao-musical/e01/MATRIZ-RASTREABILIDADE.md) (190 linhas: 160 PASS, 7 FAIL, 23 NÃO VERIFICADO), ADR-0017 e ADR-0018 atualizadas (continuam PROPOSTA), ADR-0015 as referencia, [roteiro](../../docs/evolucao-musical/e01/ROTEIRO-DEMONSTRACAO.md) (preparado, não executado; o passo do PDF depende de R5). Números: build release verde; `cargo test --workspace` 1530 verdes, 0 falhas, 11 ignorados (base 1053/6, +477 e +5 ignorados); fmt, clippy e cargo deny verdes. Critérios sem marca: o 1 (rodado em VM, não em CI; sem captura de tela), o 3 (revisão com P0/P1: FAIL) e o 7 (**aguardando aceite**, nenhum aceite humano).


**Supervisor (2026-11-28):** três rodadas de revisão independente, registradas em docs/evolucao-musical/e01/REVISAO-CODIGO.md. Na primeira: R1 P0 e R2, R3 P1. Na segunda: N1 P1. Na terceira: nenhum P0/P1. Todos os P0/P1 foram corrigidos com teste red/green. O critério 1 continua aberto até o CI do GitHub e a execução no cerebro. O aceite humano é do titular.


**Aceite (2026-11-28):** o titular Chico aceitou a E01 pelo cartão de decisão "Aceita a E01 depois de rodar o roteiro no cerebro?" → "Aceito a E01". O aceite não converte nenhum NÃO VERIFICADO em PASS.
