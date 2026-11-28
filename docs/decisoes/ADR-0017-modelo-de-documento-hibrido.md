# ADR-0017 — Modelo de documento: híbrido (IdNota + faixa de bytes + bit sujo)

**Status:** PROPOSTA com evidência medida (spike do ticket 08 e implementação da E01, tickets 10 a 26, atualizada no gate do ticket 27 em 2026-11-28); aceite humano pendente (a aceitação é do titular) · **Detalha:** [ADR-0015](ADR-0015-edicao-nativa-e-plataforma-musical.md) (referenciada por ela; ver também [ADR-0018](ADR-0018-motor-de-render-egui-bravura.md)) · **Rastreabilidade:** [MATRIZ-RASTREABILIDADE.md](../evolucao-musical/e01/MATRIZ-RASTREABILIDADE.md) · **Insumo:** `docs/evolucao-musical/e00/03-PESQUISA-EDITOR.md` seção 5, abordagem C · **Evidência:** [spike-08-medicao.txt](../evolucao-musical/e01/evidencias/spike-08-medicao.txt)

## Decisão

A hipótese **vale com ajustes**. O documento é o **texto original imutável** mais um modelo com, para cada parte, compasso e nota, a **faixa de bytes de origem** (`Node::range` do roxmltree), um **bit sujo por compasso** e um **`IdNota`** atribuído na leitura. A exportação aplica um patch por compasso sujo sobre o texto original, **uma única vez, de trás para frente**; tudo que não está sujo é copiado byte a byte.

Os ajustes à hipótese da pesquisa (seção 5-C), todos exigidos pela evidência abaixo:

1. **O compasso sujo é regenerado por peças, não do zero.** O compasso (e, dentro dele, a nota e o `<pitch>`) é decomposto em trechos alternados: texto original por faixa e elementos filhos. Editar troca só a folha mexida (`<step>`, `<octave>`, `<duration>`, `<accidental>`); espaços, comentários, instruções de processamento e filhos desconhecidos do mesmo compasso ficam onde estavam. A pesquisa previa que o compasso editado perderia formatação e comentários internos: com peças isso não acontece, e o custo de código é o mesmo.
2. **Elemento novo entra na ordem do esquema** pela regra: imediatamente antes do primeiro filho de posto maior; filhos desconhecidos nunca se movem. Tabela de postos fechada (25 nomes em 22 postos em `<note>`, 3 em `<pitch>`).
3. **O que o modelo não sabe tratar é recusado com erro tipado, nunca aceito errado**: bytes que não são UTF-8, raiz que não é `score-partwise`, entidade de DTD que expande elementos, duração não representável, reescala de `divisions` com `offset`/`figured-bass`.
4. **O arquivo lateral de ids é amarrado ao texto por impressão digital** (tamanho + FNV-1a de 64 bits): tabela gravada para outro texto é recusada.
5. **Mudar `divisions` suja a parte inteira** (limite real da abordagem, ver "O que não passa").

## Como a hipótese foi testada

Código do spike em `crates/piano-tutor-fixtures/src/documento/` (crate só de testes; nenhum crate de produção depende dele). Reutiliza o corpus, `PosCompasso`, `comparar_bytes`, `comparar_semantica` e `verificar_golden` do ticket 07 e `Fracao` do núcleo; não duplica nada. Ciclo TDD registrado: teste primeiro com stubs, vermelho (A1: 3 de 3 falharam; A2: 8 de 10 falharam; os 2 que passavam já eram atendidos pela leitura, que existia desde a fatia A1, e pelo erro que o stub devolvia para entrada inválida), depois verde.

Corpus: 16 fixtures (10 próprias, 1 gerada de 100 compassos, 3 OpenScore, 2 gravadas pelo projeto), 303 compassos, 1804 notas, de 1,3 KB a 290 KB.

## Evidência medida (o que passou)

