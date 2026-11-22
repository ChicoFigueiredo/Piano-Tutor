# F07: evidência de aceite (partitura somente leitura, ADR-0007)

Data: 2026-11-22. Escopo entregue: ler MusicXML `score-partwise` não comprimido, diagramar em duas ou mais pautas (sol/fá), tocar pelo motor de som com cursor, exportar o MusicXML carregado. Edição de notas fica fora (ADR-0007). O spike de notação egui × Verovio previsto na ADR-0007 **não foi feito**: aqui só existe a opção A (layout próprio em egui, sem fonte musical).

Comandos: `cargo test --release --workspace` (passa), `cargo clippy --release -p piano-tutor-app -p piano-tutor-core --all-targets` (sem avisos nos arquivos da F07), `rustfmt --edition 2024 --check` nos três arquivos.

## Arquivos de teste (licença em `assets/partituras/LEIA-ME.md`)

| Arquivo | Fôrma | Armadura | Pautas | Observações do conteúdo |
| --- | --- | --- | --- | --- |
| `brahms-wiegenlied-op49-4.musicxml` | 3/4 | 3 bemóis | voz + piano (sol, fá) | anacruse, ligaduras de valor e de expressão, repetição da peça, vozes múltiplas no piano |
| `schubert-heidenroeslein-d257.musicxml` | 2/4 | 1 sustenido | voz (sol 8va) + piano | acordes, notas de adorno, pausas, repetição |
| `faure-barcarolle-op7-3.musicxml` | 6/8 | 4 bemóis | voz + piano | tercinas, notas de adorno, polifonia, acordes com pontos |

Fonte: OpenScore Lieder Corpus, CC0 1.0 (domínio público declarado em cada arquivo e no repositório).

## Critérios do aceite F7

| Critério | Estado | Evidência |
| --- | --- | --- |
| Abrir ao menos 5 arquivos licenciados, incluindo `.mxl` com duas pautas, polifonia simples, 3/4, 4/4 e 6/8; compasso e nota corretos por inspeção humana | **PARCIAL / NÃO VERIFICADO** | Só 3 arquivos (3/4, 2/4, 6/8) e todos `.xml` sem compressão: **não há leitura de `.mxl`** (a tela recusa `.mxl` com mensagem; faltaria `zip`, dependência não autorizada nesta entrega). **Sem 4/4.** Testes automáticos (core): `arquivos_empacotados_tem_tres_pautas_e_formas_esperadas`, `wiegenlied_anacruse_ligadura_repeticao_e_andamento`, `heidenroeslein_primeiro_compasso_e_tempo`, `barcarolle_tem_tercinas_e_seis_oitavos_com_pulso_composto`, `execucao_dos_arquivos_empacotados_e_coerente`. A conferência humana compasso a compasso **não foi feita** |
| Exportar MusicXML e reimportar sem mudança de notas, pausas, duração escrita ou compasso; campos não suportados preservados ou alertados, nunca eliminados em silêncio | **PASS offline** | `exporta_sem_perda_e_nao_sobrescreve_sem_pedir` (tela): o arquivo exportado é idêntico byte a byte ao embutido e a releitura produz um `Partitura` igual ao carregado. Como o texto original é gravado sem reescrita, nada não suportado (letra, dinâmica, pedal, layout) é perdido. Não sobrescreve sem marcar "Sobrescrever" |
| Pauta em claves sol e fá, nota-alvo e cursor legíveis a 100% e 150% | **PASS offline (estrutura); legibilidade NÃO VERIFICADA por humano** | Testes do componente: posição de nota/linha suplementar/haste (3,5 sp)/viga (0,5 sp, ≤ 1 sp de desnível)/armadura em sol e fá, quebra apenas em compasso inteiro, `desenha_cada_partitura_a_100_e_150_por_cento_sem_panico`, `destaque_pinta_as_notas_que_soam_com_a_cor_da_mao`, `mao_esquerda_usa_a_cor_azul_na_clave_de_fa`. Inspeção visual feita só por rasterização externa dos triângulos do egui (teste `despeja_quadros_para_inspecao_visual`, ativado por `PT_PARTITURA_DUMP`), **não** em janela real nem em monitor |
| Comparação MIDI↔MusicXML distingue evento esperado de representação visual, com tolerância de tempo configurável | **NÃO IMPLEMENTADO** | Existe só a lista de eventos esperados (`Partitura::eventos_de_reproducao`, tempo exato em fração de semínima, ligaduras fundidas, repetições expandidas, separada da geometria). O comparador com tolerância é trabalho da F13 |
| `.mxl`/XML maliciosos ou zip-bomb rejeitados por tamanho, número de entradas e profundidade, sem leitura arbitrária do disco | **PASS para XML; NÃO VERIFICADO para zip** | `recusa_arquivos_invalidos_sem_panico` (limite de 8 MiB, profundidade de 5000 níveis e bomba de entidades recusadas), `entidade_externa_nao_le_o_disco`, limites de nós (2 milhões), notas (200 mil), compassos e partes no `partitura.rs`; a tela confere o tamanho antes de ler (`abrir_caminho_valida_extensao_tamanho_e_conteudo`). Zip/`.mxl`: não há leitor, portanto não há o que testar |
| `.mid`, `.pdf` e `.mxl` da música Golden | **NÃO VERIFICADO** | O usuário não disponibilizou as cópias |

## Entregas adicionais cobertas por teste

- Reprodução: `plano_respeita_ordem_tempo_e_andamento` (ordem e tempos exatos a 60 e 120 bpm, início em compasso escolhido), `atualizar_agenda_com_antecedencia_e_toca_na_ordem` (150 ms de antecedência como no ritmo; o motor de teste recebe as notas 60, 64, 67, 72), `cursor_acompanha_a_reproducao_e_parar_limpa` (inclui mudar o andamento tocando), `comecar_do_compasso_escolhido`.
- Repetições com casas 1ª/2ª: `repeticao_com_casas_expande_a_execucao`.
- Mapa de andamento: `mapa_de_andamento_converte_para_segundos`.
- Pasta de assets: `pasta_de_assets_acrescenta_partituras_novas`.

## Limitações conhecidas e riscos

- Não desenhados: letra, dinâmicas, cunhas, pedal, articulações, fermatas, arpejos, ornamentos, texto de direção, grupos de pausas de vários compassos, cifras, dedilhado. Lidos mas ignorados no modelo: `transpose`, `octave-shift`. Notas de adorno aparecem mas não tocam.
- Ligaduras de valor e de expressão são arcos simples (sem afilamento, sem desvio de colisões); ligadura entre sistemas vira meio arco. Sem vigas entre pautas, sem resolução de colisão entre vozes na mesma pauta além de deslocar pausas e cabeças em segunda.
- Glifos de claves, acidentes, pausas e chave são aproximações próprias por curvas; podem ser trocados por SMuFL/Bravura (OFL) numa evolução, sem mudar o modelo.
- Desempenho e consumo de CPU **não foram medidos** em janela real; o layout é recalculado só quando mudam largura, zoom ou partitura. Em larguras muito estreitas a diagramação pode exceder a janela (rolagem horizontal).
- Tempo de áudio depende do motor F03; precisão fina e latência do cursor em relação ao som **não medidas**.
- Dependência nova: `roxmltree` 0.21.2 (MIT OR Apache-2.0), ver `docs/seguranca/LICENCAS-E-DADOS.md`.
