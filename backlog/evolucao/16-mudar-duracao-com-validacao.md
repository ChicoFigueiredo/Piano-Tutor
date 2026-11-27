# 16: Mudar a duração com 1–9 e ponto, com validação do compasso (A4)

**Fase:** E01

**R-IDs/critérios:** R-E02, R-E04; A4, A6; histórias 8, 9; SUP: durações 1–9

**Seam:** S1

**Modelo sugerido:** Sonnet / alto

**What to build:** Com uma nota selecionada, as teclas 1 a 9 escolhem a figura (perfil MuseScore 4) e o ponto a prolonga. O serviço valida antes de aplicar: a soma das durações da voz tem de igualar a do compasso (ou o compasso incompleto precisa já estar sinalizado, como numa anacruse). Uma alteração que estoura o compasso é recusada com mensagem clara e nada muda.

**Blocked by:** 15 (Transação com undo/redo por snapshot do compasso (A5))

**Status:** feito, aguarda revisão

- [x] A4: num compasso 4/4 completo, alterar a duração de uma nota de modo que a soma estoure é recusado com mensagem em PT-BR, o modelo fica inalterado e o undo não ganha entrada (teste).
- [x] Encurtar uma nota é aceito e o espaço restante é preenchido por pausa da duração exata, com aviso visível de que a pausa foi criada; nunca por arredondamento.
- [x] A6: compasso de anacruse e compasso incompleto sinalizado continuam válidos; tercina e voz secundária têm testes de soma com `Fracao` exata (inclusive durações com denominador 3 e 5).
- [x] Uma duração que não se representa com `divisions` e Fracao exata é erro explícito, nunca arredondamento.
- [x] Notas ligadas, vigas e tuplas são recalculadas ou invalidadas com aviso; se as vigas ficarem inconsistentes, o compasso é regenerado consistente (teste semântico).
- [x] As teclas 1–9 mapeiam para as figuras do perfil e o ponto alterna; o mapeamento é remapeável pelo ticket 13.
- [x] Exportação após a edição difere do original apenas no compasso editado (golden).

**Implementação:** `crates/piano-tutor-core/src/documento/editar_duracao.rs` (contrato em linguagem de domínio no topo; comandos `MudarDuracao`, `AlternarPonto` e `DefinirDuracao` em `comando.rs`, despachados no começo de `aplicar_sem_historico`; `figura_da_tecla` mapeia 1 a 8). Atalho: `Destino::Duracao(n)` e `Destino::Ponto` em `atalhos_partitura.rs` (nenhum atalho saiu do mapa; remapear vale), e `mudar_duracao`/`alternar_ponto` em `tela_partitura.rs`. Testes: `crates/piano-tutor-fixtures/tests/documento_duracao_edicao.rs` (A4, A5, A6 com denominadores 3 e 5, anacruse, voz secundária, vigas, golden e `divisions`, em todo o corpus; o exaustivo roda com `--ignored` em release), mais os de `atalhos_partitura` e `tela_partitura`.

**Decisões e limites (para a revisão):**
- A soma de cada voz **nunca muda**: encurtar cria pausa(s) exata(s) depois da nota (aviso `PausaCriada`); alongar só cresce sobre pausas que seguem a nota na mesma voz (aviso `PausasConsumidas`). Sem pausa: `CompassoNaoFecha` (estoura, A4) ou `SemPausaParaAlongar`. Por isso o tamanho de nenhum compasso muda (inclusive a anacruse) e o início dos compassos seguintes fica como está.
- `divisions` **não muda** (decisão do supervisor): duração fora da grade é `DuracaoExigeMudarDivisions`, com a mensagem dizendo o que seria preciso. O primeiro erro reportado é o de estouro do compasso (A4), depois o de `divisions`.
- Tecla `9` (longa) é recusada com motivo: o `Figura` do modelo de partitura não tem a longa e acrescentá-la quebraria o desenho (`partitura_visual`, ticket 19). Pausa selecionada não é editável ainda (a seleção só alcança notas).
- Ponto: alterna (0 para 1 ponto, qualquer ponto para 0).
- Vigas: refeitas por pulso (mesmas regras da leitura) só se a edição as deixa inconsistentes (`VigasRefeitas`); inconsistência que já existia no arquivo é preservada. Tupla e ligadura de valor: aviso, sem mexer nas marcas.
- Identidade: a nota editada mantém o `IdNota`; pausas novas ganham id novo. Pausa só parcialmente consumida é trocada por pausas novas com o resto. A releitura de conferência (`conferir_exportacao`) passa a conferir só o total de notas depois de uma edição que cria ou tira notas; o arquivo lateral de ids fica para o ticket 17.
- `inverso` devolve a figura e os pontos de antes; em nota cujo `<type>` não bate com o `<duration>` (comum em OMR) a duração volta a ser a da figura. O desfazer (snapshot do compasso) não depende disso e volta byte a byte.

