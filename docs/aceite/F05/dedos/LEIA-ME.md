# Dedo dentro das bolinhas do teclado (nomenclatura escolhível)

Capturas em **Xvfb 1440×900** (build release, OpenGL por software), **não** no KDE do cerebro,
com uma Lição de demonstração na pasta de dados temporária (`{D4 F#4 A4}`, dedilhado `1-3-5`).

- `bolinhas-tres-nomenclaturas.png`: a mesma tecla-guia em Números (1 3 5), Letras (P M N) e
  Inglês antigo (+ 2 4).
- `tela-exercicios-letras.png`: tela Exercícios com o seletor "Dedilhado" no Ribbon (Início).

Nomenclaturas (pesquisa, nov/2026):

| Opção | Polegar → mínimo | Fonte |
|---|---|---|
| Números (padrão) | 1 2 3 4 5 | padrão "continental", universal; Synthesia usa só ele |
| Letras pt-BR | P I M A N | convenção do app (P-I-M-A vem do violão; N evita repetir M e confundir "Mi" com a nota) |
| Inglês antigo | + 1 2 3 4 | "English fingering" do séc. XIX (Wikipedia: Fingering (music)) |

Regra do dedo por nota: o texto `dedilhado` da Lição precisa cobrir todas as notas do gabarito
(acorde do grave ao agudo) ou ser um padrão puro que se repete um número inteiro de vezes.
Nas Lições embutidas, 143 de 145 exercícios com dedilhado ganham dedo; os 2 restantes têm texto
explicativo e ficam sem dedo (melhor que dedo errado). Não testado com o teclado físico.

## O dedo é do componente de teclado (todas as telas)

O teclado (`EstadoEspelho::dedos_das_guias`, pintado em `teclado_visual`) resolve o dedo de
qualquer tecla-guia sozinho; as telas só passam as guias (`definir_guias`). Regras
(`dedos::inferir`):

- Acorde de 2 a 5 notas numa oitava: tríade pela inversão (direita 1-3-5 / 1-2-5 na 1ª;
  esquerda 5-3-1 / 5-2-1 na 2ª), tétrade 1-2-3-5 / 5-3-2-1, 5 notas 1-2-3-4-5.
- Escala de uma oitava: maior e menor (natural/harmônica) pela tabela tradicional das 12
  tônicas; cromática com 3 nas pretas. Pentatônicas e blues ficam sem dedo (não há padrão).
- Mão esquerda quando todas as guias estão abaixo do ponto de divisão do som; senão direita.
- Lição com dedilhado que cobre o gabarito sobrepõe o resolvido.

Capturas: `acordes-do-maior.png` (Dicionário, C: 1-3-5) e `escala-fa-maior-letras.png`
(Fá maior em letras: P I M A P I M A).
