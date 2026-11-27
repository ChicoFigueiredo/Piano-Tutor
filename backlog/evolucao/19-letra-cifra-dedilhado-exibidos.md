# 19: Letra, cifra e dedilhado exibidos e preservados, com lista de elementos não suportados

**Fase:** E01

**R-IDs/critérios:** R-E01, R-E06, D13; A3, A15, A18; histórias 19, 20, 21

**Seam:** S1; seam de render

**Modelo sugerido:** Sonnet / alto

**What to build:** Ao abrir uma partitura, letra (inclusive várias estrofes), cifra e dedilhado já presentes aparecem desenhados junto às notas a que pertencem; dinâmicas e ornamentos são preservados; e uma lista visível diz, por partitura, quais elementos o editor ainda não renderiza nem edita. Nada disso é editável nesta fase, e tudo sobrevive ao salvar, também depois de editar a altura de uma nota do mesmo compasso.

**Blocked by:** 09 (Spike: render egui + SMuFL/Bravura contra Verovio como oráculo (fecha a pendência da ADR-0007)), 12 (Selecionar nota no render e navegar com setas e Tab, com foco visível), 14 (Selecionar nota e mudar a altura com seta (semitom, oitava, escala))

**Status:** ready-for-agent

- [ ] A18: numa fixture com letra, cifra e dedilhado, os três são desenhados junto às notas (teste de layout: cada texto tem posição e está associado à IdNota; captura de tela anexada; legibilidade NÃO VERIFICADA até o checklist visual humano).
- [ ] A15: numa fixture com elemento sem suporte de renderização, a lista de não suportados o menciona (nome do elemento e quantidade) e o elemento sobrevive ao salvar (golden byte a byte fora do compasso editado).
- [ ] A3 (reforço): editar altura em compasso com duas estrofes preserva sílabas, cifra e dedilhado associados às mesmas notas (comparação semântica).
- [ ] Dinâmicas, ornamentos e demais elementos reconhecidos mas não editáveis são preservados na exportação (matriz por elemento começa a ser preenchida: importar, renderizar, tocar, editar, exportar).
- [ ] Letra, cifra e dedilhado não podem ser editados na Oficina (teclas e cliques não os alteram; teste).
- [ ] Um elemento desconhecido nunca aborta a abertura; ele é listado e preservado (T8).

**Notas:** A matriz por elemento mantém o valor 'não suportado' explícito; não declarar conformidade integral sem corpus.
