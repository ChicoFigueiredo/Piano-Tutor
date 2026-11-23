# Formato de arquivo de Lição (`.toml`)

Decisão: [ADR-0009](../decisoes/ADR-0009-licoes-em-arquivos-toml.md). Uma Lição por arquivo. Lições de fábrica: `assets/licoes/<trilha>/<id>.toml`. Lições do usuário: `<pasta de dados>/licoes/*.toml`.

## Exemplo

```toml
[licao]
id = "escalas-maiores-sustenidos"     # [a-z0-9-], único em todo o app
trilha = "Escalas"                     # Trilha (agrupamento na tela)
ordem_trilha = 30                      # ordem da Trilha na tela (menor primeiro); vale a primeira Lição lida
ordem = 2                              # ordem da Lição dentro da Trilha
titulo = "Escalas maiores com sustenidos"
descricao = "Sol, Ré, Lá e Mi maior, uma oitava, mão direita."
nivel = "iniciante"                    # iniciante | basico | intermediario
situacao = "proposta"                  # proposta | aprovada (só o titular aprova)
fontes = ["Ciclo das quintas: teoria tradicional", "https://www.musictheory.net/lessons/24"]
requer = []                            # ids de Lições que precisam estar concluídas (opcional)

[[exercicio]]
id = "sol-maior"                       # [a-z0-9-], único dentro da Lição
titulo = "Sol maior subindo"
tipo = "escala"                        # localizar | sequencia | intervalos | escala | acorde | arpejo | ouvir
mao = "direita"                        # direita | esquerda | ambas (informativo)
objetivo = "Tocar a escala de Sol maior, uma oitava."
instrucoes = "Toque G A B C D E F# G. O Fá é sustenido (tecla preta)."
dedilhado = "1-2-3-1-2-3-4-5"          # opcional
passos = "G4 A4 B4 C5 D5 E5 F#5 G5"
roteiro = [                            # opcional, recomendado: 3 a 6 passos curtos do "como fazer"
  "Apoie o polegar direito no Sol logo acima do Dó central.",
  "Toque Sol, Lá e Si com os dedos 1, 2 e 3.",
  "Passe o polegar por baixo até o Dó e siga com 2, 3 e 4.",
  "Termine no Sol com o dedo 5, sem pressa e sem pular o Fá#.",
]
acerto = "Degrau certo."               # opcional (há padrão por tipo)
erro = "Lembre do Fá sustenido."       # opcional
conclusao = "Sol maior completa."      # opcional
# requer = ""                          # opcional: "" libera; omitido = exige o exercício anterior
```

## Notação de `passos`

Tokens separados por espaço, na ordem exigida.

| Token | Significado | Exemplo |
|---|---|---|
| `C4` | nota exata (oitava científica; C4 = Dó central = MIDI 60) | `F#3`, `Bb4`, `Eb5` |
| `C` | a nota em qualquer oitava | `localizar` todos os Dós |
| `[C4 E4]` | conjunto exato, tocado junto (intervalo) | `[C4 G4]` |
| `{C4 E4 G4}` | acorde: qualquer inversão e dobra de oitava | `{A3 C4 E4}` |
| `{C4 E4 G4}:Dó maior` | legenda opcional depois de `:` (sem espaço) | `[C4 E4]:terça_maior` (`_` vira espaço) |

Regras: nomes `C D E F G A B`, acidentes `#` ou `b` (um só), oitavas 2 a 7, faixa do teclado C2 (36) a C7 (96). Dentro de `[...]` e `{...}` as notas são separadas por espaço. Uma Lição com erro é pulada inteira e o erro aparece na tela.

## Tipos

- `localizar`, `sequencia`, `escala`, `arpejo`: uma nota por passo, em ordem.
- `intervalos`: passos `[...]`.
- `acorde`: passos `{...}`.
- `ouvir`: o app toca a frase e as teclas-guia ficam escondidas (a dica as mostra).

## Direitos

Só entram melodias em domínio público (folclóricas, compositores mortos há mais de 70 anos) e padrões técnicos genéricos (escalas, arpejos, cinco dedos, cadências). Nada de transcrever método com direito autoral (Faber, Alfred, Bastien etc.). Cite a fonte em `fontes`.
