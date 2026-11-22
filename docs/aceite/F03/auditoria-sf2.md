# Auditoria de SoundFont SF2 para distribuicao no .deb (ticket #36, F03)

Data da auditoria: 2026-11-22. Escopo: somente pesquisa (nenhum arquivo grande baixado, nenhuma alteracao no repositorio, sem git).
Criterios de aceite do ticket: licenca/atribuicao registradas; preset de piano e um segundo timbre GM contrastante (acordeon desejavel, orgao aceito).
Regra do projeto: licenca nao inventada. Quando nao houve fonte primaria, o item esta marcado como NAO VERIFICADO.

## Resumo executivo

- **Escolhido: MuseScore General** (MIT para o banco principal, com componentes PD/CC0 segundo o copyright Debian). Pacote Debian `musescore-general-soundfont-lossless` em `pool/main`. Atende a preferencia do ticket.
- **Reserva: FluidR3_GM** (MIT). Pacote Debian `fluid-soundfont-gm` 3.1-5.x em `pool/main`, 114.1 MiB. Licenca mais simples de atribuir.
- **Risco transversal principal (bloqueante para o plano):** o pacote Debian `musescore-general-soundfont` (0.2.1-1) entrega a versao **SF3 HQ**; a versao **SF2** esta no pacote `-lossless`. O `rustysynth` (F03) le SF2. Confirmar o formato exato do arquivo antes de adotar.
- **Presets de acordeon e orgao: NAO VERIFICADO em nivel de preset.** Nenhuma fonte primaria listou os 128 presets GM dos SF2 candidatos. Os bancos sao declarados como GM completos, mas isso precisa ser confirmado por escuta ou inspecao do arquivo (ex.: polyphone) antes do checklist de escuta (ADR-0006).

## Quadro comparativo

