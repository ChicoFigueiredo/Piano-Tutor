# E00 / 02 — Pesquisa independente de OMR e extração vetorial de PDF

Acesso: 2026-11-27. Autor: agente E00 (OMR). Convenção: **V** = verificado em fonte primária nesta sessão; **A** = auto-relatado pelo autor (não replicado); **NV** = NÃO VERIFICADO. Nada aqui foi testado em hardware do titular nem no REVAS. Memória de treino não foi usada como fato.

Limitações da coleta: `gh api` bloqueado para repositórios de terceiros; datas/SHAs de commit, contagens de issues e licenças de pesos foram lidas das páginas HTML (sem ano em alguns releases) e o orçamento de WebFetch esgotou antes de ler o Sapp 2017 e o Handbook do Audiveris. Onde faltou, está NV.

## 1. Resumo das decisões que mudam o rumo

1. **Não existe número publicado de precisão para piano polifônico com letra + dedilhado em nenhum motor.** Todas as métricas abaixo são OMR-NED/SER em corpora diferentes. Metas de F1 >= 97-99% da spec são **NÃO ATINGIDAS por nenhuma evidência publicada**; só o benchmark local dirá.
2. **Fusão multi-motor por votação: ganho publicado é modesto e piora com acordes** (Bugge et al., ISMIR 2011, V): combinado não superou significativamente os dois melhores motores; autores recomendam correção humana sempre. Fusão como *estratégia primária* é **NÃO CONFIRMADA**; como *sinalizador de discordância* é útil, mas concordância != precisão.
3. **Extração vetorial nativa é a única rota com potencial de precisão próxima de 100% em PDF de editor** (PDFtoMusic Pro afirma só processar PDF gerado por editor de partitura, V). Mas: o PDF **precisa ter fontes musicais como texto**. PDF gerado por verovio+cairosvg (meu PoC) tem glifos como *paths*, não texto (V, `pdffonts`) — extração por fonte é impossível ali. Isto precisa ser classificado por PDF antes de rotear.
4. **Audiveris 5.11.0** roda offline em CLI (PoC V), lê 20/20 notas e alturas de um piano sintético de 2 pautas, mas **0/14 letras e 0/14 dedilhados** recuperados no mesmo teste (com `eng` instalado). Confirma o relato anterior do repositório quanto a dedilhado; letra precisa de investigação (NV a causa).
5. **Candidato novo mais promissor (A, não replicado): `copisteria`/copista-28m** (Apache-2.0 código e pesos, CPU ~15 s/página, MusicXML, OMR-NED 11,7% em 735 páginas de teste do autor). Também **Legato** (MIT código; GPU CUDA testado; saída ABC, não MusicXML nativo) e **Transcoda** (NeurIPS 2026, licença NV). Todos exigem replicação local antes de qualquer decisão.
6. **LLM multimodal não é motor de transcrição confiável**: literatura mostra OMR-NED alto (ver §5). Útil só em recortes para triagem/rótulos, com saída sempre tratada como Hipótese.

## 2. Candidatos: fichas

