# Pacote .deb mínimo: evidência de nuvem (F03, ticket #40)

> Ambiente: **NUVEM** (container Claude Code remoto, Linux, Debian/Ubuntu com `apt-get`). **NÃO é o cerebro.** Esta evidência prova apenas que o pacote é gerado, tem o conteúdo esperado e instala. Não comprova abrir o app gráfico, tocar MIDI, latência, nem o SoundFont tocando no equipamento de referência.

## Ferramenta

- `cargo-deb` **3.8.0**, instalado com `cargo install cargo-deb --locked`.
- Toolchain: `cargo 1.98.0`.

## Como gerar

```
scripts/empacotar-deb.sh
```

O script roda `cargo deb -p piano-tutor-app` e imprime o caminho do `.deb` gerado em `target/debian/`. O diretório `target/` já está coberto por `/target/` no `.gitignore`, então o `.deb` não é commitado.

## Configuração

Em `crates/piano-tutor-app/Cargo.toml`, bloco `[package.metadata.deb]`:

- `name = "piano-tutor"`, `section = "sound"`, `priority = "optional"`.
- `maintainer = "Piano Tutor <sem-email>"` (genérico, sem e-mail pessoal).
- `depends = "$auto, fluid-soundfont-gm"`. O SoundFont **FluidR3_GM** (MIT, Debian `main`) **não é embutido** no pacote. A decisão está em [`../F03/auditoria-sf2.md`](auditoria-sf2.md).
- Assets:
  - `target/release/piano-tutor-app` vira `/usr/bin/piano-tutor` (modo 755).
  - `crates/piano-tutor-app/pacote/piano-tutor.desktop` vira `/usr/share/applications/piano-tutor.desktop` (`Categories=Audio;Music;Education;`).
  - `README.md` vira `/usr/share/doc/piano-tutor/README.md`.

## Saídas executadas

### `scripts/empacotar-deb.sh`

Compilação em release concluída e caminho impresso:

```
Finished `release` profile [optimized] target(s) in 4m 59s
/home/user/Piano-Tutor/.claude/worktrees/agent-ab03bbfa10fcb8720/target/debian/piano-tutor_0.1.0-1_amd64.deb
```

Arquivo gerado: `target/debian/piano-tutor_0.1.0-1_amd64.deb`, 4 386 636 bytes.

### `dpkg-deb --info`

```
 new Debian package, version 2.0.
 size 4386636 bytes: control archive=388 bytes.
 Package: piano-tutor
 Version: 0.1.0-1
 Architecture: amd64
 Section: sound
 Priority: optional
 Maintainer: Piano Tutor <sem-email>
 Installed-Size: 14893
 Depends: fluid-soundfont-gm, libc6 (>= 2.39)
 Description: Aplicativo "Olá Piano Tutor": janela mínima sobre a bancada-casca (F00)
  Aplicativo Olá Piano Tutor: toca MIDI com motor SoundFont, sem Python.
```

### `dpkg-deb --contents`

```
drwxr-xr-x ./usr/
drwxr-xr-x ./usr/share/
drwxr-xr-x ./usr/share/applications/
-rw-r--r-- ./usr/share/applications/piano-tutor.desktop   (242 bytes)
drwxr-xr-x ./usr/share/doc/
drwxr-xr-x ./usr/share/doc/piano-tutor/
-rw-r--r-- ./usr/share/doc/piano-tutor/README.md          (6007 bytes)
-rw-r--r-- ./usr/share/doc/piano-tutor/copyright          (155 bytes)
drwxr-xr-x ./usr/bin/
-rwxr-xr-x ./usr/bin/piano-tutor                          (15237400 bytes)
```

### `apt-get install -y ./target/debian/*.deb`

```
0 upgraded, 1 newly installed, 0 to remove and 33 not upgraded.
Setting up piano-tutor (0.1.0-1) ...
```

### `which piano-tutor`

```
/usr/bin/piano-tutor
```

A dependência `fluid-soundfont-gm` já estava instalada no container (`dpkg -l`: `ii fluid-soundfont-gm 3.1-5.3`).

## Observações

- A descrição curta do pacote ainda herda `description` do `Cargo.toml` do crate ("janela mínima sobre a bancada-casca (F00)"). É cosmética e fica como risco documentado.
- Não foi executado `piano-tutor` para abrir a janela: o critério de nuvem é apenas a instalação.
- Não há `copyright` completo com licenças de dependências transitivas. A auditoria de licenças está em `docs/seguranca/LICENCAS-E-DADOS.md`.

## NÃO VERIFICADO (exige cerebro)

- `.deb` instalado no cerebro, abrindo o app (X11/Wayland reais).
- Tocar MIDI com o SoundFont FluidR3_GM via `fluid-soundfont-gm` no equipamento de referência, sem Python.
- Critérios de latência, jitter e checklist de escuta do F03 (ADR-0006).

## Atualização: app da M1 (Monitor MIDI, roteamento e Timbres)

Regenerado na nuvem com `scripts/empacotar-deb.sh` depois de o app passar a usar `piano-tutor-midi` (`midir`) e `piano-tutor-audio` (`cpal`, `rustysynth`):

```
Finished `release` profile [optimized] target(s)
target/debian/piano-tutor_0.1.0-1_amd64.deb   (4 584 516 bytes)
Depends: fluid-soundfont-gm, libasound2t64 (>= 1.1.0), libc6 (>= 2.39)
./usr/bin/piano-tutor (15 972 504 bytes), .desktop e README como antes
```

A dependência `libasound2t64` agora aparece sozinha (`$auto`), porque o binário liga a ALSA. O binário de release abriu sob Xvfb e mostrou a tela Timbres com o FluidR3_GM carregado e `Áudio: indisponível` (sem placa de som). **NÃO VERIFICADO:** instalar no cerebro, abrir no KDE e tocar MIDI + SF2.

