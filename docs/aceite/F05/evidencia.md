# F05 — Professor assistido: exercícios, feedback e progressão: evidência automatizada

> Ambiente: testes unitários offline (`cargo test`) no Linux de desenvolvimento, em 2026-11-22. **Sem REVAS conectado, sem dispositivo de áudio e sem avaliação humana de uso** nesta execução. Os testes alimentam o classificador com notas e instantes sintéticos. Tudo abaixo é **PASS (automatizado, offline)**, **NÃO VERIFICADO** ou **pendente humano**. **Fase não aceita: aprovação humana pendente.**

## Execução

| Comando | Resultado real (2026-11-22) |
|---|---|
| `cargo fmt --all` e `cargo fmt --all -- --check` | sem diferenças |
| `cargo clippy -p piano-tutor-app --all-targets -- -D warnings` | terminou sem avisos nem erros |
| `cargo test -p piano-tutor-app exercicios` | 27 passaram, 0 falharam, 0 ignorados (26 em `exercicios::` e 1 em `tela_exercicios::`) |
| `cargo test -p piano-tutor-app` (pacote inteiro) | 269 passaram, 0 falharam, 2 ignorados (nenhum deles é desta fase) |

Código: `crates/piano-tutor-app/src/exercicios.rs` (exercícios, classificador, progressão, progresso em texto) e `tela_exercicios.rs` (tela). Prefixo dos testes abaixo: `exercicios::testes::`.

## Critérios de aceite (backlog/fases/F05.md)

