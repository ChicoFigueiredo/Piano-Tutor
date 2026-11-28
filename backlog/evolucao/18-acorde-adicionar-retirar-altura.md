# 18: Acorde: adicionar e retirar alturas de uma nota

**Fase:** E01

**R-IDs/critérios:** R-E02, R-E04; A5, A6; história 12

**Seam:** S1

**Modelo sugerido:** Sonnet / médio

**What to build:** Com uma nota selecionada, Shift+letra adiciona uma altura à nota, formando acorde, e um comando próprio retira uma altura do acorde (a última altura remanescente vira pausa). As alturas do acorde compartilham duração, voz e pauta e soam juntas.

**Blocked by:** 17 (Inserir nota ou pausa numa posição do compasso e remover nota (vira pausa))

**Status:** feito, aguarda revisão

- [x] Shift+letra adiciona a altura escolhida (mais próxima da nota selecionada) como nota de acorde com mesma posição, duração, voz e pauta; o resultado é igual ao comando equivalente do serviço.
- [x] Retirar uma altura de um acorde de duas ou mais alturas mantém as demais; retirar a única altura equivale à remoção do ticket 17.
- [x] Uma altura duplicada no acorde é recusada com mensagem.
- [x] A marca de acorde do MusicXML, a ordem das alturas e o `chord` ficam corretos na exportação (comparação semântica) e o resto do compasso é idêntico byte a byte fora do compasso editado.
- [x] Notas ligadas ao próximo (tie) e dedilhado por altura são preservados ou sinalizados.
- [x] Undo e redo restauram o acorde (A5).

## Entrega (agente, 2026-11-28)

**Núcleo** (`documento/acorde.rs`, desvio no começo de `aplicar_sem_historico`): `AcrescentarLetra` (Shift+letra: oitava mais próxima, alteração da armadura; a própria nota vira oitava acima), `AcrescentarIntervalo` (2 a 9 acima, escala da armadura), `AdicionarAltura` (forma resolvida, é o `aplicado`) e `RetirarAltura`. Retirar reaproveita `planejar_remocao` do 17 (sem duplicar); com uma só altura equivale a `RemoverNota` (mesmo resultado). Faixa de altura: piano de 88 teclas (A0 a C8), decisão provisória do supervisor, pelo `LimitesDeAltura` já existente.

**Decisões registradas:**
- A nota nova é `<chord/>` com duração, voz, figura, pontos, quiáltera, haste e pauta da principal; entra depois da principal (a principal nunca muda de lugar: carrega letra, vigas, tupla e ligadura), em ordem crescente em relação às que já estavam em ordem.
- Altura repetida = mesmo número MIDI (inclusive grafia enarmônica): `AlturaJaNoAcorde`. Pausa e nota de adorno: `AcordeSoEmNota`. Intervalo fora de 2 a 9: `IntervaloInvalido`.
- No resultado de acrescentar, `nota` é a selecionada (seleção e foco do desfazer ficam) e `estrutura.alvo` é a altura nova (IdNota novo).
- Ligadura: ao acrescentar a acorde que começa ligadura, aviso `AlturaNovaSemLigadura`; ao retirar, os avisos do 17 (letra, dedilhado, ligadura). Dedilhado e ligadura das alturas que ficam seguem intactos.
- App: `Shift+letra` (já existia como ação pendente), `Alt+2` a `Alt+9` (intervalo acima, como no MuseScore) e `Ctrl+Delete` (retirar altura) no mapa, remapeáveis; nenhum atalho removido. Tela: só um `mod acorde;` e um braço no `match` de `Destino` em `tela_partitura.rs` (código em `tela_partitura/acorde.rs`).

**Pendências / riscos:** retirar a nota principal de um acorde leva junto a letra, as vigas e a marca `<tuplet>` que ela carregava (a letra e o resto saem com aviso; vigas são refeitas pela conferência; a marca de tupla não é transferida para a nota seguinte: herdado do 17). Intervalo abaixo e adorno em acorde ficam fora. Som e interface real: NÃO VERIFICADO (só testes automatizados).

**Supervisor (2026-11-28):** aceito. Também aceito as decisões de recusar altura repetida pelo mesmo MIDI, inclusive a enarmônica, e de usar Ctrl+Delete para retirar altura. Intervalo abaixo fica para a E03. A faixa de 88 teclas ainda é provisória e será confirmada com o titular.
