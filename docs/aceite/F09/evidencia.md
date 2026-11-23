# F09 — Pads, drum machine, samples e looper: evidência automatizada

> Ambiente: testes unitários offline no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado e sem medição de áudio.** Pads, sequenciador, metrônomo e piano passam pelo mesmo `ServicoSom`; a sincronia é conferida em frames no motor de teste (sintetizador falso: um impulso por NoteOn), o que não avalia timbre, latência real nem escuta. Tudo que não tem teste fica **NÃO VERIFICADO** (ADR-0006).

## Execução

```
cargo test --locked -p piano-tutor-core
cargo test --locked -p piano-tutor-app ritmo -- --nocapture
```

Atualização de 2026-11-22 (branch `noite-ritmo`): `cargo test --locked -p piano-tutor-core` (89 testes), `cargo test --locked -p piano-tutor-app` (269 testes, 31 de ritmo) e `cargo test --locked -p piano-tutor-audio`, sem falhas, e `cargo clippy --locked -p piano-tutor-app -p piano-tutor-audio -p piano-tutor-core --all-targets -- -D warnings` sem avisos. Rodada anterior: `cargo test --release -p piano-tutor-core -p piano-tutor-app` (testes de ritmo todos passam; falham só 3 testes de `gravacao::`, de outro trabalho em curso) e `cargo clippy` sem avisos nos arquivos de ritmo. Código: `crates/piano-tutor-core/src/ritmo.rs` (peças, padrões de 16 passos, texto de salvamento, fábrica), `crates/piano-tutor-app/src/ritmo.rs` (pads, padrão salvo, parada) e `tela_ritmos.rs` (teclas 1 a 8 como pads, tela).

