# Componentes visuais: referências e identidade própria do Piano Tutor

Pesquisa de referências públicas para dois componentes: **teclado na tela** e **renderizador de partitura (MusicXML)**. Objetivo: ter repertório suficiente para criar uma identidade própria, sem copiar código nem assets.

## Regras de uso

- **Nenhum código, imagem, screenshot ou asset de terceiros entra no repositório.** As referências servem para entender convenções, não para copiar.
- Projetos **GPL / LGPL / AGPL** são **somente referência** (leitura de conceito). Não linkar, não importar, não copiar trechos.
- Projetos **MIT / BSD / Apache / MPL-2.0** só poderiam ser usados como dependência com o aviso de licença preservado. Mesmo assim, a recomendação é implementar nosso próprio renderer.
- Fontes de música (Bravura, Petaluma) são **OFL-1.1**: podem ser usadas com o aviso de licença junto. Não alteram o aspecto do componente de forma proibida, mas exigem o texto de licença no pacote.
- Limite de verificação: as informações abaixo vêm de páginas de repositório lidas em nov/2026. Pontos marcados com **(confirmar)** não foram verificados no código-fonte ou no arquivo de metadados.

## 1. Partitura (MusicXML)

### 1.1 OpenSheetMusicDisplay (OSMD)

- URL: https://github.com/opensheetmusicdisplay/opensheetmusicdisplay
- Licença: **BSD-3-Clause** (permissiva; reuso possível com aviso, mas não recomendado para nós por ser dependência de JS/VexFlow)
- Notável: lê MusicXML para um modelo de dados e renderiza via VexFlow. Gera SVG e PNG, inclusive headless. Suporta tablatura de violão e notação braille.
- Não copiar: a estrutura do modelo de dados em JS e a ordem de layout de sistemas/páginas (acoplada ao VexFlow).

### 1.2 VexFlow

- URL: https://github.com/0xfe/vexflow
- Licença: **MIT**
- Notável: API de desenho de notação em Canvas e SVG. Boa referência de primitivas (stave, voice, beam, tie) e de como separar formatação de desenho.
- Não copiar: o acoplamento de layout por "formatter" e os nomes de métodos. A fonte de engraving usada não está clara na página (confirmar no código).

### 1.3 Verovio

- URL: https://github.com/rism-digital/verovio
- Licença: **LGPL** segundo o README (o repositório também lista GPL-3.0). **Somente referência.**
- Notável: engraving baseado em SMuFL, com fonte trocável (Leipzig/Bravura são as opções típicas). Converte MusicXML, MEI, ABC e outros. É um dos renderers de referência para partitura de qualidade editorial.
- Não copiar: o pipeline MEI e a escala de unidades internas (confirmar nos fontes).

### 1.4 MuseScore

- URL: https://github.com/musescore/MuseScore
- Licença: **GPL-3.0**. **Somente referência.**
- Notável: é o caso de referência de engraving de software de uso geral. Padrões de espaçamento, quebra de linha e fonte padrão (Leland ou Bravura; confirmar na pasta `fonts`).
- Não copiar: nada do código de layout. Observar apenas o resultado visual (espaçamento, hastes, vigas).

### 1.5 LilyPond

- URL: https://github.com/lilypond/lilypond
- Licença: **GPL-3.0** (fontes Feta/Emmentaler sob OFL-1.1; documentação sob GFDL-1.3). **Somente referência.**
- Notável: a referência de "gravura clássica" de partitura. O próprio site descreve a estética como inspirada em gravura manual, com espaçamento equilibrado. É o alvo de qualidade para vigas, hastes e colisões de notas.
- Não copiar: nenhuma fonte ou regra de código. Usar a estética como critério de aceitação visual.

### 1.6 alphaTab

- URL: https://github.com/CoderLine/alphaTab
- Licença: **MPL-2.0** (fontes Bravura e Petaluma sob OFL-1.1; SONiVOX SoundFont sob Apache-2.0)
- Notável: renderiza notação tradicional e tablatura, em layouts de página, horizontal e "pergaminho", com reflow ao redimensionar. Usa fonte SMuFL trocável. Cores por elemento configuráveis.
- Atenção: o projeto exige que a marca "alphaTab.net" fique visível nas partituras, salvo autorização. **Não copiar esse rótulo nem o padrão de marca.**
- Não copiar: a estrutura de componentes e o sistema de cores por elemento como API.

### 1.7 Bravura e SMuFL (especificação)

- Bravura: https://github.com/steinbergmedia/bravura, licença **SIL OFL-1.1**. Fonte de referência do padrão SMuFL.
- SMuFL: https://www.smufl.org/ (especificação mantida pelo W3C Music Notation Community Group, iniciada pela Steinberg). Mapeia símbolos musicais para a Private Use Area do Unicode, com nomes de glifo fixos e uma unidade de medida chamada **espaço de pauta (staff space, "sp")**.
- Notável: toda a régua visual do componente pode ser expressa em sp, o que deixa a partitura proporcional a qualquer tamanho de tela.
- Não copiar: nada (é padrão aberto). Usar o padrão de metadados (`engravingDefaults`) como fonte dos números de referência.
- Não consegui baixar o arquivo `bravura_metadata.json` (404 nas duas tentativas). Os valores da seção 3 vêm de convenções de engraving e do padrão. **(confirmar no metadata antes de fixar)**.

