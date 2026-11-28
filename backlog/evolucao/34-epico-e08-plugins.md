# 34: Épico E08: Plugins (Luau assinado; Python isolado)

**Fase:** E08

**R-IDs/critérios:** R-A04, D15

**Seam:** S3; S1

**Modelo sugerido:** Opus / alto para o to-tickets do épico; os tickets filhos recebem modelo e effort próprios quando forem criados

**What to build:** **Épico (não quebrar agora).** Objetivo demonstrável: um plugin Luau assinado (dedilhado ou adaptação) propõe um patch reversível, e um plugin Python isolado faz reconhecimento ou análise.

**Decisões já tomadas na especificação:** API semântica de partitura sobre o mesmo serviço de documento; manifesto (id, API, licença, capacidades, hashes, permissões); somente plugins assinados pelo titular (D15); Luau determinístico (sem aleatoriedade não semeada, limite por contagem de interrupções); Python recusa executar sem isolamento de rede no Linux de referência e dá aviso visível no macOS; quarentena, rollback e teste hostil; WASM adiado.

Este ticket é um marcador de fase: quando seus bloqueios fecharem, o orquestrador roda `to-tickets` sobre ele e gera as fatias verticais (prefactors primeiro) antes de qualquer implementação.

**Blocked by:** 31 (Épico E05: Reconhecimento (OMR em processos separados)), 33 (Épico E07: Professor inteligente)

**Status:** ready-for-agent

- [ ] Dado plugin hostil (arquivo fora do escopo, laço infinito), ele é bloqueado e auditado.
- [ ] Versão incompatível é rejeitada.
- [ ] Plugin não assinado não executa.
- [ ] Gate da fase: fixtures de terceiros e API documentada; aceite do titular.
- [ ] NÃO VERIFICADO sem hardware: isolamento de rede no kernel do PC de referência e no macOS.
- [ ] Este épico só é considerado pronto quando seus tickets filhos (a criar) estiverem aceitos e o titular registrar o aceite humano da fase.
