# Corpus de fixtures MusicXML: índice e manifesto de licenças

Ticket 07 (E01). Só para testes; nenhum crate de produção depende deste. Um teste
(`tests/corpus.rs`) confere que cada fixture do corpus consta aqui com o SHA-256 correto e que
todo arquivo desta pasta está no corpus.

Política (T2): **nenhuma fixture é material comprado** ou de edição protegida por direitos
autorais. Só entram (a) arquivos escritos à mão pelo projeto para este corpus, (b) arquivos
gerados por script versionado e (c) arquivos já versionados em `assets/partituras/`, de licença
CC0 verificada em `assets/partituras/LEIA-ME.md`.

Licença das fixtures próprias e geradas: **CC0 1.0**, declarada no elemento `<rights>` de cada
arquivo. Autoria: projeto Piano Tutor (assistente Claude sob direção do projeto), 2026-11-27.
As fixtures próprias são só estruturas musicais mínimas inventadas para exercitar o caso
indicado; não reproduzem obra nem edição de terceiros.

Os `.mxl` (válidos e maliciosos) não são versionados (`*.mxl` está no `.gitignore`): são gerados
por `piano_tutor_fixtures::gerador::mxl_de_teste()`, sem payload perigoso real (o "zip bomb" é
só espaço em branco comprimido; os caminhos fora do escopo são nomes de entrada que o leitor
recusa, nunca gravados em disco).

| fixture | arquivo | origem e licença | casos | SHA-256 |
| --- | --- | --- | --- | --- |
| `anacruse` | crates/piano-tutor-fixtures/fixtures/anacruse.musicxml | própria (escrita à mão), CC0 1.0 | anacruse (compasso 0 implícito) | `b020b970e2d7207bfc72ccd9abc477a4520515b76b7b3afc9b871ac6509182a9` |
| `duas-vozes` | crates/piano-tutor-fixtures/fixtures/duas-vozes.musicxml | própria (escrita à mão), CC0 1.0 | duas vozes numa pauta (backup) | `d9dc23a4684d5b1f1303b522b49309519312d182217d9590d89d150f0d277d8a` |
| `tercina` | crates/piano-tutor-fixtures/fixtures/tercina.musicxml | própria (escrita à mão), CC0 1.0 | tercina (time-modification 3:2) | `b5c28b6d3286666c0776f6712a02913a113ef0598308ce8ea5e42d7217098e40` |
| `repeticao` | crates/piano-tutor-fixtures/fixtures/repeticao.musicxml | própria (escrita à mão), CC0 1.0 | repetição com casas 1 e 2 | `9a8ecfd58156022a32257a240424740f22496d8336d6f513d9530fba9bd3db22` |
| `letra` | crates/piano-tutor-fixtures/fixtures/letra.musicxml | própria (escrita à mão), CC0 1.0 | letra, sílabas, dois versos | `38632153df49fc85a7f0ac2efdedf4db192bd8442e1425f7143f29e434290581` |
| `cifra` | crates/piano-tutor-fixtures/fixtures/cifra.musicxml | própria (escrita à mão), CC0 1.0 | cifra (harmony), baixo alternativo | `ba1d7b2a9f80638a5bfde10de7085ef2bd8a5a9b85848a3a113a08035f9a0be4` |
| `dedilhado` | crates/piano-tutor-fixtures/fixtures/dedilhado.musicxml | própria (escrita à mão), CC0 1.0 | dedilhado, inclusive substituição | `40c1f172cba92f5ed007a21b18dae1070d1916bc724bab0a471d60ce14d418cb` |
| `dinamica` | crates/piano-tutor-fixtures/fixtures/dinamica.musicxml | própria (escrita à mão), CC0 1.0 | dinâmica (mf, f) e crescendo | `554e7242fb52fb99386e63ba72965751339ad875125345dfcb548575f64694e3` |
| `ornamento` | crates/piano-tutor-fixtures/fixtures/ornamento.musicxml | própria (escrita à mão), CC0 1.0 | ornamentos (trinado, grupeto, mordente) e fermata | `23b5afa29928e49a40216bf2fc2fb59d8b071c75f8e302269c06524638307291` |
| `desconhecido` | crates/piano-tutor-fixtures/fixtures/desconhecido.musicxml | própria (escrita à mão), CC0 1.0 | elementos sem suporte (grouping, figured-bass, harmônico, extensões `x-`) | `b0012c4f65635283060905e9f9740bced44a8cbd82b92f6884e62ed9e8588742` |
| `cem-compassos` | crates/piano-tutor-fixtures/src/gerador.rs | gerada por script versionado (`gerar_cem_compassos`), CC0 1.0 | peça de 100 compassos | `7751d61e41b34df20c1f1f6defec0e6cfba354c8a54bb16a79cfc7f1b906557c` |
| `brahms-wiegenlied` | assets/partituras/brahms-wiegenlied-op49-4.musicxml | OpenScore Lieder Corpus, CC0 1.0 (obra em domínio público); ver `assets/partituras/LEIA-ME.md` | letra, voz e piano | `59d96ba2e6683d432982200b5daca119e80e54386ca134a8dae3c3dd10ddbee7` |
| `schubert-heidenroeslein` | assets/partituras/schubert-heidenroeslein-d257.musicxml | OpenScore Lieder Corpus, CC0 1.0 (obra em domínio público); ver `assets/partituras/LEIA-ME.md` | letra, voz e piano | `80955f1711ff627a49ae1cb4069012090a3973f95726d6cc1b1fb0f8ee1ede7e` |
| `faure-barcarolle` | assets/partituras/faure-barcarolle-op7-3.musicxml | OpenScore Lieder Corpus, CC0 1.0 (obra em domínio público); ver `assets/partituras/LEIA-ME.md` | letra, voz e piano, 6/8 | `c48f4b4e5a928e187008096eb43398f5deacb847c914116e9123f06011969324` |
| `beethoven-ode` | assets/partituras/beethoven-ode-a-alegria-tema.musicxml | gravada pelo projeto, CC0 1.0 (obra em domínio público); ver `assets/partituras/LEIA-ME.md` | duas pautas, andamento | `f9d17f6dfbb0c0b3c6967f4be5dfcdcefa65f97177adfe1903b47b4baffff948` |
| `brilha-brilha` | assets/partituras/tradicional-brilha-brilha-estrelinha.musicxml | gravada pelo projeto, CC0 1.0 (melodia tradicional em domínio público); ver `assets/partituras/LEIA-ME.md` | duas pautas | `d1769b7b79b194b85559dd5fdb5b2e8b50838302ba2bdd4414dfbc18afc001a9` |

O MusicXML do ticket 04 pode ser acrescentado depois como fixtures adicionais.
