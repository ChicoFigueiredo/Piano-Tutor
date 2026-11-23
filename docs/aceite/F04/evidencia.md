# F04 — Núcleo de teoria musical e identificação: evidência automatizada

> Ambiente: testes unitários offline (`cargo test`) no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado e sem dispositivo de áudio** nesta execução. Tudo abaixo é **PASS (automatizado, offline)** ou **NÃO VERIFICADO**. Nada aqui é medição de instrumento ou escuta (ADR-0006).

## Execução

Comandos executados em `~/pt-f04` (ramo `noite-f04`), Linux de desenvolvimento, 2026-11-22:

```
cargo fmt --all --check                                          -> sem diferenças
cargo clippy --locked -p piano-tutor-core --all-targets -- -D warnings  -> sem avisos
cargo test --locked -p piano-tutor-core                          -> 110 passed; 0 failed (antes: 82)
```

Os 28 testes novos foram escritos antes da implementação (a compilação dos testes de `teoria` falhou por falta de `Tonalidade`, `Modo`, `classificar_gesto` etc.) e passaram depois dela. Código: `crates/piano-tutor-core/src/teoria.rs` (módulos `teoria`: tonalidades, grafia por contexto, classificação de gesto), `partitura.rs` (mapa simbólico de compassos) e `ritmo.rs` (agendador). `cargo check` do workspace inteiro (crates de app) **não** foi executado nesta rodada; a mudança em `teoria.rs` é só aditiva.

