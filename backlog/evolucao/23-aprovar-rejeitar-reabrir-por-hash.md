# 23: Aprovar, rejeitar e reabrir uma Versão com motivo, ligadas ao hash (A9)

**Fase:** E01

**R-IDs/critérios:** R-B06, T8; A9; histórias 22, 23, 25

**Seam:** S1

**Modelo sugerido:** Sonnet / médio

**What to build:** O revisor aprova uma Versão (ou a rejeita, ou a reabre) com um motivo, usando a máquina de estados já existente (Hipótese, Em revisão, Aprovada, Rejeitada, aprovação direta marcada). A aprovação é atributo da Versão, por hash, nunca do documento em edição. Editar uma Versão aprovada cria uma nova Versão Em revisão, e a aprovação anterior não vale para ela.

**Blocked by:** 21 (Salvar como nova Versão, com nota de motivo, e listar as Versões)

**Status:** em andamento — módulo pronto e revisado pelo supervisor; consumidores e UI pendentes (módulo `versoes.rs`; a UI de aprovar/rejeitar/reabrir entra com a ligação da tela, depois de 15 e 19)

- [x] A9: dada uma Versão Aprovada, uma edição seguida de salvar cria uma Versão Em revisão com hash diferente; a aprovação da anterior não é herdada (teste de estado). (teste `a9_editar_versao_aprovada_...`)
- [x] As transições válidas são as da máquina existente; transições inválidas são recusadas com mensagem (testes herdados continuam verdes). (teste `transicoes_invalidas_...` e `as_acoes_alcancam_exatamente_...`: as ações usam `transicao_valida` de `revisao`; erro `TransicaoInvalida` com mensagem em PT-BR)
- [x] Aprovar, rejeitar e reabrir exigem um motivo (curto) e registram revisor, data e hash; aprovação direta, quando usada, é marcada como direta. (`aprovar`/`rejeitar`/`reabrir`/`aprovar_direto`; motivo obrigatório até 280 caracteres; `[[decisao]]` no índice com `direto = true` na direta)
- [ ] Qualquer consumidor (prática, Exercício) consulta a aprovação por hash, nunca por nome do arquivo (teste: alterar o arquivo muda o hash e a aprovação não vale). (fechado no ticket 25: `pratica_versao::PraticaDaVersao` pergunta `aprovacao_de(hash)` ao abrir e a cada conferência; `DecisorPorHash` decide o estado do Exercício na importação (`importar_com`) pelo hash do conteúdo, e o `estado.toml` deixa de ser a autoridade da aprovação. Testes: `exercicio_consulta_a_aprovacao_por_hash_e_nao_pelo_estado_toml`, `importar_curso_com_o_decisor_por_hash`, `aprovada_so_com_registro_...`. Resíduo: a CLI `piano-tutor-curso importar` continua com `importar`, que usa o decisor legado do `estado.toml`; só o app deve chamar `importar_com` com o `DecisorPorHash`.)
- [x] A Hipótese original permanece intacta (hash) mesmo após aprovar uma Versão derivada (T8). (teste `t8_hipotese_original_...`)
- [x] Nada aprova automaticamente: nenhuma saída de IA ou plugin promove conteúdo (só o humano). (`aprovacao_de` só vale com estado Aprovada **e** registro `[[decisao]]`; estado escrito à mão não vale; só chamadas explícitas do revisor mudam o estado)

**Implementação (2026-11-28):** em `crates/piano-tutor-app/src/versoes.rs`. Decisões:
- Transições = só as de `revisao::transicao_valida`. Aprovar: Em revisão → Aprovada. Rejeitar: Hipótese ou Em revisão → Rejeitada. Reabrir: Hipótese, Aprovada ou Rejeitada → Em revisão (de Hipótese equivale a iniciar a revisão). Hipótese → Aprovada só pela aprovação direta, e Aprovada → Rejeitada não existe (reabra antes).
- Aprovação direta: só a partir de Hipótese, marcada `direto = true`; sem motivo escrito usa a nota "aprovação direta" (um clique). Aprovar, rejeitar e reabrir recusam motivo vazio e motivo acima de 280 caracteres (não trunca). Revisor obrigatório.
- Estado e registro vão na mesma gravação atômica do índice; o objeto da Versão nunca é tocado. O registro `[[decisao]]` é append-only (hash, de, para, revisor, em, motivo, direto); campos desconhecidos do índice continuam preservados.
- Nova Versão filha de aprovada já nascia Em revisão (ticket 21); agora há teste. Salvar conteúdo igual ao de Versão existente não cria Versão, logo não re-aprova nem des-aprova.
- Pendência do 21 resolvida: trava de arquivo entre processos (`indice.lock`, `File::lock` da biblioteca padrão, sem crate nova; `flock` no Unix e `LockFileEx` no Windows). Custo baixo: `registrar_origem`, `salvar` e as decisões rodam o ciclo ler-modificar-gravar sob a trava; leitura não trava. Em sistema de arquivos sem suporte (`Unsupported`) segue sem trava. Teste com 8 threads (cada uma com sua `Versoes`) salvando e aprovando: sem a trava o teste falha (corrida no temporário e no índice); com a trava nenhuma Versão ou decisão se perde. Não testado: dois processos de fato nem Windows/macOS (NÃO VERIFICADO).
- Fora do escopo: tela (botões, campo de motivo) e migração dos consumidores atuais (prática, Exercício) para `aprovacao_de(hash)`.


**Supervisor (2026-11-27):** reabri o critério dos consumidores. A consulta por hash já existe, mas a prática e o Exercício ainda leem o `estado.toml`. A migração entra no ticket 25 (praticar a Versão aprovada), e o critério só fecha quando o 25 fechar. Botões e campo de motivo na tela também ficam pendentes. A trava entre processos foi testada só com threads no Linux: NÃO VERIFICADO em dois processos reais, no Windows e no macOS.

**Ligação à tela (2026-11-28):** na lista de Versões da tela Partitura: campo de motivo e botões Aprovar, Rejeitar, Reabrir (e "Aprovar direto" na Hipótese) por linha; `EstadoPartitura::decidir_versao` mostra "Recusado: ..." com a mensagem do módulo em transição inválida, motivo vazio ou longo (teste egui sem janela). O critério dos consumidores segue aberto (ticket 25).

**Ticket 25 (2026-11-28):** critério dos consumidores fechado, com o resíduo da CLI acima. Os botões já estão na tela (ligação acima).

**Supervisor (2026-11-27, revisão do 25):** o critério dos consumidores continua aberto porque a CLI `piano-tutor-curso importar` ainda usa o decisor legado. Ele fecha quando a CLI também decidir por hash.