Máquina de medição: Linux 6.18, `rustc 1.98.0`, build `--release` para tempos. Saída crua em `docs/evolucao-musical/e01/evidencias/spike-08-medicao.txt`.

| Critério | Resultado | Como foi medido |
|---|---|---|
| **A1** abrir e exportar idêntico | **16/16** fixtures idênticas byte a byte | `tests/documento_a1.rs` |
| A1 sem atalho | **16/16**: regenerar TODOS os compassos a partir das peças reproduz o original | `exportar_regenerando_tudo`; sem isso, "idêntico" seria só devolver a cópia do original |
| A1 em variações adversas | **192/192** (12 variações × 16 fixtures) em exportação normal e regenerando tudo | BOM UTF-8, CRLF, CR solto, sem quebra final, espaços no fim, aspas simples, marcas com espaço e quebra (`<note >`, `</note\n>`, `</measure >`), atributo com `>` no valor, comentário e instrução dentro do compasso, CDATA e entidades no texto, DOCTYPE com subconjunto interno e entidade, tabs |
| **A2** alterar altura muda só aquele compasso | **1717/1717** notas afinadas do corpus (exaustivo, `--ignored`, 26,6 s em release) | para cada edição: exatamente 1 compasso difere; texto antes e depois da faixa do compasso é idêntico byte a byte (créditos, partes, identificação, comentários, vizinhos); harness semântico independente aponta o mesmo único compasso; reabrir devolve os mesmos `IdNota` e só a nota editada mudou |
| A2 com duas vozes | **6 fixtures** (`duas-vozes`, 3 OpenScore, `beethoven-ode`, `brilha-brilha`), uma nota de cada voz de compassos com `<backup>` | `a2_compasso_com_duas_vozes_em_pelo_menos_tres_fixtures` |
| A2 nas variações adversas | passa em 4 fixtures × 12 variações (CRLF, BOM, marcas com espaço, comentários etc.) | `a2_vale_tambem_nas_variantes_adversas` |
| Filhos desconhecidos | sobrevivem: `grouping`, `figured-bass`, `x-extensao-futura` dentro da própria nota editada, `x-elemento-inexistente`, desconhecido entre `step` e `octave`, entre `duration` e `voice` | `elementos_desconhecidos_*`, `desconhecido_dentro_da_propria_nota_*` |
| Ordem do esquema | `<alter>` e `<accidental>` entram no lugar certo, com o recuo do vizinho; trocar e remover restaura o texto exato | `acidente_novo_entra_na_ordem_do_esquema_*` |
| Comentários no compasso editado | todos preservados (contagem igual, texto igual) | `documento_preservacao.rs` |
| Durações exatas | `divisions` = MMC dos denominadores (1/2, 1/3, 3/4 → 12); 1/3 com `divisions=1` é `DuracaoNaoRepresentavel{necessarias: 3}` sem sujar nada; MMC e reescala que estouram são `Overflow` | `tests/documento_duracao.rs` |
| Faixas só valem para o texto original | de trás para frente dá o texto esperado (e de frente para trás dá resultado errado, também testado); `aplicar` consome os patches; cópia aplicada ao texto novo ou a outro texto é recusada (`PatchesDeOutroTexto`); patches sobrepostos, fora do texto ou no meio de caractere são recusados | `tests/documento_patches.rs` |
| IdNota | monotônico a partir de 1, ordem do documento, nunca reutilizado (contador na tabela), estável ao reabrir; id do MusicXML segue a nota mesmo com ordem trocada e id vago não é reusado | `tests/documento_ids.rs` |

**Custo de execução (release, uma execução, indicativo):**

| Fixture | Bytes | Abrir | Exportar sem editar | Editar + exportar | Regenerado na edição |
|---|---:|---:|---:|---:|---:|
| `faure-barcarolle` (maior) | 289 659 | 4,7 ms | 0,85 ms | 0,87 ms | 4 352 B (1,5%) |
| `brahms-wiegenlied` | 125 652 | 1,5 ms | 0,32 ms | 0,32 ms | 1 547 B (1,2%) |
| `cem-compassos` | 54 694 | 0,9 ms | 0,18 ms | 0,18 ms | 707 B (1,3%) |
| `tercina` (1 compasso) | 1 644 | 0,03 ms | 0,00 ms | 0,01 ms | 1 226 B (75%) |

