# E00 - Fontes de corpus público para benchmark OMR e testes MusicXML

Data de acesso: 2026-11-27 (todas as URLs abaixo foram consultadas nesta data).
Escopo: levantamento de fontes, licenças e proposta de distribuição. Nenhum conteúdo de terceiros foi baixado, copiado ou commitado. Só este arquivo foi escrito.

Legenda de status:
- **CONFIRMADA**: lida em página primária (repositório, site oficial, lei) nesta sessão.
- **NÃO VERIFICADO**: não conseguido confirmar em fonte primária. Não usar como fato.
- Limitação: as páginas foram lidas por extração de texto (resumo por ferramenta), não por cópia integral. Trechos citados foram conferidos contra o resumo, não contra o HTML bruto. Hashes/SHAs de commit não apareceram nas páginas lidas.

---

## 0. Achados que mudam decisões

1. **Único corpus de partitura de piano com licença permissiva confirmada e GT nativo: OpenScore Lieder (CC0-1.0).** Está em `.mscx` + letras `.txt` + análise `rntxt`, não em MusicXML. Converter exige MuseScore (ferramenta não verificada nesta sessão).
2. **MUSCIMA++ é CC BY-NC-SA 4.0 (confirmado).** Proíbe uso comercial e exige compartilhar derivados igual. Só pode entrar como benchmark interno não comercial, a confirmar com o titular.
3. **Não há corpus público confirmado de letras PT/EN, cifras ou dedilhado.** Esta é a maior lacuna para os critérios de letra e cifra/dedo da seção "Corpus e protocolo".
4. **"Domínio público" não basta para scans.** IMSLP avisa que o status varia por país e que edições/transcrições podem ter copyright próprio (confirmado na página "Public domain"). Cada scan precisa de checagem por edição.
5. **Os datasets maiores e com licença clara são sintéticos ou sem piano real:** DeepScores V2 (CC BY 4.0, mas detecção de objetos, sem sequência de eventos verificada) e PrIMuS (monofônico, licença NÃO VERIFICADO).
6. **Datasets de piano com piano real e GT de sequência (GrandStaff, OLiMPiC, Mayer et al.) estão sem licença verificada.** Não podem entrar no corpus distribuível até checagem.
7. **Brasil:** a Lei 9.610/98 (art. 41, confirmado) dá 70 anos pós-morte. Pela regra, Ernesto Nazareth (morte 1934) e Chiquinha Gonzaga (morte 1935) estão em domínio público para a obra, mas a edição e as datas de morte precisam de fonte primária.
8. **Ferramentas de degradação são permissivas:** augraphy (MIT) e ImageMagick (licença própria, uso comercial permitido, exige atribuição).

---

## 1. Fontes de partituras (simbólico e scans)

