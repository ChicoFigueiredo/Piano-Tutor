# 05: Prefactor: separar o documento da partitura do estado da tela

**Fase:** E01

**R-IDs/critérios:** R-A01, R-E02; ponto de atenção do diagnóstico (tela monolítica)

**Seam:** S1 (prepara); S4 intocado

**Modelo sugerido:** Sonnet / alto

**What to build:** Hoje a tela de Partitura guarda, na mesma estrutura, o texto XML carregado, o modelo, a reprodução, a prática e a interface. Este ticket extrai, sem mudar nenhum comportamento visível, um tipo de documento que fica fora da interface (abrir texto ou MXL, ter o texto original, ter o modelo derivado, exportar), de modo que a tela passe a só usá-lo. É o degrau que torna o serviço de documento (ticket 08/10) possível sem tocar na prática nem na reprodução. Fazer a mudança fácil, depois a mudança fácil.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Todo o conjunto de testes existente do workspace continua verde, sem alteração de assertivas (linha de base da E00: 1053 verdes, 6 ignorados); fmt e clippy com avisos como erro verdes.
- [ ] Abrir um arquivo, tocar, praticar (os quatro modos) e exportar na tela se comportam como antes (testes existentes da tela passam; nenhum teste foi apagado).
- [ ] O novo tipo de documento é construível e testável sem janela, áudio ou egui (um teste novo abre uma fixture e exporta sem criar interface).
- [ ] A exportação continua gravando o texto original e a releitura de conferência continua existindo.
- [ ] O diff do PR é só movimentação e delegação: nenhuma regra nova, nenhum atalho novo, nenhuma dependência nova.

**Notas:** Mantenha S4 (prática e comparação MIDI) exatamente como está; qualquer teste de prática que precisar mudar é sinal de que o prefactor passou do limite.
