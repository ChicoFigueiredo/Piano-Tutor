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

Qualquer arquivo no zip que não esteja listado em `[[arquivos]]` do manifesto, ou cujo SHA-256 não confira, **invalida** o pacote. Limites de leitura: 200 MB por pacote, 50 MB por arquivo, 2000 arquivos e 8 MB por documento de texto (manifesto, Roteiro, Alinhamento). Entradas de pasta no zip são ignoradas; nome de entrada repetido, link simbólico ou nome que não seja UTF-8 invalidam o pacote.

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
- A ordem de verificação é: zip seguro → `formato_versao` → assinatura (sobre os bytes exatos do manifesto, antes de interpretar o resto) → estrutura → arquivos (nenhum a mais, nenhum a menos, tamanho e SHA-256) → Roteiro e Alinhamentos.
- `formato_versao` maior que o suportado → erro claro ("pacote feito para versão mais nova do app").
- Só Exercício `aprovada` vira Gabarito oficial. `hipotese`/`em_revisao` aparecem na tela de revisão (modo tutor) e, para o aluno, **só com a preferência "Usar conteúdo não revisado"** (ver "Uso pessoal de conteúdo não revisado" abaixo); `rejeitada` nunca aparece para o aluno.
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

`t_audio_s` é relativo ao início de `audio`; o instante no vídeo é `offset_aula_s + t_audio_s`. `indice` segue a ordem de `piano_tutor_core::comparador::esperados_da_partitura`. **Uníssonos são fundidos antes da numeração:** quando as duas mãos tocam a mesma altura MIDI no mesmo instante (tolerância de 1 ms), há um só evento esperado (com a maior duração; ficam `parte`/`pauta` do de cima), porque o aluno só tem uma tecla. O `indice` conta essa lista já fundida; acordes de notas diferentes mantêm todas as notas. Pacotes gerados antes dessa regra, de partituras com uníssono entre pautas, devem ser refeitos (`indice` deslocado).

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

O Curso de fábrica usa o mesmo `roteiro.toml` (embutido no app; suas falas nascem como hipótese até o revisor aprová-las).

**Momentos.** `antes` é dito ao abrir o Exercício; `passo` ao chegar à nota de índice `passo`; `depois` ao terminar. `acerto` e `erro` são comentários de **trecho**: o professor os diz **ao fim de um trecho, nunca a cada nota** (a nota isolada só tem o retorno visual verde/vermelho, sem fala). Trecho sem nota errada ou faltante → `acerto`; trecho com erro → `erro` (por exemplo, "boa, agora de novo mais devagar"). O que é um trecho:

- **Exercício de partitura**: uma frase. As falas `passo` do Exercício marcam o começo de cada frase (a nota `passo` abre a frase seguinte); sem falas `passo`, a cada 2 compassos. O último trecho fecha ao concluir a prática e só conta como `acerto` se também bateu o limiar de conclusão (80 %). O comentário do trecho vem antes da fala `passo` da frase seguinte e antes do `depois`.
- **Exercício de fábrica**: a tentativa inteira, ao concluir (os exercícios têm de 1 a 10 passos; não há comentário intermediário).
- **Chamada e resposta**: cada chamada é um trecho e já é avaliada ao fim dela.

O professor não repete a mesma fala em seguida (havendo outra do mesmo momento). Por padrão só diz falas `aprovada`; com "Usar conteúdo não revisado" também diz `hipotese` e `em_revisao` (nunca `rejeitada`), e havendo uma aprovada para o mesmo momento ela tem prioridade sobre as não revisadas.

## Instalação e progresso

- Instalar = verificar tudo e extrair para `<dados>/cursos/<id>/` (substitui versão anterior só se a nova verificar).
- Progresso do Curso de tutor: `<dados>/cursos/progresso/<id>.txt`, separado do progresso do Curso de fábrica.
- Oficina do tutor (fora do pacote, só no cerebro): `~/.local/share/piano-tutor/fontes/<tutor>/` com PDFs renderizados, Hipóteses, áudio extraído e estados de revisão.

## Regras de validação (leitor v1)

Decisões que o contrato deixava em aberto, fixadas na implementação (`crates/piano-tutor-curso`):

