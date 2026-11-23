# F06 — Gravar, importar e reproduzir MIDI: evidência automatizada

> Ambiente: testes unitários offline (`cargo test`) no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado, sem dispositivo de áudio e sem escuta humana** nesta execução. A reprodução usa o motor de teste (`MotorDeTeste`, SF2 sintético) e relógio simulado. Tudo abaixo é **PASS (automatizado, offline)**, **NÃO VERIFICADO** ou **pendente humano**. Nada aqui é captura de instrumento real, medição de latência nem escuta (ADR-0006). **Fase não aceita: aprovação humana pendente.**

## Execução

| Comando | Resultado real (2026-11-22) |
|---|---|
| `cargo fmt --all` e `cargo fmt --all -- --check` | sem diferenças |
| `cargo clippy -p piano-tutor-app --all-targets -- -D warnings` | terminou sem avisos nem erros |
| `cargo test -p piano-tutor-app gravacao` | 33 passaram, 0 falharam, 1 ignorado (`gerar_arquivos_de_referencia`, gerador dos arquivos de referência, roda só com `--ignored`). O filtro também pega 2 testes de `tela_gravacao` |
| `cargo test -p piano-tutor-app` (pacote inteiro) | 269 passaram, 0 falharam, 2 ignorados (o gerador acima e `som::testes::sf2_padrao_do_sistema_tem_piano_orgao_e_acordeon`, que exige o SF2 do sistema) |

Código: `crates/piano-tutor-app/src/gravacao.rs` (modelo, SMF, estado e reprodução) e `tela_gravacao.rs` (tela). Arquivos de referência: `crates/piano-tutor-app/tests/fixtures/` (ver `LEIAME.md`).

## Critérios de aceite (backlog/fases/F06.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1a | Gravar sequência com 3 notas, acorde, pausa e pedal simulado; exportar e importar `.mid` preservando altura, ordem, canal e eventos | `gravacao::ida_e_volta_preserva_notas_velocidade_canal_ordem_e_pedal` (3 notas, pausa de 600 ms, acorde de 3 notas em 2 canais, CC64 127 e 0, programa; segunda volta estável), `gravacao::grava_so_quando_gravando_e_preserva_tudo_com_tempo_relativo` (captura pelo estado de gravação, com `NoteOff` forçado no fim), `gravacao::salvar_listar_abrir_e_importar_do_disco` | PASS |
| 1b | Temporização dentro de tolerância de resolução MIDI documentada | O arquivo é escrito a 480 PPQ (1 tick ≈ 1,04 ms a 120 bpm). Os testes de ida e volta exigem diferença ≤ 1 tick (`TICK_NS` = 1.041.667 ns) por evento, e dão iguais exatamente nas referências com tempos múltiplos de tick (`gravacao::referencia_*`) | PASS |
| 1c | Gravação a partir do teclado físico (KB-330/REVAS) e pedal físico | Sem hardware nesta execução. A captura foi testada com eventos sintéticos e pedal simulado (CC64) | NÃO VERIFICADO |
| 2a | Reproduzir em piano e acordeon com alturas e tempos iguais | `gravacao::mesmas_alturas_e_tempos_em_dois_timbres_so_muda_o_programa_do_canal_3` roda a mesma gravação com dois presets e confere mesmas notas, mesma ordem, tempos dentro de um bloco de áudio e troca do programa no canal de demonstração. Os presets são os do SF2 **sintético**, não piano e acordeon reais | PASS (alturas, tempos e troca de preset, offline) |
| 2b | Timbre realmente distinto, soando piano e acordeon | Teste ignorado `som::sf2_padrao_do_sistema_tem_piano_orgao_e_acordeon` (confere só que os programas 0, 19 e 21 existem no FluidR3_GM) **não foi executado**. Escuta não feita | NÃO VERIFICADO / pendente humano (checklist de escuta) |
| 3a | Ao fechar com gravação ativa, avisar sobre dados não salvos | `EstadoGravacao::tem_dados_nao_salvos` cobre a captura em andamento (com pelo menos um evento) e takes sem arquivo: `gravacao::captura_em_andamento_conta_como_nao_salva_e_entra_na_recuperacao` (falso sem eventos, verdadeiro com eventos, ainda gravando), `gravacao::gravacao_vazia_nao_vira_take`. O aviso visível está em `tela_gravacao::desenhar_aviso`, sem teste do desenho. Não há diálogo de confirmação ao fechar: só o aviso na tela de gravação | PASS (condição); aviso na tela NÃO VERIFICADO |
| 3b | Recuperar por salvamento temporário | `gravacao::salvar_sem_pasta_ou_sem_nome_avisa_e_recuperacao_escreve_rascunho` (takes não salvos), `gravacao::captura_em_andamento_conta_como_nao_salva_e_entra_na_recuperacao` (a captura em andamento é finalizada **em cópia** e gravada em `gravacoes/recuperacao/gravacao-em-andamento.mid`, com notas abertas e pedal soltos no fim, sem encerrar a gravação nem criar take; regravar com mais eventos reescreve), `gravacao::recuperacao_salva_takes_e_captura_juntos`. O `impl Drop for AppPianoTutor` (`app.rs`) chama `salvar_recuperacao` quando `tem_dados_nao_salvos`; esse `Drop` não tem teste. A recuperação não é reaberta automaticamente na próxima execução (o aluno abre o `.mid` da pasta) | PASS (função); `Drop` no fechamento NÃO VERIFICADO |
| 4 | Round-trip em 3 arquivos MIDI de referência, com múltiplos canais e mudança de andamento | Ver a tabela abaixo. Os 3 arquivos são sintéticos, escritos para este projeto | PASS |
| 5a | Arquivos malformados sem crash | `gravacao::arquivos_malformados_devolvem_erro_sem_panico` (vazio, cabeçalho errado, tipo 2, SMPTE, trilha truncada, VLQ longo demais, dados > 127, delta acumulado enorme), `gravacao::arquivo_acima_do_limite_e_recusado_e_cortes_e_mutacoes_nao_entram_em_panico` (todos os cortes do arquivo e 3.000 mutações pseudoaleatórias), `gravacao::referencias_cortadas_ou_mutadas_nao_entram_em_panico` (idem sobre os 3 arquivos de referência, 1.000 mutações cada) | PASS |
| 5b | Sem alocação excessiva | Limites no código: 4 MiB por arquivo, 500.000 eventos, 256 trilhas. Testes: `gravacao::muitos_eventos_sao_limitados`, o caso de arquivo acima do limite e o de trilha com tamanho declarado de 4 GiB. **Sem medição de memória** (não há teste que meça o pico de alocação) | PASS (limites) / NÃO VERIFICADO (consumo medido) |