Memória: **NÃO MEDIDA** (o documento guarda o texto original mais a árvore de peças; ordem de grandeza esperada: algumas vezes o tamanho do arquivo, sem medição).

**Conclusão sobre o arquivo lateral de ids (medida):** o corpus tem **0 de 1804 notas com atributo `id`** do MusicXML. Para edições que não mudam a estrutura (altura, duração), reabrir o arquivo exportado devolve os mesmos ids pela ordem do documento (16/16 fixtures): **o arquivo lateral é desnecessário no corpus para essas edições**. Ele passa a ser **necessário assim que houver edição estrutural** (inserir ou remover nota, ticket 17 em diante) e os ids precisarem sobreviver entre sessões. Nesse caso a chave por posição só é segura se gravada junto do texto exportado e amarrada a ele: teste mostra que, sem a amarra, remover a 1ª nota faz a 2ª herdar o `IdNota` da 1ª, erro silencioso. A persistência pelo `id` do MusicXML, quando existir, está implementada e testada com arquivos sintéticos; **que o atributo `id` exista em `<note>` no MusicXML 4.0 continua NÃO VERIFICADO** (sem XSD nesta sessão).

## O que não passa ou é recusado (sem esconder)

| Caso | Situação | Regra proposta |
|---|---|---|
| Mudar `divisions` (duração mais fina que a atual, ex.: 1/3 em `divisions=1`) | **Viola "só o compasso editado"**: todos os compassos da parte são reescritos (2 de 2 na peça mínima de teste). `definir_duracao` recusa; `definir_duracao_ajustando_divisions` faz a reescala atômica | Padrão: recusar e deixar a interface propor. Alternativa **não implementada e NÃO VERIFICADA**: declarar `<divisions>` novo só no compasso editado e re-declarar o antigo no compasso seguinte (toca 2 compassos). Depende de os consumidores aceitarem troca de `divisions` no meio da peça |
| Parte com `<divisions>` declarado mais de uma vez; `offset`/`figured-bass` na parte | Reescala **recusada** (`NaoSuportado`); documento intacto | Tratar se aparecer em corpus real |
| Entidade de DTD que expande elementos (`<!ENTITY n "<note>..">` usada no compasso) | **Recusada** em 16/16 (`FaixaInconsistente`): o roxmltree dá ao elemento expandido uma faixa dentro da declaração, não no uso | Recusa permanente; relaciona-se ao ticket 11 (segurança de XML) |
| Bytes não UTF-8 (UTF-16, Latin-1 com acento) | **Recusados** (`NaoUtf8`). Converter mudaria os bytes | Normalização na importação, produzindo derivado; A1 passa a valer sobre o derivado (decisão do ticket 10/11) |
| `score-timewise` | **Recusado** (`NaoSuportado`) | Idem; fora do escopo da E01 |
| `.mxl` | **Não coberto pelo spike** (só texto). Recompactar o zip não devolve bytes idênticos | O contrato A1 para MXL deve ser sobre o XML interno; ticket 10 define |
| Acidente explícito | `definir_altura` só atualiza `<accidental>` que já existe; não calcula acidente pela armadura nem pelo compasso | **Decidido no ticket 14**: o comando `MudarAltura` do serviço calcula o acidente (armadura e acidente em vigor no compasso) e usa `definir_altura_e_acidente`; regras em `documento/grafia.rs` e no módulo `documento/comando.rs`. `definir_altura` do spike segue como estava |
| `alter` fracionário (microtom) | `altura = None`: a nota não é editável por altura, mas é preservada | 0 ocorrências no corpus |
| `Fracao` do núcleo | **Satura em silêncio** no estouro (`1/i64::MAX + 1/(i64::MAX-1)` dá denominador `i64::MAX`), caracterizado em teste. O spike evita com `i128` e erros explícitos | Tickets de duração (16) não devem confiar em `Fracao + Fracao` perto do limite; corrigir ou envolver o `Fracao` |
| A1 da decomposição | É **sem perda por construção** (trechos contíguos cobrem a faixa inteira); o que de fato pode falhar é a validação de faixas (entidades) e a lógica de edição, e é isso que os testes exercitam | — |

