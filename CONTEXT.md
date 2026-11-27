# Piano Tutor

Professor assistido de piano e teclado, local-first, que ouve o teclado físico, toca timbres, ensina com exercícios e transforma videoaulas em conteúdo revisado.

## Linguagem

### Conteúdo pedagógico

**Aula**:
Vídeo-fonte de um tutor, mantido apenas localmente, do qual se pode derivar conteúdo.
_Evitar_: videoaula como sinônimo de lição ou de Curso

**Curso**:
Sequência ordenada de Lições de uma mesma origem, com progresso próprio: o Curso de fábrica do Piano Tutor ou o Curso de um tutor (ex.: Luciano Alves), que chega por um Pacote de Curso.
_Evitar_: módulo, trilha, programa

**Seção**:
Divisão de um Curso de tutor, na ordem do autor, que agrupa Aulas e as Lições derivadas delas.
_Evitar_: módulo, capítulo, disciplina

**Lição**:
Conteúdo pedagógico aprovado e versionado, criado à mão ou derivado de uma Aula, que agrupa Exercícios.
_Evitar_: aula, módulo

**Trilha**:
Agrupamento temático e ordenado de Lições (Fundamentos, Escalas, Acordes...); é só um rótulo da Lição, sem arquivo próprio.
_Evitar_: módulo, curso, categoria

**Exercício**:
Unidade praticável com um Gabarito, pertencente a uma Lição ou fornecida de fábrica.
_Evitar_: lição, desafio, atividade ("cada vídeo é um desafio" é jeito de falar, não termo)

**Acervo**:
Biblioteca de partituras do aplicativo, com as de domínio público que vêm com ele e as importadas pelo aluno; ao importar, o Acervo guarda sua própria cópia do arquivo.
_Evitar_: biblioteca, repositório, banco de partituras

**Pacote de Curso**:
Arquivo plugável que abastece o aplicativo com um Curso de tutor (Lições, partituras, áudios, Alinhamentos, Roteiro) sem fazer parte dele; pode ser distribuído só com licença do autor.
_Evitar_: plugin, conteúdo embutido, pacote local (o código que ele leva são os Roteiros de Aula)

**Roteiro**:
Falas aprovadas que o professor diz ao aluno durante uma Lição, ligadas aos passos dos Exercícios, ditas por voz sintética.
_Evitar_: legenda, transcrição, fala da Aula

**Chamada e resposta**:
Modo de Exercício em que o professor toca ou anuncia um trecho e o aluno o repete no teclado.
_Evitar_: eco, imitação, pergunta e resposta (nome de uma música do curso)

**Alinhamento**:
Ligação entre cada nota de uma partitura aprovada e o instante em que ela soa numa Aula.
_Evitar_: sincronia, transcrição de áudio

**Player de Aula**:
Tela em que uma Lição derivada de uma Aula é vivida de ponta a ponta: o vídeo da Aula (embutido, ou destacado numa janela própria que pode ser maximizada), a partitura e o teclado com o dedilhado andam juntos no mesmo instante, e os Exercícios são trechos da linha do tempo.
_Evitar_: tela do Curso, visualizador, "ver vídeo"

**Âncora**:
Ponto do Alinhamento fixado à mão no editor de sincronia do Player de Aula: "este instante da Aula é esta nota da partitura". Deslocar e acelerar um trecho movem as suas Âncoras; um ajuste salvo vira nova Hipótese do Alinhamento.
_Evitar_: marcador, keyframe, sincronia global

**Roteiro de Aula**:
Script em Luau, um por Aula, que conduz o Player de Aula (que trecho tocar, quando esperar o aluno, quando repetir ou avançar). Viaja assinado no Pacote de Curso e roda isolado, só com a API do player.
_Evitar_: desafio, plugin, Roteiro (que são as falas do professor)

**Modo de acompanhamento**:
Como o Player de Aula reage enquanto o aluno toca junto um trecho: **Nota a nota** (para em cada nota até a certa, sem andamento), **Espera tocar**, **Segue no tempo** (não para, pontua acerto e erro) ou **Chamada e resposta**. O aluno escolhe e troca durante a Aula; o Roteiro de Aula pode sugerir um.
_Evitar_: modo de prática (é da tela de Partitura)

