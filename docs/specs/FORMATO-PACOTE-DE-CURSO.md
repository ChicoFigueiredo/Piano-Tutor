# Formato do Pacote de Curso — versão 1

Decisão: [ADR-0009](../decisoes/ADR-0009-cursos-de-tutor-como-pacote.md). Vocabulário: [CONTEXT.md](../../CONTEXT.md). Piloto: [PILOTO-CURSO-LUCIANO.md](../planejamento/PILOTO-CURSO-LUCIANO.md).

Um Pacote de Curso é **só dados**: o app nunca executa nada que venha dele. Este documento é o contrato entre o empacotador (CLI `piano-tutor-curso`), o importador, o alinhador e o leitor no app.

## Arquivo

- Extensão `.ptcurso`; contêiner **zip** (store ou deflate), caminhos relativos com `/`, sem `..`, sem caminhos absolutos, sem links simbólicos.
- Conteúdo:

| Caminho | Obrigatório | O quê |
|---|---|---|
| `manifesto.toml` | sim | Curso → Seções → Lições → Exercícios, autor, licenciado e a lista de arquivos com SHA-256 |
| `assinatura.ed25519` | sim | 64 bytes: assinatura ed25519 dos bytes exatos de `manifesto.toml` |
| `partituras/<id>.musicxml` | por Exercício | MusicXML 4.0 descomprimido |
| `audio/<id>.opus` | opcional | trecho tocado pelo tutor (Ogg Opus, mono, 48 kHz) |
| `alinhamentos/<id>.json` | opcional | Alinhamento por nota (abaixo) |
| `roteiro.toml` | opcional | falas do professor (abaixo) |

Qualquer arquivo no zip que não esteja listado em `[[arquivos]]` do manifesto, ou cujo SHA-256 não confira, **invalida** o pacote. Limites de leitura: 200 MB por pacote, 50 MB por arquivo, 2000 arquivos.

## Assinatura e confiança

- Chave ed25519. O manifesto traz `assinatura.chave_publica` (32 bytes em hex) e o app confere a assinatura com ela.
- **Autenticidade ≠ confiança**: o app mostra "assinado por <autor> (chave `abcd…1234`)" e marca **confiável** só se a chave estiver em `<dados>/cursos/chaves-confiaveis/*.pub` (hex, uma por arquivo) ou entre as chaves públicas embutidas no app. Pacote com assinatura inválida não é instalado.
- Chave privada **nunca** no repositório (no cerebro: `~/.config/piano-tutor/chaves/`).

## `manifesto.toml`

```toml
formato_versao = 1
id = "luciano-alves.piano-1"          # [a-z0-9.-], único
titulo = "Curso de Piano 1"
versao = "0.1.0"                        # do pacote
gerado_em = "2026-11-24T23:00:00Z"
idioma = "pt-BR"
convencao_oitava = "do3-central"        # o autor chama o Dó central de Dó3 (MIDI 60)

[autor]
nome = "Luciano Alves"
[licenciado]
nome = "Chico"                           # a quem o pacote foi emitido
licenca = "Uso pessoal. Redistribuição só com licença escrita do autor."

[assinatura]
chave_publica = "<64 hex>"

[[secoes]]
id = "s03"
numero = 3
titulo = "Mão direita"

  [[secoes.licoes]]
  id = "s03-a14"
  numero = 14                            # número da Aula no curso do autor
  titulo = "Música (Cinco notas no piano)"
  tipo = "exercicio"                     # "assistir" | "exercicio"
  [secoes.licoes.aula]                   # o vídeo NÃO entra no pacote
  nome_arquivo = "14 - Música (Cinco notas no piano).mp4"   # sugestão para o aluno localizar o próprio arquivo
  duracao_s = 312.4
  sha256 = "<hex>"                       # opcional: confere se o arquivo do aluno é o mesmo

    [[secoes.licoes.exercicios]]
    id = "cinco-notas"
    titulo = "Cinco notas no piano"
    modo = "praticar"                    # "praticar" | "chamada_e_resposta"
    mao = "direita"                      # "direita" | "esquerda" | "ambas"
    estado = "hipotese"                  # "hipotese" | "em_revisao" | "aprovada" | "rejeitada"
    partitura = "partituras/cinco-notas.musicxml"
    audio = "audio/cinco-notas.opus"     # opcional
    alinhamento = "alinhamentos/cinco-notas.json"   # opcional
    trecho_aula = { inicio_s = 95.0, fim_s = 140.5 }   # onde o tutor toca, no vídeo
    bpm = 80                             # opcional: andamento sugerido

[[arquivos]]
caminho = "partituras/cinco-notas.musicxml"
sha256 = "<hex>"
bytes = 4210
```

Regras do leitor:
- `formato_versao` maior que o suportado → erro claro ("pacote feito para versão mais nova do app").
- Só Exercício `aprovada` vira Gabarito. `hipotese`/`em_revisao` aparecem apenas na tela de revisão (modo tutor); `rejeitada` nunca aparece para o aluno.
- Lição `assistir` não tem Exercícios.

## `alinhamentos/<id>.json`

```json
{
  "formato_versao": 1,
  "exercicio": "cinco-notas",
  "audio": "audio/cinco-notas.opus",
  "estado": "hipotese",
  "metodo": "dtw-croma",
  "offset_aula_s": 95.0,
  "notas": [
    { "indice": 0, "compasso": 1, "midi": 60, "t_partitura_s": 0.0, "t_audio_s": 0.42, "confianca": 0.81 }
  ]
}
```

`t_audio_s` é relativo ao início de `audio`; o instante no vídeo é `offset_aula_s + t_audio_s`. `indice` segue a ordem de `piano_tutor_core::comparador::esperados_da_partitura`.

## `roteiro.toml`

```toml
formato_versao = 1

[[falas]]
id = "s03-a14-intro"
licao = "s03-a14"
exercicio = "cinco-notas"     # opcional
momento = "antes"             # "antes" | "passo" | "acerto" | "erro" | "depois"
passo = 0                     # só com momento = "passo"
texto = "Vamos tocar as cinco notas, de Dó a Sol, com a mão direita."
estado = "hipotese"           # só "aprovada" é dita pelo professor
origem = { aula = 14, inicio_s = 12.3, fim_s = 18.0 }   # trecho da fala da Aula de onde veio
```

O Curso de fábrica usa o mesmo `roteiro.toml` (embutido no app, falas já aprovadas).

## Instalação e progresso

- Instalar = verificar tudo e extrair para `<dados>/cursos/<id>/` (substitui versão anterior só se a nova verificar).
- Progresso do Curso de tutor: `<dados>/cursos/progresso/<id>.txt`, separado do progresso do Curso de fábrica.
- Oficina do tutor (fora do pacote, só no cerebro): `~/.local/share/piano-tutor/fontes/<tutor>/` com PDFs renderizados, Hipóteses, áudio extraído e estados de revisão.