### Arquivos de referência (critério 4)

Montados byte a byte pelo gerador de testes (sem usar o escritor do app), versionados em `crates/piano-tutor-app/tests/fixtures/`. O teste `gravacao::arquivos_de_referencia_commitados_sao_os_gerados` falha se o arquivo versionado diferir do gerador. Cada teste importa o arquivo, confere cada mensagem (altura, velocidade, canal, ordem) e o instante em ns contra uma lista esperada escrita à mão, exporta (tipo 0, 480 PPQ), reimporta e compara (mensagens iguais, instantes em até 1 tick, mesmos canais, mesmas mudanças de andamento), e confere que uma terceira volta gera os mesmos bytes.

| Arquivo | Conteúdo | Teste |
|---|---|---|
| `referencia-1-tipo1-multicanal.mid` (166 bytes) | Tipo 1, 4 trilhas, 480 PPQ, 120 bpm, fórmula de compasso na trilha de andamento; canais 0 (Dó-Mi-Sol), 1 (acorde de 2 notas, programa 40) e 9 (percussão: bumbo, chimbal, caixa, chimbal); 9 notas | `gravacao::referencia_1_tipo_1_multicanal_ida_e_volta` |
| `referencia-2-mudanca-de-andamento.mid` (150 bytes) | Tipo 0, 480 PPQ; 120 bpm, 60 bpm no tick 1920 (2 s) e 240 bpm no tick 2880 (4 s); 10 notas, início de cada uma conferido em ms; as 3 metas de andamento reaparecem no arquivo exportado | `gravacao::referencia_2_mudanca_de_andamento_ida_e_volta` |
| `referencia-3-pedal-acordes-velocidades.mid` (140 bytes) | Tipo 0, 480 PPQ; pedal CC64 com 127, 0, 64 e 0; acorde de 4 notas com velocidades 1, 40, 90 e 127; acorde de 2 notas; escala de velocidades 16, 32, 64, 96, 127; `NoteOn` com velocidade 0 lido como soltar | `gravacao::referencia_3_pedal_acordes_e_velocidades_ida_e_volta` |

