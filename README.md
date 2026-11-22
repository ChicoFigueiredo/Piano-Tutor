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

| Sequência | Evidência exigida |
|---|---|
| F00 — Laboratório | Workspace, biblioteca `bancada-casca` validada, diagnósticos e ambiente documentados |
| F01 — MIDI real | Eventos do REVAS detectados, calibração das 61 teclas, teste físico |
| F02 — Espelho visual | Tecla pressionada acende e apaga corretamente, inclusive acordes |
| F03 — SoundFont | Mesmo Dó com dois timbres, latência medida, sem estalos |
| F04–F09 | Teoria, exercícios, gravação, partituras, metrônomo, bateria/pads |
| F10–F13 | Vídeo, IA multimodal e lições revisáveis |
| F14 | Homologação, empacotamento, documentação e distribuição |

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

## Começar no Windows ou WSL2

O REVAS deve ser testado inicialmente no **Windows nativo**, onde a conexão USB é direta. O WSL2 pode ser usado para editor, análise e desenvolvimento, mas MIDI USB precisa ser testado no SO em que o dispositivo está efetivamente exposto.

```bash
# Com Rust/Cargo instalado:
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# As oito skills fundamentais já estão versionadas. Para instalar outras do upstream (opcional):
./scripts/instalar-skills.sh
```

**Publicação:** este repositório já foi inicializado diretamente na `main`, com as 15 issues F00–F14 abertas para acompanhamento. As skills essenciais foram incorporadas e versionadas diretamente do repositório de Matt Pocock, com licença MIT preservada e revisão de origem fixa. Os scripts `instalar-skills` permitem atualizar ou acrescentar outras skills via `npx` (ação opcional não executada aqui). Os scripts `publicar-github` agora fazem apenas sincronização segura, nunca force-push.

**Visibilidade atual:** o GitHub indica que este repositório está **público**. Não adicionar cursos, mídia protegida, segredos ou dados pessoais; se desejar projeto privado, alterar a visibilidade nas configurações do repositório.

A shell reutilizável é **privada**: [`ChicoFigueiredo/bancada`](https://github.com/ChicoFigueiredo/bancada), `crates/bancada-casca`; a integração real será implementada em F00/F02, com dependência fixada e política de acesso da CI. Não copiar código de Chess Studio para criar uma segunda shell.

## Licenciamento, privacidade e segurança

Projeto **sem licença aberta atribuída**, com repositório atualmente público; código autoral não recebe licença de terceiros automaticamente. Sem licença aberta atribuída ao código do novo aplicativo até decisão explícita do titular. Bibliotecas e modelos têm **licenças próprias**; não subir vídeos comprados, SoundFonts de terceiros, pesos restritos, chaves, datasets pessoais ou gravações sem permissão. Consulte [Política de conteúdo e dependências](docs/seguranca/LICENCAS-E-DADOS.md).