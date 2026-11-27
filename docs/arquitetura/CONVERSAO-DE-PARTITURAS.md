# Como as partituras do curso viraram partituras do Piano Tutor

Este documento explica como os PDFs do *Curso de Piano Completo* do Luciano Alves (Udemy) foram convertidos no formato que o Piano Tutor toca, avalia e acompanha com áudio. Ele cobre a tecnologia, a estratégia de montagem e o passo a passo, do PDF até o Pacote de Curso instalado.

Decisões de origem: [ADR-0009](../decisoes/ADR-0009-cursos-de-tutor-como-pacote.md) e [plano do piloto](../planejamento/PILOTO-CURSO-LUCIANO.md). Contrato dos arquivos: [FORMATO-PACOTE-DE-CURSO.md](../specs/FORMATO-PACOTE-DE-CURSO.md).

> **Estado honesto (27/11/2026).** Tudo o que foi gerado é **Hipótese**: nenhuma partitura foi conferida por ouvido nem no MuseScore por um humano, e nada foi testado com o teclado REVAS. A fila de revisão está na oficina (`PENDENCIAS-REVISAO.md`).

> **Direitos autorais.** O material é © 2017 Luciano Alves. Nada dele (PDFs, MusicXML, áudios, falas) entra neste repositório público. Aqui ficam só o código, o formato e este relato. O conteúdo derivado vive na *oficina*, no cerebro, e chega ao app por um pacote assinado.

---

## 1. Resumo em uma frase

Os PDFs foram **transcritos para MusicXML 4.0** por um agente de IA (Claude Sonnet) que lê a imagem da página, o texto vetorial do PDF e a fala cronometrada da aula; o **Audiveris** (OMR) faz uma leitura independente para conferir; um **alinhador próprio em Rust** (croma + DTW) marca em que instante do vídeo cada nota soa; e tudo é **empacotado e assinado** num arquivo `.ptcurso`, que o app instala como um plugin só de dados.

## 2. O que é a "partitura do Piano Tutor"

O Piano Tutor não inventou uma notação nova. A partitura dele é **MusicXML 4.0** (`score-partwise`, descomprimido), o padrão aberto que MuseScore, Finale e Sibelius leem. O que o app acrescenta em volta é:

| Peça | Formato | Para quê |
|---|---|---|
| Partitura | `partituras/<id>.musicxml` | notas, ritmo, dedilhado (`<fingering>`) e solfejo (`<lyric>`) |
| Estado | `estado.toml` na oficina → campo `estado` no manifesto | Hipótese, Em revisão, Aprovada ou Rejeitada |
| Áudio do trecho | `audio/<id>.opus` (Ogg Opus, mono, 48 kHz) | o Luciano tocando a peça, recortado do vídeo |
| Alinhamento | `alinhamentos/<id>.json` | instante de cada nota no áudio, com confiança |
| Roteiro | `roteiro.toml` | falas do professor (voz Piper) antes, durante e depois |
| Manifesto | `manifesto.toml` + `assinatura.ed25519` | Curso → Seções → Lições → Exercícios, hashes e assinatura |

Tudo isso vai num zip `.ptcurso`. O app lê o MusicXML com o leitor próprio (`crates/piano-tutor-core/src/partitura.rs`, sobre `roxmltree`) e desenha com o renderizador próprio (`crates/piano-tutor-app/src/partitura_visual.rs`).

## 3. Tecnologia usada

| Etapa | Ferramenta | Onde roda |
|---|---|---|
| Página → imagem | `pdftoppm` (Poppler), 200 a 500 dpi | cerebro |
| Página → texto e posições | `pdftotext -layout` e `-bbox-layout` (Poppler) | cerebro |
| Leitura da partitura | **Claude Sonnet** (subagentes) olhando o PNG | Claude Code no cerebro (Remote Control) |
| Conferência por OMR | **Audiveris 5.11** (flatpak `--user`), exportando `.mxl` | cerebro |
| Comparação OMR × transcrição | script Python de comparação de alturas e durações | cerebro |
| Validação do XML | `xml.dom.minidom` (Python) e soma de durações por compasso | cerebro |
| Extração de áudio | `ffmpeg` (WAV mono 22 050 Hz para alinhar; Opus 64 kbit/s para o pacote) | cerebro |
| Alinhamento | `piano-tutor-alinhar` (crate `piano-tutor-alinhamento`, Rust puro) | cerebro |
| Plano do curso | `gerar-plano.py` (lê inventário, estados e roteiros) | oficina |
| Empacotar, assinar, instalar | `piano-tutor-curso` (crate `piano-tutor-curso`: `ed25519-dalek`, `sha2`, `zip`, `toml`) | cerebro |
| Correção humana | MuseScore 4.7.5 (flatpak) + tela **Tutor > Revisão do tutor** | cerebro |

