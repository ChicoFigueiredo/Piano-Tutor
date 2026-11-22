# F04 — Núcleo de teoria musical e identificação: evidência automatizada

> Ambiente: testes unitários offline (`cargo test`) no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado e sem dispositivo de áudio** nesta execução. Tudo abaixo é **PASS (automatizado, offline)** ou **NÃO VERIFICADO**. Nada aqui é medição de instrumento ou escuta (ADR-0006).

## Execução

```
cargo test --locked -p piano-tutor-core
```

Resultado: sem falhas em `piano-tutor-core` (módulos `teoria`, `teclado`, `ritmo` e `tempo`). Código: `crates/piano-tutor-core/src/teoria.rs`, `teclado.rs` e, para compassos, `ritmo.rs`.

## Critérios de aceite (backlog/fases/F04.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | Testes para os 128 números MIDI válidos e as 12 classes de altura | `teoria::classes_de_altura_para_os_128_numeros_midi` | PASS |
| 1b | Principais intervalos | `teoria::intervalos_basicos` (terça, quinta, oitava, uníssono, trítono, intervalo composto) | PASS |
| 1c | Conversão A4 = 440 Hz (com A4 parametrizado) | `teclado::frequencias`, `teclado::nomes_pt_e_internacionais` | PASS |
| 1d | Principais tonalidades | `teoria::escalas_e_campo_harmonico` cobre só a escala maior de Dó e o campo harmônico de Sol maior. Escalas menores e as demais tonalidades não têm teste | NÃO VERIFICADO |
| 2a | Notas isoladas e pares não recebem rótulo de acorde | `teoria::sem_rotulo_quando_nao_ha_casamento` | PASS |
| 2b | Distinguir arpejo (eventos sequenciais) de acorde simultâneo | Não há teste de classificação de arpejo. Em `tela_acordes.rs`, `ouvir_em_sequencia` apenas toca as notas em sequência, sem classificar | NÃO VERIFICADO |
| 3a | Identificar Dó-Mi-Sol e inversões (C, C/E, C/G) | `teoria::do_mi_sol_e_suas_inversoes`, `teoria::construir_acorde_e_inversoes_e_reidentificar` (todas as qualidades, todas as raízes, todas as inversões) | PASS |
| 3b | Acordes com sétima e outras qualidades | `teoria::qualidades_comuns`, `teoria::notas_dobradas_em_oitavas_nao_mudam_o_acorde` | PASS |
| 3c | Enarmonia depende do contexto declarado, não de uma grafia obrigatória | Sem teste específico de grafia por contexto | NÃO VERIFICADO |
| 4a | 3/4, 4/4 e 6/8 com acentos e agrupamentos corretos (módulo `ritmo.rs`, reaproveitado pelo F08) | `ritmo::acentos_por_formula`, `ritmo::subdivisao_em_6_8_da_tres_colcheias_por_pulso` | PASS |
| 4b | Durações racionais sem erro acumulado (tempo em nanossegundos inteiros) | `ritmo::a_120_bpm_a_seminima_tem_500_ms_exatos` | PASS |
| 5 | Sem acúmulo de erro relevante após 1.000 compassos no mapa simbólico | `ritmo::vinte_minutos_sem_deriva` cobre 1.940 pulsos a 97 bpm (cerca de 485 compassos de 4/4), com erro de até 1 ns. Não existe teste de 1.000 compassos | NÃO VERIFICADO |

## Pendências e limitações

- Itens 1d, 2b, 3c e 5 precisam de teste próprio para virar PASS. Sugestão: teste de escalas menores e de todas as tonalidades; teste de classificação arpejo × acorde; teste de grafia enarmônica por contexto; teste literal de 1.000 compassos.
- Escuta de intervalos e acordes e reconhecimento humano dos acordes não foram avaliados.
- Biblioteca de testes por propriedades (`proptest`) não foi usada; os testes são exemplos fixos.
- Licenças: código próprio, sem bibliotecas de terceiros novas nesta fase.

## Status geral

Itens automatizados de 1a a 4b: **PASS**. Itens 1d, 2b, 3c e 5: **NÃO VERIFICADO**. A fase não está aceita: a matriz exige aprovação humana.