## Critérios de aceite (backlog/fases/F09.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | Oito pads disparam 8 sons distintos em reprodução simultânea | `core::ritmo::os_oito_pads_sao_oito_sons_gm_distintos_e_auditados`: as 12 peças têm notas GM únicas e dentro de 35 a 81; as 8 dos pads são 36 Bass Drum 1, 38 Acoustic Snare, 42 Closed Hi-Hat, 46 Open Hi-Hat, 39 Hand Clap, 37 Side Stick, 49 Crash Cymbal 1 e 45 Low Tom (nomes GM conferidos no teste), nenhuma colide com os cliques 76 e 77. Percussão no canal 10 não tem oitava nem transposição aplicada: a nota enviada é a nota GM. `app::ritmo::f09_1_oito_pads_no_mesmo_instante_soam_oito_notas_gm_distintas_no_canal_10`: 8 NoteOn de 8 notas distintas no mesmo frame, 8 notas ativas ao mesmo tempo, todas soltas 90 ms depois | PASS (agendamento e mapeamento offline); que as 8 notas **soam como 8 sons diferentes** depende do SoundFont GM real e não foi ouvido: NÃO VERIFICADO |
| 1b | Choke groups configurados quando aplicáveis | `core::ritmo::chimbal_fechado_corta_o_aberto_e_so_ele` (grupo 42 corta 46) e `app::ritmo::chimbal_fechado_corta_o_aberto_no_mesmo_instante_no_canal_10` (NoteOff da 46 no mesmo instante do NoteOn da 42, canal 10; chimbal fechado mudo não corta). O corte é do plano de agendamento: o efeito no som real depende do SF2 e não foi ouvido | PASS (agendamento offline; som NÃO VERIFICADO) |
| 1c | Pads disparados por clique e pelas teclas 1 a 8 | `tela_ritmos::teclas_1_a_8_sao_oito_teclas_distintas` e `tela_ritmos::cada_tecla_de_1_a_8_dispara_o_pad_dela_no_canal_10` (evento de tecla injetado no egui: tecla n toca `INSTRUMENTOS[n-1]` no canal 10 e não inicia o sequenciador). O disparo por clique do mouse nos botões dos pads não tem teste | PASS (teclas 1 a 8, offline); clique do mouse NÃO VERIFICADO |
| 2a | Sequenciador de 16 passos salva, recarrega e reproduz padrão idêntico | `app::ritmo::salvar_e_carregar_padrao_identico` (salvar e carregar), `ritmo::padroes_de_fabrica_sao_validos_e_sobrevivem_a_ida_e_volta`, `ritmo::texto_invalido_vira_erro_sem_panico` | PASS (a igualdade do arquivo) |
| 2b | Reproduzir o padrão de forma idêntica no áudio | `app::ritmo::f09_2_padrao_salvo_e_recarregado_renderiza_identico`: padrão editado, salvo e recarregado produz os mesmos NoteOn (frame e peça) e o mesmo áudio renderizado offline (mais de 50 batidas) que o original | PASS (render offline com sintetizador falso: um impulso por NoteOn, não o timbre) |
| 2c | 3/4 e 6/8 com grade e agrupamentos próprios ou explicação clara | Grade: `core::ritmo::grade_do_sequenciador_por_formula` (4/4 = 16 passos por compasso; 3/4 = 12; 6/8 = 12; 2/4 = 8; 12/8 = 24; o passo cabe exatamente no pulso), `grade_de_tres_por_quatro_tem_doze_passos_e_um_acento_de_tres` e `grade_de_seis_por_oito_tem_doze_passos_em_dois_pulsos_de_tres_colcheias` (instantes dos passos dos padrões Valsa e Balada 6/8, e explicação do 6/8 com "2 pulsos compostos" e "1-2-3 4-5-6"). Render: `app::ritmo::f09_2_padroes_em_4_4_3_4_e_6_8_renderizados_offline_caem_nos_passos_da_notacao` (Funk 4/4: 119 batidas; Valsa 3/4: 18; Balada 6/8: 47; todas no frame calculado da notação, desvio máximo 0 frame). Observação: o "16 passos" vale para 4/4; em 3/4 e 6/8 a grade tem 12 passos por compasso e isso é a explicação de subdivisão adotada | PASS (offline) |
| 3 | Metrônomo, sequenciador e piano no mesmo clock de áudio, sem dois backends | `app::ritmo::f09_3_metronomo_sequenciador_e_piano_caem_no_mesmo_frame_do_mesmo_relogio_de_audio`: clique, bumbo, chimbal (sequenciador) e nota de piano (`evento_midi` com timestamp) do 1º pulso caem no mesmo frame; o mesmo no 2º pulso (clique, caixa, piano), 24 000 frames depois (500 ms); piano 1 ms fora da grade cai 48 frames fora (controle negativo); a saída de áudio foi aberta uma única vez | PASS (offline, motor de teste) |
| 4a | 10 minutos com bateria e piano sincronizados | `app::ritmo::dez_minutos_de_bateria_e_metronomo_em_quadros_de_16_ms_sem_deriva`: padrão Funk com metrônomo e semicolcheias, 117 bpm, agendado em quadros de 16 ms com a antecedência real; cada NoteOn cai exatamente (0 ns) no instante teórico do tique. **Piano:** `app::ritmo::f09_4_dez_minutos_de_bateria_metronomo_e_piano_juntos_no_mesmo_relogio_de_audio`: 600 s a 120 bpm, Funk com metrônomo e uma nota de piano por pulso, renderizado no motor de teste: 1 199 pulsos, diferença piano menos clique de 0 frame em todos, desvio ao tique teórico de 0 frame (média, p95 e máximo), 8 698 NoteOn de bateria no canal 10 | PASS (offline); piano real tocado por pessoa NÃO VERIFICADO |
| 4b | Jitter p95 de agendamento ≤ 10 ms offline | Os dois testes acima: desvio do instante agendado = 0 ns (primeiro) e 0 frame no render (segundo), p95 = 0, logo ≤ 10 ms. Com despertares do callback modelados (0 a 1 ms mais picos de 2 ms), o metrônomo medido na F08-1 dá jitter p95 de cerca de 0,13 ms (ver `docs/aceite/F08/evidencia.md`). É o instante pedido ao motor; a latência real da saída não foi medida | PASS (agendamento offline); jitter de áudio real NÃO VERIFICADO |
| 5a | Parada encerra loops e pads | `app::ritmo::metronomo_agenda_no_canal_10_com_acento` confere que `parar()` desliga `tocando()` | PASS (só o sinalizador) |
| 5b | Nenhuma nota ou sample infinito após a parada | `app::ritmo::parar_calar_todas_as_pecas_depois_do_ultimo_instante_agendado` (NoteOff de todas as 12 peças e dos 2 cliques em último instante agendado + 1 ms, uma vez só) e `parar_e_silenciar_chega_ao_motor_e_nao_deixa_nota_soando` (no motor de teste: zero notas ativas e NoteOff de cada peça no canal 10 após a parada). Pads são notas GM únicas, sem loop de sample. Silêncio audível não foi ouvido | PASS (motor de teste offline); som real NÃO VERIFICADO |
| 5c | Mudo e solo por peça | `app::ritmo::mudo_cala_so_a_peca_e_nao_o_metronomo`, `solo_deixa_so_a_peca_e_vence_o_mudo`, `mudo_e_solo_nao_entram_no_arquivo_do_padrao`; botões M e S por linha na grade (`tela_ritmos`, teste de desenho sem pânico). Estado da sessão, fora do arquivo do padrão | PASS (lógica); interface só desenhada em teste, NÃO VERIFICADO visualmente |
| 6 | Padrão de acompanhamento livre ou licenciado para praticar com o REVAS | Há 11 padrões de fábrica em `ritmo::padroes_de_fabrica`, validados por `ritmo::padroes_de_fabrica_sao_validos_e_sobrevivem_a_ida_e_volta` e listados por nome em `ritmo::a_fabrica_tem_os_onze_padroes_documentados`. Autoria e licença: [padroes-de-fabrica.md](padroes-de-fabrica.md) (conteúdo original do projeto, **a confirmar pelo titular**). Praticar com o REVAS real depende da calibração (F01) | Documentado; confirmação do titular e uso com REVAS NÃO VERIFICADO |

