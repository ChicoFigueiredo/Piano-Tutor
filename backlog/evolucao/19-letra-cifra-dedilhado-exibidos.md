# 19: Letra, cifra e dedilhado exibidos e preservados, com lista de elementos não suportados

**Fase:** E01

**R-IDs/critérios:** R-E01, R-E06, D13; A3, A15, A18; histórias 19, 20, 21

**Seam:** S1; seam de render

**Modelo sugerido:** Sonnet / alto

**What to build:** Ao abrir uma partitura, letra (inclusive várias estrofes), cifra e dedilhado já presentes aparecem desenhados junto às notas a que pertencem; dinâmicas e ornamentos são preservados; e uma lista visível diz, por partitura, quais elementos o editor ainda não renderiza nem edita. Nada disso é editável nesta fase, e tudo sobrevive ao salvar, também depois de editar a altura de uma nota do mesmo compasso.

**Blocked by:** 09 (Spike: render egui + SMuFL/Bravura contra Verovio como oráculo (fecha a pendência da ADR-0007)), 12 (Selecionar nota no render e navegar com setas e Tab, com foco visível), 14 (Selecionar nota e mudar a altura com seta (semitom, oitava, escala))

**Status:** feito pelo time, revisado pelo supervisor; aceite de A18 depende do checklist visual do titular (capturas em docs/evolucao-musical/e01/capturas-19/)

- [x] A18: numa fixture com letra, cifra e dedilhado, os três são desenhados junto às notas (teste de layout: cada texto tem posição e está associado à IdNota; captura de tela anexada; legibilidade NÃO VERIFICADA até o checklist visual humano).
- [x] A15: numa fixture com elemento sem suporte de renderização, a lista de não suportados o menciona (nome do elemento e quantidade) e o elemento sobrevive ao salvar (golden byte a byte fora do compasso editado).
- [x] A3 (reforço): editar altura em compasso com duas estrofes preserva sílabas, cifra e dedilhado associados às mesmas notas (comparação semântica).
- [x] Dinâmicas, ornamentos e demais elementos reconhecidos mas não editáveis são preservados na exportação (matriz por elemento começa a ser preenchida: importar, renderizar, tocar, editar, exportar).
- [x] Letra, cifra e dedilhado não podem ser editados na Oficina (teclas e cliques não os alteram; teste).
- [x] Um elemento desconhecido nunca aborta a abertura; ele é listado e preservado (T8).

**Notas:** A matriz por elemento mantém o valor 'não suportado' explícito; não declarar conformidade integral sem corpus.

**Entrega (ticket 19):**

- Núcleo: `Documento::exibicao()` (`crates/piano-tutor-core/src/documento/exibicao.rs`) lê letra (estrofes, hífen, melisma), cifra e dedilhado, cada um ligado à `IdNota`, e a lista de elementos sem suporte (nome, categoria, quantidade). Não toca `comando.rs` nem `servico.rs`.
- Render: `calcular_layout_com_exibicao` (`partitura_visual/textos.rs`) reserva uma linha de altura por estrofe, põe a cifra acima da pauta e o dedilhado com glifos SMuFL da Bravura (`partitura_visual/bravura.rs`, fonte registrada em `app::config`). O golden de primitivas do ticket 06 **não foi alterado** (os caminhos sem exibição são os de antes; teste de igualdade).
- Oficina: lista visível "Elementos que o editor ainda não desenha nem edita" (`tela_partitura.rs`).
- Matriz por elemento (começo): `docs/evolucao-musical/e01/MATRIZ-POR-ELEMENTO.md`. Capturas para o titular e checklist visual: `docs/evolucao-musical/e01/capturas-19/`.
- **NÃO VERIFICADO:** legibilidade (checklist humano), GPU real, HiDPI, Windows e macOS, exportações reais de outros programas.

**Supervisor (2026-11-27):** a exibição agora é recalculada depois de editar, desfazer e refazer (`DocumentoPartitura::recalcular`). Lacunas aceitas para a E01: o prolongamento de melisma e a elisão não são desenhados, e a cifra sai com # e b em vez de ♯ e ♭.