### 2.1 Audiveris (open source, raster)
| Item | Evidência |
|---|---|
| Versão atual | 5.11.0 (release "11 Jul", ano não exibido; anteriores 5.10.2 "27 Mar", 5.10.1 "21 Mar") V https://github.com/Audiveris/audiveris/releases ; commit 9e1e55cd2746037d059345881c53e6a6754bffbd (saída de `-version`, V PoC) |
| Licença código | AGPL-3.0 V (página do repo) |
| Dados/pesos | Classificador de glifos embutido no repositório; licença dos dados de treino NV |
| Manutenção | 6.032 commits; 214 issues abertas (V, HTML); cadência declarada "a cada 6 a 12 meses" |
| Plataformas | Instaladores: Windows MSI, Linux `.deb` **x86_64** (Ubuntu 22.04/24.04), Flatpak, macOS **arm64 e x86_64** `.dmg` (V, lista de assets). Linux ARM: NV (build de fontes). JRE embutido; Java 25 no PoC (V) |
| CPU/GPU | CPU apenas. PoC: 6,6 s (parede) numa pauta dupla pequena, JVM fria |
| CLI | `-batch -transcribe -export -output DIR`, `-sheets`, `-constant K=V`, `@arquivo`; sem flags de OCR no CLI (V, docs). Paralelismo não documentado nessa página |
| OCR | Tesseract 5.5.2 (V `-version`). Desde 5.5 **nenhum idioma vem instalado**; sem idioma o passo TEXTS é ignorado (V docs). Idioma padrão `eng`; combinação `por+eng` por constante/parâmetro de livro (NV se `por` consta na lista) |
| Saída | MusicXML 4.0 (subconjunto) `.mxl` V |
| Piano 2 pautas | PoC: 2 partes, 20/20 notas com alturas corretas V (sintético limpo, n=1) |
| Letra | Docs não discutem; PoC 0/14 (V) |
| Dedilhado | 0/14 no PoC V; relato anterior no repo: "não lê" (consistente) |
| Precisão publicada | Não encontrei benchmark do próprio Audiveris em dataset público nesta sessão: NV. Benchmarks de terceiros (notes2tone) citados no homr, sem números |

### 2.2 HOMR (liebharc/homr)
| Item | Evidência |
|---|---|
| Licença | AGPL-3.0 V; licença dos pesos ONNX **NV** (não declarada na página) |
| Manutenção | 447 commits, 9 issues abertas V; releases-armazém de ONNX "18 Aug", benchmark "18 Jun" (ano não exibido). Sem tag de versão semântica visível |
| Instalação | Python 3.11/3.12; `uvx --from 'homr[cpu]' homr <img>`; CPU; GPU opcional CUDA 12.1/ROCm 7.0 V |
| Escopo declarado | "foca em altura e ritmo em clave de sol ou fá"; ignora dinâmica, articulação, dobrados; polifonia e letra **não mencionadas** V |
| Saída | MusicXML V |
| Precisão (A) | Benchmark do próprio repo (OMR-NED, menor é melhor): polish-scores 14,6 nativo / 23,0 musicdiff; SMB 13,0 / 13,8; sobre 10 amostras SMB homr 24,2 vs oemer 38,0 (nativo) e 16,0 vs 77,3 (musicdiff). Autor admite viés (parsers próprios; barras de ligação não escritas) V. Audiveris **não está** na tabela V |

### 2.3 oemer
| Item | Evidência |
|---|---|
| Licença | MIT código V; pesos/datasets (CvcMuscima-Distortions, DeepScores-extended) **licença não declarada** |
| Manutenção | v0.1.8 "16 Nov" (ano não exibido), 168 commits, 13 issues V |
| Execução | onnxruntime padrão, TF opcional; GPU 3-5 min/página, CPU sem tempo declarado; ~62 s/página no paper de LLM (V, 2506.23009) |
| Observações | só notação ocidental impressa; falhas com inclinação |
| Precisão | Ver benchmarks do homr acima (A) e oemer usado como baseline em paper de LLM |

