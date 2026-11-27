# 15: Transação com undo/redo por snapshot do compasso (A5)

**Fase:** E01

**R-IDs/critérios:** R-E02; A5, A4 (pré-requisito: recusa sem entrada no undo); história 13

**Seam:** S1

**Modelo sugerido:** Sonnet / alto

**What to build:** Cada comando bem-sucedido vira uma transação que guarda o instantâneo do compasso afetado (e as identidades de nota), permitindo desfazer com Ctrl+Z e refazer com Ctrl+Shift+Z quantas edições forem feitas. Comando recusado não cria entrada no histórico. Uma sequência de edições pode ser agrupada, se for um único gesto do usuário.

**Blocked by:** 14 (Selecionar nota e mudar a altura com seta (semitom, oitava, escala))

**Status:** feito, aguarda revisão

- [x] A5 (propriedade): para N edições aleatórias válidas sobre cada fixture, desfazer N vezes exporta idêntico ao original (byte a byte) e refazer N vezes exporta idêntico ao estado final (semelhança byte a byte com a exportação feita antes do undo).
- [x] Um comando recusado deixa o histórico exatamente como estava (testado: tamanho da pilha antes e depois).
- [x] Editar após um undo descarta o ramo de redo (testado).
- [x] O histórico tem limite de memória definido e documentado; ao atingi-lo, a edição mais antiga é descartada com aviso, nunca com corrupção (teste com limite pequeno).
- [x] Ctrl+Z e Ctrl+Shift+Z funcionam na tela e respeitam o perfil de atalhos; desfazer restaura também a seleção.
- [x] O indicador de modificado (sujo) volta a limpo quando o undo leva ao estado salvo.

**Implementação:** `crates/piano-tutor-core/src/documento/historico.rs` (pilha no `DocumentoAberto`; `aplicar` registra a transação; `desfazer`, `refazer`, `iniciar_grupo`/`encerrar_grupo`, `modificado`/`marcar_salvo`, limite de 16 MiB estimado, aviso `EdicaoMaisAntigaDescartada`). Testes: `crates/piano-tutor-fixtures/tests/documento_historico.rs` (A5 em todo o corpus) e `tela_partitura` (Ctrl+Z, Ctrl+Shift+Z, perfil de atalhos, seleção). Tela real (janela) e som: NÃO VERIFICADO fora dos testes de egui. `marcar_salvo` fica para o ticket 21 chamar ao gravar.

**Notas:** O limite de snapshot por compasso é um preço de simplicidade; lote atômico e diff musical completos ficam para a E06.