Ninguém usou serviço de OCR na nuvem. As únicas chamadas externas foram as do próprio Claude, com autorização do Chico para a IA ver as páginas e as transcrições (uso pessoal, nada publicado).

## 4. Estratégia de montagem

### 4.1 Por que IA + OMR, e não só um deles

Na primeira rodada de decisões havia três caminhos: só Audiveris, um leitor próprio dos glifos do PDF, ou só IA lendo a imagem. A escolha foi **IA transcrevendo e Audiveris conferindo**, porque:

- Os PDFs são **vetoriais** (Finale 2017, fonte Maestro), não escaneados. O texto extraído traz os glifos das figuras (`œ` semínima, `˙` mínima, `w` semibreve, `Œ` pausa…) com coordenadas, o que permite casar glifo, dedo e nome da nota pela posição.
- Debaixo de cada nota o Luciano imprime o **dedilhado** (1 a 5) e o **solfejo por extenso** (dó, ré, mi…). Isso dá uma segunda fonte da altura na própria página.
- Na **fala cronometrada** de cada aula ele diz as notas e os dedos em voz alta. É uma terceira fonte, independente do PDF.
- O Audiveris lê bem as alturas e as figuras, mas não lê dedilhado nem solfejo (não havia idiomas de OCR instalados) e falha em folhas sem fórmula de compasso. Sozinho, não bastaria.

Com três ou quatro fontes independentes, a divergência entre elas vira a lista do que o humano precisa olhar.

### 4.2 Nada é verdade até alguém aprovar

Toda partitura nasce como **Hipótese**, com `confianca` (0 a 1, estimativa do agente), `proveniencia` (o que foi usado e se o Audiveris concordou) e um `divergencias.md`. Só uma partitura **Aprovada** vira Gabarito oficial. Para o Chico estudar já, a preferência "Usar conteúdo não revisado" (ligada por padrão) deixa abrir as Hipóteses com o selo "[!] não revisado".

### 4.3 Fatia pequena primeiro, depois o curso inteiro

1. **Piloto:** Seção 3 (Mão direita), 6 PDFs. Serviu para provar o formato, o brief de transcrição e o alinhador.
2. **Escala:** com o formato estável, um subagente por Seção, todos em paralelo, seguindo o mesmo brief. A cada Seção pronta o pacote era refeito e instalado, para o curso aparecer aos poucos no app.

### 4.4 Divisão do trabalho entre modelos

- **Opus** orquestrou e revisou.
- **Sonnet** fez o trabalho que exige leitura: transcrição e conferência, alinhamento, Roteiro e telas.
- **Haiku** fez o mecânico: inventário, importador e testes.

### 4.5 Conteúdo separado do app

O curso é um **plugin só de dados**: o app nunca executa nada que venha do pacote. O pacote é assinado com ed25519 (autenticidade), sem DRM. A chave privada fica em `~/.config/piano-tutor/chaves/` e nunca entra no git. Isso permite, no futuro e com licença escrita do autor, distribuir o curso como parceria.

## 5. Passo a passo

A oficina fica em `~/.local/share/piano-tutor/fontes/luciano-alves/` no cerebro. A fonte (`/mnt/e2/Musica/Curso-Piano-1/`) é só leitura.

### Passo 1: Inventário e triagem

Um subagente leu o índice do curso (`00 - Índice do curso.md`), o `catalogo-udemy.json` e as pastas, e gerou `inventario-completo.json`: Seções, Aulas, duração dos vídeos e qual PDF cada Aula anexa. Cada PDF foi classificado:

- partitura de música ou de exercício → vira **Exercício**;
- texto, teoria ou lista (alongamentos, tipos de piano…) → a Aula fica só para **assistir**;
- PDF com várias peças independentes → vários Exercícios (`-1`, `-2`…).

Cada Exercício recebeu um id `s<NN>-<slug>` (por exemplo `s06-frere-jacques`).

### Passo 2: Renderizar a página e extrair o texto