### 2.4 Modelos neurais ponta a ponta (pesquisa)
| Modelo | Evidência |
|---|---|
| SMT / SMT++ (Ríos-Vila et al.) | Código `fp-smt` MIT V, **repositório "não mais mantido"** V; licença dos pesos NV; datasets exigem formulário. No SMB (685 págs) baseline pianoform OMR-NED 57-77% (V, 2506.10488) — ruim. No benchmark do homr, SMT 75,6-78 (A) |
| Legato (ICLR 2026, arXiv 2506.19065) | Código MIT V (`guang-yng/legato`), pesos HF `guangyangmusic/legato` (licença NV); Llama-3.2-11B-Vision congelado + decoder 101M (**licença do Llama NV**); saída **ABC** (MusicXML exige MuseScore GUI no pipeline deles) V; CUDA 12.4. IMSLP piano 32 págs: OMR-NED 44,3 vs SMT++ 91,9 (A); TEDn 29,7 vs 97,7 |
| Legato 2 (arXiv 2607.05769) | segmenta sistemas (YOLOv8) + VLM; OMR-NED 17-44 em seus conjuntos; **licença/pesos "serão liberados" (NV)**; avalia Gemini 3.1 Pro (89-94 NED) — **não avalia Claude** V |
| Transcoda (NeurIPS 2026; arXiv 2605.10835) | 59M parâmetros, saída **kern, grammar-constrained; sintético: OMR-NED 18,46 vs Legato 43,91; escaneados poloneses 63,97 vs SMT++ 80,16 (A). Licença/pesos NV |
| copisteria / copista-28m / copista-2m (`kobimusic`) | **Apache-2.0 código e pesos** V (HF); CPU ~15 s/pág (28m), ~5 s (2m), Python >= 3.12, PDF exige Poppler, saída `.musicxml` + viewer HTML + JSON por símbolo V. OMR-NED (autor, A): quartetos 8,7/5,8, Lieder 17,7/14,6, piano polonês escaneado 34,4; média 11,7% em 735 págs. Mantenedor desconhecido; data de commit NV; benchmark do autor, não replicado |
| TrOMR / PrIMuS / OLiMPiC / LMX | Não verifiquei nesta sessão (orçamento): NV. Hipótese a checar |

### 2.5 Comerciais (marcar; PDFs de terceiros não devem ir a serviços)
| Produto | Evidência |
|---|---|
| **PDFtoMusic Pro** (Myriad) | **Só PDF gerado por editor** ("scanned sheet music cannot be managed"); trial: toca/exporta **só a 1ª página**, sem limite de tempo; Windows, macOS (10.7+), **Linux (testado Ubuntu 18.04)** V. Licença vitalícia por plataforma, atualizações incluídas; **preço não exibido (NV)**. **CLI existe** (`p2mp arquivo.pdf`; `-format XML/MXL/...`, `-multivoices`, `-chords`, `-lyrics`, `-firstpage/-lastpage`, `-matchlevel`; arquivo de status `C2F_<id>.txt` com códigos 3="não extrai codificação de texto" e 5="sem dados musicais") V. **Restrição de uso do manual: modo escravo é "pessoal"; uso automatizado em benefício de terceiro é proibido** V — impede embutir no app distribuído; uso apenas como benchmark local. Arquitetura x86 vs ARM: NV |
| SmartScore 64 Pro (Musitek) | US$ 399; Windows e macOS 10.13+; letras, cifras, MusicXML V (página do fabricante, afirmações de marketing) |
| ScanScore | US$ 9/39/79 por ano (Melody/Ensemble/Professional); Win/macOS + apps móveis; notas, letras, cifras, MusicXML; lote NV V |
| PlayScore 2 | iOS/iPadOS/macOS(M1+); Professional US$ 6,99/mês ou 59,99/ano exporta MusicXML V |
| Soundslice | Plus US$ 5/mês (100 págs/mês); nuvem V; upload de PDF de terceiros a serviço é vedado pelas regras E00 (material do titular) |
| MuseScore OMR / SheetMagic, Newzik etc. | Não verificados: NV |

## 3. Extração vetorial de PDF (Poppler, mutool, etc.)

