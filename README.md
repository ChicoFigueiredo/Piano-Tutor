# Piano Tutor 🎹

**Um professor assistido de piano e teclado, local-first, construído em Rust.**

[Especificação integral](docs/specs/ESPECIFICACAO-COMPLETA-v1.md) · [Fases e aceites](docs/aceite/MATRIZ-DE-ACEITE.md) · [Arquitetura](docs/arquitetura/README.md) · [Como trabalhar com agentes](AGENTS.md)

## Visão

O projeto evolui do espelho MIDI das 61 teclas do **REVAS KB-330** para instrumento virtual SoundFont, exercícios musicais, leitura/edição de partituras MusicXML, gravação MIDI, metrônomo, bateria, pads e, por último, uma **Fábrica de Lições** que extrai rascunhos revisáveis de videoaulas (áudio + fala + visão superior das teclas e mãos).

**Regra:** nada de construir quinze módulos ao mesmo tempo. Entregamos **uma fase demonstrável por vez**, com testes, evidência e aceite humano antes da seguinte.

## Arquitetura-alvo

```mermaid
flowchart LR
  REVAS[REVAS KB-330] -->|USB MIDI| RUST[Aplicativo Rust]
  RUST --> UI[bancada-casca / egui]
  RUST --> AUDIO[Motor de áudio / SF2]
  AUDIO --> OUT[Alto-falantes]
  RUST --> MUSIC[MusicXML, MIDI e teoria]
  RUST --> STORE[(SQLite local)]
  VIDEO[Videoaula autorizada] --> ML[Worker Python opcional]
  ML -->|hipóteses com confiança| REVIEW[Revisão humana]
  REVIEW --> MUSIC
```

A IA **não** entra no caminho crítico MIDI → áudio. O aplicativo deve tocar normalmente quando Python, modelos e rede não estiverem disponíveis. Luau (`mlua`) é uma possibilidade *posterior* para regras pedagógicas limitadas, não o motor inicial de ML.

## Primeiras entregas

| Marco | Fases (ordem de execução) | Demonstração de aceite |
|---|---|---|
| M1 — Tocar com som | F00 → F01 → F02 → F03 | Instalar o `.deb`, ligar o REVAS, ver as 61 teclas acendendo e ouvir piano e um segundo timbre com p95 ≤ 50 ms |
| M2 — O app ensina | F04 → F08 → F06 → F05 | Exercício com metrônomo, feedback com compasso e replay da Gravação |
| M3 — Ler e ritmo | spike de notação → F07 → F09 | MusicXML licenciado em duas claves, playback e pads |
| M4 — Fábrica de Lições | F10 → F11 → F12 → F13 | Aula local → Hipóteses → revisão → Lição → "Praticar este trecho" |
| M5 — Homologação | F14 | 60 min de uso real, `.deb` limpo sem Python, checklist assinado |

Detalhes em [PLANO-DE-MARCOS.md](docs/planejamento/PLANO-DE-MARCOS.md); vocabulário em [CONTEXT.md](CONTEXT.md). A plataforma única da v1 é **Linux** ([ADR-0004](docs/decisoes/ADR-0004-somente-linux.md)).

[Abra a matriz F00–F14](docs/aceite/MATRIZ-DE-ACEITE.md) para os critérios detalhados.

## Estado real deste repositório

**Repositório de planejamento e bootstrap.** Os documentos e uma crate de domínio mínima estão prontos. **Não** afirmar que já existe conexão MIDI, sintetizador, interface final, partitura ou IA funcionando; isso só ocorre mediante implementação e homologação de cada fase.

## Organização

- `docs/specs/`: especificação canônica consolidada (1.0).
- `backlog/fases/`: 15 fichas executáveis, cada uma com seus critérios de aceite.
- `docs/aceite/`: matriz e regras de homologação.
- `docs/arquitetura/` e `docs/decisoes/`: arquitetura e ADRs iniciais.
- `docs/skills/`: política de skills; 8 skills Matt Pocock versionadas em `.agents/skills/` (Codex) e `.claude/skills/` (Claude Code).
- `.github/agents/`, `.claude/agents/`, `AGENTS.md`: instruções dos agentes.
- `crates/piano-tutor-core`: primeiro núcleo puro em Rust, **sem hardware nem áudio**.
- `scripts/`: verificação, preparação da equipe, publicação e geração de issues.

## Começar no Linux (cerebro)

A v1 é desenvolvida e homologada **somente em Linux**, no PC de referência **cerebro** (Kubuntu 26.04), com o REVAS em USB nativo (ALSA/PipeWire) e áudio onboard via PipeWire.

```bash
# Pré-requisitos do sistema (apt, PipeWire e rustup):
./scripts/bootstrap-linux.sh        # checar (não instala nada)
./scripts/bootstrap-linux.sh --sim  # instalar pacotes via apt

# Com Rust/Cargo instalado:
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# As oito skills fundamentais já estão versionadas. Para instalar outras do upstream (opcional):
./scripts/instalar-skills.sh
```

**Publicação:** este repositório já foi inicializado diretamente na `main`, com as 15 issues F00–F14 abertas para acompanhamento. As skills essenciais foram incorporadas e versionadas diretamente do repositório de Matt Pocock, com licença MIT preservada e revisão de origem fixa. Os scripts `instalar-skills` permitem atualizar ou acrescentar outras skills via `npx` (ação opcional não executada aqui). Os scripts `publicar-github` agora fazem apenas sincronização segura, nunca force-push.

**Visibilidade atual:** o GitHub indica que este repositório está **público**. Não adicionar cursos, mídia protegida, segredos ou dados pessoais; se desejar projeto privado, alterar a visibilidade nas configurações do repositório.

A shell reutilizável é [`ChicoFigueiredo/bancada`](https://github.com/ChicoFigueiredo/bancada), `crates/bancada-casca`, e **será pública (MIT OR Apache-2.0)** — *pendência do titular:* mudar a visibilidade e a licença no repositório `bancada`; só então a CI compila sem credencial. A integração real será implementada em F00/F02, com dependência fixada. Não copiar código de Chess Studio para criar uma segunda shell.

## Licenciamento, privacidade e segurança

Projeto **sem licença aberta atribuída**, com repositório atualmente público; código autoral não recebe licença de terceiros automaticamente. Sem licença aberta atribuída ao código do novo aplicativo até decisão explícita do titular. Bibliotecas e modelos têm **licenças próprias**; não subir vídeos comprados, SoundFonts de terceiros, pesos restritos, chaves, datasets pessoais ou gravações sem permissão. Consulte [Política de conteúdo e dependências](docs/seguranca/LICENCAS-E-DADOS.md).