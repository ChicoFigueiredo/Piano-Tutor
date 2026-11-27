# 01: Auditoria de dependências e licenças com cargo deny na CI

**Fase:** Infra (E00, antes de E05/E06/E07)

**R-IDs/critérios:** R-N03, T6, T10; Definição de pronto (auditoria de dependências e licenças)

**Seam:** N/A (infraestrutura de CI)

**Modelo sugerido:** Sonnet / médio (decisões de licença ficam para o titular; Opus só se surgir conflito de licença)

**What to build:** Toda alteração de dependência passa por uma verificação automática de licenças permitidas (MIT OR Apache-2.0 e compatíveis), avisos de segurança (advisories), fontes não registradas e versões duplicadas relevantes, antes que `rmcp`, provedores de IA ou qualquer adaptador de OMR entrem no projeto. O resultado da primeira execução sobre o workspace atual fica registrado, com cada exceção justificada.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Existe configuração versionada do `cargo deny` (licenças, advisories, bans, sources) e um passo da CI que falha quando a regra é violada.
- [ ] A primeira execução sobre o workspace atual passa; toda exceção tem justificativa escrita na própria configuração.
- [ ] Um teste negativo documentado (dependência com licença não permitida em um branch descartável) mostra o passo falhando; o log fica na evidência do PR.
- [ ] A bancada-casca (dependência por git) e as ferramentas AGPL/GPL/LGPL só como processos separados estão tratadas explicitamente na política (T6): nada AGPL, GPL ou LGPL é linkado.
- [ ] O tempo do passo é medido e anotado; não exige credencial; não executa em PR de documentação pura se a CI já tiver filtro por caminho.
- [ ] Documentação curta (PT-BR) diz como rodar localmente e como pedir exceção.

**Notas:** Obrigatório antes de qualquer ticket que adicione `rmcp`, provedor de IA ou adaptador de OMR (E05, E06, E07).