| Pergunta | Resposta com evidência |
|---|---|
| Poppler (24.02.0 local, V) dá glifos/bbox? | `pdftotext -bbox-layout` e `-layout` já foram usados no repo anterior com PDFs Finale/Maestro (relato interno `CONVERSAO-DE-PARTITURAS.md`; não reverifiquei). `pdffonts` lista fontes e se estão embutidas (V) |
| E linhas de pauta, hastes, ligaduras? | São *paths* de desenho, não texto. Para obtê-los é preciso um parser de conteúdo de PDF (operadores `re/l/c`). `pdftocairo -svg` os exporta como geometria, mutool `draw -F stext/svg` também: **NV, não testei** (mutool ausente no ambiente; MuPDF é AGPL) |
| Dá para recuperar notas de PDF de MuseScore/Finale/Sibelius/LilyPond? | **Parcialmente plausível, NV por teste meu.** PDFtoMusic existe como produto para isso e depende de a fonte musical estar embutida e com codificação extraível (códigos de erro 3 e 5 do CLI confirmam as falhas típicas) V. Verovio→cairosvg, ao contrário, produz paths sem fonte (V, `pdffonts` do PoC listou só LiberationSerif/Inter) |
| Trabalhos publicados | Encontrei apenas (a) `pdf_oxide` (detecta regiões musicais por fonte+5 linhas finas; **não extrai notas**) V; (b) Sapp 2017 "Graphic to Symbolic" (zenodo 923829): título confirmado, **conteúdo não lido (NV)**; (c) nenhum paper de extração por glifo comparado a OMR em piano. Lacuna de literatura registrada |
| SMuFL | PDFs com fontes SMuFL (Bravura, Leland, Emmentaler) têm nomes de glifo/códigos estáveis; Maestro/Finale/Petrucci usam mapeamento próprio; fontes subsetadas podem perder `ToUnicode`. NV por teste; hipótese para a PoC |

Conclusão: vetor "puro" (glifo+coordenada) só vale em PDFs de editor com fonte embutida e mapeamento conhecido; custo é escrever o interpretador de layout (clave, armadura, vozes, ligaduras como curvas). **Recomendo medir primeiro com PDFtoMusic Pro (trial 1ª página, depois licença) como oráculo de viabilidade**, não como componente.

## 4. Arquitetura: fusão multi-motor

### O que a literatura mostra
| Fonte | Achado |
|---|---|
| Bugge, Juncher, Mathiasen, Simonsen, ISMIR 2011 (V) | Alinhamento múltiplo + voto por maioria sobre 4 motores comerciais (49 páginas): ranking médio combinado 2,43 vs SharpEye 2,83 / SmartScore 2,86. **Significativo só contra Capella (p<=0,001) e PhotoScore (p=0,049); não contra os dois melhores.** Pior com acordes (corpus A). "Melhorias substanciais podem exigir métodos mais elaborados"; correção humana quase sempre necessária |
| Byrd & Simonsen 2015 (testbed) (V) | Erro por símbolo engana (exemplo: 19/20 símbolos baixos certos = 95%, mas 1/9 de alto nível = 11%); clave errada propaga dezenas de erros; recomenda contar esforço de correção. Não traz resultados de múltiplos reconhecedores |
| Byrd & Schindele 2006/2007 "Prospects for Improving OMR with Multiple Recognizers" | Citado, **não lido (NV)** |
| Fusão neural + clássico, 2024-2026 | Não encontrei estudo (NV). Hipótese de pesquisa |

### "Não confundir concordância com precisão" — CONFIRMADO como princípio
Motores baseados na mesma família (detecção de linhas, mesmo erro de clave/armadura/oitava) cometem **erros correlacionados**: dois motores concordando num dó errado é concordância sem precisão. Concordância serve para **triagem** (discordância = onde olhar), não para atestar. O PoC mostra: neste fixture Audiveris detectou um "ottava alta" espúrio em 66 vs 53 símbolos (concordância com a verdade cai, mas nenhum segundo motor foi comparado — não generalizar).