## Entregáveis não encontrados no código

Estes itens do escopo não aparecem na implementação atual. Não há teste para eles porque não existe o recurso:

- Kit de pads: criar, salvar e carregar um kit (só o padrão do sequenciador é salvo).
- Importação de samples locais autorizados (WAV, FLAC ou OGG).
- Looper de 1 a 4 compassos como recurso separado: o padrão já tem 1 a 4 compassos e repete em laço, mas não há gravação de camadas.

## Pendências e limitações

- Falta, para fechar: ouvir os 8 pads com um SoundFont GM real (1a), clique do mouse nos pads (1c), jitter e sincronia medidos no dispositivo (4a/4b), e a confirmação do titular sobre os padrões de fábrica (6). Os testes de sincronia, render e jitter offline agora existem; usam o sintetizador falso do motor de teste e não avaliam timbre.
- Os entregáveis acima precisam ser implementados antes de serem aceitos.
- O aceite com REVAS (acompanhar o piano sobre o ritmo) depende da calibração (F01), que ainda não foi aceita em hardware.
- Esta fase é módulo didático. Não é DAW, nem scratch, nem beatmatching (gate da ficha).

## Status geral

**PASS offline:** 1a (mapeamento e disparo simultâneo), 1b (agendamento do corte), 1c (teclas 1 a 8), 2a (igualdade do arquivo), 2b e 2c (render e grades), 3 (mesmo frame de clique, sequenciador e piano), 4a/4b (bateria, metrônomo e piano, 0 frame de deriva em 10 min), 5a, 5b (motor de teste) e 5c (lógica de mudo/solo). O item 6 está documentado, com confirmação pendente do titular. Todos os demais, e tudo que depende de hardware, REVAS ou de ouvir o som: **NÃO VERIFICADO**. A fase não está aceita.