Limites da evidência: o corpus real é **amigável** (UTF-8, LF, sem BOM, sem comentários, sem `id` em nota); as variações adversas são **sintéticas** derivadas dele. Exportações reais de MuseScore, Finale, Sibelius, OMR (Audiveris, HOMR) **não foram testadas**: NÃO VERIFICADO. Nada aqui toca hardware ou áudio.

## Alternativas

| Alternativa | Situação |
|---|---|
| **A. DOM original + overlay de edições** | Descartada por argumento da pesquisa (invariantes musicais dispersos no DOM, undo = diff de DOM). **Não foi prototipada**; não há medição própria |
| **B. Modelo tipado + `extras` por entidade** | Descartada por argumento (ordem dos filhos e referências cruzadas fáceis de corromper, perda silenciosa em campo esquecido). **Não foi prototipada** |
| **C da pesquisa, regenerando o compasso do zero** | Substituída pelo ajuste 1: regenerar por peças preserva espaços, comentários e desconhecidos do compasso sem custo extra de código |
| **C com ajustes (esta ADR)** | Adotada, com a evidência acima |

## Custo de manutenção (medido)

- **Código do spike:** 1356 linhas em 8 arquivos (`texto` 230, `leitura` 265, `exportar` 127, `edicao` 327, `ids` 143, `duracao` 50, `erro` 64, `mod` 150). O núcleo da hipótese (peças, leitura, patches) tem **622 linhas**; edição de altura, acidente e duração (com reescala de `divisions`) são 377. Testes: 42 testes em 7 arquivos, 1262 linhas (inclui variações adversas e medição).
- **Casos especiais no código (13):** marca de abertura com aspas (`>` em atributo); elemento autofechado; fechamento com espaço; faixa de filho fora do pai; UTF-8 apenas; raiz diferente de `score-partwise`; `id` repetido cai para posição; alteração fracionária; inserção em ordem do esquema com tabela de postos; acidente acompanha `alter`; reescala de `divisions` (declaração única, `offset`/`figured-bass` recusados); tabela de ids amarrada ao texto; `Fracao` evitada em favor de `i128`.
- **Custo recorrente esperado:** cada novo comando de edição (duração, inserir, remover, acorde) precisa (a) saber qual folha ou elemento trocar, (b) respeitar a tabela de postos do esquema, (c) atualizar o modelo semântico e (d) ganhar o teste de A2 já parametrizado. O harness de A2 (`editar_e_conferir`) é reaproveitável por comando.

## Riscos remanescentes

1. Corpus real amigável; arquivos reais de outros editores e de OMR podem ter casos não vistos (marque como bug de corpus, não de modelo, quando recusados).
2. Troca de `divisions` mid-part e mid-piece não implementada; custo de mudar `divisions` é a parte inteira.
3. Persistência de `IdNota` sob edição estrutural (ticket 17+) depende de gravar o lateral junto de cada exportação; falta de decisão do titular sobre arquivo lateral versus gravar `id` no XML (sujaria compassos não editados, o que quebraria A1/A2).
4. `Fracao` do núcleo satura em silêncio.
5. Dois tipos `IdNota` hoje: um em `piano-tutor-app/src/geometria.rs` (ticket 06) e outro no spike; ticket 10 precisa unificar no núcleo.
6. Memória e latência medidas só com 1 execução em 1 máquina; sem baseline formal (ticket 26).