### Estratégia primária/secundária proposta (hipótese a testar, não decisão)
1. **Roteamento por PDF** (barato): `pdffonts` + presença de texto musical embutido + linhas vetoriais => "vetor"; imagem raster => "scan".
2. **Vetor limpo:** primária = extração nativa (própria ou PDFtoMusic só como oráculo local); secundária = OMR raster do mesmo PDF rasterizado só para discordância. Valor da vetorial é maior em: armadura/fórmula/ritmo exatos, dedilhado e cifras (texto real), letras (texto real, sem OCR).
3. **Scan:** primária = melhor motor neural/clássico na calibração (candidatos copisteria, Audiveris, homr); secundária = outro motor de família diferente para discordância; Tesseract/idioma `por` para texto.
4. **LLM opcional:** só em recortes marcados por discordância, saída como sugestão rotulada, nunca sobrescreve.
5. Toda saída entra como Hipótese; cada compasso recebe confiança (discordância, soma de durações por compasso, validação MusicXML).
6. Critério de parada: se a fusão não melhorar F1 de evento >= 1 ponto percentual sobre o melhor motor isolado com IC 95%, adotar um motor + marcação de baixa confiança.

## 5. LLM multimodal: evidência publicada
| Estudo | Resultado (V = lido) |
|---|---|
| arXiv 2506.23009 (Phi-3-MusiX) V | GPT-4o zero-shot: acerto exato em OMR (armadura, altura, duração) **4,0%**, +RAG+oemer 8,4%; fine-tune com kern+ 68,4%; dataset **sintético**; "GPT-4o imita o formato" |
| arXiv 2611.17870 OmniScore-Bench V (nomes de modelos como no artigo; não reconheço todos) | Transcrição de uma página, corpus NED: melhor 26,8%, Opus 5.5 43,5%; localização de símbolos F1 de 7,9% a 91%; Claude varia muito por tarefa. 3.000 perguntas; licença do dataset NV |
| Legato 2 V | Gemini 3.1 Pro OMR-NED 89-94 vs 17-44 do modelo especializado |
| Blog Songscription | Vendor; sem citação: **não usar como evidência** |

Conclusão: LLM generalista **não** substitui OMR; adequado a auxiliar perguntas visíveis (título, tempo, letras legíveis) com validação. Nenhuma medição em piano com Claude em partituras comerciais encontrada.

## 6. Matriz final

Esforço: B/M/A (baixo/médio/alto) em horas de integração estimadas pelo agente (não medidas). Precisão = relatada, em dataset **diferente do domínio** do Piano Tutor.

| Opção | Evidência | Licença (código / pesos) | Precisão relatada | Recursos | Esforço | Risco | Manutenção | Recomendação |
|---|---|---|---|---|---|---|---|---|
| Audiveris 5.11.0 | PoC V; docs V | AGPL-3.0 / n.d. | sem benchmark próprio; PoC 20/20 notas, 0 letras, 0 dedos (n=1) | CLI, offline, 2 pautas, OCR Tesseract, MusicXML | B | Médio (AGPL: só processo separado; decisão jurídica do titular) | Ativa, releases 6-12 m, 214 issues | **Baseline obrigatório do benchmark**; candidato primário raster |
| HOMR | docs V; bench A | AGPL-3.0 / NV | OMR-NED 13-23 (A) | CPU; MusicXML; sem letra/polifonia declaradas | B | Médio-alto (pesos NV) | Ativa, 447 commits, 9 issues | Benchmarkar; não primário até licença dos pesos |
| oemer | docs V | MIT / n.d. | NED 38-77 (A, 10 amostras) | CPU lento | B | Alto (precisão baixa; releases espaçadas) | Baixa-média | Rejeitar como primário; manter só como baseline |
| copisteria | HF V; métricas A | Apache-2.0 / Apache-2.0 | 11,7% geral, piano escaneado 34% (A) | CPU, MusicXML, Python 3.12 | M | Alto (autor desconhecido, não replicado) | NV | **Replicar na PoC; candidato mais barato de licença** |
| Legato (1/2) | paper V | MIT / HF n.d. (+Llama NV) | piano IMSLP 44,3 (A) | GPU, ABC | A | Alto (GPU, licença Llama, ABC->MusicXML) | Pesquisa | Só estudo; GPU fora do alvo |
| SMT/SMT++ | paper V | MIT / n.d. | pianoform 57-77 (V) | GPU recomendada | A | Alto | Repo sem manutenção | Rejeitar |
| Transcoda | paper V | NV | 63,97 em escaneados (A) | NV | NV | Alto | NV | Acompanhar; verificar licença |
| Extração vetorial própria | Poppler V; viabilidade NV | nossa | desconhecida | texto/dedos/letras exatos | A | Médio (depende de fontes) | nossa | **Spike de 2 dias** em PDFs de editor, depois decidir |
| PDFtoMusic Pro | docs V | proprietário (lic. pessoal, uso automatizado proibido) | NV | CLI, Linux/macOS/Win, MusicXML, vozes, letras, cifras | B | Alto (proibição de uso automatizado; preço NV) | NV | **Oráculo local de benchmark apenas** |
| SmartScore/ScanScore/PlayScore | sites V | proprietário | NV | letras/cifras, MusicXML | B | Alto (GUI, sem lote verificado) | NV | Opcional, baseline manual de 5 páginas |
| LLM multimodal | papers V | API | NED 27-94 (V) | qualquer idioma | B | Alto (alucina; privacidade) | n/a | Complementar, recortes autorizados |
| Fusão por voto | Bugge 2011 V | nossa | ganho não significativo vs melhor motor | marca discordância | M | Médio | nossa | Usar como detector de discordância; não como "verdade" |

