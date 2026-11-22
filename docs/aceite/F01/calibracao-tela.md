# F01 — Tela Calibrar teclado e Monitor MIDI: evidência automatizada

> Ambiente: testes unitários e de integração offline no Linux de desenvolvimento, em 2026-11-22. **Nenhum REVAS foi conectado nesta execução.** Os testes de desconexão e de dez mil pares usam fonte MIDI simulada. Esta evidência **não** prova a calibração física das 61 teclas nem o comportamento de USB real no cerebro. Gate da ficha: hardware real, NÃO VERIFICADO bloqueia F01 (ADR-0006).

## Execução

```
cargo test --locked -p piano-tutor-core -p piano-tutor-midi
cargo test --locked -p piano-tutor-app -- calibracao::
```

Resultado: sem falhas. Código: `crates/piano-tutor-app/src/calibracao.rs` (estado da calibração, perfil, relatório `rev-as-calibracao.md`), `tela_calibracao.rs` (tela, sem teste próprio), `crates/piano-tutor-core/src/midi.rs` e `teclado.rs`, e `crates/piano-tutor-midi` (`capacidades.rs`, `registro.rs`, `fonte.rs`, `gerenciador.rs`).

## Critérios de aceite (backlog/fases/F01.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | Cada uma das 61 posições tem exatamente um código MIDI, com registro de liberação (software) | `core::teclado::todas_as_61_simultaneas`, `app::calibracao::fluxo_completo_com_61_teclas_velocity_e_pedal` (61 teclas conferidas e 61 liberações), `midi::gerenciador::dez_mil_pares_sem_perda_ponta_a_ponta` | PASS (software) |
| 1b | As 61 teclas físicas do REVAS identificadas uma a uma | Exige o teclado conectado. Não executado | NÃO VERIFICADO |
| 2a | 100 notas de teste e 20 acordes conferidos manualmente, sem mapeamento errado | Conferência humana com o REVAS. Não executada | NÃO VERIFICADO |
| 3a | 10.000 pares NoteOn/NoteOff sem perda no pipeline de software | `core::midi::dez_mil_pares_note_on_off_sem_perdas`, `core::midi::dez_mil_pares_em_fluxo_running_status`, `midi::gerenciador::dez_mil_pares_sem_perda_ponta_a_ponta` (fonte injetada em outra thread, zero pares perdidos) | PASS (software) |
| 3b | Estresse físico com o hardware, documentado à parte | Não executado | NÃO VERIFICADO |
| 4a | Desconexão muda o status em até 3 s | `midi::gerenciador::desconexao_muda_status_em_ate_3_s_e_limpa_notas`, `midi::gerenciador::desconexao_detectada_em_ate_2_intervalos_com_atualizar_a_cada_1_s` (fonte simulada) | PASS (software) |
| 4b | Desconexão limpa notas pendentes | `core::teclado::limpar_tudo_na_desconexao`, `midi::gerenciador::desconexao_muda_status_em_ate_3_s_e_limpa_notas` | PASS (software) |
| 4c | Reconexão restaura a operação sem crash | `midi::gerenciador::reconexao_restaura_operacao_mesmo_com_novo_cliente_alsa`, `midi::gerenciador::reconectar_forcado_reabre_a_mesma_porta`, `midi::gerenciador::porta_ausente_vira_erro_e_conecta_sozinha_quando_aparece` | PASS (software) |
| 4d | Hot-plug USB real no ALSA, no cerebro, sem reiniciar o aplicativo | Não executado; a fonte real (`fonte_midir`) só tem teste de listagem sem dispositivo | NÃO VERIFICADO |
| 5a | Velocity, pedal ou program change não transmitidos aparecem como "não suportado pelo dispositivo", sem valor inventado | `app::calibracao::velocity_constante_e_sem_pedal_viram_nao_suportado` (confere a frase no relatório), `midi::capacidades::nada_recebido_e_tudo_nao_observado`, `midi::capacidades::velocity_fixa_nao_conta_como_variavel`, `midi::gerenciador::velocity_fixa_e_sem_pedal_ficam_nao_observados` | PASS (software) |
| 5b | Confirmação com um teclado real que não transmite velocity ou pedal | Não executado | NÃO VERIFICADO |

## Experimentos e entregáveis da ficha

| Item | Testes | Status |
|---|---|---|
| NoteOn com velocity 0 como desligamento | `core::midi::note_on_velocity_zero_e_desligamento`, `core::midi::note_off_explicito` | PASS (software) |
| CC64 e program change | `core::midi::cc64_sustain_e_program_change`, `midi::capacidades::cc64_e_programa_observados` | PASS (software) |
| Calibração de faixa: faixa errada avisa e não gera perfil | `app::calibracao::faixa_errada_avisa_e_nao_gera_perfil` | PASS (software) |
| Perfil salvo (`teclado.txt`) e carregamento inválido recusado | `app::calibracao::perfil_invalido_nao_carrega`, ida e volta de `PerfilTeclado` dentro de `fluxo_completo_com_61_teclas_velocity_e_pedal` | PASS (software) |
| Relatório `rev-as-calibracao.md` com a tabela tecla ↔ MIDI | Gerado em `fluxo_completo_com_61_teclas_velocity_e_pedal`. A tabela contém números **medidos** só com o REVAS | PASS (estrutura); valores medidos NÃO VERIFICADO |
| Log NDJSON anonimizado por sessão | `midi::gerenciador::ndjson_de_sessao_e_valido_e_anonimizado`, `midi::registro::porta_e_mensagem_sao_sanitizadas_e_escapadas`, `midi::registro::nome_de_arquivo_nao_tem_dados_pessoais` | PASS (software) |
| Monitor MIDI: seleção manual de porta | `midi::gerenciador::selecao_manual_conecta_e_entrega_eventos_com_timestamp_do_relogio` | PASS (software) |

## Pendências e limitações

- Todos os itens NÃO VERIFICADO dependem do REVAS no cerebro: 1b, 2a, 3b, 4d, 5b, e os valores medidos do relatório.
- A tela de calibração (`tela_calibracao.rs`) não tem teste de interface. O fluxo está coberto pela lógica de `calibracao.rs`.
- Não se assume "C4 = tecla X": a correspondência só vale após a calibração física (ficha F01).
- Dependências de terceiros (como `midir`) seguem a política em `docs/seguranca/LICENCAS-E-DADOS.md`. Não foram reavaliadas nesta evidência.

## Status geral

Itens automatizados de 1a, 3a, 4a–4c, 5a e dos experimentos: **PASS (software, offline)**. Itens 1b, 2a, 3b, 4d, 5b e os valores medidos: **NÃO VERIFICADO**. A fase F01 não está aceita e não pode avançar: o gate exige hardware real.