## Recomendação ao orquestrador

**Seguir com o ticket 10, sem replanejar S1.** Condições para o ticket 10:

1. Mover `documento/*` para `piano-tutor-core` (sem dependência nova: `roxmltree` já está no núcleo); mover `IdNota` do app para o núcleo e reexportar em `geometria`; trocar `harness::PosCompasso` por um tipo do núcleo (3 usos).
2. Manter as recusas tipadas como parte do contrato do serviço (UTF-8, `score-partwise`, entidades, duração) e documentá-las em linguagem de domínio.
3. Fixar **decisão do titular** sobre o arquivo lateral de ids antes do ticket 17.
4. Planejar `divisions` como caso à parte no ticket 16 (duração), com a alternativa de redeclaração local ou recusa.
5. Reaproveitar `editar_e_conferir` e as variações adversas como testes de A2 por comando nos tickets 14 a 18.

## Como reproduzir

```
cargo test -p piano-tutor-fixtures                                   # tudo, em debug
cargo test --release -p piano-tutor-fixtures --test documento_a2 -- --ignored --nocapture   # 1717 edições
cargo test --release -p piano-tutor-fixtures --test documento_medicao -- --nocapture --test-threads=1
```

## Atualização do gate da E01 (ticket 27, 2026-11-28)

O status **continua PROPOSTA**: esta seção só acrescenta a evidência da implementação (tickets 10 a 26). A decisão acima não mudou. Cada afirmação aponta para a linha da [matriz de rastreabilidade](../evolucao-musical/e01/MATRIZ-RASTREABILIDADE.md); "rodado" significa que o teste passou em `cargo test --workspace` no gate, e "registro" significa que a fonte é um documento versionado, não reexecutado.

### O que a E01 confirmou

| Ponto da decisão | Evidência da E01 | Matriz |
|---|---|---|
| A hipótese vale com os 5 ajustes | O módulo do spike foi promovido ao núcleo (`piano_tutor_core::documento`) e virou o serviço `DocumentoAberto` (ticket 10). A1 passa pelo serviço nas 16 fixtures e também para o XML dentro de `.mxl`; o dono de `IdNota` é o núcleo (risco 5 do spike resolvido). | A1, T10.1 |
| Edição de altura toca só o compasso | Seis comandos de altura (semitom, grau, oitava) pelo comando tipado `MudarAltura`; A2, A3 e A6 passam por comando, em 6 fixtures com duas vozes. O acidente é calculado por `documento/grafia.rs` (armadura e acidente em vigor), como a linha "Acidente explícito" já previa. | A2, A3, A6, T14.* |
| Desfazer por snapshot do compasso | A5 passa em todo o corpus (N desfazeres = original, N refazeres = estado final, byte a byte); histórico com limite de 16 MiB estimado e aviso ao descartar a edição mais antiga (ticket 15). | A5, T15.* |
| Soma da voz é invariante | Duração e inserção/remoção mantêm a soma de cada voz com `Fracao` exata (denominadores 3 e 5 testados); estouro é recusado sem entrada no undo (tickets 16 e 17). | A4, T16.*, T17.* |
| Acorde e pausa | Inserir, remover, acrescentar e retirar altura passam no corpus inteiro e desfazem byte a byte (tickets 17 e 18). | T17.*, T18.* |
| Recusa tipada, nunca aceitação errada | DTD continua ligado porque o corpus tem `DOCTYPE`, mas entidade externa nunca é resolvida; o ticket 11 achou e corrigiu uma explosão quadrática de entidades (`xml_seguro::verificar_entidades`) e adicionou fuzz (1,88 milhão de mutações em 120 s, 0 falhas). | A13, T11.* |

### Decisões tomadas durante a E01 que mexem nesta ADR