| # | Critério | Testes que cobrem | Status |
|---|---|---|---|
| 1 | Exercício de 10 notas com acertos e erros injetados; classificador de acerto responde 100% conforme gabarito nos casos determinísticos | `exercicio_de_dez_notas_com_acertos_e_erros_injetados_classifica_100_por_cento` (exercício 2; roteiro de 14 toques com 4 erros injetados; cada toque confere a classificação esperada; no fim 10 acertos, 4 erros, aproveitamento 71%, 14 eventos no registro). Reforço: `dez_exercicios_progressivos_tem_campos_completos_e_gabarito_jogavel` (tocar o gabarito dos 10 exercícios conclui todos, sem erro) | PASS |
| 2a | Acorde aceito por conjunto de notas dentro de janela temporal configurável | `acorde_dentro_da_janela_e_aceito`, `acorde_fora_da_janela_nao_e_aceito_nem_conta_erro`, `janela_e_configuravel` (padrão 150 ms, faixa 10 a 2000 ms), `acorde_aceita_inversao_dobra_e_rejeita_nota_extra_ou_errada`, `tercas_e_quintas_exigem_as_notas_exatas` | PASS |
| 2b | Arpejo exige a sequência correta | `arpejo_exige_a_ordem_correta` | PASS |
| 3a | Não penalizar eventos extra de `NoteOff` | `noteoff_extra_nunca_penaliza` | PASS |
| 3b | Não impedir a correção de nota errada | `nota_errada_nao_avanca_e_permite_corrigir` (a nota errada conta erro, o passo não avança, a nota certa em seguida é aceita) | PASS |
| 4a | 10 exercícios com objetivo, instruções, gabarito, pré-requisito e feedback | `dez_exercicios_progressivos_tem_campos_completos_e_gabarito_jogavel` (F05-4: exatamente 10; ids 1 a 10; títulos únicos; tipo esperado de cada um; título, objetivo, instruções, feedback de acerto, feedback de erro e conclusão preenchidos; gabarito não vazio, notas dentro das 61 teclas (36 a 96), forma do gabarito conforme o tipo; pré-requisito encadeado; dedilhado onde se aplica; gabarito em texto com um trecho por passo), `fabrica_tem_dez_exercicios_completos_e_encadeados`, `gabaritos_batem_com_a_teoria` (tríades do exercício 8 e escala do 6 conferidas contra `piano-tutor-core::teoria`) | PASS |
| 4b | Progressão avança e retrocede corretamente | `progressao_avanca_retrocede_e_bloqueia` (bloqueia até concluir o anterior, `avancar`, `voltar`, voltar não perde o progresso, não avança além do último) | PASS |
| 5a | Feedback inclui replay do trecho usando a Gravação de F06 | `exercicios::tentativa_vira_gravacao_da_f06_com_o_que_o_aluno_tocou` (o registro da tentativa vira uma `Gravacao` da F06: seis toques, certos e errados, na ordem e nos instantes relativos; NoteOff não entra; exporta para `.mid` e reimporta com as mesmas mensagens), `tela_exercicios::ouvir_tentativa_toca_de_novo_o_que_o_aluno_tocou_inclusive_o_erro` (a mesma lógica de agendamento da reprodução da F06, `gravacao::agendar_gravacao`, agenda no motor de teste as notas na ordem e nos intervalos tocados, no canal de demonstração, e solta tudo no fim), `gravacao::agendar_gravacao_toca_no_canal_de_demonstracao_e_solta_o_que_ficar_aberto`, `exercicios::sem_toques_nao_ha_grade_nem_erros_nem_replay`. Botão "Ouvir o que toquei" em `tela_exercicios.rs`. O replay é da tentativa inteira (até 256 eventos), não de um trecho recortado ao redor do erro | PASS (offline, motor de teste); botão e som reais NÃO VERIFICADOS |
| 5b | Feedback indica o compasso do erro com a grade de F08 | `exercicios::erro_e_localizado_no_compasso_e_tempo_da_grade_4_por_4` (grade `piano_tutor_core::ritmo::Agendador` em 4/4 a 60 bpm, compasso 1 = primeira nota da tentativa: erros em "compasso 1, tempo 3" e "compasso 2, tempo 2", com passo e nota esperados e texto `[X]`), `exercicios::andamento_da_grade_muda_o_compasso_do_erro` (a 120 bpm os mesmos erros caem em "compasso 2, tempo 1" e "compasso 3, tempo 4"), `exercicios::bpm_da_grade_e_salvo_e_valores_fora_da_faixa_sao_ignorados`. A tela lista "Onde errei" e tem o ajuste de andamento da grade. **Limitação:** o exercício não tem andamento próprio nem metrônomo; a grade é uma convenção (4/4, andamento escolhido pelo aluno, compasso 1 na primeira nota), então o "compasso" mede o tempo decorrido, não a pulsação de um metrônomo tocando junto. Tela (texto e botão) sem teste | PASS (lógica, offline); tela NÃO VERIFICADA |
| 6a | Progresso sobrevive a reinicialização | `progresso_sobrevive_a_reinicializacao_e_pode_ser_apagado_e_exportado` (salva em pasta temporária, cria novo estado a partir do disco), `progresso_ida_e_volta_em_texto` | PASS |
| 6b | Progresso pode ser apagado e exportado por comando explícito | `progresso_sobrevive_a_reinicializacao_e_pode_ser_apagado_e_exportado` (chama `apagar_progresso` e `exportar`, que grava `progresso-exportado.txt`). A confirmação em dois cliques para apagar existe só na tela (`tela_exercicios.rs`) e **não tem teste**, `exportar_sem_pasta_informa_em_vez_de_falhar` | PASS |
| 6c | Arquivo de progresso corrompido não derruba o app | `arquivo_corrompido_volta_aos_padroes_sem_panico`, `arquivo_binario_no_disco_usa_padroes` (extra, não pedido na ficha) | PASS |

## Escopo da ficha coberto por teste, fora da lista de aceite

- Modo ouvir e repetir esconde as guias, e a dica as mostra: `ouvir_e_repetir_esconde_as_guias_e_a_dica_as_mostra`.
- Pausa e modo sem limite de tempo (limite opcional): `pausa_e_limite_de_tempo_opcionais`; reiniciar e repetir: `reiniciar_e_repetir`.
- Feedback com texto e símbolo, sem depender só de cor: `feedback_texto` começa com `[OK]` ou `[X]`, conferido em `exercicio_de_dez_notas...` e `nota_errada_nao_avanca_e_permite_corrigir`. A tela escolhe uma cor de apoio (`cor_do_tom`); o contraste e a leitura na tela **não foram avaliados**.
- Estatísticas por sessão: `Contagem` conferida no teste do exercício de 10 notas (10 acertos, 4 erros, 71%).

