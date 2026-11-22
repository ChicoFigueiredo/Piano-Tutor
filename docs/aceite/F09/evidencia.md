# F09 — Pads, drum machine, samples e looper: evidência automatizada

> Ambiente: testes unitários offline no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado e sem medição de áudio.** Pads, sequenciador e metrônomo passam pelo mesmo `ServicoSom`, mas nenhum teste confere a sincronia com o piano nem o som final. Tudo que não tem teste fica **NÃO VERIFICADO** (ADR-0006).

## Execução

```
cargo test --locked -p piano-tutor-core
cargo test --locked -p piano-tutor-app -- ritmo::
```

Resultado: sem falhas. Código: `crates/piano-tutor-core/src/ritmo.rs` (peças, padrões de 16 passos, texto de salvamento, fábrica), `crates/piano-tutor-app/src/ritmo.rs` (pads, padrão salvo, parada) e `tela_ritmos.rs` (teclas 1 a 8 como pads, tela).

## Critérios de aceite (backlog/fases/F09.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | Oito pads disparam 8 sons distintos em reprodução simultânea | Não há teste. `ritmo::pad` agenda `INSTRUMENTOS[i]` no relógio de áudio, e as sete primeiras peças têm notas GM distintas (36, 38, 42, 46, 39, 37, 49). A oitava não foi auditada | NÃO VERIFICADO |
| 1b | Choke groups configurados quando aplicáveis | Não encontrados no código; não implementados | NÃO VERIFICADO |
| 1c | Pads disparados por clique e pelas teclas 1 a 8 | Mapeamento `TECLAS_PADS` em `tela_ritmos.rs`; sem teste automatizado | NÃO VERIFICADO |
| 2a | Sequenciador de 16 passos salva, recarrega e reproduz padrão idêntico | `app::ritmo::salvar_e_carregar_padrao_identico` (salvar e carregar), `ritmo::padroes_de_fabrica_sao_validos_e_sobrevivem_a_ida_e_volta`, `ritmo::texto_invalido_vira_erro_sem_panico` | PASS (a igualdade do arquivo) |
| 2b | Reproduzir o padrão de forma idêntica no áudio | Teste verifica o arquivo, não o som. Sem teste de render do padrão | NÃO VERIFICADO |
| 2c | 3/4 e 6/8 com grade e agrupamentos próprios ou explicação clara | `ritmo::alternar_e_redimensionar` (3/4 com 2 compassos = 24 passos). Para 6/8 não há teste de grade (12 passos por compasso, por cálculo). A explicação de texto `Formula::explicacao` não tem teste | NÃO VERIFICADO |
| 3 | Metrônomo, sequenciador e piano no mesmo clock de áudio, sem dois backends | Metrônomo e pads agendam pelo `ServicoSom`, que é a saída única. Por inspeção de código, mas sem teste que cubra a sincronia com o piano | NÃO VERIFICADO |
| 4a | 10 minutos com bateria e piano sincronizados | Não há teste. `ritmo::vinte_minutos_sem_deriva` cobre só o metrônomo (scheduler), não bateria nem piano | NÃO VERIFICADO |
| 4b | Jitter p95 de agendamento ≤ 10 ms offline | Não há teste de jitter para bateria ou pads | NÃO VERIFICADO |
| 5a | Parada encerra loops e pads | `app::ritmo::metronomo_agenda_no_canal_10_com_acento` confere que `parar()` desliga `tocando()` | PASS (só o sinalizador) |
| 5b | Nenhuma nota ou sample infinito após a parada | Não há teste de silêncio após a parada. Pads são disparos únicos de nota GM, sem loop de sample | NÃO VERIFICADO |
| 6 | Padrão de acompanhamento livre ou licenciado para praticar com o REVAS | Há 11 padrões de fábrica em `ritmo::padroes_de_fabrica` (validados por teste). A licença e a autoria desses padrões não estão declaradas no código | NÃO VERIFICADO |

## Entregáveis não encontrados no código

Estes itens do escopo não aparecem na implementação atual. Não há teste para eles porque não existe o recurso:

- Kit de pads: criar, salvar e carregar um kit (só o padrão do sequenciador é salvo).
- Importação de samples locais autorizados (WAV, FLAC ou OGG).
- Looper de 1 a 4 compassos.
- Mute e solo por peça.
- Choke groups (ver 1b).

## Pendências e limitações

- Para fechar 1a, 2b, 2c, 3, 4a, 4b, 5b e 6: teste de sincronia com piano e bateria, teste de jitter offline para pads e padrões, teste de render, e declaração de licença dos padrões de fábrica.
- Os entregáveis acima precisam ser implementados antes de serem aceitos.
- O aceite com REVAS (acompanhar o piano sobre o ritmo) depende da calibração (F01), que ainda não foi aceita em hardware.
- Esta fase é módulo didático. Não é DAW, nem scratch, nem beatmatching (gate da ficha).

## Status geral

Item 2a (igualdade do arquivo) e 5a (sinalizador de parada): **PASS**. Todos os demais: **NÃO VERIFICADO**. A fase não está aceita.
