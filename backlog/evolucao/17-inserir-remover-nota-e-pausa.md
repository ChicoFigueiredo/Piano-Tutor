# 17: Inserir nota ou pausa numa posição do compasso e remover nota (vira pausa)

**Fase:** E01

**R-IDs/critérios:** R-E02, R-B06; A5, A6; histórias 10, 11

**Seam:** S1

**Modelo sugerido:** Sonnet / alto

**What to build:** O revisor insere uma nota ou uma pausa numa posição do compasso, recuperando um símbolo que o OMR perdeu, e remove uma nota que o OMR inventou, que vira pausa de mesma duração sem desalinhar o compasso. A inserção respeita a soma do compasso; ocupar espaço de uma pausa substitui a pausa.

**Blocked by:** 16 (Mudar a duração com 1–9 e ponto, com validação do compasso (A4))

**Status:** ready-for-agent

- [ ] Inserir uma nota onde há pausa livre a substitui, preservando a soma do compasso; inserir onde não há espaço é recusado com mensagem e sem alteração (teste, sem entrada no undo).
- [ ] Inserir uma pausa em posição válida é aceito; inserir pausa sobre nota existente é recusado ou pede remoção explícita (decisão registrada no teste).
- [ ] Remover uma nota a transforma em pausa de mesma duração e posição; voz, pauta e demais notas do compasso não se movem (propriedade).
- [ ] Remover a última nota de uma voz não deixa a voz com soma inválida; a voz fica com pausas até completar o compasso (teste).
- [ ] Desfazer e refazer cada uma das duas operações restaura o modelo (A5 em testes dedicados).
- [ ] Exportação difere do original só no compasso editado; ornamentos e letra associados a uma nota removida são tratados com aviso explícito (letra/cifra/dedilhado órfãos viram lista de avisos e nunca somem em silêncio).
- [ ] A posição de inserção usa as colunas do layout e o cursor de entrada; o teste usa a posição, não coordenadas de pixel.