## O que NÃO foi feito ou NÃO VERIFICADO

- Replay e compasso do erro (5a e 5b): lógica testada offline; o botão "Ouvir o que toquei", o texto "Onde errei" e o som real não foram vistos nem ouvidos por ninguém, e o replay não recorta um trecho em torno do erro.
- Só há teste do replay na tela de exercícios (`tela_exercicios.rs`), nenhum de desenho: layout, legibilidade, clique e leitura por humano ficam **NÃO VERIFICADO**.
- "Ouvir" (tocar a frase do exercício 10 pelo motor de som) usa o `ServicoSom`; não há teste de que a frase soa, e a escuta não foi feita.
- Uso com teclado real (REVAS/KB-330), latência entre tocar e feedback e acorde tocado por mãos humanas (janela de 150 ms): **NÃO VERIFICADO**, sem hardware.
- Efeito pedagógico dos 10 exercícios (se a ordem e a dificuldade funcionam para um aluno): não avaliado; exige revisão humana (professor).
- Progressão por pré-requisito linear: não há ramificação nem recomendação adaptativa (fora do escopo da ficha).
- Intervalos, escalas e arpejos de outras tonalidades, e dedilhado só como texto sugerido (sem verificação do dedilhado tocado), não existem nesta entrega.

## Riscos, licenças e limitações

- **Licenças:** código próprio, sem biblioteca nova. Os exercícios são autorais e simples (escala de Dó, tríades, arpejo); a frase do exercício 10 (Mi-Mi-Fá-Sol-Sol-Fá) é um padrão elementar de escala, sem arquivo de curso, partitura ou mídia de terceiros.
- Replay agenda no máximo 256 eventos (`REPLAY_EVENTOS_MAX`) para não encher a fila do motor; cada nota do replay dura 3/4 do intervalo até a próxima (até 400 ms), não a duração real do aluno (o registro guarda só o início de cada nota).
- O progresso é salvo em texto simples (`progresso.txt`), sem SQLite; a ficha cita SQLite "quando necessário". Sem dados pessoais além de contadores.
- Janela temporal do acorde padrão 150 ms; valores fora de 10 a 2000 ms são limitados (`JANELA_MIN_MS`, `JANELA_MAX_MS`).
- Um erro dentro de um conjunto descarta as notas já tocadas do conjunto (o aluno recomeça o acorde).
- Registro de eventos da tentativa limitado a `MAX_REGISTRO` (2.000) entradas.

## Status geral

PASS automatizado, offline: 1, 2a, 2b, 3a, 3b, 4a, 4b, 5a, 5b (lógica), 6a, 6b (e 6c, extra). NÃO VERIFICADO ou pendente humano: tela e acessibilidade visual, botão e escuta do replay, teclado real, avaliação pedagógica, e a convenção de grade do compasso (sem andamento próprio do exercício). **A fase não está aceita:** falta a aprovação humana.

## Biblioteca de Lições em arquivo (ADR-0009), 2026-11-23

- Os 10 exercícios saíram do código para `assets/licoes/fundamentos/primeiros-passos.toml`; o progresso antigo (`exN.*`) migra para `primeiros-passos/N` (teste `progresso_v1_migra_para_ids_da_licao_e_v2_volta_igual`).
- 39 Lições em 6 Trilhas (Fundamentos, Escalas, Acordes, Técnica, Ouvido e intervalos, Melodias), todas `situacao = "proposta"`: escritas por IA a partir de fontes públicas (ver `docs/referencias/fontes-licoes-*.md`), **sem revisão humana e sem teste com aluno ou com o REVAS**.
- `licoes::testes::licoes_de_fabrica_sao_todas_validas_e_unicas` lê todas as Lições embutidas e falha se alguma tiver erro de formato, nota fora das 61 teclas ou pré-requisito inexistente. A teoria (notas de escalas e acordes) foi conferida por validadores Python dos agentes, fora da CI: NÃO VERIFICADO por humano.
- Tela: seletores de grupo (Trilha e Lição) e de exercício lado a lado, ficha com roteiro e sequência, atalhos de teclado (testes `atalhos_*`). Captura só em Xvfb; avaliação visual no KDE pendente.
