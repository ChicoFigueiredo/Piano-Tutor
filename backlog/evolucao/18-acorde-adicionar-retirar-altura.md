# 18: Acorde: adicionar e retirar alturas de uma nota

**Fase:** E01

**R-IDs/critérios:** R-E02, R-E04; A5, A6; história 12

**Seam:** S1

**Modelo sugerido:** Sonnet / médio

**What to build:** Com uma nota selecionada, Shift+letra adiciona uma altura à nota, formando acorde, e um comando próprio retira uma altura do acorde (a última altura remanescente vira pausa). As alturas do acorde compartilham duração, voz e pauta e soam juntas.

**Blocked by:** 17 (Inserir nota ou pausa numa posição do compasso e remover nota (vira pausa))

**Status:** ready-for-agent

- [ ] Shift+letra adiciona a altura escolhida (mais próxima da nota selecionada) como nota de acorde com mesma posição, duração, voz e pauta; o resultado é igual ao comando equivalente do serviço.
- [ ] Retirar uma altura de um acorde de duas ou mais alturas mantém as demais; retirar a única altura equivale à remoção do ticket 17.
- [ ] Uma altura duplicada no acorde é recusada com mensagem.
- [ ] A marca de acorde do MusicXML, a ordem das alturas e o `chord` ficam corretos na exportação (comparação semântica) e o resto do compasso é idêntico byte a byte fora do compasso editado.
- [ ] Notas ligadas ao próximo (tie) e dedilhado por altura são preservados ou sinalizados.
- [ ] Undo e redo restauram o acorde (A5).
