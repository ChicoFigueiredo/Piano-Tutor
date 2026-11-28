# 24: Autosave em intervalos curtos e recuperação após queda (A7)

**Fase:** E01

**R-IDs/critérios:** R-E02; A7; história 14

**Seam:** S1

**Modelo sugerido:** Sonnet / alto

**What to build:** O trabalho em andamento é salvo sozinho em intervalos curtos num arquivo de recuperação separado das Versões. Se o app for encerrado abruptamente, ao reabrir a Oficina oferece recuperar o trabalho, com perda de no máximo o intervalo de autosave. O autosave nunca cria Versão e nunca toca o original.

**Blocked by:** 15 (Transação com undo/redo por snapshot do compasso (A5))

**Status:** feito, aguarda revisão — A7 reaberto pela revisão independente (R1) corrigido e fechado com teste; falta a re-revisão independente do gate 27 (item 3) e o aceite humano. NÃO VERIFICADO com processo morto real nem em janela.

- [x] A7: simulando encerramento abrupto (processo morto sem fechar) com edições não salvas, a reabertura oferece a recuperação e o trabalho recuperado equivale ao último autosave; a perda máxima é o intervalo configurado (teste com relógio injetado). (`tela_partitura::testes::a7_queda_com_edicoes_oferece_recuperar_...` e, para o rascunho **pendente de outra sessão**, `testes_revisao::r1_*`)
- [x] O autosave é atômico: queda durante a escrita não corrompe o arquivo anterior (teste de interrupção simulada).
- [x] O arquivo de recuperação é apagado ao salvar uma Versão ou ao descartar a recuperação; recusar a recuperação não apaga o original nem as Versões.
- [x] O autosave não roda se não há edição, não bloqueia a interface e não aparece como Versão; o intervalo e o local são configuráveis e documentados.
- [x] Recuperar restaura o documento e o histórico de undo, ou apenas o documento (decisão registrada; undo perdido é aviso explícito).
- [x] A pasta de recuperação é por documento e não mistura partituras de usuários diferentes.


**Supervisor (2026-11-27):** A7 pede que a reabertura do app ofereça a recuperação e que a gravação não bloqueie a interface. Até agora isso só foi provado no módulo, então os dois critérios ficam abertos até a ligação com a tela. Outro ponto: o documento recuperado precisa contar como modificado.

**Ligação à tela (2026-11-28):** `EstadoPartitura::autosave_tick(ms)` roda a cada quadro (exporta o texto e grava numa thread via `GravadorDeRascunho`; a interface só consulta o canal); `Autosave::pendente()` ao abrir partitura da Oficina mostra "Recuperar"/"Descartar"; `ao_salvar_versao()` após salvar Versão; o recuperado usa `DocumentoPartitura::de_recuperado` (novo `DocumentoAberto::marcar_modificado` em `historico.rs`) e conta como modificado, com aviso de desfazer perdido. Testes egui sem janela com relógio injetado em ms: sem edição não grava e não vira Versão; queda simulada (reabre pelo arquivo) oferece e recupera igual ao último autosave; descartar só apaga o rascunho. Intervalo/pasta: `config_autosave` (padrão 30 s, ao lado da partitura); tela de Preferências não feita. NÃO VERIFICADO com processo morto real nem em janela.


**Supervisor (2026-11-28, revisão independente):** A7 reaberto. R1 (P0): salvar uma Versão apaga o rascunho de recuperação que ainda espera decisão. Ver docs/evolucao-musical/e01/REVISAO-CODIGO.md.


**Correção da revisão independente (2026-11-28):** R1 (P0) corrigido. Teste que reproduz escrito antes (red: `r1_salvar_versao_fica_bloqueado_com_dica_enquanto_ha_rascunho_pendente` e `r1_descartar_o_rascunho_pendente_libera_o_salvar` falhavam: o salvar passava e o rascunho sumia), depois a correção.
- **Decisão registrada (a mais segura das duas):** enquanto há rascunho pendente de decisão, "Salvar como Versão" fica **bloqueado** com a dica "recupere ou descarte o rascunho antes de salvar uma Versão" (`pode_salvar_versao` e `motivo_sem_salvar_versao`); `salvar_versao` recusa pelo mesmo motivo, então `ao_salvar_versao()` nunca roda com rascunho de outra sessão. Preservar o rascunho e salvar mesmo assim foi descartado: a Versão salva seria a do documento aberto, que não contém o trabalho do rascunho, e dois rascunhos disputariam a mesma pasta. Também fica bloqueado "Continuar a partir desta Versão" (ticket 21) pelo mesmo motivo.
- Depois de Recuperar (o documento passa a ser o do rascunho) ou Descartar, o salvar volta a valer e apaga o rascunho como antes ("o trabalho virou Versão" agora é verdade). O critério "o arquivo de recuperação é apagado ao salvar uma Versão ou ao descartar" segue [x].
- Lacuna conhecida, não corrigida aqui: com a decisão pendente o `autosave_tick` não grava, então edições feitas nesse intervalo não têm proteção até o usuário decidir (e Recuperar as substitui). O banner continua visível; não se bloqueou a edição para não mudar o comportamento da tela além do pedido.

**Supervisor (2026-11-28):** aceito bloquear o salvar enquanto há rascunho pendente. A lacuna aberta ainda é perda de dados: editar com o rascunho pendente não tem autosave, e "Recuperar" substitui essas edições. A correção é bloquear a edição até o titular escolher entre recuperar e descartar, e vai para a rodada de re-revisão.
