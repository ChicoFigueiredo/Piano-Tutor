# 20: PDF original ao lado, com zoom e rolagem sincronizados por compasso (A17)

**Fase:** E01

**R-IDs/critérios:** R-B06; A17; histórias 1, 2

**Seam:** S1 (mapa compasso→página); seam de render

**Modelo sugerido:** Sonnet / médio–alto

**What to build:** Na Oficina, ao abrir uma Hipótese convertida de um PDF, o PDF original (ou suas páginas já renderizadas no repositório de oficina) aparece ao lado, com zoom e rolagem; selecionar um compasso na partitura leva à página correspondente do PDF, e rolar o PDF indica o compasso correspondente. Sem região mapeada, a ligação é por página.

**Blocked by:** 12 (Selecionar nota no render e navegar com setas e Tab, com foco visível)

**Status:** ready-for-agent

- [ ] A17: dada uma Hipótese com PDF/páginas de origem, abrir na Oficina mostra o PDF ao lado, com zoom e rolagem independentes do render da partitura.
- [ ] Selecionar um compasso na partitura leva o painel do PDF à página correspondente (teste do mapa compasso→página e da ação de rolar; captura de tela anexada).
- [ ] Quando existe região mapeada, o painel destaca a região; quando não existe, leva à página inteira e informa que o mapa é por página (teste dos dois casos).
- [ ] O painel é opcional: sem PDF de origem, a Oficina funciona e mostra aviso discreto; nunca depende de Python, OMR ou rede (T1).
- [ ] O PDF e as páginas nunca são alteradas pela Oficina (somente leitura, comparação de hash antes e depois).
- [ ] Limite de memória para páginas grandes é aplicado e documentado; um PDF acima do limite não derruba a tela.

**Notas:** A origem do mapa compasso→página e a renderização de página reutilizam o que a Oficina atual já tem (páginas em PNG); qualquer dependência nova de renderização de PDF exige decisão e passa pelo ticket 01.