| Candidato | URL oficial / fonte primaria | Versao | Tamanho | Licenca (SPDX) | Redistribuicao e atribuicao | Debian main | Piano | Acordeon / orgao | Riscos |
|---|---|---|---|---|---|---|---|---|---|
| **MuseScore General** | https://src.opensuse.org/rpm/musescore/src/branch/factory/MuseScore_General_License.md (copia do arquivo de licenca do projeto); copyright Debian: https://people.debian.org/~tg/sf/MuseScore_General_Full/copyright | 0.2 (13/05/2020, conforme licenca); Debian 0.2.1-1 | Debian `musescore-general-soundfont-lossless_0.2.1-1` 314.6 MiB (.deb); `musescore-general-soundfont_0.2.1-1` 72.3 MiB (SF3 HQ, instalado 84 MB) | **MIT** (SPDX `MIT`) para o banco HQ; partes **PD ou CC0** (Debian copyright). Os componentes de ensemble derivam de VSCO 2 CE e VCSL (fontes citadas na copia Debian; licencas dessas fontes NAO VERIFICADO nesta auditoria) | Permitida com manutencao do aviso de copyright e do texto MIT em copias e partes substanciais. Sem outra restricao listada na licenca. Sem garantia | **Sim**: `pool/main/m/musescore-general-soundfont` (listagem FreeBSD mirror) | Sim, GM (esperado; NAO VERIFICADO por preset) | Acordeon e orgao: NAO VERIFICADO por preset (banco declarado GM) | (1) Pacote Debian default e SF3, nao SF2. (2) Copyright Debian menciona "Copyright ... S. Christian Collins" e "Michael Schorsch" etc.; manter lista completa. (3) Tamanho ~314 MiB no .deb; RAM/Disco maior. (4) Versao 0.2 com data de 2020; verificar se ha versao mais nova no site oficial. |
| **FluidR3_GM** | Copyright Debian: https://metadata.ftp-master.debian.org/changelogs/main/f/fluid-soundfont/stable_copyright ; upstream README citado no mesmo arquivo ("I hereby release Fluid under the MIT license, as described in COPYING") | Debian `fluid-soundfont-gm` 3.1-5.1 / 5.2 / 5.3 | 114.1 MiB (.deb) | **MIT** (SPDX `MIT`) | Permitida com manutencao do aviso de copyright e do texto MIT ("shall be included in all copies or substantial portions of the Software") | **Sim**: `pool/main/f/fluid-soundfont` | Sim, GM (esperado; NAO VERIFICADO por preset) | Acordeon e orgao: NAO VERIFICADO por preset | (1) Banco mais antigo (2000-2008). (2) Debian copyright cita material "public-domain" e gravacoes do autor, sem licenca separada para esses trechos (NAO VERIFICADO). (3) Some versoes de MuseScore General incluem FluidR3; a fonte primaria de cada versao deve ser conferida. |
| **GeneralUser GS** | https://www.schristiancollins.com/generaluser.php (pagina nao carregou conteudo de licenca); texto da licenca em https://scancode-licensedb.aboutcode.org/generaluser-gs-2.0.LICENSE (copia de terceiro) | 2.0 (licenca); README do fork de terceiro cita 1.47.1 (npm `generaluser@1.47.1`) | NAO VERIFICADO (tamanho do SF2 nao encontrado) | `LicenseRef-scancode-generaluser-gs-2.0` (nao e SPDX oficial; categoria "Permissive" segundo ScanCode) | Trechos do texto: uso "sem restricao" para criacao musical "private or commercial"; modificacao e redistribuicao permitidas ("feel free to use it in your software projects, and to modify"); sem exigencia de atribuicao segundo o ScanCode. Ressalva do autor: nao garante a origem de todas as amostras. **Texto integral nao verificado na fonte primaria.** | **NAO VERIFICADO.** Discussao debian-legal 2023/03 nao carregou; a classificacao no Debian nao foi confirmada | Sim (GM, 259 presets e 11 kits segundo o README de fork) | NAO VERIFICADO | Licenca nao-SPDX, texto integral nao lido na fonte primaria, historico de discussao no debian-legal (2023) nao lido. Nao recomendado como primario sem leitura do texto oficial e parecer. |
| **Salamander Grand Piano** | https://sfzinstruments.github.io/pianos/salamander ; https://github.com/sfzinstruments/SalamanderGrandPiano | v3 (48 kHz/24 bit) | WAV: 1.18 GB (48 kHz/24 bit) e 394 MB (44.1 kHz/16 bit). Sem SF2 oficial | **CC-BY-3.0** (SPDX `CC-BY-3.0`), autor Alexander Holm | Permitida com atribuicao ao autor; o repositorio pede nota de que o derivado vem deste instrumento | **NAO VERIFICADO**: pacote Debian nao localizado (busca sem resultado) | Sim (piano apenas) | Nao | (1) So SFZ oficial; uma versao SF2 seria conversao de terceiros com licenca propria NAO VERIFICADA. (2) Apenas piano: nao atende segundo timbre sozinho. (3) Tamanho grande. |
| **FreePats** | https://freepats.zenvoid.org/ ; copyright Debian: https://metadata.ftp-master.debian.org/changelogs/main/f/freepats/stable_copyright | Pacote Debian `freepats` 20060219-4 (23.9 MiB) | 23.9 MiB (.deb) | **Mista / NAO VERIFICADO por instrumento.** Copyright Debian cita "GPL" (versao 2 ou posterior) para o "patch set", com excecao de composicao; nao ha licenca de piano ou acordeon identificada | GPL com excecao: NAO VERIFICADO quanto ao conjunto de amostras de piano/acordeon/orgao | Sim (`pool/main/f/freepats`) | Sim (site lista "Acoustic Grand Piano"), licenca NAO VERIFICADO | Acordeon e orgao listados no site (sem licenca por item) | (1) Licenca por instrumento nao verificada. (2) Qualidade e formato (GUS/patch) podem exigir conversao para SF2 (licenca da conversao nao verificada). (3) Versao de 2006. |

## Fichas por candidato

### 1. MuseScore General (escolhido)