**Espera tocar**:
Modo da prática, na tela de Partitura e no Player de Aula, que anda no andamento da Lição e só para quando a janela de uma nota passa sem a nota certa (nota errada é marcada mas não para; num acorde, faltar qualquer nota para). Cada parada conta como falta e é somada nas Paradas do resumo; no Player de Aula, vídeo e som param junto. Ao acertar, conta dois tempos e retoma voltando 0, 1, 2 ou 4 notas, ou do início, conforme a escolha do aluno; as notas voltadas são tocadas e conferidas de novo.
_Evitar_: Espera, Nota a nota (que para em toda nota), modo lento

**Nota a nota**:
Modo da prática que para em cada nota até o aluno tocar a certa, sem andamento.
_Evitar_: Espera (nome antigo)

**Calibração de vídeo**:
Mapa, por Aula, de onde fica cada tecla no quadro da câmera de cima, proposto automaticamente pela fábrica e conferido pelo revisor; é dele que sai a Hipótese de dedilhado da Aula.
_Evitar_: calibração do teclado (é a do instrumento MIDI)

**Versão**:
Estado salvo e imutável de uma partitura do Acervo, criado a cada edição ou importação; o PDF e o MusicXML de origem são a primeira Versão e nunca são sobrescritos. Aprovação e Gabarito valem para uma Versão específica.
_Evitar_: revisão (é o ato de conferir; ver Em revisão), rascunho, cópia

**Gabarito**:
Resposta esperada de um Exercício; só existe a partir de conteúdo aprovado, nunca de saída bruta de IA.
_Evitar_: resposta da IA, ground truth

### Prática

**Sessão**:
Período contínuo em que o aluno toca no aplicativo, com seus resultados e estatísticas.
_Evitar_: arquivo de sessão, projeto

**Projeto**:
Arquivo que o aluno abre e salva, reunindo partitura, gravações e configurações de trabalho.
_Evitar_: sessão

**Gravação**:
Registro MIDI de uma execução (um take), sem quantização.
_Evitar_: sessão, partitura

### Conteúdo gerado por IA

**Hipótese**:
Conteúdo proposto por IA (notas, dedos, compassos, fala) com confiança e proveniência, ainda sem valor de verdade.
_Evitar_: rascunho, proposta, candidato, sugestão da IA

**Em revisão**:
Estado de uma Hipótese que um revisor humano está conferindo e corrigindo.
_Evitar_: needs_review, unreviewed

**Aprovada**:
Estado do conteúdo que o revisor humano aceitou; só ele pode virar Lição ou Gabarito. Normalmente passa por Em revisão; o revisor também pode **aprovar direto** uma Hipótese num clique, e o registro marca que foi direta.
_Evitar_: final, validado pela IA

**Rejeitada**:
Estado da Hipótese descartada pelo revisor; fica registrada, mas nunca é usada como conteúdo.

**Job**:
Execução técnica de um processamento de IA sobre uma Aula; seus estados (pendente, rodando, falhou, cancelado) não são estados de conteúdo.
_Evitar_: usar estado do job como estado da Lição

### Homologação

**Fase**:
Incremento demonstrável F00–F14 com critérios próprios, aceito por humano antes da próxima.
_Evitar_: sprint, épico, marco

**Marco**:
Conjunto de Fases que forma uma entrega utilizável (M1 = F00–F03, tocar com som no Linux).
_Evitar_: MVP, fase

**Gate**:
Condição de bloqueio de uma Fase; nos gates de hardware (F01, F02, F03, F14) qualquer critério NÃO VERIFICADO bloqueia.
_Evitar_: aceite, demonstração

**Bug crítico**:
Crash, nota presa, perda de dados ou latência acima da meta da Fase.
_Evitar_: bug grave, bloqueador (sem definição)

**Checklist de escuta**:
Julgamento auditivo (timbre, estalo, clipping) registrado e assinado pelo revisor humano como evidência de critério subjetivo.
_Evitar_: "soa bem", inspeção informal

**Plataforma de referência**:
Sistema e máquina onde os Gates são verificados; hoje Linux no cerebro.
_Evitar_: plataforma principal, sistema oficial

**Plataforma de teste pessoal**:
Sistema onde o titular usa o app com as mesmas funcionalidades, sem valer como evidência de Gate; hoje macOS no Mac Home.
_Evitar_: porte, versão beta, segunda plataforma

**Meta experimental**:
Meta de qualidade de IA cuja falha libera a Fase como beta, sem permitir uso do conteúdo sem revisão.

**PC de referência**:
O computador cerebro (Kubuntu, saída de áudio onboard) onde as metas de latência são medidas.
