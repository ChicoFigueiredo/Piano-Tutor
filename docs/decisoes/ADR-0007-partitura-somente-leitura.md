# ADR-0007 — F07 somente leitura, precedida de spike de notação

**Status:** SUBSTITUÍDA por ADR-0015 — 2026-11-27

Antes de F07, um **spike** de tempo limitado compara (a) layout próprio em egui com a fonte SMuFL Bravura e (b) Verovio (C++, LGPL-3.0) gerando SVG, nos mesmos ≥ 5 arquivos MusicXML licenciados. Cada opção é avaliada por legibilidade a 100%/150%, duas claves, desempenho, licença e esforço de integração. O resultado vira uma ADR de escolha de motor. **F07 entrega só visualizar, navegar, sincronizar com o playback e exportar MusicXML.** A edição de notas fica para depois de F13.

**Por quê:** a própria spec (§15) reconhece que o Bravura não resolve o layout. Visualização, edição e round-trip numa fase só era o maior risco de prazo do projeto, e F13 precisa apenas de leitura.

**Nota de 2026-11-27:** substituída por [ADR-0015](ADR-0015-edicao-nativa-e-plataforma-musical.md). A edição de notas, adiada para depois de F13, passa a ser requisito; o aceite de leitura, playback, navegação e exportação de F07 segue como regressão.