```sh
pdftoppm -r 200 -png "<pdf>" paginas/<id>          # imagem para a IA e para a tela de revisão
pdftotext -layout "<pdf>" -                         # glifos Maestro, dedos e solfejo em ordem
pdftotext -bbox-layout "<pdf>" caixas.html          # posição (x, y) de cada glifo
```

Quando a imagem não bastava, o agente recortava e ampliava a região (`pdftoppm -r 300 -x … -y … -W … -H …`).

### Passo 3: Transcrição por IA

O subagente olha a imagem e escreve o MusicXML à mão, seguindo o brief (`oficina/BRIEF-TRANSCRICAO.md`):

- **Convenção de oitava:** o Luciano chama o Dó central de **Dó3**. No arquivo, Dó3 = MIDI 60 = `<step>C</step><octave>4</octave>`.
- Uma parte "Piano". Peças a duas mãos usam `<staves>2</staves>` (direita na pauta 1, esquerda na pauta 2, com `<backup>` entre elas); acordes com `<chord/>`; cifras como `<harmony>`.
- Cada nota leva `<pitch>`, `<duration>`, `<type>`, o dedo em `<technical><fingering>` e o solfejo em `<lyric>`.
- Fórmula de compasso, armadura, ligaduras, barras de repetição e casas, como no PDF.
- `<encoding><software>Transcrição assistida (Hipótese)</software>` marca a origem.

### Passo 4: Conferência contra a fala

O agente lê a `-Fala.Cronometrada.txt` da Aula (linhas `[hh:mm:ss.mmm] texto`), compara as notas e os dedos ditos com os escritos e anota em que instantes o Luciano toca a peça inteira. Exemplo real, da Aula 14: ele diz a sequência de notas dos compassos 1 a 4, e ela bate com o PDF. Quando a fala diverge (erro da transcrição automática da voz, por exemplo), o PDF prevalece e a divergência é registrada.

### Passo 5: Conferência pelo Audiveris (OMR)

```sh
audiveris -batch -export -output ~/.cache/pt-ferramentas/audiveris-conferencia/<id> "<pdf>"
```

O `.mxl` exportado é descompactado e comparado nota a nota com a transcrição da IA (alturas e durações). O resultado entra na `proveniencia`, como em "Audiveris 5.11: rodado e concordou: concordância de alturas 100% (63/63)".

### Passo 6: Validação

- O XML precisa ser bem formado (`python3 -c "import xml.dom.minidom,sys; xml.dom.minidom.parse(sys.argv[1])" arquivo`).
- A soma das durações de cada compasso precisa bater com a fórmula.
- O leitor do app (`piano_tutor_core::partitura`) precisa abrir o arquivo sem erro.

### Passo 7: Estado e divergências

Para cada Exercício, a oficina ganha:

- `partituras/<id>.estado.toml`: `estado = "hipotese"`, `proveniencia`, `confianca`, PDF de origem, página PNG, número de compassos e notas, mão, `bpm_sugerido` e `trecho_aula`;
- `partituras/<id>.divergencias.md`: o que foi conferido, cada dúvida (compasso, o que o PDF mostra, o que a fala diz, o que o Audiveris deu, o que foi escolhido) e o que o Chico deve olhar no MuseScore.

### Passo 8: Alinhamento com o vídeo

O objetivo não é transcrever o áudio, e sim **marcar no vídeo o instante de cada nota** de uma partitura já conhecida, para o cursor acompanhar o Luciano.

```sh
ffmpeg -nostdin -y -i "<aula>.mp4" -vn -ac 1 -ar 22050 audio-aula/aula-<n>.wav
piano-tutor-alinhar … --inicio-s A --fim-s B [--bpm N] -o alinhamentos/<id>.json
ffmpeg -nostdin -y -ss <inicio> -to <fim> -i "<aula>.mp4" -vn -ac 1 -ar 48000 -c:a libopus -b:a 64k audio/<id>.opus
```

O método `dtw-croma` do crate `piano-tutor-alinhamento`:

1. calcula o **croma** do áudio (STFT, 12 classes de altura);
2. sintetiza o croma esperado da partitura, com harmônicos;
3. roda **DTW em modo subsequência** dentro da janela em que a fala indica que ele toca (antes ele costuma falar);
4. ajusta cada nota ao **onset** mais próximo (±80 ms), com tratamento de notas repetidas;
5. dá uma **confiança** por nota e recorta o trecho.

