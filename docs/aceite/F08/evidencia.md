# F08 — Metrônomo, compassos e sincronização pedagógica: evidência automatizada

> Ambiente: testes unitários offline no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado e sem medição no dispositivo de áudio.** Os testes de agendamento usam relógio simulado (`RelogioManual`) e render offline. Nada aqui comprova latência real nem escuta humana (ADR-0006).

## Execução

```
cargo test --locked -p piano-tutor-core
cargo test --locked -p piano-tutor-app ritmo -- --nocapture
cargo clippy --locked -p piano-tutor-app -p piano-tutor-audio -p piano-tutor-core --all-targets -- -D warnings
```

Resultado (2026-11-22, branch `noite-ritmo`): core 89 testes, app 269 testes (31 de ritmo) e audio sem falhas; clippy sem avisos. Código: `crates/piano-tutor-core/src/ritmo.rs` (agendador, fórmulas, julgamento, padrões), `crates/piano-tutor-app/src/ritmo.rs` (serviço: metrônomo via `ServicoSom`, exercício de pulso, pads) e `tela_ritmos.rs` (tela).

## Critérios de aceite (backlog/fases/F08.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | A 120 bpm, semínima = 500 ms exatos (agendamento teórico) | `ritmo::a_120_bpm_a_seminima_tem_500_ms_exatos` | PASS |
| 1b | Erro médio ≤ 2 ms e jitter p95 ≤ 10 ms nos onsets em render offline | `ritmo::f08_1_a_120_bpm_o_metronomo_renderizado_offline_cumpre_erro_medio_e_jitter`, `ritmo::f08_1_o_aceite_vale_em_outros_andamentos_com_periodo_fora_da_grade_de_frames`, `ritmo::f08_1_com_despertar_irregular_modelado_continua_dentro_do_aceite` (app). Números na seção "Medições offline" abaixo | PASS (offline; ver ressalvas) |
| 1c | Medição real no dispositivo, publicada separadamente | Não realizada (exige cerebro e áudio) | NÃO VERIFICADO |
| 2a | 3/4 acentua 1 de 3 | `ritmo::acentos_por_formula` | PASS |
| 2b | 4/4 acentua 1 de 4 | `ritmo::acentos_por_formula` | PASS |
| 2c | 6/8 com 2 pulsos compostos e subdivisão 3+3; não conta seis semínimas | `ritmo::acentos_por_formula` (acento a cada 2 pulsos), `ritmo::subdivisao_em_6_8_da_tres_colcheias_por_pulso` | PASS |
| 3a | Julgamento de toques adiantados, certos e atrasados contra a grade | `ritmo::julgar_toques_adiantados_certos_e_atrasados`, `ritmo::exercicio_de_pulso_julga_e_resume` (app) | PASS |
| 3b | Tolerâncias configuráveis pela interface e feedback visual adequado | Lógica: `ritmo::a_tolerancia_configurada_muda_o_julgamento_do_mesmo_toque` e `ritmo::feedback_de_cada_julgamento_tem_texto_e_cor_proprios` (core); `ritmo::f08_3_tolerancia_configurada_pelo_usuario_muda_o_julgamento_do_toque` e `ritmo::f08_3_tres_exercicios_adiantado_certo_e_atrasado_sao_resumidos` (app). O controle (slider de 10 a 150 ms) usa `definir_tolerancia_ms`, que limita a faixa; `tela_ritmos::desenha_tocando_com_exercicio_do_pulso` só garante que a tela desenha sem pânico | PASS (lógica); aparência e legibilidade do feedback NÃO VERIFICADO (ninguém olhou a tela) |
| 4a | Ajuste de BPM durante a execução sem clique duplo nem batida pulada | `ritmo::mudar_bpm_no_meio_nao_duplica_nem_salta` | PASS |
| 4b | Sinalização do ajuste ao usuário | `ritmo::f08_4_mudar_o_andamento_tocando_avisa_o_usuario_com_o_de_e_o_para` e `ritmo::f08_4_o_tap_tempo_tocando_tambem_avisa` (texto em `EstadoRitmo::aviso_andamento`, que a tela mostra em âmbar abaixo do transporte). `ritmo::f08_4_no_audio_renderizado_a_troca_de_andamento_nao_duplica_nem_pula_clique` confere no áudio renderizado offline (120 para 60 bpm): nenhum intervalo menor que 500 ms nem maior que 1 s, um só intervalo de transição, forte a cada 4 cliques | PASS (lógica do aviso e áudio offline); a exibição na tela NÃO VERIFICADO visualmente |
| 5a | 20 minutos sem drift contra referência teórica (offline) | `ritmo::vinte_minutos_sem_deriva` (1.940 pulsos a 97 bpm, erro de até 1 ns) | PASS |
| 5b | 20 minutos de metrônomo real no áudio | Não realizado | NÃO VERIFICADO |