- **Identificadores** (`id` do Curso, da Seção, da Lição, do Exercício e da fala): `[a-z0-9.-]`, começando por letra minúscula ou número, até 100 caracteres, únicos no pacote dentro de cada tipo. O `id` do Curso vira nome de pasta, por isso `chaves-confiaveis` e `progresso` (pastas reservadas de `<dados>/cursos/`) são proibidos.
- **Caminhos** (em `[[arquivos]]`, `partitura`, `audio`, `alinhamento` e nas entradas do zip): relativos, separados por `/`, sem componente vazio, `.` ou `..`, sem `\`, sem `:` e sem caracteres de controle, até 240 caracteres. `manifesto.toml` e `assinatura.ed25519` nunca constam em `[[arquivos]]`.
- `partitura`, `audio` e `alinhamento` de cada Exercício precisam constar em `[[arquivos]]`; `sha256` é hexadecimal minúsculo de 64 dígitos.
- Lição `assistir` não pode ter Exercícios. `aula.nome_arquivo` é só o nome do arquivo (sem `/` nem `\`). `bpm` fica entre 1 e 1000; `trecho_aula` exige `0 <= inicio_s < fim_s`.
- **Roteiro**: `licao` precisa existir; `exercicio`, se houver, precisa pertencer a essa Lição; `passo` é obrigatório com `momento = "passo"` e proibido nos outros; `texto` não pode ser vazio.
- **Alinhamento**: `exercicio` e `audio` precisam bater com o Exercício que o cita; `indice` estritamente crescente; `midi` em 0..=127; `confianca` em 0..=1; tempos finitos e não negativos.
- **Estados**: só `aprovada` serve de Gabarito (`Exercicio::serve_de_gabarito`); `exercicios_para_aluno` exclui `hipotese`, `em_revisao` e `rejeitada`. O mesmo vale para as falas do Roteiro (`falas_aprovadas`). Essas APIs são estritas e não mudam. Para o uso pessoal do licenciado há APIs explícitas: `Exercicio::utilizavel(permitir_nao_revisado)`, `Manifesto::exercicios_para_aluno_com(permitir)` (e a de `PacoteVerificado`), `Fala::utilizavel(permitir)` e `Roteiro::falas_utilizaveis(permitir)`; com `permitir = false` equivalem às estritas, com `true` aceitam também `hipotese` e `em_revisao`, nunca `rejeitada`.

### Uso pessoal de conteúdo não revisado

O app tem a preferência **"Usar conteúdo não revisado"** (padrão **ligada**; gravada em `preferencias-professor.txt`, chave `nao_revisado`, junto da de "Professor fala"). Ligada, o aluno vê e abre Exercícios `hipotese`/`em_revisao`, com o selo "[!] não revisado" (símbolo e texto, nunca só cor) ao lado do título, e o professor diz também as falas dessas etapas, com o mesmo selo na legenda. Cada Lição mostra "N não revisados". Desligada, vale o comportamento estrito (só `aprovada`) e a tela mostra "N exercício(s) aguardando revisão do tutor". A revisão continua como fila (Aprovar, Rejeitar, "Aprovar direto"); aprovar remove o selo. Isto não muda o Gabarito: **o Gabarito oficial continua sendo só o aprovado**.
- **Empacotador**: lê `manifesto.toml` da pasta de origem (sem `[assinatura]` nem `[[arquivos]]`), inclui todos os arquivos da pasta exceto o manifesto, `assinatura.ed25519` e nomes que começam por ponto, grava a chave pública da chave privada em `[assinatura]`, regrava o manifesto (comentários e formatação da origem se perdem; esses bytes é que são assinados) e valida tudo antes de assinar. A saída é determinística para a mesma origem e chave.
- **Instalação**: extrai para uma pasta temporária oculta ao lado e troca por `rename`; se o curso já instalado estiver íntegro e tiver sido assinado por **outra chave**, a instalação é recusada (um autor não sobrescreve o curso de outro; para trocar de autor, remova a pasta do curso).
- **Chaves confiáveis**: lista embutida no app (vazia por enquanto) mais `<dados>/cursos/chaves-confiaveis/*.pub`; arquivos `.pub` ilegíveis são ignorados.

## Uso da CLI

```
piano-tutor-curso chave-gerar <pasta-saida>
piano-tutor-curso empacotar <pasta-origem> --chave <arquivo> -o <saida.ptcurso>
piano-tutor-curso verificar <arquivo.ptcurso> [--confiaveis <pasta>] [--dados <pasta>]
piano-tutor-curso instalar <arquivo.ptcurso> [--dados <pasta>]
piano-tutor-curso listar [--dados <pasta>]
piano-tutor-curso importar --plano <plano.toml> --oficina <pasta> -o <pasta-origem> [--somente-aprovadas]
```

- `chave-gerar` grava `curso.chave` (privada, hexadecimal, permissão 0600 no Unix, nunca sobrescreve) e `curso.pub` (pública). Guarde a privada fora do repositório (no cerebro: `~/.config/piano-tutor/chaves/`).
- `empacotar` usa a pasta de origem descrita acima e grava o `.ptcurso`.
- `verificar` confere assinatura, estrutura e hashes e imprime curso, autor, licenciado, chave curta (`abcd…1234`), confiança (confiável ou desconhecida) e a contagem de Seções, Lições e Exercícios por estado. Sem `--confiaveis`, usa as chaves de `<dados>/cursos/chaves-confiaveis/`.
- `instalar` verifica e extrai para `<dados>/cursos/<id>/`; avisa se a chave não for confiável (a instalação é feita mesmo assim, pois autenticidade não é confiança).
- `listar` mostra os cursos instalados, com a chave e se ela é confiável.
- `--dados` padrão: `<pasta de dados do usuário>/piano-tutor` (no Linux, `~/.local/share/piano-tutor`).
- Códigos de saída: 0 sucesso, 1 falha (pacote inválido, arquivo ilegível), 2 uso incorreto.

## Plano de importação

Entrada de `piano-tutor-curso importar`. É um TOML escrito à mão, fora do pacote e fora do git; descreve a estrutura do Curso. O importador gera uma **pasta-origem** para `empacotar` a partir dele e da oficina.

```toml
formato_versao = 1
id = "luciano-alves.piano-1"
titulo = "Curso de Piano 1"
versao = "0.1.0"
idioma = "pt-BR"
convencao_oitava = "do3-central"

[autor]
nome = "Luciano Alves"

[licenciado]
nome = "Chico"
licenca = "Uso pessoal. Redistribuição só com licença escrita do autor."

[[secoes]]
id = "s03"
numero = 3
titulo = "Mão direita"

  [[secoes.licoes]]
  id = "s03-a14"
  numero = 14
  titulo = "Música (Cinco notas no piano)"
  tipo = "exercicio"                    # "assistir" | "exercicio"

    [secoes.licoes.aula]
    nome_arquivo = "14 - Música (Cinco notas no piano).mp4"
    duracao_s = 248.427
    sha256 = "<hex>"                    # opcional

    [[secoes.licoes.exercicios]]
    id = "cinco-notas"
    titulo = "Cinco notas no piano"
    modo = "praticar"                   # "praticar" | "chamada_e_resposta"
    mao = "direita"                     # "direita" | "esquerda" | "ambas"
    bpm = 80                            # opcional; sem ele, vale o bpm_sugerido do estado.toml
```

Da **oficina** vêm, por Exercício `<id>`:

| Arquivo | Obrigatório | Uso |
|---|---|---|
| `partituras/<id>.musicxml` | sim (exceto Exercício omitido por `--somente-aprovadas`) | vai para `partituras/<id>.musicxml` |
| `partituras/<id>.estado.toml` | não | `estado` (`hipotese` \| `em_revisao` \| `aprovada` \| `rejeitada`; ausente = `hipotese`), `trecho_aula = { inicio_s, fim_s }`, `bpm_sugerido`; os demais campos (proveniência, confiança, notas) são ignorados |
| `audio/<id>.opus` | não | vai para `audio/<id>.opus` e vira `audio` do Exercício |
| `alinhamentos/<id>.json` | não | vai para `alinhamentos/<id>.json` e vira `alinhamento` do Exercício |

Na raiz da oficina, `roteiro.toml` é opcional e vai para a raiz da pasta-origem.

Regras do importador:

- O estado do Exercício é **sempre** o do `estado.toml`; o importador nunca promove conteúdo. Sem `estado.toml`, entra como `hipotese` e isso vai para os avisos.
- Arquivo opcional ausente só deixa de entrar, com aviso. Exercício sem partitura é erro, com o caminho esperado.
- A pasta-origem tem `manifesto.toml` sem `[assinatura]` nem `[[arquivos]]`, o que `empacotar` preenche.
- `gerado_em` é o instante da importação (UTC). `bpm` do plano tem precedência sobre `bpm_sugerido`.
- `roteiro.toml` é conferido com as mesmas regras do leitor (Lições e Exercícios citados precisam existir) antes de gravar.
- `--somente-aprovadas`: omite Exercícios que não são `aprovada`, falas que não são `aprovada` e falas ligadas a um Exercício omitido (sem isso o Roteiro não valida).
- O destino (`-o`) não pode existir com conteúdo. Tudo é lido e conferido antes da primeira gravação; uma falha não deixa pasta-origem pela metade.
- Identificadores são validados antes de montar qualquer caminho da oficina.
- O áudio de aula inteira (`audio-aula/*.wav` da oficina) não entra: o importador só copia `audio/<id>.opus`, por Exercício.

Uso: `piano-tutor-curso importar --plano <plano.toml> --oficina <pasta> -o <pasta-origem> [--somente-aprovadas]`. Ao final imprime Lições, Exercícios por estado, arquivos copiados e avisos.