| Fonte | URL | Licença (status) | Tipo / instrumentos | Tamanho | Formato | Uso recomendado |
|---|---|---|---|---|---|---|
| OpenScore Lieder | https://github.com/OpenScore/Lieder | **CC0-1.0 (CONFIRMADA** no README; "See LICENSE.txt"; pedido de crédito não é condição) | Canções com piano (pianoforte), letras em `.txt` (idioma NÃO VERIFICADO; nome sugere alemão) | 239 commits na página. Contagem de partituras NÃO VERIFICADO no repo; abstract de 2022 cita "mais de 1.200" canções (fonte secundária) | `.mscx`, `lc<id>.txt`, `analysis_automatic.rntxt`, `corpus_conversion.json` | Calibração e conjunto limpo. Render por MuseScore CLI; GT = `.mscx` |
| OpenScore String Quartets | https://github.com/OpenScore/StringQuartets | **CC0-1.0 (CONFIRMADA)**, "See LICENSE.txt" | Quarteto de cordas, **sem piano** | 41 commits; contagem NÃO VERIFICADO | `.mscx` (`sq<id>.mscx`); MusicXML só por conversão | Controle multivoz não-piano; teste de voz |
| MuseScore.com (PD/CC0) | https://musescore.org/en/node/294820 ; https://musescore.org/en/node/315935 | **Por partitura.** CC0 = renúncia só do uploader; flag "PD" = uploader confirma (CONFIRMADA em post de fórum/FAQ, fonte não oficial de licença) | Variado | NÃO VERIFICADO | MSCZ/MusicXML/PDF | **Não usar em massa.** Direitos de compositor, letrista, arranjador e editor podem persistir (confirmado no texto). Só itens auditados individualmente |
| Mutopia Project | https://www.mutopiaproject.org/legal.html | **CC BY, CC BY-SA, PD** por contribuidor (termos CONFIRMADOS; licença de cada peça NÃO VERIFICADO) | Muito piano (NÃO VERIFICADO contagem) | NÃO VERIFICADO | Fonte LilyPond; MusicXML NÃO VERIFICADO | Vetor limpo com GT de fonte. CC BY exige atribuição; CC BY-SA obriga derivados com a mesma licença (atenção a distribuição) |
| IMSLP | https://imslp.org/wiki/Public_domain | Domínio público **depende do país**; edições/transcrições têm copyright próprio (CONFIRMADA na página "Public domain"). EUA: publicado em 1930 ou antes (CONFIRMADA). UE: vida+70. Canadá/Taiwan: vida+50 até 1971 (CONFIRMADA) | Scans de edições impressas, piano e canto | Milhares; contagem NÃO VERIFICADO | PDF/JPG de scan | Scans reais "bom". Escolher edição publicada em 1930 ou antes (EUA) **e** autor falecido há mais de 70 anos (UE), com a regra de cada país a verificar. Página de disclaimers falhou no acesso (NÃO VERIFICADO) |
| KernScores (Humdrum) | https://kern.humdrum.org/ | **NÃO VERIFICADO.** A página não declara licença e diz que partes exigem login ("copyright restricted") | ~108.703 arquivos (7.866.496 notas, CONFIRMADA na página); piano e orquestra | 108.703 arquivos | Formato `**kern`; sem MusicXML declarado | Não usar como corpus público até checar licença por obra. Útil para casos de notação após checagem |
| Sheet Music Benchmark (SMB) | https://arxiv.org/html/2506.10488v2 | Licença dos dados NÃO VERIFICADO (CC BY 4.0 é do paper) | CWMN, 685 páginas, 4.039 regiões; kern da KernScores | 685 páginas | `**kern` | Só referência de baseline. Não é corpus de GT verificado |
| Musica Brasilis (PD brasileira) | https://musicabrasilis.org.br/en/not%C3%ADcias/28953/ (notícia de 06/03/2023) | "Domínio público" declarado; **licença NÃO VERIFICADO** | Meta de 7.000 partituras de compositores brasileiros; lista preliminar de 22 (inclui Chiquinha Gonzaga e Ernesto Nazareth). Estado atual NÃO VERIFICADO | Não confirmado | PDF 300 dpi, uma parte por instrumento; ISMN; metadados em Wikidata | Fonte brasileira futura. Hoje: verificar item a item |
| IMS - Acervo digital | https://acervos.ims.com.br/index.php/Front/Index?lang=en_US | Termos de uso NÃO VERIFICADO (link para `/termsofuse` não aberto). Seção "In public domain" existe (CONFIRMADA) | Partituras e gravações de música brasileira (Chiquinha, Nazareth) | NÃO VERIFICADO | NÃO VERIFICADO | Candidato a scan brasileiro **após** ler os termos de uso |

---

## 2. Datasets de OMR

