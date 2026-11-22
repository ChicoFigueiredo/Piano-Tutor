# Tela Timbres, pânico e robustez: evidência de nuvem (F03, #38)

> Ambiente: **NUVEM / Xvfb**, sem placa de som (`cannot find card '0'`). **NÃO é o cerebro.** Nada aqui é escuta, medição de latência nem prova de som saindo de um alto-falante (ADR-0006).

## O que foi executado

- Tela **Timbres** (`crates/piano-tutor-app/src/som.rs` e `tela_timbres.rs`), no lugar do "em breve":
  - SF2 padrão `/usr/share/sounds/sf2/FluidR3_GM.sf2` (pacote `fluid-soundfont-gm`, dependência do `.deb`, MIT) lido numa thread própria ao abrir o app. Ausente: mensagem que cita o pacote e o app segue sem som.
  - Campo de caminho para outro `.sf2` (Carregar / Usar o padrão). SF2 inválido ou ausente: **erro recuperável** na tela; se já havia um SF2, o som continua com ele.
  - Lista dos presets do SF2 (filtro por nome; percussão, banco 128, aparece desabilitada).
  - Atalhos Piano (GM 0), Acordeon (GM 21) e Órgão (GM 19), desabilitados quando o preset não existe no SF2. A troca de timbre corta as notas soando (spec §3.4) e envia banco + programa a todos os canais menos o 10.
  - Volume 0–100 % (ganho mestre do motor, com rampa).
  - **Pânico**: botão na tela e ação do Ribbon (grupo Som) com atalho **Ctrl+.** (Esc já é da casca). Corta todas as vozes e o sustain no motor (não depende de vaga na fila), limpa o Espelho, e reenvia timbre e volume.
  - Saída de áudio indisponível: status `Áudio: indisponível`, botão "Reabrir saída de áudio"; Espelho, Monitor e simulação seguem funcionando. Falha do dispositivo durante a execução: pânico, uma reabertura automática, depois indisponível.
- A simulação (mouse e teclado do PC) também toca no motor (canal 1, velocidade constante 100, no próximo bloco).

## Testes sem janela e sem hardware

| Teste | O que prova |
|---|---|
| `som::sf2_sintetico_carrega_lista_presets_abre_saida_e_poe_piano_em_15_canais` | SF2 sintético (próprio, sem amostras de terceiros) carregado pelo caminho real de arquivo, presets listados, piano em 15 canais |
| `som::troca_de_preset_chega_ao_sintetizador_e_preset_inexistente_e_recusado` | troca de preset chega ao sintetizador; preset ausente e percussão são erros recuperáveis |
| `som::panico_silencia_ja_e_reenvia_timbre_e_volume` | 4 notas soando → 0 depois do pânico; timbre reenviado |
| `som::sf2_invalido_e_ausente_viram_erro_recuperavel_sem_derrubar` | lixo, arquivo inexistente e lixo depois de um SF2 bom: nenhum pânico, som anterior preservado |
| `som::sem_dispositivo_de_audio_fica_indisponivel_e_comandos_sao_ignorados` | sem saída: "Áudio: indisponível", comandos ignorados sem erro; reabrir reaplica o timbre |
| `som::falha_do_dispositivo_em_execucao_reabre_uma_vez_e_depois_fica_indisponivel` | falha do dispositivo com o app aberto: uma reabertura automática, depois "indisponível" sem tentar a cada quadro; o botão reabre |
| `som::programa_e_banco_vindos_do_dispositivo_nao_trocam_o_timbre` | só CC64 passa; program change e bank select do dispositivo ficam fora |
| `app::panico_corta_o_motor_e_limpa_o_espelho`, `app::ctrl_ponto_dispara_o_panico_e_limpa_a_simulacao` | pânico pelo app e pelo atalho |
| `app::simulacao_toca_no_motor_quando_ha_audio` | mouse/teclado do PC ligam e soltam no motor |
| `app::sem_audio_o_app_funciona_e_status_diz_indisponivel` | todas as telas desenham sem áudio |
| `app::a_fonte_da_janela_tem_os_glifos_usados_nos_textos_das_telas` | nenhum texto novo usa glifo ausente na fonte embutida |
| `som::sf2_padrao_do_sistema_tem_piano_orgao_e_acordeon` (ignorado por padrão) | com o FluidR3_GM instalado: 189 presets, piano "Yamaha Grand Piano", acordeon "Accordian", órgão 19 presente. Executado nesta nuvem com `--ignored`: **passou** |

## Captura

[`timbres-nuvem.png`](timbres-nuvem.png) — binário de depuração sob Xvfb 1280x800, clique sintético em "Timbres". FluidR3_GM carregado (189 presets, piano 0:0 "Yamaha Grand Piano" marcado), `Áudio: indisponível` porque o container não tem placa de som. SHA-256 `cdcea81a45a09fbcd10d96c97814c993a3e890026e453600e77ca09703905f1b`.

## NÃO VERIFICADO

- Som saindo da placa onboard do cerebro (cpal/ALSA ou PipeWire), com o app aberto.
- Checklist de escuta assinado: piano × acordeon (ou órgão) reconhecivelmente diferentes; sem estalo; sem clipping audível no volume padrão.
- Acordes de 10 vozes sustentadas sem cortes e volume respondendo **de ouvido** (os testes offline cobrem o motor, não a escuta).
- Pânico durante execução real (nota presa audível some de imediato).
- Troca de dispositivo de áudio real (fone USB/HDMI) e a reabertura automática.
- Latência MIDI → áudio por captura/loopback (meta p95 ≤ 50 ms) e configuração de buffer recomendada.
- `.deb` instalado no cerebro abrindo o app e tocando MIDI + SF2.