1. **Arquivo lateral de ids: adotado** (ticket 17 e 17b), por decisão do supervisor, a confirmar pelo titular. O risco 3 do spike era "falta de decisão"; a decisão foi gravar `<arquivo>.ids` ao lado do texto exportado, `objetos/<hash>.ids` por Versão e `rascunho.ids` no autosave, sempre amarrado ao texto por impressão digital (o núcleo ganhou `serie`, a linhagem das identidades). Tabela ausente, estragada ou de outro texto é recusada com aviso e o documento abre sem ela. Gravar `id` no XML foi descartado (sujaria compassos não editados e quebraria A1 e A2). Para Versões irmãs, a linha opcional `derivada-de=<hash>` evita parear ids que descendem do mesmo ancestral.
2. **`divisions` não muda** (ticket 16, decisão do supervisor): uma duração fora da grade atual é recusada com `DuracaoExigeMudarDivisions`, dizendo o que seria preciso. A alternativa "declarar `<divisions>` novo só no compasso editado" segue **não implementada e NÃO VERIFICADA**. Consequência medida: a peça de 100 compassos do ticket 26 teve de ser adaptada (`divisions` 1 para 4) para permitir colcheia e ponto.
3. **A exibição lê o texto atual**, não o original (ticket 17): depois de inserir ou remover notas, os ordinais do texto original deixam de valer.
4. **`conferir_exportacao`** voltou a conferir as identidades por inteiro depois de edição que cria ou tira notas (o ticket 16 a reduzira à contagem de notas; restaurada no 17).

### Custo medido na peça de 100 compassos (ticket 26, release, VM de nuvem ocupada)

Mediana, em ms: abrir MusicXML 1,94; abrir `.mxl` 2,01; exportar 0,13; exportar e conferir 2,40; altura 0,005; duração 0,02; inserir/remover 0,016; acorde 0,02; desfazer 0,0005. Nenhum comando passou de 100 ms (pior máximo isolado: 16,1 ms, diff entre Versões). Isso é **observação desta VM**: a meta de 100 ms continua "a calibrar" e o PC de referência é NÃO VERIFICADO. Fonte: `docs/evolucao-musical/e01/desempenho/RELATORIO.md`. A **memória do documento continua NÃO MEDIDA** (só o limite estimado do histórico).

### Riscos remanescentes, depois da E01

| # | Risco do spike | Situação |
|---|---|---|
| 1 | Corpus real amigável | **Aberto.** Nenhuma exportação real de MuseScore, Finale, Sibelius, Audiveris ou HOMR foi testada: NÃO VERIFICADO. |
| 2 | Trocar `divisions` | **Aberto**, decidido como recusa; alternativa não implementada. |
| 3 | Persistência de `IdNota` sob edição estrutural | **Resolvido pela decisão 1**, com ressalva: `*.ids` é um segundo arquivo que o usuário pode separar do texto; nesse caso o documento abre pela ordem do documento e o diff cai no conteúdo. A confirmação do titular está pendente. |
| 4 | `Fracao` do núcleo satura em silêncio | **Aberto**: o teste de caracterização `fracao_do_nucleo_satura_em_silencio_no_estouro` continua existindo; os comandos usam `i128` e erros explícitos. |
| 5 | Dois tipos `IdNota` | **Resolvido** no ticket 10 (um tipo no núcleo). |
| 6 | Memória e latência de uma execução | **Parcial**: baseline do ticket 26 (latência, uma VM); memória NÃO MEDIDA. |
| 7 (novo) | Estado do `.ids` e do rascunho só testado em Linux | Trava de arquivo (`File::lock`) testada só com threads no Linux; dois processos reais, Windows e macOS: NÃO VERIFICADO. |

### O que continua NÃO VERIFICADO (não convertido em PASS por inferência)

Som real da nota editada; prática com o REVAS físico e latências; estabilidade prolongada; janela real (aparência, GPU, HiDPI); Windows e macOS; leitor de tela. A lista completa, com o que fecha cada um, está na seção "NÃO VERIFICADO" da matriz.
