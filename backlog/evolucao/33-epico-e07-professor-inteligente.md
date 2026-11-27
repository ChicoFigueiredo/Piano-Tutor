# 33: Épico E07: Professor inteligente

**Fase:** E07

**R-IDs/critérios:** R-A03, R-L02, T3, T4, D3, D6, D18

**Seam:** S3 (contrato de provedor)

**Modelo sugerido:** Opus / alto para o to-tickets do épico; os tickets filhos recebem modelo e effort próprios quando forem criados

**What to build:** **Épico (não quebrar agora).** Objetivo demonstrável: chat no app com quatro provedores (Claude, OpenAI, Gemini, Ollama), ferramentas compartilhadas com o MCP e custo controlado.

**Decisões já tomadas na especificação:** camada própria fina de provedor sobre o cliente HTTP síncrono (biblioteca de terceiros como plano B); ferramentas devolvem evidência calculada pelo Rust, o modelo explica e propõe; contexto mínimo; memória pedagógica por opt-in; limite de passos, tokens, custo e tempo; teto de US$ 10/mês com parada obrigatória; consentimento por lote registrado; logs auditáveis com segredos redigidos; chaves no cofre do SO; conteúdo de PDF é dado, não instrução; um único portão de rede; fallback offline.

Este ticket é um marcador de fase: quando seus bloqueios fecharem, o orquestrador roda `to-tickets` sobre ele e gera as fatias verticais (prefactors primeiro) antes de qualquer implementação.

**Blocked by:** 32 (Épico E06: MCP), 01 (Auditoria de dependências e licenças com cargo deny na CI)

**Status:** ready-for-agent

- [ ] Dados dois provedores (um local ou simulado por contrato, mais um teste real separado e opcional), o fluxo completo funciona.
- [ ] Dado o teto atingido, a chamada seguinte é recusada.
- [ ] Dado prompt injection em PDF, nenhuma ação é executada.
- [ ] Dado offline, edição e treino seguem.
- [ ] Gate da fase: 10 cenários pedagógicos com cálculos conferidos pelo Rust; nenhum Gabarito alterado sem aprovação; aceite do titular.
- [ ] NÃO VERIFICADO sem hardware: provedores reais pagos sem chaves do titular; Ollama no hardware real; qualidade de voz.
- [ ] Este épico só é considerado pronto quando seus tickets filhos (a criar) estiverem aceitos e o titular registrar o aceite humano da fase.