## Critérios de aceite (backlog/fases/F04.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | Testes para os 128 números MIDI válidos e as 12 classes de altura | `teoria::classes_de_altura_para_os_128_numeros_midi` | PASS |
| 1b | Principais intervalos | `teoria::intervalos_basicos` (terça, quinta, oitava, uníssono, trítono, intervalo composto) | PASS |
| 1c | Conversão A4 = 440 Hz (com A4 parametrizado) | `teclado::frequencias`, `teclado::nomes_pt_e_internacionais` | PASS |
| 1d | Principais tonalidades: as 15 maiores e as 15 menores (nome da tônica, armadura, sete notas na grafia certa, classes de altura), relativas, paralelas, ordem dos acidentes, recusa de armadura fora de ±7; escalas (todas as 7 de `TipoEscala`) nas 12 tônicas, escalas menores de Lá e Dó | `teoria::as_15_tonalidades_maiores_tem_nome_e_armadura_corretos`, `teoria::as_15_tonalidades_menores_tem_nome_e_armadura_corretos`, `teoria::relativas_e_paralelas`, `teoria::acidentes_da_armadura_seguem_a_ordem_tradicional`, `teoria::armadura_fora_de_7_bemois_ou_7_sustenidos_e_recusada`, `teoria::tonalidade_relativa_menor_tem_as_mesmas_notas_da_maior`, `teoria::escalas_para_as_12_tonicas_seguem_a_formula_de_intervalos`, `teoria::escalas_menores_de_la_e_de_do`, `teoria::escalas_e_campo_harmonico` | PASS |
| 2a | Notas isoladas e pares não recebem rótulo de acorde | `teoria::sem_rotulo_quando_nao_ha_casamento`, `teoria::nota_unica_e_nota_isolada_sem_acorde`, `teoria::duas_notas_nunca_ganham_rotulo_de_acorde_nem_de_arpejo` | PASS |
| 2b | Distinguir simultaneidade (acorde), sequência (arpejo) e nota isolada, sem impor rótulo na ambiguidade (`teoria::classificar_gesto`) | `teoria::do_mi_sol_quase_juntos_e_acorde_simultaneo`, `teoria::do_mi_sol_em_sequencia_e_arpejo_ascendente`, `teoria::arpejo_descendente_e_arpejo_de_inversao`, `teoria::arpejo_de_ordem_misturada_tem_direcao_mista`, `teoria::sequencia_que_nao_forma_acorde_nao_vira_arpejo`, `teoria::intervalo_entre_simultaneo_e_sequencia_e_ambiguo`, `teoria::notas_muito_espacadas_sao_notas_separadas_e_nao_arpejo`, `teoria::classificacao_ignora_a_ordem_de_chegada_dos_eventos`, `teoria::mesma_nota_repetida_em_sequencia_nao_e_arpejo`, `teoria::janelas_personalizadas_mudam_o_limite` | PASS (unitário, tempos sintéticos) |
| 3a | Identificar Dó-Mi-Sol e inversões (C, C/E, C/G) | `teoria::do_mi_sol_e_suas_inversoes`, `teoria::construir_acorde_e_inversoes_e_reidentificar` (todas as qualidades, todas as raízes, todas as inversões) | PASS |
| 3b | Acordes com sétima e outras qualidades | `teoria::qualidades_comuns`, `teoria::notas_dobradas_em_oitavas_nao_mudam_o_acorde` | PASS |
| 3c | Enarmonia depende do contexto declarado, não de uma grafia obrigatória (`Tonalidade::grafia`, `Tonalidade::candidatas`) | `teoria::mesma_altura_tem_grafias_diferentes_conforme_a_tonalidade` (F#/Gb, Bb/A#, Cb/B, E#/F por tonalidade), `teoria::grafia_sempre_representa_a_mesma_classe_de_altura` (15 armaduras x 2 modos x 12 classes), `teoria::tonalidades_possiveis_para_uma_tonica_incluem_as_grafias_enarmonicas` | PASS |
| 4a | 3/4, 4/4 e 6/8 com acentos e agrupamentos corretos (módulo `ritmo.rs`, reaproveitado pelo F08) | `ritmo::acentos_por_formula`, `ritmo::subdivisao_em_6_8_da_tres_colcheias_por_pulso` | PASS |
| 4b | Durações racionais sem erro acumulado (tempo em nanossegundos inteiros) | `ritmo::a_120_bpm_a_seminima_tem_500_ms_exatos` | PASS |
| 5 | Sem acúmulo de erro relevante após 1.000 compassos no mapa simbólico | `partitura::mil_compassos_de_tercinas_nao_acumulam_erro_no_mapa_simbolico` (1.000 compassos 4/4 de 12 notas de 1/3 de semínima, a 97 bpm: início de cada compasso e de cada um dos 12.000 eventos exatos em racionais; total 4.000 semínimas), `partitura::mil_compassos_a_120_bpm_dao_exatamente_2000_segundos` (erro 0 ns), `partitura::mil_compassos_de_formulas_mistas_3_4_4_4_e_6_8_somam_exatamente` (total 3.333 semínimas exato), `ritmo::mil_compassos_no_agendador_nao_acumulam_erro_em_nenhuma_formula` (todas as fórmulas, 6 andamentos, erro <= 1 ns em cada compasso), `ritmo::mil_compassos_de_4_4_a_97_bpm_batem_com_o_total_teorico`; complementa `ritmo::vinte_minutos_sem_deriva` (~485 compassos) | PASS |

## Pendências e limitações

- **Nada disso está ligado à interface.** `Tonalidade`, `Tonalidade::grafia` e `classificar_gesto` existem só no núcleo e nos testes; `tela_acordes.rs` e `acordes.rs` ainda não os usam (o botão "Arpejo" continua apenas tocando as notas em sequência). Integração com a interface e com o teclado real: **NÃO VERIFICADO**.
- **Janelas de tempo não calibradas.** `JanelasGesto::PADRAO` (50 ms / 120 ms / 1,5 s) é valor inicial de projeto, testado só com tempos sintéticos. Não houve medição com mãos reais nem com o REVAS: **NÃO VERIFICADO**. A classificação usa só o instante de ataque (ignora soltura e pedal).
- **Pares de notas.** `classificar_gesto` nunca rotula um par como acorde. Já `identificar_acorde` continua devolvendo `C5` para um par Dó-Sol (comportamento anterior, coberto por `qualidades_comuns`). Decisão pendente de Chico: manter "C5" em pares ou passar a exigir três classes de altura também em `identificar_acorde`.
- **Grafia fora da escala.** Para notas fora da escala da tonalidade, `Tonalidade::grafia` usa a grafia mais próxima do centro da armadura (empate resolvido para o bemol). É regra de projeto, não norma; só as notas da escala têm grafia garantida por teste. Dobrados e duplos acidentes não são produzidos.
- A armadura vem declarada (como em `partitura::Armadura`); **não há detecção automática de tonalidade** a partir das notas tocadas.
- Escuta de intervalos e acordes e reconhecimento humano dos acordes não foram avaliados.
- Biblioteca de testes por propriedades (`proptest`) não foi usada; os testes são exemplos fixos mais varreduras exaustivas (128 notas, 12 classes, 30 tonalidades).
- O teste de 1.000 compassos usa o mapa simbólico (racionais exatos) e o agendador de ritmo; não mede tempo real nem áudio.
- Licenças: código próprio, sem bibliotecas de terceiros novas nesta fase.

## Status geral

Critérios 1a a 5: **PASS (automatizado, offline)** nos testes unitários listados. Pendências acima (integração na interface, calibração das janelas, decisão sobre `C5`) seguem **NÃO VERIFICADAS**. **A fase não está aceita**: a matriz exige aprovação humana, e este arquivo não altera `progresso.json` nem a `MATRIZ-DE-ACEITE.md`.
