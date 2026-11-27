# 04: Preparo do corpus CC0 para o Reconhecimento (OpenScore Lieder + PDFs e scans degradados)

**Fase:** E05 (preparo, paralelo)

**R-IDs/critérios:** R-B04, R-B05, T2, T7, D5; Resoluções do supervisor (OpenScore .mscx, MUSCIMA++ fora)

**Seam:** N/A (preparo de dados; alimenta o benchmark da E05 e pode alimentar fixtures da E01)

**Modelo sugerido:** Sonnet / médio (Haiku para a parte de README e licenças)

**What to build:** Existe um conjunto de peças com MusicXML de licença verificada (OpenScore Lieder, CC0), convertido a partir do formato do MuseScore por uma ferramenta externa usada só no preparo do corpus (fora do app e da CI), mais PDFs vetoriais gerados do MusicXML e versões escaneadas degradadas (por augraphy) com semente fixa. Os scripts e o registro de como tudo foi gerado são reproduzíveis; o app e a CI não dependem de nada disso.

**Blocked by:** None (can start immediately)

**Status:** feito, aguarda revisão

- [x] Há uma lista de peças escolhidas (piano duas pautas e lead sheet, com e sem letra) com a licença de cada uma conferida na origem, com URL, data de acesso e versão/SHA do conjunto.
- [x] Para cada peça, o MusicXML gerado registra o comando e a versão do MuseScore usados; a conversão é conferida por amostra (compassos e notas contados contra o original) e o resultado da amostra fica registrado.
- [x] PDFs vetoriais são gerados a partir do MusicXML por procedimento documentado; scans degradados são gerados com augraphy, sob semente e parâmetros registrados, e são reproduzíveis byte a byte ou, se não for possível, com a razão registrada.
- [x] Nenhum PDF de terceiros, scan sem checagem de edição, MUSCIMA++ ou material comprado entra no repositório (T2); só scripts, MusicXML CC0 de tamanho razoável e métricas são versionados; PDFs/scans grandes ficam fora do git com receita de regeneração.
- [x] Lacunas declaradas: sem corpus público verificado de cifra e dedilhado; cobertura de letras em português depende de peças de domínio público a escolher pelo titular.
- [x] Nada é executado na CI; a parte que depende de MuseScore instalado é marcada NÃO VERIFICADO se não puder rodar no ambiente do agente e fica para execução no PC de referência.

**Notas:** Sem bloqueios; pode correr em paralelo a qualquer outro ticket. O MusicXML CC0 resultante pode ser reaproveitado como fixtures pelo ticket 07, mas este ticket não depende dele nem o contrário. Se ficar grande demais para um contexto, dividir em (a) conversão e (b) PDFs/scans degradados.

**Entrega (2026-11-27):** `scripts/corpus/` (corpus_omr.py, requirements.txt com versões fixadas, selecao.json, manifesto.json, LEIAME.md, exemplos/ com 4 MusicXML CC0). 14 peças do OpenScore Lieder no SHA 38c5db5; CC0 conferido no metaTag de cada .mscx. MuseScore CLI ausente: conversão usa o .mxl oficial do espelho e está **NÃO VERIFICADO** como execução nossa; compassos e notas conferidos contra o .mscx (14/14 iguais). Scans: 81 PNGs (leve/médio/difícil), duas execuções completas com sha256 idêntico (0 divergências em 81); PDF não é byte a byte (razão no manifesto). BadPhotoCopy e BleedThrough do augraphy não são reproduzíveis e foram evitados. Lacunas: ver LEIAME.md (lead sheet, peça sem letra, cifra, dedilhado, letra PT, MuseScore/LilyPond).
