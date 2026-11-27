# 24: Autosave em intervalos curtos e recuperação após queda (A7)

**Fase:** E01

**R-IDs/critérios:** R-E02; A7; história 14

**Seam:** S1

**Modelo sugerido:** Sonnet / alto

**What to build:** O trabalho em andamento é salvo sozinho em intervalos curtos num arquivo de recuperação separado das Versões. Se o app for encerrado abruptamente, ao reabrir a Oficina oferece recuperar o trabalho, com perda de no máximo o intervalo de autosave. O autosave nunca cria Versão e nunca toca o original.

**Blocked by:** 15 (Transação com undo/redo por snapshot do compasso (A5))

**Status:** feito, aguarda revisão (módulo autosave.rs; ligação à tela_partitura e às Preferências fica para quando a UI da Oficina for ligada)

- [x] A7: simulando encerramento abrupto (processo morto sem fechar) com edições não salvas, a reabertura oferece a recuperação e o trabalho recuperado equivale ao último autosave; a perda máxima é o intervalo configurado (teste com relógio injetado).
- [x] O autosave é atômico: queda durante a escrita não corrompe o arquivo anterior (teste de interrupção simulada).
- [x] O arquivo de recuperação é apagado ao salvar uma Versão ou ao descartar a recuperação; recusar a recuperação não apaga o original nem as Versões.
- [x] O autosave não roda se não há edição, não bloqueia a interface e não aparece como Versão; o intervalo e o local são configuráveis e documentados.
- [x] Recuperar restaura o documento e o histórico de undo, ou apenas o documento (decisão registrada; undo perdido é aviso explícito).
- [x] A pasta de recuperação é por documento e não mistura partituras de usuários diferentes.