### 1.8 Verificação de tecnologia alternativa: egui (Rust)

- egui: https://github.com/emilk/egui, licença **MIT ou Apache-2.0**. Uso permitido como dependência, mas a licença exige aviso junto.
- Notável para nós: o `epaint` desenha linhas, círculos, texto, polígonos convexos e curvas de Bézier cúbicas e quadráticas. **Não está claro se cobre caminhos côncavos gerais**, então para glifos e vigas com formato livre confirme antes de depender disso.
- Pergunta em aberto: o Piano Tutor já usa egui no crate `piano-tutor-app`? (conferir o Cargo.toml antes de decidir)

## 2. Teclado na tela e aprendizado de piano

### 2.1 Neothesia

- URL: https://github.com/PolyMeilex/Neothesia
- Licença: **GPL-3.0**. **Somente referência.**
- Stack: Rust com **wgpu**.
- Notável: mostra notas como **blocos coloridos caindo** sobre um piano virtual, no estilo "Synthesia". Cada faixa ou mão tem cor própria.
- Não copiar: o renderer wgpu, shaders, paleta e layout dos blocos. Pela licença, nada de código.

### 2.2 PianoBooster

- URL: https://github.com/pianobooster/PianoBooster (confirmada por busca; a página do repositório deu 404 na leitura direta, então o detalhe visual não foi verificado)
- Licença: **GPL** (fontes conflitantes: o README cita GPL v3, o SourceForge lista GPLv2; Debian lista GPL-3+). **Somente referência.**
- Notável: é um player de MIDI que mostra a partitura e ensina a tocar. Acompanha um teclado MIDI real e espera a nota certa antes de avançar. É o modelo mais próximo da proposta do Piano Tutor no aspecto pedagógico.
- Não copiar: nada. Observar o fluxo "espera e avança" e a forma de marcar notas erradas.

### 2.3 Synthesia (comercial, referência de mercado)

- Produto proprietário, sem código aberto. Não há licença para reuso.
- Notável: a convenção visual de "notas caindo" é de domínio público, mas o visual específico (cores, brilho, animação de tecla) é a identidade do produto. Não copiar cores, sons ou animações.

### 2.4 O que o mercado repete (convenções livres de uso)

- Teclas brancas e pretas em alturas diferentes, com tecla preta mais curta e mais escura.
- Tecla pressionada muda de cor e/ou desce um pouco.
- Notas caindo com comprimento proporcional à duração.
- Cor por mão ou por faixa.

Essas convenções são práticas gerais de interface de piano. Podemos usá-las, mas precisamos de um tratamento próprio (seção de identidade).

## 3. Regras de engraving em unidades de espaço de pauta (sp)

Valores de referência para a implementação. O símbolo **(confirmar)** indica que o número vem de convenção e precisa ser checado no `bravura_metadata.json`.

| Elemento | Valor de referência | Observação |
|---|---|---|
| Espessura da linha de pauta | ~0,13 sp (confirmar) | Linhas finas e uniformes |
| Espessura da haste | ~0,12 sp (confirmar) | Um pouco mais fina que a linha de pauta |
| Comprimento da haste | **3,5 sp** | Convenção padrão para notas dentro da pauta; haste para cima se a nota está abaixo da 3ª linha, para baixo se está acima |
| Espessura da viga | ~0,5 sp (confirmar) | Viga é a linha grossa que une colcheias |
| Espaço entre vigas | ~0,25 sp (confirmar) | Distância entre vigas de nível 2 e 3 |
| Espessura da linha suplementar | ~0,16 sp (confirmar) | Linha suplementar para notas fora da pauta |
| Extensão da linha suplementar | ~0,4 sp além da cabeça da nota (confirmar) | Cada lado |
| Inclinação da viga | Máximo de ~1 sp de desnível no grupo | Proposta nossa: evita vigas "esticadas" |
| Espaçamento horizontal | Proporcional ao log da duração | Notas mais longas recebem mais espaço, mas não proporcional linear, para não ficar largo demais |

Regras que o renderer deve respeitar desde o início:

1. Haste sempre 3,5 sp, com a direção definida pela posição em relação à 3ª linha.
2. Vigas agrupam por tempo (pulso), não só por nota adjacente.
3. Ligadura de expressão e ligadura de nota ficam em curvas distintas, com espessura diferente.
4. Acidentes não se sobrepõem entre si; a coluna de acidentes tem espaçamento mínimo fixo.

## 4. Identidade própria do Piano Tutor

Proposta para os agentes que vão construir os componentes. Todos os valores de cor são **tokens** em `:root`, com redefinição para os temas **Noite**, **Claro** e **Ébano**.