| Dataset | Fonte / URL | Licença (status) | Tipo (impresso/manuscrito; músico) | Tamanho | Formato do GT | Uso recomendado |
|---|---|---|---|---|---|---|
| DeepScores V2 | https://zenodo.org/record/4012193 (DOI 10.5281/zenodo.4012193) | **CC BY 4.0 (CONFIRMADA)** | Impresso renderizado; piano NÃO VERIFICADO | Completo: 255.385 imagens, 80,9 GB. Denso: 1.714 imagens, 741,8 MB (CONFIRMADA) | Bounding boxes (orientadas e não), segmentação semântica e de instância; formato de arquivo NÃO VERIFICADO | Teste de **detecção** de símbolos. Não serve como GT de eventos (pitch/onset) sem conversão não verificada |
| MUSCIMA++ v2.1 | https://ufal.mff.cuni.cz/muscima | **CC BY-NC-SA 4.0 (CONFIRMADA)**. Não comercial e share-alike | Manuscrito; 50 escritores (CVC-MUSCIMA); 140 imagens anotadas de 1.000 | 91.255 símbolos (CONFIRMADA) | XML (MuNG), com bounding boxes, máscaras e arestas de relação | Benchmark interno de símbolos. **Não redistribuir** imagens (CVC não incluído). Não comercial |
| CVC-MUSCIMA | link no site MUSCIMA++ | Licença NÃO VERIFICADO para as imagens | Manuscrito; 1.000 imagens binárias, pautas removidas, 20 páginas x 50 escritores (CONFIRMADA) | ~1.000 imagens; tamanho NÃO VERIFICADO | Não distribuído pelo MUSCIMA++ | Só se a licença for verificada |
| DoReMi | https://elonashatri.github.io/doremi-dataset.html | **NÃO VERIFICADO.** Página diz que metade da música tem copyright e que o subset livre está em "GitHub release" (URL NÃO informada) | Renderizado (Dorico), PNG 300 dpi, 2475x3504, binarizado; ~6.432 imagens; ~1 milhão de objetos em 94 classes (CONFIRMADA) | ~6.432 imagens | MusicXML, OMR metadata XML (bounding boxes, máscaras, atributos), MEI, MIDI (CONFIRMADA) | Calibração de símbolos e GT rico, após localizar o release e a licença |
| GrandStaff (Ríos-Vila) | https://grfia.dlsi.ua.es/sheet-music-transformer/ ; https://rua.ua.es/dspace/handle/10045/134340 | **NÃO VERIFICADO** | **Pianoform**, polifônico, sintético (renderizado); 53.882 sistemas (7.000 de teste originais + 46.882 aumentados) (CONFIRMADA) | 53.882 sistemas | "digital score encoding", formato NÃO NOMEADO na página | Calibração de piano duas claves. Limpo e sintético, sem variação real. Camera GrandStaff = versão distorcida (mesmas imagens) |
| Quartets (Ríos-Vila) | mesma página acima | **NÃO VERIFICADO** | Quarteto, sintético; 38.051 excertos de ~7 s; treino: 18.162 Haydn, 7.435 Mozart, 12.454 Beethoven (CONFIRMADA) | 38.051 | Humdrum `**kern` (CONFIRMADA) | Multivoz não-piano. Leakage por compositor a controlar |
| OLiMPiC | citado em https://arxiv.org/html/2506.10488v2 | **NÃO VERIFICADO** (licença e tamanho) | Pianoform; "restricted size"; GT em XML linearizado (citação indireta) | NÃO VERIFICADO | Linearized XML (NÃO VERIFICADO) | Não usar até achar a fonte primária |
| Dados de Mayer et al. (pianoform) | https://arxiv.org/abs/2403.13763 | **NÃO VERIFICADO** para os dados (CC BY-SA 4.0 vale só para o paper) | Pianoform. Dev 1.438 e test 1.493 sistemas; GT MusicXML a partir do OpenScore Lieder; imagens do IMSLP (CONFIRMADA) | 1.438 + 1.493 sistemas | MusicXML | **Precedente útil** de como parear GT do OpenScore com scan do IMSLP. Pedir/checar licença dos dados |
| PrIMuS | https://grfia.dlsi.ua.es/primus/ | **NÃO VERIFICADO** (página não declara) | Monofônico (incipits); 87.678 incipits (CONFIRMADA) | 87.678 | 5 arquivos por item: PAE/Plaine&Easie, PNG, MEI, semântico, agnóstico (CONFIRMADA) | Teste de monofonia/linha única. Não cobre piano |
| Camera-PrIMuS | página PrIMuS (distorções) e https://github.com/Zurvanos/CameraPrIMuS (**falhou no acesso**) | **NÃO VERIFICADO** | Monofônico (herda PrIMuS); imagens com distorção de câmera | NÃO VERIFICADO | PNG + GT herdado | Só como referência de degradação, após checar a licença |
| OMRTestCorpus (Byrd & Simonsen) | citado em arXiv 2506.10488 | NÃO VERIFICADO | 34 páginas (citação indireta) | 34 páginas | NÃO VERIFICADO | Só se a fonte primária aparecer |

Nota: o repositório Hugging Face `Zurvanos/CameraPrIMuS` que apareceu na busca mostrou um arquivo parquet de ~126 kB, marcado como MIT, com "< 1K" registros. **Não é** o Camera-PrIMuS completo e não confirma a licença do dataset oficial.

---

## 3. Suítes de teste MusicXML (round-trip e importação)