## 7. Benchmark reproduzível

### 7.1 Ferramentas de avaliação
| Ferramenta | Licença / versão | Papel |
|---|---|---|
| **musicdiff** 6.0 (PyPI, 22/09/2026) | MIT V | Diferença estrutural MusicXML/kern; `-o omrned` produz OMR-NED; `-i/-x` seleciona categorias; modo `--ml_training_evaluation` em lote. Requer Python >= 3.12, music21 >= 10.5 V. **Rodei no PoC (§8)** |
| **OMR-NED** | Definição: (I+D)/(N1+N2) sobre símbolos, com quebra por categoria V (arXiv 2506.10488) | Implementado no musicdiff (V no PoC) |
| **SMB** (Sheet Music Benchmark) | 685 páginas, kern; **licença NV**; HF `PRAIG/SMB` | Referência externa de comparação |
| TEDn | Implementado no repo Legato (MIT) V; não li o código | Alternativa; só se comparar com Legato |
| mir_eval | NV nesta sessão | Útil só para notas-evento em MIDI (F1 onset/pitch); **não substitui** métrica de partitura |
| Métrica própria (spec) | a escrever | F1 por evento alinhado (pauta/voz, altura, onset, duração) |

### 7.2 Protocolo
1. **Gabarito:** MusicXML conhecido (fixture gerada) ou transcrição humana revisada por 2 pessoas; versão + SHA-256. Consenso de motores **nunca** é gabarito.
2. **Normalização antes de comparar:** partwise, 1 parte "Piano" 2 pautas, expandir repetições conforme decisão do titular, remover `<print>`/layout, converter oitava (convenção Dó3=C4 do curso Luciano), `makeNotation` consistente.
3. **Alinhamento:** por compasso (contagem + soma de durações), depois por evento (pauta, voz, onset em quarto de nota). Tolerância de onset 0 (exato) e 1/16 para relatório secundário. Define-se antes da avaliação e congela.
4. **Métricas:** P/R/F1 de (pitch), (pitch+onset), (pitch+onset+duração) por evento; taxa de compasso perfeito; erro de símbolos críticos (armadura, fórmula, repetições, ligaduras, andamento); OMR-NED total e por categoria (musicdiff); WER/CER de letras e acerto sílaba-nota; cifras e dedos presentes/corretos; falha = página sem saída (conta no denominador); tempo p50/p95/página, RAM de pico (`/usr/bin/time -v`), minutos de correção humana por página.
5. **Estratos:** vetor limpo (20), scan bom (20), scan difícil (20), alinhado a `docs/evolucao-musical/e00/05-CORPUS.md`; reportar média, IC 95% por bootstrap (seed fixo) e por estrato/motor; separar calibração e cego por obra/edição.
6. **Ablação:** cada motor isolado; vetor; fusão; OCR texto; LLM opcional.
7. **Reprodutibilidade:** `requirements.txt` com versões, SHA do .deb (`f20113aa...a3`), seeds, scripts em repositório sem conteúdo protegido.

