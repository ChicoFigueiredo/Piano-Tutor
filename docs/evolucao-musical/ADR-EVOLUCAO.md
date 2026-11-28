# ADR proposta — edição musical nativa e plataforma agentiva
Status: **direção de produto autorizada pelo titular; especificação técnica em revisão**. Data: 2026-11-27.
## Decisão
1. Substituir a proibição de edição pós-F13 do ADR-0007: a edição nativa é requisito prioritário da nova evolução. Preservar os aceites F07 de leitura, playback, navegação, segurança e exportação como regressões, não como teto funcional.
2. Atualizar AGENTS.md, CONTEXT.md, especificação integral, matriz e plano de marcos após grill-with-docs. Registrar ADR numerada livre (há numeração duplicada histórica); não apagar histórico. A restrição antiga não pode ser usada para recusar o requisito autorizado.
3. Rust/egui e shell bancada, motor musical e editor próprios. MuseScore/Smoosic/Denemo/Verovio como referências funcionais e de engenharia; não incorporar um editor externo/web, nem transcrever código GPL para um projeto sem licença compatível. MuseScore externo opcional para conferência/interoperabilidade, nunca dependência do editor.
4. Isolar OMR Java/Python e modelos como processos opcionais; Rust orquestra. Luau restrito a regras e transformações verificáveis, API versionada; WASM somente após necessidade e spike.
5. Separar ferramentas de domínio, servidor MCP local e harness/LLM opcional. Nenhuma LLM no caminho crítico do áudio/MIDI.
6. PDF original e MusicXML importado imutáveis; edições e adaptações são revisões/derivações distintas; nenhum envio externo sem consentimento específico.
7. Linux cerebro referência de homologação; macOS teste pessoal conforme ADR-0014, validar paridade pretendida e dependências por arquitetura. Não ampliar para Windows sem ADR.
## Alternativas contestáveis
Modelo de documento e preservação XML; desenho/hit testing; OMR primário/secundário; transporte de worker; MCP SDK/transporte; camada de providers; armazenamento e migração; limites e métricas. Equipe deve apresentar matriz de decisão com evidência e custos. Não contestar a edição nativa como se fosse uma restrição anterior imutável; se inviável demonstrar e escalar ao titular.