- URL oficial: https://musescore.org/en/handbook/soundfonts (pagina de handbook; nao foi lida na auditoria). Arquivo de licenca do projeto: https://src.opensuse.org/rpm/musescore/src/branch/factory/MuseScore_General_License.md
- Versao: 0.2 (13 mai 2020), segundo o arquivo de licenca; Debian 0.2-3 e 0.2.1-1.
- Licenca: MIT (texto "Permission is hereby granted, free of charge..."), com partes PD/CC0 segundo o copyright Debian.
- Redistribuicao e atribuicao: manter o aviso de copyright e o texto MIT em todas as copias e partes substanciais (citacao do arquivo Debian). Lista de titulares nos textos Debian: Frank Wen (2000-2002, 2008); Michael Cowgill (2014-2017); Ethan Winer (2002, Temple Blocks); Michael Schorsch (2016, Drumline); S. Christian Collins (adaptacao 2018-2021); mirabilos (empacotamento Debian, 2018-2021, nao obrigatorio para o app).
- Debian: `musescore-general-soundfont-lossless` (0.2.1-1, 314.6 MiB) e `musescore-general-soundfont` (0.2.1-1, 72.3 MiB, SF3). Ambos em `pool/main`. Confirmacao via listagem do mirror: https://ftp.nl.freebsd.org/pub/debian/pool/main/m/musescore-general-soundfont
- Presets: piano esperado (GM). Acordeon e orgao: NAO VERIFICADO por preset.
- Riscos: formato SF3 no pacote padrao; tamanho; versao de 2020; componentes de terceiros (VSCO 2 CE, VCSL) com licencas nao verificadas aqui.

### 2. FluidR3_GM (reserva)

- Upstream: Frank Wen; README "I hereby release Fluid under the MIT license, as described in COPYING" (citado na copia Debian).
- Licenca: MIT. Atribuicao: manter aviso de copyright e texto MIT.
- Debian: `fluid-soundfont-gm` 3.1-5.1/5.2/5.3, 114.1 MiB, `pool/main/f/fluid-soundfont`.
- Presets: GM esperado; acordeon e orgao NAO VERIFICADO por preset.
- Riscos: banco antigo; partes de amostras "public domain" e gravacoes do autor sem licenca separada descrita (NAO VERIFICADO).

### 3. GeneralUser GS (nao recomendado sem leitura do texto oficial)

- Licenca: GeneralUser GS License v2.0, identificada pelo ScanCode como `LicenseRef-scancode-generaluser-gs-2.0` (nao e SPDX oficial). Texto integral obtido apenas em resumo de terceiros; nao foi possivel ler a fonte primaria (pagina do autor sem conteudo).
- Discussao debian-legal (2023/03): nao carregada; classificacao Debian NAO VERIFICADO.
- Fonte para o fork: README em https://cdn.jsdelivr.net/npm/generaluser@1.47.1/README.md (fork de terceiro, nao o original).

### 4. Salamander Grand Piano (apenas piano; opcional)

- Licenca: CC-BY-3.0, autor Alexander Holm (pagina sfzinstruments). Formato oficial SFZ/WAV, nao SF2.
- Debian: NAO VERIFICADO. Qualquer SF2 derivado tem licenca propria NAO VERIFICADA.

### 5. FreePats (nao recomendado para o caso)

- Licenca por instrumento NAO VERIFICADA. Copyright Debian cita GPL com excecao para o patch set. Acordeon e orgao listados pelo site, sem licenca por item.

## Recomendacao

1. **Escolhido: MuseScore General, versao SF2 (`-lossless`)**, desde que (a) confirmado que o arquivo entregue e `.sf2` e nao `.sf3`, (b) lista de presets GM confirmada (piano + acordeon ou orgao) por inspecao, (c) tamanho aceito para o .deb.
2. **Reserva: FluidR3_GM (MIT, Debian main, 114 MiB)**, mesmo cuidado de confirmar presets.

Criterio de aceite do ticket: "Licenca/atribuicao de cada SF2 distribuido registrada" pode ser marcado para o escolhido com base nas fontes acima. "SF2 aprovado contem preset de piano e segundo timbre" continua **NAO VERIFICADO** ate inspecao do arquivo e escuta.

## Texto de atribuicao sugerido (MuseScore General)

> Este aplicativo inclui o SoundFont MuseScore_General (arquivo `MuseScore_General.sf2`), distribuido sob a licenca MIT, com partes em dominio publico ou CC0. Copyright (c) 2000-2002, 2008 Frank Wen; Copyright (c) 2002 Ethan Winer (Temple Blocks); Copyright (c) 2014-2017 Michael Cowgill (versao mono); Copyright (c) 2016 Michael Schorsch (Drumline); Copyright (c) 2018-2021 S. Christian Collins (adaptacao para MuseScore_General). Fonte do pacote: Debian `musescore-general-soundfont`. O texto da licenca MIT deve ser reproduzido integralmente junto ao aviso.
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions: The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.