| Suíte | URL | Licença (status) | Conteúdo | Uso |
|---|---|---|---|---|
| W3C MusicXML 4.0 - examples | https://www.w3.org/2021/06/musicxml40/musicxml-reference/examples/ | **NÃO VERIFICADO.** Página sem licença; documento sob W3C Document License não lido | ~294 páginas de exemplo (contagem manual nossa; ~274 de elementos, 14 de clefs, 6 tutoriais) (CONFIRMADA) | Referência de semântica. **Não confirmado** se há arquivos MusicXML baixáveis por exemplo; a página mostra PNGs e links por exemplo |
| LilyPond - MusicXML regression tests | https://lilypond.org/doc/v2.25/Documentation/contributor/musicxml-tests ; repo `input/regression/musicxml/` (GitHub bloqueou o acesso por robots, NÃO VERIFICADO no repo) | Termos dos arquivos **NÃO VERIFICADO**. Suíte oficial (CONFIRMADA) | Testes para `musicxml2ly` (importação MusicXML para LilyPond). Contagem NÃO VERIFICADO | Casos de entrada/importação. Não é round-trip (a página não descreve exportação de volta). Sem GT de partitura |

Round-trip (MusicXML -> modelo -> MusicXML) precisa de ferramenta de conversão e de comparação. Nenhuma foi verificada nesta sessão. Pendência em "Lacunas".

---

## 4. Domínio público brasileiro

| Item | Fonte | Status |
|---|---|---|
| Prazo de direitos patrimoniais: 70 anos, contados de 1º de janeiro do ano seguinte ao falecimento (art. 41, Lei 9.610/98) | https://www.planalto.gov.br/ccivil_03/leis/l9610.htm | **CONFIRMADA** (texto lido) |
| Obras de autor sem sucessores e de autor desconhecido também são domínio público (art. 45) | mesma lei | **CONFIRMADA** |
| Ernesto Nazareth (1863-1934) | fonte secundária: Musica Brasilis | Datas NÃO VERIFICADO em fonte primária. Cálculo nosso: morte 1934 -> domínio público a partir de 01/01/2005 |
| Chiquinha Gonzaga (1847-1935) | fonte secundária: Musica Brasilis | Datas NÃO VERIFICADO em fonte primária. Cálculo nosso: morte 1935 -> domínio público a partir de 01/01/2006 |
| Regras de transição da Lei 9.610 para obras de autores mortos antes de 1998 | - | **NÃO VERIFICADO** |
| Edições e arranjos (editor, revisão, digitalização) | IMSLP (regra geral, CONFIRMADA) | Direitos próprios possíveis. Verificar cada edição |
| Itens de Chiquinha no IMSLP (ex.: "As pombas", "Musiciana", "Maria") | imslp.org (apareceram em busca) | Existência CONFIRMADA na busca; **licença de cada edição NÃO VERIFICADO** |

---

## 5. Ferramentas de degradação reproduzível

| Ferramenta | Licença (status) | Versão | Efeitos relevantes | Notas |
|---|---|---|---|---|
| augraphy | **MIT (CONFIRMADA)**, https://github.com/sparkfish/augraphy | 8.2.6 (CONFIRMADA na citação; data de release NÃO VERIFICADO; página de releases vazia) | Tinta: InkBleed, BleedThrough, BadPhotoCopy, Faxify, Dithering, DotMatrix, Letterpress, Jpeg, WaterMark. Espaço: Folding, BookBinding, PageBorder, Geometric, InkShifter, Squish. Papel: textura | Fixar versão no lockfile. Reprodutibilidade por seed **NÃO VERIFICADO** (testar: duas execuções, mesmo hash) |
| ImageMagick | **ImageMagick License (CONFIRMADA)**, https://imagemagick.org/license/. Uso comercial livre; redistribuição permitida com cópia da licença; exige atribuição a "ImageMagick Studio LLC" | NÃO VERIFICADO (fixar no lockfile) | Operações determinísticas: `-blur`, `-attenuate`, `+noise`, `-rotate`, `-random-threshold`, JPEG | Afirmação "Apache 2.0 based" **NÃO VERIFICADO** (a página não confirma; não usar) |
| MuseScore CLI / Verovio (render) | Licenças NÃO VERIFICADO nesta sessão | NÃO VERIFICADO | Render de MusicXML/mscx para PDF | Anotar versão exata usada no render |

### Pipeline proposto (reproduzível)