### 4.1 Paleta (tokens)

| Token | Noite | Claro | Ébano | Uso |
|---|---|---|---|---|
| `--fundo` | #14161C | #F6F3EC | #0B0A09 | Fundo da tela |
| `--superficie` | #1E2129 | #FFFFFF | #171512 | Painéis |
| `--tinta` | #E8E4DA | #1D1B17 | #E9DCC4 | Notação, rótulos |
| `--tinta-suave` | #8C8A84 | #6B675F | #8A7D68 | Linhas auxiliares, texto secundário |
| `--mao-direita` | #E2A24B | #B5651D | #E2A24B | Cor da mão direita (âmbar) |
| `--mao-esquerda` | #4FA3C7 | #1F6F8B | #6FB6D4 | Cor da mão esquerda (azul) |
| `--acerto` | #6FCF97 | #2E7D4F | #7FD1A0 | Nota certa |
| `--erro` | #E8706A | #B3261E | #E8706A | Nota errada (sempre com forma ou texto, não só cor) |

Âmbar e azul foram escolhidos por se distinguirem em protanopia e deuteranopia. Mesmo assim, **toda diferença de mão também tem uma marca de forma**: a mão direita usa haste/nota cheia, a esquerda usa contorno, ou uma etiqueta "D"/"E" pequena (a definir).

### 4.2 Teclado

- **Material:** teclas brancas em marfim fosco (nunca brilhante), com um gradiente de 2% entre topo e base. Teclas pretas em grafite com um filete superior mais claro (simula a borda chanfrada).
- **Sombra:** sombra suave só sob o teclado, sem sombra entre as teclas, para que o conjunto pareça um bloco único.
- **Tecla pressionada:** desce 2 px (escala Y de 0,98 com transição de 60 ms) e recebe a cor da mão em 40% de opacidade sobre a tecla. Sem brilho de neon.
- **Tecla-guia (a próxima a tocar):** contorno de 2 px na cor da mão, com pulso lento (1,2 s). Pulso desligável nas preferências.
- **Tecla errada:** flash curto em `--erro` com o texto da nota em cima da tecla.
- **Rótulos:** nome da nota só nas teclas-guia, pequeno, em `--tinta-suave`. Nunca em todas as teclas.

### 4.3 Partitura

- **Unidade:** tudo em sp. Escala global ajusta o sp em pixels, sem mudar proporções.
- **Pauta:** linhas de 0,13 sp em `--tinta`. Espaço entre linhas = 1 sp.
- **Hastes:** 3,5 sp, espessura 0,12 sp, na cor `--tinta`.
- **Vigas:** 0,5 sp de espessura, sem inclinação acima de ~1 sp por grupo.
- **Cabeças de nota:** elipse levemente inclinada (como em gravura clássica), sem contorno. Cheia para semínimas e mais curtas, oca para mínimas e semibreves.
- **Espaçamento:** log da duração (seção 3), com espaço mínimo fixo entre colunas.
- **Cursor de leitura:** faixa vertical de `--mao-direita` a 15% de opacidade, sem contorno.
- **Cor de nota certa/errada:** a cabeça da nota muda para `--acerto` ou `--erro`, e a haste acompanha. Sempre acompanhada de um símbolo (✓ / ✗) pequeno quando couber.

### 4.4 Tipografia

- **Notação:** Bravura (OFL-1.1), fonte SMuFL de referência, para ter glifos padronizados e licença clara. Incluir o arquivo de licença no pacote.
- **Interface e rótulos:** fonte de texto aberta e sem serifa, com licença OFL ou Apache (escolha a confirmar, como Inter ou Noto Sans). Pesos de 400 e 600 apenas.
- **Tamanhos:** rótulo de tecla 11 px; título de lição 18 px; nenhum texto menor que 11 px.
- **Não usar:** fontes de sistema que mudam de aparência entre Windows e Linux para a notação.

### 4.5 Regras de engraving a implementar primeiro

1. Haste de 3,5 sp com direção pela 3ª linha.
2. Vigas por pulso, não por adjacência simples.
3. Linhas suplementares com extensão de 0,4 sp.
4. Espaçamento horizontal proporcional ao log da duração, com mínimo fixo.
5. Acidentes em coluna fixa, sem sobreposição.
6. Espessura das linhas de pauta uniforme em qualquer escala.
7. Cabeça de nota oca para mínimas e semibreves, cheia para o resto.
8. Reflow de sistemas com quebra de linha em compasso inteiro, nunca no meio de um compasso.

Ordem sugerida de implementação: 1, 3, 7, 4, 2, 6, 5, 8.

## 5. Próximos passos

- Confirmar os valores marcados **(confirmar)** no `bravura_metadata.json` antes de fixar as constantes.
- Verificar no `Cargo.toml` se egui já é a base do app. Se sim, o teclado e a partitura devem ser desenhados com o painter do egui, sem dependência nova.
- Não adotar código de nenhum projeto da seção 1 ou 2 como base. Usar apenas as regras e o critério visual.
