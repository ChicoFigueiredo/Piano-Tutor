# 17: Inserir nota ou pausa numa posição do compasso e remover nota (vira pausa)

**Fase:** E01

**R-IDs/critérios:** R-E02, R-B06; A5, A6; histórias 10, 11

**Seam:** S1

**Modelo sugerido:** Sonnet / alto

**What to build:** O revisor insere uma nota ou uma pausa numa posição do compasso, recuperando um símbolo que o OMR perdeu, e remove uma nota que o OMR inventou, que vira pausa de mesma duração sem desalinhar o compasso. A inserção respeita a soma do compasso; ocupar espaço de uma pausa substitui a pausa.

**Blocked by:** 16 (Mudar a duração com 1–9 e ponto, com validação do compasso (A4))

**Status:** feito, aguarda revisão (núcleo aceito em 2026-11-28; 17b entregue no app: arquivo `*.ids`, ramos irmãos e cursor de entrada)

- [x] Inserir uma nota onde há pausa livre a substitui, preservando a soma do compasso; inserir onde não há espaço é recusado com mensagem e sem alteração (teste, sem entrada no undo).
- [x] Inserir uma pausa em posição válida é aceito; inserir pausa sobre nota existente é recusado ou pede remoção explícita (decisão registrada no teste).
- [x] Remover uma nota a transforma em pausa de mesma duração e posição; voz, pauta e demais notas do compasso não se movem (propriedade).
- [x] Remover a última nota de uma voz não deixa a voz com soma inválida; a voz fica com pausas até completar o compasso (teste).
- [x] Desfazer e refazer cada uma das duas operações restaura o modelo (A5 em testes dedicados).
- [x] Exportação difere do original só no compasso editado; ornamentos e letra associados a uma nota removida são tratados com aviso explícito (letra/cifra/dedilhado órfãos viram lista de avisos e nunca somem em silêncio).
- [x] A posição de inserção usa as colunas do layout e o cursor de entrada; o teste usa a posição, não coordenadas de pixel.

**Supervisor (2026-11-27):** quando o arquivo lateral de IdNota existir, trocar o pareamento do diff (`documento/diff.rs`, ticket 22) para usá-lo, com teste de inserção no meio de compasso sem `id` no MusicXML.

**Supervisor (2026-11-27):** restaurar a conferência completa de `conferir_exportacao` (`servico.rs`) em edições que criam ou tiram notas, que o ticket 16 reduziu à contagem de notas, e atualizar `tabela_ids_para` no compasso editado.

## Entrega (agente, 2026-11-27)

**Onde:** núcleo `documento/inserir_remover.rs` (contrato em linguagem de domínio no cabeçalho), comandos `InserirNota`, `InserirPausa` e `RemoverNota` em `documento/comando.rs`, atalho `Delete` (Apagar) no mapa e na tela. Testes: `crates/piano-tutor-fixtures/tests/documento_inserir_remover.rs` e `documento_ids_estaveis.rs`; red registrado antes da implementação (os dois arquivos não compilavam: a API não existia).

**Decisões registradas:**
- Pausa em cima de nota é **recusada** (`PausaSobreNota`, pede remoção explícita); pausa sobre pausa regrupa as pausas; pausa idêntica à que já está ali não muda nada.
- Posição = coluna do layout (`Nota::posicao` do modelo de tocar, `Fracao` em semínimas), voz e pauta; nunca pixel.
- Remover nota deixa a mesma `IdNota` na pausa; membro de acorde e nota de adorno saem sem virar pausa. Letra, dedilhado, ornamento e ligadura que saem com a nota viram avisos (`LetraRemovida` etc.); `<tuplet>` fica; cifra é do compasso e não some.
- A exibição (letra, cifra, dedilhado, lista de não suportados) passou a ler o texto **atual** (exportável), não o original, porque depois de inserir ou remover notas os ordinais do original deixam de valer (defeito latente do ticket 16, que já criava pausas).

**Notas do supervisor:**
1. Arquivo lateral de `IdNota` (núcleo): `TabelaIds` ganhou a `serie` (linhagem das identidades, FNV do primeiro texto); `DocumentoAberto::abrir_texto_com_ids`, `abrir_bytes_com_ids`, `tabela_ids` e `tabela_ids_para`. A tabela é ligada ao texto por impressão digital e recusada se o texto for outro.
2. `conferir_exportacao` voltou a conferir as identidades por inteiro (relê com a tabela do texto gravado) e `tabela_ids_para` é refeita a partir do modelo atual, nota por nota, também no compasso editado.
3. Diff (ticket 22): com identidades comparáveis (mesma série) o pareamento é só por `IdNota` (nada de adivinhação); sem série comum, o pareamento de conteúdo de antes. Teste de inserção no meio de compasso sem `id` no MusicXML.