1. **Fonte limpa**: partitura CC0 ou CC BY verificada (`.mscx`/MusicXML). Registrar URL, commit/SHA (NÃO VERIFICADO na página) e hash do arquivo.
2. **Render**: MuseScore CLI (versão fixada) -> PDF -> PNG 300 dpi. Registrar hash da imagem limpa. Este é o "vetor limpo".
3. **Degradação**: três presets com seed fixa registrada:
   - leve (scan bom): augraphy InkBleed + Jpeg q 60-75 + rotação até 1 grau;
   - médio/difícil: BleedThrough + Folding + BadPhotoCopy + Faxify + ImageMagick (`-attenuate`, `+noise`);
   - misto: combinar com textura de papel.
4. **Invariante**: geometria da degradação é só imagem. GT não muda. Se houver deslocamento (rotação, squish), transformar as coordenadas ou declarar que o GT é só de conteúdo (sem posição).
5. **Registro**: `manifest.csv` com fonte, seed, preset, versões, hash de entrada e saída. Scripts em repositório separado sem dados de terceiros.
6. **Limites**: degradação sintética não substitui scan real. Reportar em estratos separados.

---

## 6. Proposta de distribuição: 60 peças (somente material público verificado)

Regra adotada: só entram peças com licença confirmada por fonte primária (CC0, CC BY, PD por item verificado) ou material próprio gerado. Nada de NC, SA sem checagem, ou licença NÃO VERIFICADO no conjunto distribuível.

### 6.1 Vetor limpo (20)

| Lote | Quantidade | Origem | Licença | Cobertura |
|---|---|---|---|---|
| Lieder (piano, duas claves) | 12 | OpenScore Lieder, render MuseScore CLI | CC0 (CONFIRMADA) | Piano duas claves, letras (idioma a verificar), anacruse e repetições a confirmar por auditoria |
| Mutopia (piano) | 6 | Mutopia, render LilyPond | CC BY ou PD por peça (verificar cada) | Piano, repetições, mudança de compasso a confirmar |
| String Quartets | 2 | OpenScore String Quartets | CC0 (CONFIRMADA) | Multivoz sem piano (controle) |

Divisão: 10 calibração e 10 conjunto cego, **por obra** (nenhuma obra nos dois conjuntos). Para Lieder, separar por canção; para Mutopia, por peça.

### 6.2 Scan bom (20)

| Lote | Quantidade | Origem | Licença | GT |
|---|---|---|---|---|
| Scan real IMSLP (edição PD verificada) | 10 | IMSLP, edição publicada antes de 1930 e autor com morte pré-1955 (regra adotada) | Checagem por edição (NÃO VERIFICADO até auditar) | GT da transcrição do OpenScore Lieder da **mesma** obra (modelo de Mayer et al.), com revisão humana dupla do alinhamento página/compasso |
| Degradação leve sobre vetor (sintético) | 10 | Render do lote de vetor + preset leve | Herda a licença da fonte | GT da fonte |

### 6.3 Difícil / misto (20)

| Lote | Quantidade | Origem | Licença | GT |
|---|---|---|---|---|
| Degradação médio/difícil (sintética) | 8 | Vetor + presets médio/difícil (augraphy + ImageMagick) | Herda da fonte | GT da fonte |
| Scan real antigo com manchas (IMSLP) | 6 | IMSLP, edições antigas | Checagem por edição (NÃO VERIFICADO) | Transcrição dupla humana; sem GT de motor |
| Próprio (PD brasileira engravada + degradação) | 6 | Engraving próprio de melodia brasileira em domínio público (Nazareth ou Chiquinha, após checar datas e edição), com letra PT | Material próprio (licença a definir pelo titular) | GT da própria engraving |

Cobertura por critério, após proposta:
- Piano duas claves: coberto (Lieder, Mutopia, scans IMSLP).
- Múltiplas vozes: coberto parcialmente (SQ, e Lieder com acompanhamento).
- Letras PT: coberto só pelo lote próprio (6). **Letras EN: NÃO COBERTO** sem fonte verificada.
- Cifras: **NÃO COBERTO** (nenhuma fonte verificada).
- Dedilhado: **NÃO COBERTO** (nenhuma fonte verificada).
- Tuplets, ornamentos, anacruses, mudança de clave/tempo, partitura sem compasso: a verificar por auditoria em Lieder/Mutopia; NÃO VERIFICADO.

---

## 7. Lacunas

