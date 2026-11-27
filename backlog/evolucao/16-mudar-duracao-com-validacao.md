# 16: Mudar a duração com 1–9 e ponto, com validação do compasso (A4)

**Fase:** E01

**R-IDs/critérios:** R-E02, R-E04; A4, A6; histórias 8, 9; SUP: durações 1–9

**Seam:** S1

**Modelo sugerido:** Sonnet / alto

**What to build:** Com uma nota selecionada, as teclas 1 a 9 escolhem a figura (perfil MuseScore 4) e o ponto a prolonga. O serviço valida antes de aplicar: a soma das durações da voz tem de igualar a do compasso (ou o compasso incompleto precisa já estar sinalizado, como numa anacruse). Uma alteração que estoura o compasso é recusada com mensagem clara e nada muda.

**Blocked by:** 15 (Transação com undo/redo por snapshot do compasso (A5))

**Status:** ready-for-agent

- [ ] A4: num compasso 4/4 completo, alterar a duração de uma nota de modo que a soma estoure é recusado com mensagem em PT-BR, o modelo fica inalterado e o undo não ganha entrada (teste).
- [ ] Encurtar uma nota é aceito e o espaço restante é preenchido por pausa da duração exata, com aviso visível de que a pausa foi criada; nunca por arredondamento.
- [ ] A6: compasso de anacruse e compasso incompleto sinalizado continuam válidos; tercina e voz secundária têm testes de soma com `Fracao` exata (inclusive durações com denominador 3 e 5).
- [ ] Uma duração que não se representa com `divisions` e Fracao exata é erro explícito, nunca arredondamento.
- [ ] Notas ligadas, vigas e tuplas são recalculadas ou invalidadas com aviso; se as vigas ficarem inconsistentes, o compasso é regenerado consistente (teste semântico).
- [ ] As teclas 1–9 mapeiam para as figuras do perfil e o ponto alterna; o mapeamento é remapeável pelo ticket 13.
- [ ] Exportação após a edição difere do original apenas no compasso editado (golden).