### 7.3 Fixtures públicas propostas (3 a 5)
| # | Fixture | Fonte e licença | Para quê |
|---|---|---|---|
| F1 | Melodia tradicional 2 pautas com letra e dedos | **Gerada por script** (domínio público: melodia tradicional) — usada no PoC | Pipeline completo, controle total do gabarito |
| F2 | Minueto em Sol (Anh. 114, ex-Bach) | Domínio público; escolher edição MusicXML com licença verificada (Mutopia/OpenScore/IMSLP): **NV por peça** | Piano duas claves, vozes |
| F3 | Peça de Burgmüller op.100 ou Für Elise | Domínio público, MusicXML a verificar licença por arquivo | Acordes, tuplets, repetições |
| F4 | Hino/canção com letra PT | Texto/música em domínio público; transcrição CC0 se existir: NV | Letra PT, cifras |
| F5 | Variante degradada de F1-F3 (rotação 1-2 graus, ruído, JPEG 150 dpi), seeds fixos | Derivada | Scan sintético |

Para fixtures vetoriais *com texto musical*: renderizar F2/F3 com **MuseScore e LilyPond** (CLI) e verificar `pdffonts` antes de contar como "vetor". Verovio+cairosvg **não serve** (glifos como path).

## 8. PoC executada (barata, V)

Ambiente: Linux x86_64 Ubuntu 24.04.5, Python 3.13, venv no scratchpad; sem GPU. Tudo local salvo downloads.

```
pip install verovio==6.4.1 music21==10.5.2 cairosvg==2.9.1 musicdiff==6.0   # ~9 s
curl -L .../Audiveris-5.11.0-ubuntu24.04-x86_64.deb ; dpkg -x ... ex/        # sha256 f20113aaa33b3149ec8d6a09b2a7963360e65fafd92d69389987a85bbc3ec7a3 (74 MB, sem instalar no sistema)
curl -L https://raw.githubusercontent.com/tesseract-ocr/tessdata/main/eng.traineddata -o ~/.config/AudiverisLtd/audiveris/tessdata/eng.traineddata
python make_fixture.py ref.musicxml          # F1: 2 pautas, 14 notas MD, letras, dedilhados, acordes MG
verovio -> ref.svg ; cairosvg --scale 3 -> ref300.png (3780x1029)
Audiveris -batch -export -output out2 ref300.png    # 6-8 s
python -m musicdiff [-i ...] -o omrned -- ref.musicxml out2/x/ref300.xml
```
Resultados:
| Medida | Resultado |
|---|---|
| Versão | Audiveris 5.11.0, commit 9e1e55cd..., Java 25.0.3, Tesseract 5.5.2 |
| Notas lidas | 20/20 (partes 2), alturas da mão direita idênticas às 14 do gabarito |
| Letras | 0/14 (gabarito 14) |
| Dedilhado | 0/14 (gabarito 14) |
| OMR-NED só notas/pausas (`-i decoratednotesandrests`) | 0,118 (ED 14; 66 vs 53 símbolos: sobra espúria, ottava detectada) |
| OMR-NED notas+letras | 0,407 |
| OMR-NED tudo | 0,481 |
| Primeira tentativa (imagem 1260x343, `tessdata_fast`) | OCR falhou "legacy mode" — `tessdata_fast` serve só LSTM; usar `tessdata` padrão. Falha de instalação, não de motor |
Limites: n=1, sintético limpo, fonte Leipzig do verovio (não Finale), 1 resolução. **Não generaliza**; é só prova de execução e do comparador.

