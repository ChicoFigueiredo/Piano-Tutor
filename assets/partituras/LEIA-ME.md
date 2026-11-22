# Partituras empacotadas (F07)

Três partituras MusicXML 4.0 sem compressão (`.musicxml`), todas do **OpenScore Lieder Corpus**, de obras de domínio público transcritas a partir de edições do IMSLP.

- Repositório de origem: https://github.com/OpenScore/Lieder (commit `38c5db510224d9facdc4b08d741fc788cfb58ea8`, consultado em 2026-11-22)
- Licença da transcrição: **CC0 1.0** (declarada no `LICENSE.txt` do repositório e no elemento `<rights>OpenScore (CC0)</rights>` de cada arquivo). As obras (Brahms, Schubert, Fauré) estão em domínio público.
- Data do download: 2026-11-22.
- Os originais do repositório são `.mxl` (zip). O `score.xml` interno de cada um foi extraído, sem nenhuma alteração de conteúdo, e renomeado.

| Arquivo | Obra | Fôrma | Armadura | Origem (URL bruta, caminho em `scores/`) | SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `brahms-wiegenlied-op49-4.musicxml` | Brahms, Wiegenlied, Op.49 nº 4 | 3/4 | 3 bemóis | `Brahms,_Johannes/5_Lieder,_Op.49/4_Wiegenlied/lc5701612.mxl` | `59d96ba2e6683d432982200b5daca119e80e54386ca134a8dae3c3dd10ddbee7` |
| `schubert-heidenroeslein-d257.musicxml` | Schubert, Heidenröslein, D.257 | 2/4 | 1 sustenido | `Schubert,_Franz/Op.3/3_Heidenröslein,_D.257/lc30321236.mxl` | `80955f1711ff627a49ae1cb4069012090a3973f95726d6cc1b1fb0f8ee1ede7e` |
| `faure-barcarolle-op7-3.musicxml` | Fauré, Barcarolle, Op.7 nº 3 | 6/8 | 4 bemóis | `Fauré,_Gabriel/3_Songs,_Op.7/3_Barcarolle/lc6772888.mxl` | `c48f4b4e5a928e187008096eb43398f5deacb847c914116e9123f06011969324` |

URL de cada arquivo: `https://raw.githubusercontent.com/OpenScore/Lieder/main/scores/<caminho acima>`.

Cada arquivo tem duas partes: voz (uma pauta, clave de sol) e piano (duas pautas, sol e fá), ou seja, três pautas por sistema. A melodia vocal e o piano são tocados juntos pelo botão Tocar. A letra cantada é lida do arquivo mas **não é desenhada** nesta versão.

Estes arquivos são embutidos no executável com `include_str!` (`crates/piano-tutor-app/src/tela_partitura.rs`). Partituras de terceiros protegidas por direitos autorais **não** devem ser adicionadas aqui (ver `docs/seguranca/LICENCAS-E-DADOS.md`).