Nota: o texto MIT acima e o padrao MIT; a lista de titulares vem da copia Debian e deve ser conferida contra o COPYING do arquivo antes do release. O copyright Debian tambem cita a fonte FluidR3 (MIT), que deve permanecer na atribuicao porque o banco o inclui.

## Fontes consultadas

- Issue #36: https://github.com/ChicoFigueiredo/Piano-Tutor/issues/36
- Fase F03: /home/user/Piano-Tutor/backlog/fases/F03.md
- Politica de licencas: /home/user/Piano-Tutor/docs/seguranca/LICENCAS-E-DADOS.md
- Licenca MuseScore General (copia): https://src.opensuse.org/rpm/musescore/src/branch/factory/MuseScore_General_License.md
- Copyright Debian MuseScore General: https://people.debian.org/~tg/sf/MuseScore_General_Full/copyright
- Pacote Debian musescore-general-soundfont (info): https://packages-pkgmirror-csail.debian.org/sid/musescore-general-soundfont
- Pool Debian musescore-general-soundfont: https://ftp.nl.freebsd.org/pub/debian/pool/main/m/musescore-general-soundfont
- Copyright Debian FluidR3 (fluid-soundfont): https://metadata.ftp-master.debian.org/changelogs/main/f/fluid-soundfont/stable_copyright
- Copyright Debian FluidR3Mono_GM: https://people.debian.org/~tg/sf/FluidR3Mono_GM/copyright
- Pool Debian fluid-soundfont: https://ftp.nl.freebsd.org/pub/debian/pool/main/f/fluid-soundfont
- Copyright Debian freepats: https://metadata.ftp-master.debian.org/changelogs/main/f/freepats/stable_copyright
- Pool Debian freepats: https://ftp.nl.freebsd.org/pub/debian/pool/main/f/freepats
- GeneralUser GS (texto ScanCode, terceiro): https://scancode-licensedb.aboutcode.org/generaluser-gs-2.0.LICENSE
- Salamander Grand Piano: https://sfzinstruments.github.io/pianos/salamander ; https://github.com/sfzinstruments/SalamanderGrandPiano

## Limitacoes desta auditoria

- Paginas de pacote Debian em packages.debian.org e sources.debian.org nao carregaram (conteudo JavaScript); usei copias de mirror e copyright em texto.
- Nenhuma lista de presets GM foi lida na fonte primaria; acordeon e orgao continuam NAO VERIFICADO.
- Tamanho do SF2 MuseScore General (arquivo descompactado) nao foi confirmado; apenas o .deb.
- Copia do COPYING do GitHub do MuseScore retornou erro do site; a licenca MIT foi confirmada pela copia Debian e pelo arquivo de licenca do pacote openSUSE.
- Nenhum arquivo grande foi baixado. Nenhuma alteracao no repositorio. Nenhum commit.

## Atualização 2026-11-22: download na primeira execução

Para funcionar nas três plataformas sem `sudo`, o app procura o SF2 nos caminhos comuns do
sistema (`CANDIDATOS_SF2` em `crates/piano-tutor-app/src/som.rs`) e, se não achar nenhum, baixa
o `FluidR3_GM.sf2` original (MIT, Frank Wen; pacote Ubuntu `fluid-soundfont-gm` 3.1-5.3build1)
da release `soundfont-fluidr3-gm-1` deste repositório, com a licença ao lado
(`LICENSE-FluidR3_GM.txt`). O download confere tamanho (148 398 306 bytes) e SHA-256
(`74594e8f4250680adf590507a306655a299935343583256f3b722c48a1bc1cb0`), grava como `.part` e só
renomeia depois de conferido. Destino: `dirs::data_dir()/piano-tutor/soundfonts/` (Linux
`~/.local/share`, Windows `%APPDATA%`, macOS `~/Library/Application Support`).
`PIANO_TUTOR_SF2` escolhe outro arquivo. Código: `crates/piano-tutor-app/src/baixar_sf2.rs`.
