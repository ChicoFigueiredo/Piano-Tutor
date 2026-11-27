# 15: Transação com undo/redo por snapshot do compasso (A5)

**Fase:** E01

**R-IDs/critérios:** R-E02; A5, A4 (pré-requisito: recusa sem entrada no undo); história 13

**Seam:** S1

**Modelo sugerido:** Sonnet / alto

**What to build:** Cada comando bem-sucedido vira uma transação que guarda o instantâneo do compasso afetado (e as identidades de nota), permitindo desfazer com Ctrl+Z e refazer com Ctrl+Shift+Z quantas edições forem feitas. Comando recusado não cria entrada no histórico. Uma sequência de edições pode ser agrupada, se for um único gesto do usuário.

**Blocked by:** 14 (Selecionar nota e mudar a altura com seta (semitom, oitava, escala))

**Status:** ready-for-agent

- [ ] A5 (propriedade): para N edições aleatórias válidas sobre cada fixture, desfazer N vezes exporta idêntico ao original (byte a byte) e refazer N vezes exporta idêntico ao estado final (semelhança byte a byte com a exportação feita antes do undo).
- [ ] Um comando recusado deixa o histórico exatamente como estava (testado: tamanho da pilha antes e depois).
- [ ] Editar após um undo descarta o ramo de redo (testado).
- [ ] O histórico tem limite de memória definido e documentado; ao atingi-lo, a edição mais antiga é descartada com aviso, nunca com corrupção (teste com limite pequeno).
- [ ] Ctrl+Z e Ctrl+Shift+Z funcionam na tela e respeitam o perfil de atalhos; desfazer restaura também a seleção.
- [ ] O indicador de modificado (sujo) volta a limpo quando o undo leva ao estado salvo.

**Notas:** O limite de snapshot por compasso é um preço de simplicidade; lote atômico e diff musical completos ficam para a E06.
