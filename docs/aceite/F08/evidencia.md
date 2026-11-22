# F08 — Metrônomo, compassos e sincronização pedagógica: evidência automatizada

> Ambiente: testes unitários offline no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado e sem medição no dispositivo de áudio.** Os testes de agendamento usam relógio simulado (`RelogioManual`) e render offline. Nada aqui comprova latência real nem escuta humana (ADR-0006).

## Execução

```
cargo test --locked -p piano-tutor-core
cargo test --locked -p piano-tutor-app -- ritmo::
```

Resultado: sem falhas. Código: `crates/piano-tutor-core/src/ritmo.rs` (agendador, fórmulas, julgamento, padrões), `crates/piano-tutor-app/src/ritmo.rs` (serviço: metrônomo via `ServicoSom`, exercício de pulso, pads) e `tela_ritmos.rs` (tela).

## Critérios de aceite (backlog/fases/F08.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | A 120 bpm, semínima = 500 ms exatos (agendamento teórico) | `ritmo::a_120_bpm_a_seminima_tem_500_ms_exatos` | PASS |
| 1b | Erro médio ≤ 2 ms e jitter p95 ≤ 10 ms nos onsets em render offline | Não há teste de render offline do metrônomo com essas métricas. O `jitter_offline` da F03 mede o agendador de notas genérico, não o metrônomo | NÃO VERIFICADO |
| 1c | Medição real no dispositivo, publicada separadamente | Não realizada (exige cerebro e áudio) | NÃO VERIFICADO |
| 2a | 3/4 acentua 1 de 3 | `ritmo::acentos_por_formula` | PASS |
| 2b | 4/4 acentua 1 de 4 | `ritmo::acentos_por_formula` | PASS |
| 2c | 6/8 com 2 pulsos compostos e subdivisão 3+3; não conta seis semínimas | `ritmo::acentos_por_formula` (acento a cada 2 pulsos), `ritmo::subdivisao_em_6_8_da_tres_colcheias_por_pulso` | PASS |
| 3a | Julgamento de toques adiantados, certos e atrasados contra a grade | `ritmo::julgar_toques_adiantados_certos_e_atrasados`, `ritmo::exercicio_de_pulso_julga_e_resume` (app) | PASS |
| 3b | Tolerâncias configuráveis pela interface e feedback visual adequado | Tolerância é um campo do exercício, testado só com o valor padrão. O feedback visual não tem teste | NÃO VERIFICADO |
| 4a | Ajuste de BPM durante a execução sem clique duplo nem batida pulada | `ritmo::mudar_bpm_no_meio_nao_duplica_nem_salta` | PASS |
| 4b | Sinalização do ajuste ao usuário | Não há teste; depende de avaliação visual | NÃO VERIFICADO |
| 5a | 20 minutos sem drift contra referência teórica (offline) | `ritmo::vinte_minutos_sem_deriva` (1.940 pulsos a 97 bpm, erro de até 1 ns) | PASS |
| 5b | 20 minutos de metrônomo real no áudio | Não realizado | NÃO VERIFICADO |

Extras cobertos por teste, fora da lista de aceite: contagem antes do padrão (`ritmo::padrao_toca_os_passos_e_a_contagem_vem_antes`), tap tempo (`ritmo::tap_tempo_estima_e_recomeca_apos_pausa`), posição e passo para a tela (`ritmo::posicao_e_passo_para_a_tela`), metrônomo no canal de percussão com acento (`ritmo::metronomo_agenda_no_canal_10_com_acento`, app).

## Pendências e limitações

- Itens 1b, 3b, 4b e 5b precisam de medição ou avaliação. Para 1b, falta um teste que renderize o metrônomo offline e calcule erro médio e jitter p95 dos onsets.
- `metronomo_agenda_no_canal_10_com_acento` confere que o metrônomo para com `parar()` pelo sinalizador, mas não confere que nenhum clique sai depois disso. Esse ponto é coberto na F09.
- A medição de latência no cerebro (ADR-0006) segue pendente e não foi feita.
- Sem licenças novas nesta fase: o agendador é código próprio.

## Status geral

Itens 1a, 2a–2c, 3a, 4a e 5a: **PASS** (automatizado, offline). Itens 1b, 1c, 3b, 4b e 5b: **NÃO VERIFICADO**. A fase não está aceita: a matriz exige aprovação humana e evidência de hardware.
