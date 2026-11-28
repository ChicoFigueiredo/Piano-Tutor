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

## Complemento: PDF ao lado na aba Partitura (achado R5 da revisão E01)

- A edição agora acontece com o PDF ao lado: na aba Partitura, quando o documento é de um item da Oficina com PDF/páginas, um painel lateral redimensionável (e com "Esconder"/"Mostrar PDF ao lado") mostra a página do PDF levada ao compasso da seleção de nota ou do cursor de entrada (`compasso_em_foco`). Região destacada quando o mapa tem região; senão página inteira e o aviso "Mapa por página". Zoom e rolagem independentes.
- Código novo em `crates/piano-tutor-app/src/tela_partitura/pdf_lateral.rs` (estado do painel, desenho e testes). A regra compasso→página/região segue em `pdf_sincronizado.rs` (`MapaCompassos`, `Destino`, `compasso_na_pagina`), a mesma da aba Revisão; só o estado do painel e o desenho têm uma segunda implementação, porque o da Revisão é privado de `EstadoRevisao`. O arquivo principal ganhou 4 pontos mínimos: `mod`, um campo, a ligação em `vincular_a_oficina` e `desenhar_tela` delegando ao submódulo.
- Sem PDF no item ou arquivo solto: não há painel; uma dica discreta diz por quê. O painel só vale com o vínculo da Oficina (`versoes`).
- PDF e páginas só são lidos (teste com SHA-256 antes e depois de apontar compassos, trocar página e esconder/mostrar). Mesmos limites de memória de `ler_png`.
- Testes egui sem janela (7, em `pdf_lateral::testes`): painel acompanha nota selecionada; cursor de entrada leva o PDF; região x por página; esconder/mostrar; larguras estreita e larga sem pânico; sem PDF; arquivo solto; hash. Os testes foram escritos junto com o código, não antes (sem registro de "red").
- NÃO VERIFICADO: legibilidade do painel e do destaque numa janela real, arrastar a borda com o mouse, HiDPI e captura de tela (sem display neste ambiente). O roteiro de demonstração (`docs/evolucao-musical/e01/ROTEIRO-DEMONSTRACAO.md`) não existe no repositório; nada foi alterado nele.
