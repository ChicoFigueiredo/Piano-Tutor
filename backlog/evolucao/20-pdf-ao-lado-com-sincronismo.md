# 20: PDF original ao lado, com zoom e rolagem sincronizados por compasso (A17)

**Fase:** E01

**R-IDs/critérios:** R-B06; A17; histórias 1, 2

**Seam:** S1 (mapa compasso→página); seam de render

**Modelo sugerido:** Sonnet / médio–alto

**What to build:** Na Oficina, ao abrir uma Hipótese convertida de um PDF, o PDF original (ou suas páginas já renderizadas no repositório de oficina) aparece ao lado, com zoom e rolagem; selecionar um compasso na partitura leva à página correspondente do PDF, e rolar o PDF indica o compasso correspondente. Sem região mapeada, a ligação é por página.

**Blocked by:** 12 (Selecionar nota no render e navegar com setas e Tab, com foco visível)

**Status:** feito, aguarda revisão

- [x] A17: dada uma Hipótese com PDF/páginas de origem, abrir na Oficina mostra o PDF ao lado, com zoom e rolagem independentes do render da partitura.
- [x] Selecionar um compasso na partitura leva o painel do PDF à página correspondente (teste do mapa compasso→página e da ação de rolar; captura de tela anexada).
- [x] Quando existe região mapeada, o painel destaca a região; quando não existe, leva à página inteira e informa que o mapa é por página (teste dos dois casos).
- [x] O painel é opcional: sem PDF de origem, a Oficina funciona e mostra aviso discreto; nunca depende de Python, OMR ou rede (T1).
- [x] O PDF e as páginas nunca são alteradas pela Oficina (somente leitura, comparação de hash antes e depois).
- [x] Limite de memória para páginas grandes é aplicado e documentado; um PDF acima do limite não derruba a tela.

**Notas:** A origem do mapa compasso→página e a renderização de página reutilizam o que a Oficina atual já tem (páginas em PNG); qualquer dependência nova de renderização de PDF exige decisão e passa pelo ticket 01.

## Implementação (feito, aguarda revisão)

- Módulo puro `crates/piano-tutor-app/src/pdf_sincronizado.rs` (mapa compasso→página, destino, rolagem, compasso indicado ao rolar). Leitura do mapa em `revisao.rs` (`ItemPartitura::paginas` e `::mapa`); painel em `tela_revisao.rs`; compasso apontado vem de `selecao_partitura` via `EstadoPartitura::compasso_em_foco`.
- Formato opcional no `estado.toml` (números a partir de 1; região = fração `[x, y, largura, altura]` da página): `paginas = ["paginas/p-1.png", ...]` e `[[mapa_compassos]]` com `compasso`, `pagina`, `regiao`. Sem `paginas`, vale `pagina_png` (uma página). Sem `mapa_compassos`: uma página serve todos os compassos (por página); várias páginas sem mapa mostram "Sem mapa de compassos".
- Sem dependência nova: usa os PNG da Oficina. Rasterizar PDF (pdftoppm) NÃO foi implementado; fica como opcional futuro.
- Limite de memória: só a página em foco vira textura; PNG acima de 40 milhões de pixels ou 64 MiB vira mensagem na coluna (a tela e a partitura seguem); no máximo 400 páginas por partitura.
- Captura de tela: NÃO VERIFICADO (sem display no ambiente); o visual real do destaque e da rolagem não foi visto.
