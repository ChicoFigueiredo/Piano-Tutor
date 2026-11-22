# Monitor MIDI e roteamento M1: evidência de nuvem (F01 #32, com F02 #34 e F03 #38)

> Ambiente: **NUVEM / Xvfb** (container Claude Code remoto, OpenGL por software, sem `/dev/snd/seq` e sem placa de som). **NÃO é o cerebro**, não há REVAS nem nenhum teclado MIDI. Nada aqui homologa MIDI real, hot-plug USB ou latência (ADR-0006).

## O que foi executado

- Painel **Monitor MIDI** (`crates/piano-tutor-app/src/monitor.rs` e `tela_monitor.rs`), no lugar do item "em breve": lista de portas, seleção manual, Conectar / Desconectar / Reconectar, estado (`Desconectado`, `Desconectado — aguardando a porta voltar`, `Conectando…`, `Conectado`, `Erro`), tabela rolável dos últimos 200 eventos, capacidades com **"não suportado pelo dispositivo"** para o que não foi observado, contadores e botão Iniciar/Parar log NDJSON.
- Log NDJSON por sessão em `$XDG_DATA_HOME/piano-tutor/sessoes` ou `~/.local/share/piano-tutor/sessoes` (`sessao-midi-<unix>.ndjson`, nunca sobrescreve). Ligar o log com a porta já aberta grava a linha `conectado` da porta atual.
- Barra de status: `MIDI: <estado>` (clicável, abre o Monitor) e `Áudio: <estado>` (clicável, abre Timbres).
- Roteamento a cada quadro (`AppPianoTutor::processar_quadro`): `GerenciadorMidi::atualizar()` → Espelho (`EventoTecla` com origem `DispositivoFisico`, o aviso SIMULAÇÃO some) e motor de áudio (agendado pelo `timestamp_monotonico_ns`). `LimparNotas` → `EstadoEspelho::limpar_tudo` **e** pânico do motor (corte imediato, sustain solto). O próximo quadro é agendado com `request_repaint_after(proxima_varredura_em)`, e a thread do backend MIDI pede um quadro a cada mensagem (aviso de chegada), para o som não esperar o quadro agendado.
- **Relógio único**: `piano_tutor_core::tempo::{Relogio, RelogioMonotonico, RelogioManual}`; `piano-tutor-midi` e `piano-tutor-audio` só reexportam. O app cria um `Arc<dyn Relogio>` e o entrega ao `GerenciadorMidi` (carimbo dos eventos) e à `SaidaAudio` (âncora frames ↔ ns no callback).
- **Produtor único da fila `rtrb`**: só a thread da UI (que é a thread de controle) envia comandos, pelo `ServicoSom`; a thread do SF2 só devolve o `SoundFont`.

## Testes sem janela e sem hardware (`cargo test --workspace`)

Com `FonteFalsa` + `RelogioManual` comum + motor real com `SintetizadorFalso` renderizado offline:

| Teste | O que prova |
|---|---|
| `app::evento_do_teclado_chega_ao_espelho_e_ao_sintetizador_no_frame_esperado` | NoteOn injetado (carimbado pelo relógio comum) liga a tecla no Espelho (aviso de simulação some) e soa no frame `instante + latência alvo` exato do relógio mestre, com o "callback" lendo o mesmo relógio que o gerenciador |
| `app::o_relogio_do_midi_e_o_do_audio_sao_o_mesmo_tipo_e_a_mesma_instancia` | tipo único (verificado pelo compilador) e mesma instância |
| `app::desconexao_limpa_espelho_e_motor_e_status_mostra_aguardando` | porta removida → Espelho vazio, motor silenciado, status `MIDI: Desconectado — aguardando a porta voltar` |
| `monitor::desconexao_mostra_aguardando_e_reconexao_volta_a_conectado` | reconexão automática quando a porta volta |
| `monitor::capacidade_nao_observada_aparece_como_nao_suportada` | velocity fixa continua "não suportado"; liberação passa a observada |
| `monitor::log_ndjson_e_criado_na_pasta_de_sessoes_e_encerrado` | arquivo criado na pasta de sessões, 3 linhas válidas, encerramento conta linhas |
| `monitor::xdg_tem_prioridade_relativo_e_ignorado_e_home_e_o_reserva` | regra XDG |
| `gerenciador::log_liga_e_desliga_em_tempo_de_execucao_e_marca_a_porta_atual` | log em tempo de execução, nada gravado depois de parar |
| `gerenciador::aviso_de_chegada_e_chamado_a_cada_mensagem_inclusive_em_conexao_ja_aberta` | gancho que acorda a UI |
| `tela_monitor::desenha_vazio_conectado_e_com_eventos_sem_panico` | painel desenha vazio, com 20 eventos e com falha de listagem |

## Captura

[`monitor-nuvem.png`](monitor-nuvem.png) — binário de depuração sob Xvfb 1280x800, clique sintético (XTest) no item "Monitor MIDI". Mostra o estado real da nuvem: sem sequenciador ALSA, a listagem falha com `backend MIDI indisponível: MIDI support could not be initialized`, nenhuma porta, botões desabilitados, todas as capacidades como "não suportado pelo dispositivo" (nada foi recebido). SHA-256 `df652ee71e66f27eb9ff9abee6e35c53bfc45acd9f581d6fb3e35b5d61188073`.

## NÃO VERIFICADO

- REVAS / KB-330 físico em USB nativo (ALSA/PipeWire) no cerebro: listagem real, seleção, primeira nota, 61 teclas, velocity, CC64, program change.
- Desconexão e reconexão USB reais: status em até 3 s, notas limpas no Espelho e no som, retomada sem reiniciar.
- Log NDJSON de uma sessão real e conferência manual de 100 notas e 20 acordes.
- Latência MIDI → destaque (F02, p95 ≤ 60 ms) e MIDI → áudio (F03, p95 ≤ 50 ms): **nada medido**. Ver riscos.

## Riscos e limitações

- **Salto pela thread da UI:** o evento MIDI vai do backend para a fila do gerenciador e só chega ao motor quando a UI roda `atualizar()` no quadro seguinte. O aviso de chegada pede o quadro imediatamente, mas o atraso real depende do agendamento do quadro (vsync, carga). Com latência alvo de 10 ms, eventos que chegarem depois do frame de destino tocam no início do bloco seguinte e são contados em "eventos atrasados" (tela Timbres, diagnóstico). Medir no cerebro; se ficar acima da meta, a correção é uma thread de controle dedicada como produtor único (mudança de arquitetura, só com aprovação).
- Na nuvem, sem `/dev/snd/seq`, a ALSA escreve uma linha em stderr a cada varredura (1 s). No cerebro não deve acontecer; não há filtro desse ruído.
- Mudança de programa e seleção de banco vindas do dispositivo **não** trocam o timbre (quem escolhe é a tela Timbres); aparecem só no Monitor.
- Faixa do Espelho continua a padrão de teste (36–96, não calibrada) até a calibração da F01.