## Cobertura extra (fora da lista de aceite)

- Importação tipo 1 com running status, trilhas juntas e mudança de andamento: `gravacao::tipo_1_junta_trilhas_e_respeita_mudanca_de_andamento`; ignorar meta, SysEx, pitch bend e aftertouch: `gravacao::leitor_ignora_meta_sysex_e_mensagens_fora_do_escopo_e_corta_no_fim_da_trilha`; andamento padrão de 120 bpm: `gravacao::andamento_padrao_e_120_bpm_e_mudanca_de_andamento_sobrevive`.
- Reprodução: `gravacao::reproducao_agenda_no_canal_de_demonstracao_com_tempos_relativos`, `gravacao::pausar_solta_as_notas_e_retomar_continua_do_ponto`, `gravacao::parar_solta_notas_ate_as_que_ainda_estao_na_fila_e_o_pedal`, `gravacao::notas_abertas_no_arquivo_soltam_no_fim_e_percussao_vai_para_o_canal_10`, `gravacao::nao_toca_durante_gravacao_nem_sem_selecao`.
- Contagem antes de gravar: `gravacao::contagem_agenda_quatro_cliques_e_ignora_toques_antes_do_inicio`, `gravacao::cancelar_durante_a_contagem_nao_cria_gravacao`.
- Gerenciar takes e linha do tempo: `gravacao::remover_take_em_reproducao_para_e_ajusta_indices`, `gravacao::notas_pareiam_ligar_e_soltar`, `tela_gravacao::linha_do_tempo_com_muitas_notas_nao_trava_nem_entra_em_panico`.

## O que NÃO foi feito ou NÃO VERIFICADO

- Renderizar WAV e "comparação entre original e reprodução" (listados nos experimentos e entregáveis da ficha) **não estão implementados**: `gravacao.rs` e `tela_gravacao.rs` não têm código nem teste para isso (busca por "wav" e "compar"). Não fazem parte dos critérios de aceite F6, mas são entregáveis da ficha.
- Nenhum arquivo de referência foi aberto por outro leitor MIDI (DAW, `midly`, `mido`): não havia nenhum instalado no ambiente. Os arquivos foram validados só pelo leitor do próprio app e pela montagem independente dos bytes no teste. Abrir os 3 `.mid` em outro programa é verificação pendente.
- Escuta de piano e acordeon, captura do KB-330 real e pedal físico: pendentes de hardware e de humano.

## Riscos, licenças e limitações

- **Licenças:** SMF lido e escrito à mão, sem biblioteca nova (o backlog cita `midly`, mas não foi adotada). Os 3 `.mid` são sintéticos, de autoria deste projeto, **CC0 1.0** (declarado em `tests/fixtures/LEIAME.md`). Nenhum arquivo de terceiros, curso ou SoundFont foi adicionado.
- Escrita sempre em tipo 0 a 480 PPQ; a leitura aceita tipo 0 e 1. Tipo 2 e divisão SMPTE são recusados com erro.
- Na importação, pitch bend, aftertouch, SysEx e metas (exceto andamento) são ignorados; na reprodução só o pedal (CC64) é repassado e programa e volume do arquivo não mexem no timbre do app.
- Durante a contagem os toques não são gravados.
- Erro de escrita de até meio tick por evento na primeira exportação (≈ 0,5 ms a 120 bpm), nenhum depois.
- A recuperação da captura em andamento tem a duração do último evento (sem relógio no `Drop`); a contagem antes de gravar não gera rascunho.
- `gravacao::agendar_gravacao` (reuso da reprodução pelos exercícios da F05): `gravacao::agendar_gravacao_toca_no_canal_de_demonstracao_e_solta_o_que_ficar_aberto`.

## Status geral

PASS automatizado, offline: 1a, 1b, 2a (offline), 3a (condição), 3b (função, takes e captura), 4, 5a e 5b (limites). NÃO VERIFICADO ou pendente humano: 1c, 2b, aviso na tela, `Drop` no fechamento, medição de memória, renderização de WAV e comparação original×reprodução. **A fase não está aceita:** a matriz exige aprovação humana e ensaio com hardware.
