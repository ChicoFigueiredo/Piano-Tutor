# 23: Aprovar, rejeitar e reabrir uma Versão com motivo, ligadas ao hash (A9)

**Fase:** E01

**R-IDs/critérios:** R-B06, T8; A9; histórias 22, 23, 25

**Seam:** S1

**Modelo sugerido:** Sonnet / médio

**What to build:** O revisor aprova uma Versão (ou a rejeita, ou a reabre) com um motivo, usando a máquina de estados já existente (Hipótese, Em revisão, Aprovada, Rejeitada, aprovação direta marcada). A aprovação é atributo da Versão, por hash, nunca do documento em edição. Editar uma Versão aprovada cria uma nova Versão Em revisão, e a aprovação anterior não vale para ela.

**Blocked by:** 21 (Salvar como nova Versão, com nota de motivo, e listar as Versões)

**Status:** feito, aguarda revisão (módulo `versoes.rs`; a UI de aprovar/rejeitar/reabrir entra com a ligação da tela, depois de 15 e 19)

- [x] A9: dada uma Versão Aprovada, uma edição seguida de salvar cria uma Versão Em revisão com hash diferente; a aprovação da anterior não é herdada (teste de estado). (teste `a9_editar_versao_aprovada_...`)
- [x] As transições válidas são as da máquina existente; transições inválidas são recusadas com mensagem (testes herdados continuam verdes). (teste `transicoes_invalidas_...` e `as_acoes_alcancam_exatamente_...`: as ações usam `transicao_valida` de `revisao`; erro `TransicaoInvalida` com mensagem em PT-BR)
- [x] Aprovar, rejeitar e reabrir exigem um motivo (curto) e registram revisor, data e hash; aprovação direta, quando usada, é marcada como direta. (`aprovar`/`rejeitar`/`reabrir`/`aprovar_direto`; motivo obrigatório até 280 caracteres; `[[decisao]]` no índice com `direto = true` na direta)
- [x] Qualquer consumidor (prática, Exercício) consulta a aprovação por hash, nunca por nome do arquivo (teste: alterar o arquivo muda o hash e a aprovação não vale). (`aprovacao_de(hash)`, `aprovacao_do_conteudo`, `aprovacao_do_arquivo`; o `estado.toml` da partitura segue sem mudança e os consumidores atuais ainda não migraram para a consulta por hash)
- [x] A Hipótese original permanece intacta (hash) mesmo após aprovar uma Versão derivada (T8). (teste `t8_hipotese_original_...`)
- [x] Nada aprova automaticamente: nenhuma saída de IA ou plugin promove conteúdo (só o humano). (`aprovacao_de` só vale com estado Aprovada **e** registro `[[decisao]]`; estado escrito à mão não vale; só chamadas explícitas do revisor mudam o estado)

**Implementação (2026-11-28):** em `crates/piano-tutor-app/src/versoes.rs`. Decisões:
- Transições = só as de `revisao::transicao_valida`. Aprovar: Em revisão → Aprovada. Rejeitar: Hipótese ou Em revisão → Rejeitada. Reabrir: Hipótese, Aprovada ou Rejeitada → Em revisão (de Hipótese equivale a iniciar a revisão). Hipótese → Aprovada só pela aprovação direta, e Aprovada → Rejeitada não existe (reabra antes).
- Aprovação direta: só a partir de Hipótese, marcada `direto = true`; sem motivo escrito usa a nota "aprovação direta" (um clique). Aprovar, rejeitar e reabrir recusam motivo vazio e motivo acima de 280 caracteres (não trunca). Revisor obrigatório.
- Estado e registro vão na mesma gravação atômica do índice; o objeto da Versão nunca é tocado. O registro `[[decisao]]` é append-only (hash, de, para, revisor, em, motivo, direto); campos desconhecidos do índice continuam preservados.
- Nova Versão filha de aprovada já nascia Em revisão (ticket 21); agora há teste. Salvar conteúdo igual ao de Versão existente não cria Versão, logo não re-aprova nem des-aprova.
- Pendência do 21 resolvida: trava de arquivo entre processos (`indice.lock`, `File::lock` da biblioteca padrão, sem crate nova; `flock` no Unix e `LockFileEx` no Windows). Custo baixo: `registrar_origem`, `salvar` e as decisões rodam o ciclo ler-modificar-gravar sob a trava; leitura não trava. Em sistema de arquivos sem suporte (`Unsupported`) segue sem trava. Teste com 8 threads (cada uma com sua `Versoes`) salvando e aprovando: sem a trava o teste falha (corrida no temporário e no índice); com a trava nenhuma Versão ou decisão se perde. Não testado: dois processos de fato nem Windows/macOS (NÃO VERIFICADO).
- Fora do escopo: tela (botões, campo de motivo) e migração dos consumidores atuais (prática, Exercício) para `aprovacao_de(hash)`.
