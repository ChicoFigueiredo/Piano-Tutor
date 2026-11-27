# 23: Aprovar, rejeitar e reabrir uma Versão com motivo, ligadas ao hash (A9)

**Fase:** E01

**R-IDs/critérios:** R-B06, T8; A9; histórias 22, 23, 25

**Seam:** S1

**Modelo sugerido:** Sonnet / médio

**What to build:** O revisor aprova uma Versão (ou a rejeita, ou a reabre) com um motivo, usando a máquina de estados já existente (Hipótese, Em revisão, Aprovada, Rejeitada, aprovação direta marcada). A aprovação é atributo da Versão, por hash, nunca do documento em edição. Editar uma Versão aprovada cria uma nova Versão Em revisão, e a aprovação anterior não vale para ela.

**Blocked by:** 21 (Salvar como nova Versão, com nota de motivo, e listar as Versões)

**Status:** ready-for-agent

- [ ] A9: dada uma Versão Aprovada, uma edição seguida de salvar cria uma Versão Em revisão com hash diferente; a aprovação da anterior não é herdada (teste de estado).
- [ ] As transições válidas são as da máquina existente; transições inválidas são recusadas com mensagem (testes herdados continuam verdes).
- [ ] Aprovar, rejeitar e reabrir exigem um motivo (curto) e registram revisor, data e hash; aprovação direta, quando usada, é marcada como direta.
- [ ] Qualquer consumidor (prática, Exercício) consulta a aprovação por hash, nunca por nome do arquivo (teste: alterar o arquivo muda o hash e a aprovação não vale).
- [ ] A Hipótese original permanece intacta (hash) mesmo após aprovar uma Versão derivada (T8).
- [ ] Nada aprova automaticamente: nenhuma saída de IA ou plugin promove conteúdo (só o humano).
