# 12: Selecionar nota no render e navegar com setas e Tab, com foco visível

**Fase:** E01

**R-IDs/critérios:** R-E03, R-E04 (mínimo), R-B06; histórias 3, 4, 28; T9

**Seam:** S1 (cursor e seleção por IdNota); seam de render

**Modelo sugerido:** Sonnet / médio–alto

**What to build:** Na Oficina, clicar numa nota a seleciona, com destaque visível e de alto contraste; as setas ← e → andam entre as notas na ordem de leitura e Tab troca de voz e pauta. Tudo funciona sem mouse, com foco visível. A seleção é guardada por identidade de nota (não por posição), de modo que sobrevive a uma edição.

**Blocked by:** 06 (Prefactor: geometria do layout sem tipos do egui, com ligação nota a nota), 09 (Spike: render egui + SMuFL/Bravura contra Verovio como oráculo (fecha a pendência da ADR-0007)), 10 (Abrir uma partitura na Oficina e exportar idêntico (serviço de documento, 1ª fatia))

**Status:** ready-for-agent

- [ ] Dado um clique sobre uma cabeça de nota, a nota correspondente fica selecionada e o serviço informa a mesma IdNota (teste de hit test sobre o layout e do estado do cursor, sem janela).
- [ ] ← e → andam para a nota anterior e seguinte na ordem de leitura, inclusive entre compassos e acordes; nos limites, o cursor permanece e há aviso discreto (testado no serviço e na tela).
- [ ] Tab alterna entre voz/pauta existentes e nunca para uma voz inexistente.
- [ ] O foco do teclado é visível com contraste verificável por teste de cor (relação mínima documentada) e há opção de alto contraste; leitor de tela real é NÃO VERIFICADO.
- [ ] Clique fora de nota seleciona o compasso (comportamento atual preservado) e não perde a seleção por engano.
- [ ] Digitar em campo de texto da casca não move a seleção (a Oficina só recebe teclas quando está em foco).
- [ ] Nenhuma edição é possível ainda; abrir e navegar não marca o documento como sujo.