Extras cobertos por teste, fora da lista de aceite: contagem antes do padrão (`ritmo::padrao_toca_os_passos_e_a_contagem_vem_antes`), tap tempo (`ritmo::tap_tempo_estima_e_recomeca_apos_pausa`), posição e passo para a tela (`ritmo::posicao_e_passo_para_a_tela`), metrônomo no canal de percussão com acento (`ritmo::metronomo_agenda_no_canal_10_com_acento`, app).

## Medições offline (item 1b)

Método: `renderizar_ritmo` (testes do app) roda `EstadoRitmo` com `RelogioManual`, quadros de 16 ms como a UI, o `ServicoSom` real e o motor de teste (`MotorAudio` + `SintetizadorFalso`, 48 kHz, blocos de 256 frames, latência alvo constante de 480 frames = 10 ms, descontada da medição). O sintetizador falso emite um impulso por NoteOn; os onsets são os frames com amostra não nula do **áudio renderizado**, conferidos com o registro de NoteOn. O erro de cada onset é `frame observado − frame ideal` do instante teórico `inicio + k·60/bpm`; "erro médio" é a média do erro absoluto, "jitter" é `|erro − erro médio|` e p95 é o percentil 95.

| Cenário (offline) | Onsets | Erro médio | Jitter p95 | Jitter máx. |
|---|---|---|---|---|
| 120 bpm, 60 s, despertares ideais | 120 | 0,0000 ms | 0,0000 ms | 0,0000 ms |
| 40, 97, 117 e 220 bpm, 30 s, despertares ideais | 20, 49, 59, 110 | 0,0000 ms | 0,0000 ms | maior desvio: 0 frame |
| 120 bpm, 60 s, **modelo** de despertar do callback (0 a 1 ms uniforme, mais 2 ms em 5% dos blocos), sementes 1, 42 e 2026 | 120 | 0,5899, 0,6085 e 0,5908 ms | 0,1316, 0,1293 e 0,1384 ms | 0,1809, 0,2040 e 0,2009 ms |

Intervalo entre onsets a 120 bpm: 24 000 frames exatos (500 ms) em todos os pares. Limites do aceite: erro médio ≤ 2 ms e jitter p95 ≤ 10 ms; os dois ficam abaixo, com folga grande.

Ressalvas, para não superestimar o resultado:

- Com despertares ideais, o erro 0 é esperado: o relógio mestre posiciona cada evento no frame calculado do mesmo instante, então a medição prova que o agendador e o motor não introduzem erro, não que o dispositivo seja preciso.
- A linha do modelo usa atrasos pseudoaleatórios inventados (mesmo gerador de `jitter.rs`). É um modelo, não medição do PC. O erro médio de cerca de 0,6 ms vem da âncora inicial do relógio (média móvel com o primeiro despertar atrasado); não é viés do metrônomo.
- Não inclui latência do driver, do conversor nem do USB, nem a escuta humana. Medição real do dispositivo segue em 1c.

## Pendências e limitações

- Itens 1c, 3b (aparência), 4b (exibição) e 5b precisam de medição ou avaliação humana no hardware.
- `metronomo_agenda_no_canal_10_com_acento` confere que o metrônomo para com `parar()` pelo sinalizador, mas não confere que nenhum clique sai depois disso. Esse ponto é coberto na F09.
- A medição de latência no cerebro (ADR-0006) segue pendente e não foi feita.
- Sem licenças novas nesta fase: o agendador é código próprio.

## Status geral

Itens 1a, 1b, 2a–2c, 3a, 3b (lógica), 4a, 4b (lógica) e 5a: **PASS** (automatizado, offline). Itens 1c, 5b e a aparência de 3b/4b na tela: **NÃO VERIFICADO**. A fase não está aceita: a matriz exige aprovação humana e evidência de hardware.