**Pendências explícitas (fora do que foi entregue):**
- **Gravar e ler o arquivo lateral no app.** O núcleo entrega a tabela, mas o app ainda não grava `*.ids` ao lado do texto exportado, nem guarda a tabela junto de cada Versão (`versoes.rs`: objeto por hash) ou do rascunho do autosave. Enquanto isso o diff de Versões da Oficina (`versoes_diff.rs`) abre cada Versão sem tabela e usa o pareamento de conteúdo. Para fechar: `Versoes::salvar` guardar `objetos/<hash>.ids` com `DocumentoAberto::tabela_ids_para`, `Versoes::abrir` devolvê-lo e `versoes_diff::abrir_documento` abrir com `abrir_texto_com_ids`. Fica para um ticket de app porque mexe em `versoes.rs` e no ponto de salvar de `tela_partitura.rs` (outra equipe).
- **Cursor de entrada de notas e atalho de inserir** na tela: os comandos de inserir existem no serviço (posição pela coluna do layout), mas a Oficina ainda não tem o cursor de entrada nem a seleção de pausa; só `Delete` foi ligado.
- Ramos irmãos (duas Versões que saem do mesmo ancestral) herdam o mesmo contador de ids: o pareamento por id entre irmãos pode juntar notas inseridas diferentes como "alterada" (entre ancestral e descendente, não).
- Hardware, áudio e REVAS: NÃO VERIFICADO (nada daqui depende deles).


**Supervisor (2026-11-28):** núcleo aceito: os comandos, o IdNota estável por série, `conferir_exportacao` completo de novo, o diff por IdNota e a correção de `exibicao()`. Reabri o critério do cursor de entrada, porque a Oficina ainda não tem cursor, seleção de pausa nem atalho de inserir. Ele fecha no 17b, junto com a gravação e leitura do arquivo `*.ids` no app (exportação, Versões e autosave) e com a correção dos ramos irmãos.

## Entrega 17b (agente, 2026-11-28)

**Onde:** `ids_lateral.rs` (arquivo lateral), `cursor_entrada.rs` (cursor), `documento_partitura.rs`, `versoes.rs`, `versoes_diff.rs`, `autosave.rs`, `atalhos_partitura.rs`, `tela_partitura.rs` e, na vista, `partitura_visual.rs` e `adaptador_egui.rs` (coluna clicada e marca do cursor). O núcleo não foi tocado.

**Arquivo `*.ids`:**
- Nome: `<arquivo>.ids` ao lado do exportado (`ode.musicxml.ids`); `objetos/<hash>.ids` por Versão (aditivo, o índice não mudou: Versões antigas sem o arquivo seguem no pareamento por conteúdo); `rascunho.ids` no autosave. Escrita atômica, nunca sobrescrita nas Versões. Tabela ausente, estragada, grande demais (16 MB) ou de outro texto é recusada com aviso, nunca com erro fatal: o documento abre e cai no pareamento por conteúdo.
- O diff de Versões da Oficina abre com `abrir_texto_com_ids`.

**Ramos irmãos (decisão):** `Versoes::salvar` na Oficina é sempre chamado com `pai = None` e cada sessão reabre a Hipótese com numeração nova, então a cadeia `pai` do índice não representa a linhagem dos ids. O arquivo guarda uma primeira linha opcional `derivada-de=<hash>` (a Versão de que a numeração deriva; o texto do núcleo é intocado). `Versoes::ids_comparaveis(a, b)` só vale se `a == b` ou uma descende da outra por essa linha; irmãs, primas e Versões sem `.ids` comparam por conteúdo. Testes em `versoes_diff.rs`: irmãs não pareiam por IdNota (red registrado ao forçar `por_ids = true`: 0/0/2 contra 1/1/0), nova sessão, cadeia de uma sessão e linhagem inválida descartada.

**Cursor de entrada:** posição = faixa (parte, pauta, voz) + compasso + índice da coluna do layout (teste prova que as colunas são as de `Layout.compassos[c].colunas`); nunca pixel. Seleciona nota ou pausa (inclusive meio de pausa e voz só de pausas). Atalhos novos, padrão e remapeáveis (a lista foi de 31 para 43, nenhuma removida): letras A a G inserem nota (oitava mais próxima da última nota da voz, ou da linha central da clave; alteração da armadura), `0` insere pausa, `Ctrl+←/→` move o cursor, `Alt+↑/↓` troca de voz/pauta. A duração de entrada é a das teclas 1 a 8 e `.`; com nota selecionada as teclas também mudam a nota. Clique em compasso leva o cursor à coluna mais próxima (`Layout::coluna_mais_proxima`). Corrigido junto: `→`/`←` com pausa selecionada partiam do começo da peça (andavam um passo a mais); agora partem do cursor.

**Limites e pendências:** `Shift+letra` (acrescentar nota ao acorde) é do ticket 18 e segue pendente. Voz só de pausas se alcança pelo cursor (`Alt+↑/↓`), não por clique em nota. Sem janela real: o desenho do cursor foi exercitado em quadro egui sem pânico, não foi visto por pessoa. Hardware, áudio e REVAS: NÃO VERIFICADO (nada daqui depende deles).

**Evidência:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace` limpos (app: 860 passam, 5 ignorados). Testes egui sem janela do cursor em `tela_partitura.rs` (`selecionar_a_pausa_pela_coluna_voz_e_pauta_...`, `letra_sobre_nota_ou_sem_posicao_...`, `ctrl_setas_movem_o_cursor_...`, `zero_insere_pausa_...`, `a_coluna_clicada_vira_posicao_de_entrada_sem_pixel_...`, `atalhos_de_entrada_sao_remapeaveis_...`) e unitários em `cursor_entrada.rs`.
