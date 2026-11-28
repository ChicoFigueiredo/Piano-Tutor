# 23: Aprovar, rejeitar e reabrir uma Versão com motivo, ligadas ao hash (A9)

**Fase:** E01

**R-IDs/critérios:** R-B06, T8; A9; histórias 22, 23, 25

**Seam:** S1

**Modelo sugerido:** Sonnet / médio

**What to build:** O revisor aprova uma Versão (ou a rejeita, ou a reabre) com um motivo, usando a máquina de estados já existente (Hipótese, Em revisão, Aprovada, Rejeitada, aprovação direta marcada). A aprovação é atributo da Versão, por hash, nunca do documento em edição. Editar uma Versão aprovada cria uma nova Versão Em revisão, e a aprovação anterior não vale para ela.

**Blocked by:** 21 (Salvar como nova Versão, com nota de motivo, e listar as Versões)

**Status:** feito, aguarda revisão — módulo, consumidores (prática, Exercício, CLI) e botões da tela (módulo `versoes.rs`; a UI de aprovar/rejeitar/reabrir entra com a ligação da tela, depois de 15 e 19)

- [x] A9: dada uma Versão Aprovada, uma edição seguida de salvar cria uma Versão Em revisão com hash diferente; a aprovação da anterior não é herdada (teste de estado). (teste `a9_editar_versao_aprovada_...`)
- [x] As transições válidas são as da máquina existente; transições inválidas são recusadas com mensagem (testes herdados continuam verdes). (teste `transicoes_invalidas_...` e `as_acoes_alcancam_exatamente_...`: as ações usam `transicao_valida` de `revisao`; erro `TransicaoInvalida` com mensagem em PT-BR)
- [x] Aprovar, rejeitar e reabrir exigem um motivo (curto) e registram revisor, data e hash; aprovação direta, quando usada, é marcada como direta. (`aprovar`/`rejeitar`/`reabrir`/`aprovar_direto`; motivo obrigatório até 280 caracteres; `[[decisao]]` no índice com `direto = true` na direta)
- [x] Qualquer consumidor (prática, Exercício) consulta a aprovação por hash, nunca por nome do arquivo (teste: alterar o arquivo muda o hash e a aprovação não vale). (prática: `pratica_versao::PraticaDaVersao` pergunta `aprovacao_de(hash)`; Exercício: `DecisorPorHash` decide na importação pelo hash do conteúdo. A CLI `piano-tutor-curso importar` também: `importar` agora usa `DecisorPorHash`, que mora em `piano-tutor-curso` (`aprovacao.rs`, leitura mínima do índice; o app o reexporta e `pratica_versao::estado_do_conteudo` delega a ele, sem duplicar a regra). Testes na CLI: `cli_alterar_o_arquivo_muda_o_hash_e_a_aprovacao_nao_vale`, `cli_estado_toml_aprovada_sem_decisao_nao_aprova`, `indice_malformado_e_erro_e_nunca_aprovacao`; no app: `exercicio_consulta_a_aprovacao_por_hash_e_nao_pelo_estado_toml`, `importar_curso_com_o_decisor_por_hash`. O decisor legado `EstadoDoArquivo` só roda por `importar_com` explícito.)
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

**CLI (2026-11-27):** `piano-tutor-curso importar` decide por hash (mesma regra do app, em `piano-tutor-curso::aprovacao`). Escrita do índice continua só no app. Dois processos reais, Windows e macOS: NÃO VERIFICADO.

**Supervisor (2026-11-28):** critério fechado e aceito, com duas dívidas registradas. (1) A regra de aprovação existe em dois lugares: `curso::aprovacao` e `Indice::aprovacao` no app. O app deve passar a delegar ao curso, que é a fonte única. (2) Migração: aprovações antigas que só existem no `estado.toml` agora viram Hipótese. Antes do gate da E01 (ticket 27), é preciso um passo único que transforme as aprovações antigas do `estado.toml` em Versão com `[[decisao]]` (revisor "migração", direto = true), com aviso ao titular.

**Dívidas do supervisor resolvidas (2026-11-28):**
- (1) Fonte única: a regra vive em `piano_tutor_curso::decisao_que_aprova` (aprovada = estado `aprovada` **e** última `[[decisao]]` do hash com `para = "aprovada"`). `piano_tutor_curso::estado_do_conteudo_no_indice` e `Indice::aprovacao` (app) chamam a mesma função; o app só mantém `revisor`/`motivo`/`direto` (API pública igual). Teste `app_e_curso_nao_divergem_na_regra_de_aprovacao`: matriz de 4 estados x históricos de decisões (vazio, uma, duas, de outro hash) comparando app e curso sobre o mesmo índice.
- (2) Migração: `piano-tutor-app/src/migracao_aprovacao.rs`. Partitura com `estado.toml` = `aprovada` e **sem índice** ganha a origem e uma `[[decisao]]` direta (`direto = true`, revisor "migração", motivo "migração do estado.toml") para o hash ATUAL do `.musicxml`. Idempotente (com índice não faz nada, nem desfaz reabertura), nunca altera o `.musicxml` nem o `estado.toml`, índice ilegível ou arquivo ausente só vira aviso. Roda em `EstadoRevisao::recarregar_oficinas` (abrir a Oficina) e mostra o aviso com a lista migrada. Testes só com oficinas temporárias.
- **Decisão registrada:** se o arquivo mudou desde a aprovação antiga, não há como saber. A aprovação migrada vale para o hash de agora (o aviso diz isso e manda reabrir se o titular desconfiar). Importação pela CLI não migra (a escrita do índice continua só no app); só a abertura da Oficina no app migra. Se o processo cair entre registrar a origem e a decisão, a partitura fica Hipótese (falha segura) e não é migrada de novo. Gate da E01 (ticket 27) desbloqueado quanto a este passo; NÃO VERIFICADO com Oficina real do titular, nem Windows/macOS.

**Supervisor (2026-11-28, revisão da migração):** fonte única e migração aceitas, com um ajuste meu. Se a migração caísse entre registrar a origem e gravar a decisão, a aprovação antiga se perdia em silêncio. Agora um índice que só tem a origem, sem decisão, é completado na próxima abertura. Há teste para esse caso.