| Lacuna | Motivo |
|---|---|
| Letras PT e EN com licença pública | Nenhuma fonte encontrada nesta sessão |
| Cifras (chord symbols) públicas com GT | Não encontrado |
| Dedilhado público | Não encontrado |
| Contagem atual e SHA das partituras OpenScore | Página não mostra; abstract de 2022 é fonte secundária |
| Licença do OLiMPiC, PrIMuS, GrandStaff, DoReMi (URL do subset), Camera-PrIMuS | Não confirmado em fonte primária |
| Licença do KernScores por obra | Página sem licença; partes com login |
| Termos dos arquivos de teste W3C e LilyPond | Não confirmado; GitHub de LilyPond bloqueou acesso |
| Página de disclaimers do IMSLP | Falhou no acesso |
| Termos de uso do acervo IMS | Link `/termsofuse` não aberto |
| Datas de morte de Nazareth e Chiquinha em fonte primária | Só fonte secundária (Musica Brasilis) |
| Regras de transição da Lei 9.610 | Não verificado |
| Ferramenta de round-trip MusicXML (conversão e comparação) | Não pesquisada |
| Reprodutibilidade por seed do augraphy | Não testada |
| "ImageMagick Apache 2.0 based" | Não confirmado; removido |
| Spec da proposta (`docs/evolucao-musical/*.md`, AGENTS.md, CONTEXT.md) | Só li a seção "Corpus e protocolo" da pesquisa. **Nenhuma conclusão da spec foi classificada como CONFIRMADA ou REFUTADA neste documento** |

Diferença de fonte: Musica Brasilis mostra a data de morte de Euclides da Cunha como "1854-1829" (provável erro). Não usada aqui.

---

## Perguntas ao titular

1. **Uso de MUSCIMA++ (CC BY-NC-SA)?**
   - A) Só benchmark interno, não distribuído, não comercial. *(Recomendação)*
   - B) Excluir de todo o corpus.
   - C) Pedir parecer jurídico antes de usar.
   - Recomendação: A, com marcação explícita no relatório de que é NC.

2. **Scans reais do IMSLP entram no corpus?**
   - A) Só CC0/CC BY verificados e material próprio. Sem scan real de IMSLP.
   - B) Scans IMSLP com checagem de licença por edição (trabalho manual, mais realista).
   - C) Ambos, sem checagem por edição. *(Não recomendado.)*
   - Recomendação: B para o lote de scan bom; A se não houver tempo para auditoria.

3. **Redistribuir o corpus ou só métricas e scripts?**
   - A) Só métricas agregadas e scripts, sem conteúdo. *(Recomendação, por causa de CC BY-SA e NC e do uso de scans IMSLP.)*
   - B) Corpus em repositório público, só com itens CC0.

4. **Investir em engraving próprio de melodia brasileira em domínio público para cobrir letras PT?**
   - A) Sim, com checagem de datas e edição. *(Recomendação; é a única forma de cobrir PT com licença controlada.)*
   - B) Não, aceitar lacuna de letras PT e reportar como não coberto.

5. **PDFs comprados do titular (laboratório local)?** Confirmar que ficam fora de git, CI e nuvem, e que só métricas agregadas saem. Opções: A) confirmar. B) Excluir do benchmark. *(Recomendação: A, conforme REGRAS-EQUIPE.)*

---

## Resumo final ao supervisor (10 linhas)

- CONFIRMADO: OpenScore Lieder e String Quartets em CC0; DeepScores V2 em CC BY 4.0; MUSCIMA++ em CC BY-NC-SA 4.0; augraphy em MIT; ImageMagick com uso comercial livre.
- CONFIRMADO: Lei 9.610 art. 41 (70 anos). Datas de Nazareth e Chiquinha só por fonte secundária.
- Maior lacuna: nenhuma fonte pública confirmada para letras EN, cifras ou dedilhado.
- GrandStaff, OLiMPiC, DoReMi, PrIMuS e Camera-PrIMuS sem licença verificada: fora do corpus até checar.
- Proposta de 60 peças: vetor 20 (Lieder CC0 + Mutopia + SQ), scan bom 20 (IMSLP por edição + degradação leve), difícil 20 (degradação + IMSLP + engraving próprio).
- Recomendação principal: publicar só métricas e scripts; MUSCIMA++ só interno.
- Spec não foi auditada aqui; nenhuma conclusão classificada.
- Arquivo: /home/user/Piano-Tutor/docs/evolucao-musical/e00/05-CORPUS.md
