> Numeração duplicada histórica; ver também ADR-0009-cursos-de-tutor-como-pacote.md

# ADR-0009 — Lições e Exercícios em arquivos TOML declarativos; script fica para depois

**Status:** PROPOSTA — 2026-11-23 · **Complementa:** F05 (professor assistido), CONTEXT.md (Lição, Exercício, Gabarito, Trilha)

## Contexto

Os 10 exercícios da F05 estão escritos em Rust (`exercicios_de_fabrica`). Para crescer a biblioteca (escalas nos 12 tons, acordes, técnica, ouvido, leitura) sem recompilar e para permitir Lições criadas à mão pelo titular, o conteúdo precisa sair do código. Foi pedida também a avaliação de script (Lua via `mlua`) para regras mais ricas.

## Decisão

1. **Uma Lição por arquivo `.toml`**, com metadados (`[licao]`) e uma lista `[[exercicio]]`. Formato descrito em [FORMATO-LICOES.md](../referencias/FORMATO-LICOES.md).
2. **Hierarquia Trilha → Lição → Exercício.** A Trilha é só um rótulo (`trilha = "Escalas"`) e uma ordem; não tem arquivo próprio.
3. **Gabarito em texto musical legível**: `passos = "C4 D4 E4 [C4 E4] {C4 E4 G4}"`. Nota com oitava é exata; nota sem oitava vale em qualquer oitava; `[...]` é conjunto exato tocado junto; `{...}` é acorde (qualquer inversão e dobra). Bemol `b`, sustenido `#`, oitava científica (Dó central = C4 = MIDI 60). O carregador recusa notas fora do teclado de 61 teclas (C2–C7).
4. **Fontes de conteúdo:** as Lições de fábrica ficam em `assets/licoes/` e são **embutidas no binário** (`include_str!`), então o app funciona sem arquivos soltos. Lições do usuário ficam em `<pasta de dados>/licoes/*.toml` e são somadas às de fábrica (mesmo `id` do usuário substitui o de fábrica).
5. **Revisão:** cada Lição declara `situacao = "proposta" | "aprovada"`. Conteúdo gerado por IA nasce `proposta` e a tela mostra isso; só o titular muda para `aprovada` (CONTEXT.md: Gabarito nunca vem de saída bruta de IA).
6. **Progresso** passa a ser chaveado por `licao/exercicio` (texto) em `progresso.txt`. Chaves antigas `exN.*` migram para `primeiros-passos/N`.
7. **Desbloqueio:** dentro da Lição, cada Exercício exige o anterior (salvo `requer = ""`). Lições ficam livres por padrão; `requer` na Lição pode exigir outras Lições concluídas.
8. **Script: não agora.** O arquivo reserva a chave `script`, mas o carregador a ignora com aviso. Quando houver regra pedagógica que o formato declarativo não exprime, a opção preferida é **Luau via `mlua` (feature `luau`, `vendored`)**, sandbox nativa e sem acesso a arquivo/rede, conforme o mandato do AGENTS.md ("Luau somente hipótese de regras pedagógicas após F13").

## Por quê

- TOML é legível e editável à mão, tem parser maduro em Rust (`toml` + `serde`, MIT/Apache-2.0) e já está na árvore de dependências.
- Todo exercício atual (localizar, sequência, intervalo, escala, acorde, arpejo, ouvir e repetir) cabe no formato declarativo; nada pede lógica.
- Script traria compilação C/C++ (`mlua` vendored), superfície de segurança e testes de sandbox antes de existir uma necessidade real; o AGENTS.md já adia isso para depois da F13.

## Consequências

- Novas Lições não exigem recompilar quando ficam na pasta de dados.
- Erros de arquivo (TOML inválido, nota inexistente, id repetido) viram mensagens na tela e a Lição é pulada; nunca pânico.
- A troca de `id: u8` por texto muda a API interna de `exercicios.rs` e os testes da F05.
