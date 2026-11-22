# F09 — Pads, drum machine, samples e looper: evidência automatizada

> Ambiente: testes unitários offline no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado e sem medição de áudio.** Pads, sequenciador e metrônomo passam pelo mesmo `ServicoSom`, mas nenhum teste confere a sincronia com o piano nem o som final. Tudo que não tem teste fica **NÃO VERIFICADO** (ADR-0006).

## Execução

```
cargo test --locked -p piano-tutor-core
cargo test --locked -p piano-tutor-app -- ritmo::
```

Atualização: `cargo test --release -p piano-tutor-core -p piano-tutor-app` (testes de ritmo todos passam; falham só 3 testes de `gravacao::`, de outro trabalho em curso) e `cargo clippy` sem avisos nos arquivos de ritmo. Código: `crates/piano-tutor-core/src/ritmo.rs` (peças, padrões de 16 passos, texto de salvamento, fábrica), `crates/piano-tutor-app/src/ritmo.rs` (pads, padrão salvo, parada) e `tela_ritmos.rs` (teclas 1 a 8 como pads, tela).

## Critérios de aceite (backlog/fases/F09.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | Oito pads disparam 8 sons distintos em reprodução simultânea | Não há teste. `ritmo::pad` agenda `INSTRUMENTOS[i]` no relógio de áudio, e as sete primeiras peças têm notas GM distintas (36, 38, 42, 46, 39, 37, 49). A oitava não foi auditada | NÃO VERIFICADO |
| 1b | Choke groups configurados quando aplicáveis | `core::ritmo::chimbal_fechado_corta_o_aberto_e_so_ele` (grupo 42 corta 46) e `app::ritmo::chimbal_fechado_corta_o_aberto_no_mesmo_instante_no_canal_10` (NoteOff da 46 no mesmo instante do NoteOn da 42, canal 10; chimbal fechado mudo não corta). O corte é do plano de agendamento: o efeito no som real depende do SF2 e não foi ouvido | PASS (agendamento offline; som NÃO VERIFICADO) |
| 1c | Pads disparados por clique e pelas teclas 1 a 8 | Mapeamento `TECLAS_PADS` em `tela_ritmos.rs`; sem teste automatizado | NÃO VERIFICADO |
| 2a | Sequenciador de 16 passos salva, recarrega e reproduz padrão idêntico | `app::ritmo::salvar_e_carregar_padrao_identico` (salvar e carregar), `ritmo::padroes_de_fabrica_sao_validos_e_sobrevivem_a_ida_e_volta`, `ritmo::texto_invalido_vira_erro_sem_panico` | PASS (a igualdade do arquivo) |
| 2b | Reproduzir o padrão de forma idêntica no áudio | Teste verifica o arquivo, não o som. Sem teste de render do padrão | NÃO VERIFICADO |
| 2c | 3/4 e 6/8 com grade e agrupamentos próprios ou explicação clara | `ritmo::alternar_e_redimensionar` (3/4 com 2 compassos = 24 passos). Para 6/8 não há teste de grade (12 passos por compasso, por cálculo). A explicação de texto `Formula::explicacao` não tem teste | NÃO VERIFICADO |
| 3 | Metrônomo, sequenciador e piano no mesmo clock de áudio, sem dois backends | Metrônomo e pads agendam pelo `ServicoSom`, que é a saída única. Por inspeção de código, mas sem teste que cubra a sincronia com o piano | NÃO VERIFICADO |
| 4a | 10 minutos com bateria e piano sincronizados | `app::ritmo::dez_minutos_de_bateria_e_metronomo_em_quadros_de_16_ms_sem_deriva`: padrão Funk com metrônomo e semicolcheias, 117 bpm, agendado em quadros de 16 ms com a antecedência real; cada NoteOn cai exatamente (0 ns) no instante teórico do tique, o clique forte cai nos tiques múltiplos de 48, e o resultado em quadros é idêntico ao agendado de uma vez. Cobre bateria e metrônomo; **piano não entra no teste** | PASS (bateria + metrônomo, offline); piano NÃO VERIFICADO |
| 4b | Jitter p95 de agendamento ≤ 10 ms offline | Mesmo teste: desvio do instante agendado = 0 ns em todos os eventos (p95 = 0), logo ≤ 10 ms. É o instante pedido ao motor; a latência real da saída não foi medida | PASS (agendamento offline); jitter de áudio real NÃO VERIFICADO |
| 5a | Parada encerra loops e pads | `app::ritmo::metronomo_agenda_no_canal_10_com_acento` confere que `parar()` desliga `tocando()` | PASS (só o sinalizador) |
| 5b | Nenhuma nota ou sample infinito após a parada | `app::ritmo::parar_calar_todas_as_pecas_depois_do_ultimo_instante_agendado` (NoteOff de todas as 12 peças e dos 2 cliques em último instante agendado + 1 ms, uma vez só) e `parar_e_silenciar_chega_ao_motor_e_nao_deixa_nota_soando` (no motor de teste: zero notas ativas e NoteOff de cada peça no canal 10 após a parada). Pads são notas GM únicas, sem loop de sample. Silêncio audível não foi ouvido | PASS (motor de teste offline); som real NÃO VERIFICADO |
| 5c | Mudo e solo por peça | `app::ritmo::mudo_cala_so_a_peca_e_nao_o_metronomo`, `solo_deixa_so_a_peca_e_vence_o_mudo`, `mudo_e_solo_nao_entram_no_arquivo_do_padrao`; botões M e S por linha na grade (`tela_ritmos`, teste de desenho sem pânico). Estado da sessão, fora do arquivo do padrão | PASS (lógica); interface só desenhada em teste, NÃO VERIFICADO visualmente |
| 6 | Padrão de acompanhamento livre ou licenciado para praticar com o REVAS | Há 11 padrões de fábrica em `ritmo::padroes_de_fabrica` (validados por teste). A licença e a autoria desses padrões não estão declaradas no código | NÃO VERIFICADO |

## Entregáveis não encontrados no código

Estes itens do escopo não aparecem na implementação atual. Não há teste para eles porque não existe o recurso:

- Kit de pads: criar, salvar e carregar um kit (só o padrão do sequenciador é salvo).
- Importação de samples locais autorizados (WAV, FLAC ou OGG).
- Looper de 1 a 4 compassos como recurso separado: o padrão já tem 1 a 4 compassos e repete em laço, mas não há gravação de camadas.

## Pendências e limitações

- Para fechar 1a, 2b, 2c, 3, 4a (piano), 4b (jitter real) e 6: teste de sincronia com piano e bateria, teste de jitter offline para pads e padrões, teste de render, e declaração de licença dos padrões de fábrica.
- Os entregáveis acima precisam ser implementados antes de serem aceitos.
- O aceite com REVAS (acompanhar o piano sobre o ritmo) depende da calibração (F01), que ainda não foi aceita em hardware.
- Esta fase é módulo didático. Não é DAW, nem scratch, nem beatmatching (gate da ficha).

## Status geral

**PASS offline:** 1b (agendamento do corte), 2a (igualdade do arquivo), 4a/4b (bateria e metrônomo, 0 ns de deriva), 5a, 5b (motor de teste) e 5c (lógica de mudo/solo). Todos os demais, e tudo que depende de hardware, REVAS ou de ouvir o som: **NÃO VERIFICADO**. A fase não está aceita.
