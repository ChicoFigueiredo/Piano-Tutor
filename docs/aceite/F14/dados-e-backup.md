# F14-5 — Dados do usuário: versão, backup e rollback

> Critério (backlog/fases/F14.md): "Migrações preservam dados e rollback/backup por versão é demonstrado em cópias de teste."
> Estado: **teste automatizado, offline, em pastas temporárias** (2026-11-22). Não há demonstração com os dados reais do usuário nem instalação do `.deb`: **NÃO VERIFICADO**. A fase F14 não está aceita.

## O que existe

Módulo `crates/piano-tutor-app/src/dados_usuario.rs` (somente arquivos, sem janela, áudio ou MIDI). Ponto de entrada:

```rust
pub fn preparar(pasta: &Path) -> Result<RelatorioDados, String>
```

Deve ser chamada **uma vez na inicialização**, antes de abrir qualquer arquivo da pasta de dados (`~/.local/share/piano-tutor`). **Ainda não está ligada ao `app.rs`**: a chamada na inicialização é pendência de integração (NÃO VERIFICADO no app real).

| Regra | Comportamento |
|---|---|
| Versão | `versao-dados.txt` na raiz da pasta, um inteiro. Arquivo ausente = versão 0 (dados anteriores ao módulo). Gravação atômica (temporário + renomear). |
| Versão atual | `VERSAO_ATUAL = 1`. Lista ordenada `MIGRACOES`; hoje só `v0 -> v1`, que apenas registra a versão e não altera arquivos. |
| Backup antes de migrar | Só quando há algo a migrar. Copia a pasta inteira para `backups/<versao>-<AAAAMMDD-HHMMSS>/` (UTC; sufixo `-2`, `-3`... em colisão). A pasta `backups/` nunca é copiada para dentro de um backup. A cópia vai para `backups/.tmp-...` e só então é renomeada, então backup incompleto não aparece na lista. Pasta vazia ou inexistente: nada a guardar, sem backup. |
| Plano antes de agir | A cadeia de migrações é montada e conferida antes do backup; se faltar um passo, erro sem alterar nada. |
| Falha numa migração | A pasta volta ao estado do backup (apagando o que a migração criou e devolvendo o que apagou, inclusive a versão). A mensagem de erro diz se a restauração deu certo. Em pasta que estava vazia, volta a vazia. |
| Limite | No máximo `MAX_BACKUPS = 5`; remove os mais antigos (ordem pela data no nome), nunca o recém-criado (mínimo 1). Só poda depois de migrar com sucesso. |
| Rollback | `listar_backups(pasta)` (mais novo primeiro) e `restaurar_backup(pasta, nome)`. Antes de restaurar, faz backup do estado atual (então o rollback também é reversível). A cópia do backup vai para uma área temporária antes de qualquer arquivo ser apagado. Nome inexistente ou com `..`, `/` etc. é recusado sem mudar nada. |
| Proteções | Versão mais nova que a do app: erro, nada é alterado. Arquivo de versão ilegível: erro, nada é alterado. Links simbólicos são ignorados (nem copiados nem seguidos). |

Depois de um rollback os dados voltam à versão do backup; um app mais novo migra de novo na próxima inicialização. Para permanecer na versão antiga é preciso usar o app daquela versão.

## Evidência (comandos reais)

```
cargo fmt --all                                                         -> sem diferenças
cargo clippy --locked -p piano-tutor-app --all-targets -- -D warnings   -> sem avisos
cargo test --locked -p piano-tutor-app dados_usuario                    -> 17 passed; 0 failed
cargo test --locked -p piano-tutor-app                                  -> 289 passed; 0 failed; 1 ignored
```

Todos os testes usam pastas temporárias únicas (`$TMPDIR/piano-tutor-dados-<pid>-...`), apagadas ao fim. Nenhum toca a pasta de dados real.

| Aspecto | Teste |
|---|---|
| Migração preserva dados e cria `backups/<versao>-<data>/` idêntico ao original | `dados_legados_ganham_backup_e_continuam_intactos` |
| Pasta nova, sem backup | `pasta_inexistente_e_criada_na_versao_atual_sem_backup` |
| Idempotência (2ª chamada não migra nem copia) | `segunda_chamada_nao_migra_nem_faz_backup` |
| Backup não copia `backups/` | `backup_nao_copia_a_propria_pasta_de_backups` |
| Limite de N backups; limite 0 mantém o recém-criado | `limite_de_backups_remove_os_mais_antigos` |
| **Falha no meio da migração volta ao estado anterior** (migração que altera, apaga e cria arquivos, seguida de outra que falha) | `falha_no_meio_da_migracao_devolve_o_estado_anterior` |
| Falha na 1ª migração de dados legados volta à versão 0 | `falha_na_primeira_migracao_de_dados_legados_volta_a_versao_zero` |
| Falha em pasta vazia volta a vazia | `falha_em_pasta_vazia_devolve_pasta_vazia` |
| Versão futura e arquivo ilegível não alteram nada | `versao_mais_nova_ou_arquivo_ilegivel_nao_alteram_nada` |
| Cadeia incompleta recusada antes de mudar algo | `migracao_ausente_na_cadeia_e_recusada_antes_de_qualquer_mudanca` |
| Rollback restaura e é reversível | `rollback_restaura_um_backup_e_guarda_o_estado_atual` |
| Rollback recusa nomes fora dos backups | `rollback_recusa_nomes_inexistentes_e_caminhos_fora_dos_backups` |
| Backup incompleto não é listado | `backup_incompleto_nao_aparece_na_lista` |
| Links simbólicos ignorados | `links_simbolicos_nao_sao_seguidos_nem_copiados` |
| Data UTC (valores conferidos com `date -u`) | `data_utc_de_valores_conhecidos` |
| Cadeia `MIGRACOES` contígua até `VERSAO_ATUAL` | `migracoes_conhecidas_formam_uma_cadeia_ate_a_versao_atual` |
| API pública `preparar` | `preparar_publico_funciona_numa_pasta_temporaria` |

## Limitações e decisões pendentes

- **Não integrado.** `preparar` não é chamada pelo app ainda.
- A única migração real (`v0 -> v1`) não altera dados; a prova de "preserva dados" para migrações com efeito usa migrações **de teste**. Migrações SQLite (citadas na ficha F14) não existem ainda.
- **Tamanho.** O backup é cópia completa, inclusive `soundfonts/` (o FluidR3_GM baixado tem dezenas de MB). Com 5 backups isso multiplica o espaço. Decisão de Chico: excluir `soundfonts/` (re-baixável) do backup, ou manter a cópia completa.
- Sem proteção contra corrupção silenciosa de disco, sem verificação de hash do backup e sem bloqueio contra duas instâncias do app abrindo a mesma pasta ao mesmo tempo.
- Sem interface de rollback; hoje só a API (`listar_backups`, `restaurar_backup`).
- Mudança de relógio do sistema para trás pode fazer a ordem por data do nome divergir da ordem real.