Se a confiança média ficasse abaixo de 0,4, o alinhamento e o Opus eram descartados, e o Exercício funciona sem eles. O `bpm_sugerido` foi medido pelo próprio alinhamento quando o autor não diz o andamento.

### Passo 9: Roteiro do professor

Para cada Aula e Exercício, o subagente escreveu falas curtas e parafraseadas (`roteiros/s<NN>.toml`): `antes`, `passo` no começo de cada frase, `acerto` e `erro` por trecho (nunca por nota) e `depois`. Cada fala guarda a `origem` (Aula e instantes) e nasce como Hipótese.

### Passo 10: Seções sem partitura

As Seções 14 e 15 não têm PDF. Ali os Exercícios de **Chamada e resposta** ("eu toco, você repete") foram escritos em MusicXML simples a partir do que o Luciano toca e pede para repetir na aula, com `proveniencia = "gerado da fala da Aula"`.

### Passo 11: Plano, pacote, assinatura e instalação

```sh
~/.local/share/piano-tutor/fontes/luciano-alves/publicar.sh
```

Esse script:

1. roda `gerar-plano.py`, que junta inventário, estados e roteiros em `plano-completo.toml` e `oficina/roteiro.toml`;
2. chama `scripts/atualizar-pacote-curso.sh`, que executa:
   - `piano-tutor-curso importar` (gera a pasta-origem a partir do plano e da oficina, sem nunca promover o estado);
   - `piano-tutor-curso empacotar --chave …` (lista e calcula o SHA-256 de cada arquivo e assina o manifesto com ed25519);
   - `piano-tutor-curso verificar` (confere zip, assinatura, estrutura e hashes);
   - `piano-tutor-curso instalar` (extrai em `~/.local/share/piano-tutor/cursos/<id>/`).

Depois é só reabrir a tela **Cursos** no app.

### Passo 12: Revisão humana (pendente)

Na tela **Tutor > Revisão do tutor > Partituras**, o Chico vê o PDF e a partitura lado a lado, ouve, abre no MuseScore para corrigir e marca Aprovar, Rejeitar ou "Aprovar direto". Depois roda `publicar.sh` de novo para atualizar o pacote.

## 6. Resultado em números

Contado na oficina em 27/11/2026:

| Item | Quantidade |
|---|---|
| Seções / Aulas | 18 / 163 |
| Exercícios | 113 (95 transcritos de PDF, 18 gerados da fala) |
| Audiveris com saída comparável | cerca de 74, a maioria com 95 a 100% das alturas iguais |
| Audiveris sem saída útil | 21 (sobretudo folhas de leitura sem fórmula de compasso) |
| Confiança média da transcrição | 0,89 (estimativa do agente) |
| Exercícios com áudio alinhado | 85 |
| Falas do Roteiro | 1.284 |
| Fila de revisão | 30 de prioridade alta, 43 média, 40 baixa |

## 7. Limitações conhecidas

- **Nada foi conferido por um humano.** Confiança e concordância são medidas automáticas.
- Nas **folhas de leitura** (Seções 4, 6 e 9) o PDF não traz ritmo: o ritmo e as pausas de fim de linha foram inventados para virar Exercício tocável.
- Em algumas peças a **repetição foi retirada** para combinar com o áudio da aula.
- O alinhamento **não foi conferido por ouvido**. Ele pode pegar contagem do metrônomo, voz por cima ou cortar a última nota.
- Os Exercícios das Seções 14 e 15 foram **escritos a partir da fala**, sem partitura de origem.
- Quando as duas mãos tocam a mesma nota ao mesmo tempo, o app conta **uma tecla só** (uníssonos fundidos). Alinhamentos anteriores a essa regra foram refeitos.

## 8. Como refazer ou estender para outro curso

1. Criar `~/.local/share/piano-tutor/fontes/<tutor>/oficina/` e um inventário.
2. Dar a cada subagente o `BRIEF-TRANSCRICAO.md` e o `BRIEF-SECAO.md` (estão na oficina do Luciano e servem de modelo).
3. Gerar uma chave: `piano-tutor-curso chave-gerar ~/.config/piano-tutor/chaves`.
4. Escrever o plano de importação (formato em [FORMATO-PACOTE-DE-CURSO.md](../specs/FORMATO-PACOTE-DE-CURSO.md#plano-de-importação)) e rodar `scripts/atualizar-pacote-curso.sh <tutor> <plano>`.

O mesmo caminho serve para o curso da Lastlink ou qualquer outro material em PDF, desde que o direito de uso permita.
