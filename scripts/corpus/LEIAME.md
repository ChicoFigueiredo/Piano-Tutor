# Corpus CC0 para o benchmark de reconhecimento (E05, ticket 04)

Preparo de dados. O app e a CI nao dependem de nada daqui; nada disto roda na CI.

## O que e

14 pecas do OpenScore Lieder (CC0-1.0), repositorio `https://github.com/OpenScore/Lieder`, SHA `38c5db510224d9facdc4b08d741fc788cfb58ea8`, acessado em 2026-11-27 (lista em `selecao.json`, escolhida com semente 20261127). Para cada peca: MusicXML, PDF limpo, PNG 300 dpi, 3 scans degradados por pagina (leve, medio, dificil).

Licenca por peca: o script le o `metaTag copyright` do proprio `.mscx` (texto "OpenScore (CC0)" ou variante) e recusa a peca se nao contiver "CC0"; tambem confere que o `LICENSE.txt` do repositorio e CC0 1.0. O texto lido e o SHA do `LICENSE.txt` ficam em `manifesto.json`.

## Como reproduzir

```bash
python3 -m venv .venv-corpus && .venv-corpus/bin/pip install -r scripts/corpus/requirements.txt
P=.venv-corpus/bin/python
$P scripts/corpus/corpus_omr.py baixar      # clona no SHA fixado (~850 MB), confere CC0
$P scripts/corpus/corpus_omr.py converter   # MusicXML + conferencia de compassos e notas
$P scripts/corpus/corpus_omr.py pdf         # PDF limpo + PNG 300 dpi
$P scripts/corpus/corpus_omr.py degradar    # scans augraphy (~15 min)
$P scripts/corpus/corpus_omr.py manifesto --conferir   # compara sha256 com o manifesto
```

Saida em `data/corpus-omr/` (ignorada pelo git via `/data/`). Nao commite PDFs, PNGs nem scans.

## Conversao mscx -> MusicXML (MuseScore)

Se `mscore`/`musescore` estiver no PATH, `converter` roda `mscore <peca>.mscx -o <peca>.musicxml` e registra `--version`. No ambiente do agente MuseScore estava ausente: a conversao ficou **NAO VERIFICADO** como execucao nossa. Neste caso o script usa o `.mxl` oficial do espelho (gerado pelo mantenedor com MuseScore; versao declarada no `<encoding><software>` do XML, registrada no manifesto) e confere contra o `.mscx`: compassos e notas contados (ver `conferencia_conversao`). Resultado desta execucao: 14 de 14 pecas com compassos e notas iguais.

Para regerar no cerebro: instalar MuseScore (anotar a versao) e rodar `converter` de novo; o manifesto passa a registrar "VERIFICADO". Lote oficial alternativo: `mscore -j data/corpus_conversion.json` dentro do clone.

## PDF limpo

verovio 6.4.1 le o MusicXML, renderiza SVG por pagina; cairosvg 2.9.1 gera PDF e PNG (2480x3508 = A4 a 300 dpi). Os glifos musicais saem como *paths* (nao como fonte): serve para raster e para medir OMR, **nao** para testar extracao vetorial por fonte. Para isso, renderizar tambem com MuseScore/LilyPond no cerebro (NAO VERIFICADO aqui; ambos ausentes). Reprodutibilidade: PNG limpo e identico entre execucoes; o PDF **nao** e byte a byte (ids/datas do cairo e pdfunite variam), por isso o hash de referencia e o do PNG.

## Scans degradados

augraphy 8.2.6 (MIT). Semente de cada (peca, preset, pagina) = sha256 de `20261127:<id>:<preset>:<pagina>`; parametros no codigo (`pipeline_augraphy`) e presets no manifesto. Achados de reprodutibilidade:

- `augraphy.Folding` com parametros aleatorios nao e reproduzivel mesmo semeando `random` e `numpy`; usamos `fold_x` fixo, `fold_deviation=(0,0)`, `fold_noise=0`.
- `augraphy.BadPhotoCopy` variou entre execucoes mesmo com `random`, `numpy` e `cv2.setRNGSeed` fixados (testado em combinacao com outras); removido dos presets. O script tambem reexecuta com `PYTHONHASHSEED=0` por precaucao.
- Presets finais: leve = InkBleed+Geometric(rot<=1 grau)+Jpeg; medio = InkBleed forte+NoiseTexturize+Folding+Jpeg; dificil = InkBleed forte+Squish+Faxify+Folding+NoiseTexturize+Jpeg.
- Com isso, duas execucoes completas em processos distintos produziram sha256 identicos (ver resultado no relatorio do ticket). Vale para as versoes fixadas, na mesma plataforma; outra plataforma ou build do OpenCV pode diferir: conferir com `manifesto --conferir`.

Os scans sao sinteticos e nao substituem scan real; reportar em estrato separado.

## Versionado vs. fora do git

Versionado: estes scripts, `selecao.json`, `manifesto.json` (sha256, licenca, origem, comandos, contagens) e `exemplos/` (4 MusicXML pequenos, CC0). Fora do git: tudo em `data/corpus-omr/`, incluindo o clone do Lieder.

## Lacunas declaradas

- Sem corpus publico verificado de **cifra** e **dedilhado**.
- **Lead sheet**: nenhuma peca do Lieder e lead sheet; o Lieder e voz + piano. Cobertura de lead sheet fica a cargo de material proprio do titular.
- **Sem letra**: toda peca selecionada tem letra (criterio); nao ha peca sem letra neste lote. Piano solo sem letra tambem nao existe no Lieder; vira lacuna.
- **Letras em portugues**: depende de pecas de dominio publico a escolher pelo titular. O Lieder tem "Dona Adelaide" (Chiquinha Gonzaga, `lc6610193`) cujo idioma da letra **nao foi verificado**.
- LilyPond, MuseScore: ausentes no ambiente do agente (NAO VERIFICADO).
- MUSCIMA++ (CC BY-NC-SA), material comprado e PDFs de terceiros: fora, conforme T2.