## 9. Confirmações / refutações da spec e do relato anterior

| Afirmação | Status |
|---|---|
| "Audiveris 5.11" é versão atual | CONFIRMADA (releases, 5.11.0 no topo, 2026-11-27) |
| "Audiveris não lê dedilhado" | CONFIRMADA no PoC (0/14); para solfejo/letra, REFINADA: com `eng` ainda 0/14 neste fixture, causa NV |
| "Audiveris falha em folhas sem fórmula de compasso" | NÃO VERIFICADA (não testei) |
| Spec R-B04: fusão entre Poppler, Audiveris, HOMR | NÃO CONFIRMADA como ganho; literatura mostra ganho modesto |
| Metas F1 >= 97-99% | NÃO VERIFICADA / sem suporte publicado em piano polifônico |
| PDFtoMusic só como benchmark | CONFIRMADA; restrição contratual de uso automatizado encontrada |
| Vetor exato para PDF Finale/Maestro | NÃO VERIFICADA aqui; plausível, depende de fonte embutida |
| LLM multimodal como precisão de partitura | REFUTADA como motor primário (papers) |

## 10. Plano de PoC mínima (próximo passo)
1. Instalar `lilypond` e `mscore` CLI; gerar PDFs de F2/F3 e checar `pdffonts` (vetor com fonte x path).
2. Rodar Audiveris, HOMR (`uvx`), copisteria (28m e 2m) em F1-F5; registrar tempo/RAM; musicdiff em lote.
3. Spike vetorial 2 dias: extrair glifos de notas/pausas com `pdftotext -bbox-layout` / `pdftocairo -svg` e comparar com F2 renderizado em MuseScore/LilyPond.
4. Testar `por+eng` e `tessdata` padrão para letras PT em F4.
5. 5 páginas do curso real só em laboratório local sem rede: PDFtoMusic Pro trial 1ª página (com autorização do titular).
Critérios de parada: motor sem saída em >5% páginas, ou compasso perfeito < 50% em vetor limpo => sem fusão, apenas OMR + revisão humana. Custo de mão de obra: medir minutos de revisão antes/depois.

## Perguntas ao titular
1. **Licença AGPL (Audiveris/HOMR)**: aceitar uso como processo externo separado distribuído com o app? Opções: (a) sim, só processo e documentado; (b) não, apenas motores Apache/MIT; (c) instalar à parte pelo usuário. Recomendo (c) até parecer jurídico.
2. **PDFtoMusic Pro**: comprar licença só para benchmark local (preço NV)? Opções: sim; não; só trial 1ª página. Recomendo trial 1ª página mais 1 licença pessoal se o spike vetorial for promissor.
3. **Autorização para mandar recortes de partituras a LLM** (nunca material do curso de terceiros)? Opções: nunca; só fixtures públicas; recortes autorizados. Recomendo só fixtures públicas.
4. **Prioridade de recurso**: letras PT e dedilhado vs polifonia completa? Opções: letras/dedos; polifonia; ambos. Recomendo polifonia + letras primeiro; dedilhado via vetor/OCR dedicado.
5. **Hardware alvo**: aceitar GPU (CUDA) ou só CPU? Recomendo só CPU no produto.

## Lacunas
- Datas de commit e ano de releases (homr, oemer, copisteria, Legato) não visíveis.
- Licença dos pesos: homr, oemer, SMT, Legato, Legato 2, Transcoda NV; licença do Llama-3.2 no Legato NV.
- Preço do PDFtoMusic Pro; ARM Linux do PDFtoMusic e Audiveris.
- Benchmark independente do Audiveris em piano; TrOMR/OLiMPiC/LMX/MUSCIMA++/PrIMuS; Sapp 2017; Byrd&Schindele 2006/07; mutool; mir_eval.
- Replicação local de copisteria/homr; macOS (Intel e ARM) não testado.
- Fixtures com licença de edição verificada (F2-F4).
