# Especificação — Evolução musical do Piano Tutor (edição, Acervo, reconhecimento, MCP, professor inteligente)

> Substitui a proposta em docs/evolucao-musical/ (mantida como histórico). Complementa ESPECIFICACAO-COMPLETA-v1 para as fases F00–F14 já existentes; em conflito, prevalece esta para edição de partitura (ADR-0015).

Status: rascunho da E00 (etapa to-spec), 2026-11-27. Decisões do titular (D1–D18, tabela em 00-DECISOES da E00) prevalecem sobre a proposta antiga e sobre as pesquisas. Decisões de engenharia reversíveis do supervisor são citadas como "SUP: tema". Vocabulário segue o CONTEXT.md: Acervo, Versão, Hipótese, Em revisão, Aprovada, Rejeitada, Gabarito, Lição, Exercício, Sessão, Gravação, Job, Gate, Fase.

Convenção de status dos requisitos: **mantido** (texto da proposta vale), **alterado** (com o porquê e a decisão que o altera), **adiado** (continua válido, fase posterior declarada). Convenção de verificação: PASS / FAIL / NÃO VERIFICADO / N/A, como na Definição de Pronto. Qualquer coisa que dependa do REVAS (teclado físico de 61 teclas) ou de áudio real é NÃO VERIFICADO até medição no PC de referência (Linux, cerebro); CI verde nunca substitui Gate físico.

---

## Requisitos transversais (valem para todas as fases)

| Id | Requisito | Origem |
|---|---|---|
| T1 | O app funciona offline e sem Python, sem LLM e sem OMR instalados. Cada recurso que depende deles degrada com aviso visível; abrir, editar, salvar Versão e praticar nunca dependem deles. | R-N01, R-A05, D12, D16 |
| T2 | Material comprado do titular (cursos, PDFs de terceiros) nunca entra no git nem na CI; testes com ele são só locais no PC de referência. Fixtures versionadas são apenas de licença verificada. | R-N01, SUP: material comprado |
| T3 | Nada de material comprado vai à nuvem sem consentimento por lote; o padrão é local; o que saiu é registrado (o quê, para qual provedor, quando, tamanho). Recortes de PDF são tratados como dado não confiável, nunca como instrução. | D3, SUP: material comprado |
| T4 | Teto de IA na nuvem de US$ 10/mês com parada obrigatória (a chamada seguinte é recusada, não apenas avisada); Ollama local sem limite. | D18 |
| T5 | Plataforma de referência Linux (cerebro) onde os Gates são verificados; macOS (Mac Home) é plataforma de teste pessoal, mesmas funções, evidência separada que não vale como Gate (ADR-0014). | R-N03 |
| T6 | Licença do código MIT OR Apache-2.0; distribuição gratuita em código aberto. Ferramentas AGPL (Audiveris, HOMR) só como processos separados empacotados com avisos e fontes AGPL; o app funciona sem elas; o empacotamento permanece adiado para 2027. Piper (GPL) idem, processo separado. Verovio (LGPL) nunca linkado. | D9, D10, D16, SUP: render |
| T7 | Metas de precisão de reconhecimento (OMR) são "a calibrar após baseline" e nunca prometidas; a fusão de motores só detecta discordância; nenhum resultado automático vira Gabarito. | SUP: OMR, R-B05 |
| T8 | Toda edição salva uma nova Versão; a Hipótese e o original (PDF e MusicXML de origem) nunca são sobrescritos. Aprovação e Gabarito valem para uma Versão (por hash); editar invalida a aprovação anterior. | D8, SUP: edição, R-B06 |
| T9 | Atalhos são remapeáveis; nenhum atalho de edição interfere no Pânico nem em Ctrl+K (paleta da casca). Interface e documentação em PT-BR; telemetria desligada. | R-E04, R-N03, SUP: atalhos |
| T10 | Os Gates existentes de latência (MIDI→áudio p95 ≤ 50 ms, MIDI→tela ≤ 60 ms, jitter, estabilidade) não regridem; novos workers rodam com prioridade menor e são medidos sob carga. | R-N02 |

---

## Problema

O titular (músico e revisor) já converteu partituras de cursos em MusicXML por OMR e IA, mas o resultado traz erros típicos (altura, duração, pausas, acordes) e o aplicativo hoje só consegue **exibir e tocar** o que recebeu. O modelo de partitura é derivado do XML, sem identidade estável de nota, e descarta letra, cifra, dedilhado, dinâmica, ornamentos e transposição; a "exportação sem perda" atual é uma cópia do texto e se quebra na primeira edição. A tela é monolítica (estado, reprodução, prática e UI na mesma estrutura), o layout depende do egui e não há hit test por nota; a revisão atual passa por MuseScore externo. Não existe Acervo (banco de partituras), nem fila de jobs, nem contrato com workers, nem MCP, nem provedores de IA. A proposta anterior ordenava o trabalho pelo reconhecimento automático e por um catálogo completo, adiando o que o usuário precisa primeiro: corrigir à mão o que já tem e praticar a partir disso. Também prometia precisão de OMR sem evidência: a pesquisa independente não encontrou número publicado de precisão para piano polifônico com letra e dedilhado em nenhum motor.

## Solução

Do ponto de vista do usuário, em ordem de entrega:

1. **Oficina de correção (E01)**: abrir uma partitura já convertida com o PDF original ao lado, selecionar notas, corrigir altura e duração, inserir e remover nota, pausa e acorde, desfazer e refazer, ter autosave, **salvar como nova Versão** (a Hipótese e o original ficam intactos) e **praticar a Versão** nos Exercícios existentes. Letra, cifra e dedilhado aparecem e são preservados, mas ainda não editados.
2. **Acervo (E02)**: importar em lote, com cópia imutável identificada por SHA-256, busca por texto e histórico de Versões, tudo local.
3. **Editor completo (E03)**: letra, cifra, dedilhado, ligaduras, tercinas, vozes, repetições e perfil de atalhos MuseScore.
4. **REVAS no editor (E04)**: inserir notas tocando no teclado (nota a nota, acorde, trocar altura) e gravar com quantização não destrutiva.
5. **Reconhecimento (E05)**: arrastar um PDF e obter uma Hipótese com divergências marcadas na Oficina, por motores abertos em processos separados, medidos em um benchmark público reproduzível.
6. **MCP (E06)**, **professor inteligente (E07)** com quatro provedores e teto de custo, **plugins (E08)** e **pesquisa de produção musical (E09)**.

O código é aberto, gratuito, funciona offline e não promete precisão de OMR; a correção humana é o caminho principal, a máquina só propõe.

---

## Matriz de requisitos (R-IDs estáveis da proposta)

IDs mantidos como na proposta; "Fase" é a fase do novo roadmap onde o requisito é entregue (a primeira, quando parcial). Siglas: D# = decisão do titular; SUP = decisão de engenharia do supervisor.

| R-ID | Resumo | Status | Fase | Por quê / observação |
|---|---|---|---|---|
| R-B01 | Importar PDF/MusicXML/MXL, tipo vetor/scan/misto, hash, deduplicação | alterado | E02 (E01: abrir XML e PDF já existentes) | D4: importar copia o arquivo para o Acervo, imutável, identificado por SHA-256 (original nunca sobrescrito). D7: "Acervo", não "biblioteca". D8: "Versão", não "revisão". Separar várias obras de um PDF por seleção de páginas fica em E05 (depende de reconhecimento); mantida a regra de não misturar movimentos sem confirmação. |
| R-B02 | SQLite com migrações, FTS, metadados, backup | alterado | E02 | D7 (termo Acervo). Escada de migrações e FTS mantidos. Busca por contorno/intervalos musicais adiada (sem decisão de uso; revisar após E03). Campos desconhecidos continuam nulos explícitos. |
| R-B03 | Fila durável de Jobs, cancelável, retomável | alterado | E05 | Só é necessária com OMR (E05); em E02 a importação em lote usa fila simples em processo. D12: CPU obrigatório, GPU só acelera. T10: prioridade menor que áudio/MIDI. |
| R-B04 | Adaptadores Poppler, Audiveris, HOMR e candidatos; OCR; LLM multimodal opcional | alterado | E05 | D17: PDFtoMusic Pro removido (benchmark só com ferramentas abertas). D16: Audiveris e HOMR AGPL como processos separados, com avisos. D12: CPU obrigatório. SUP: LLM multimodal nunca motor primário (só triagem de recortes, sempre Hipótese, D3). SUP: roteamento vetor/scan por inspeção de fontes do PDF. Candidatos novos (ex.: copista) só após replicação local. |
| R-B05 | Normalizar e comparar candidatos, divergências, validação XSD/semântica | alterado | E05 (validação semântica básica já em E01) | SUP: fusão multi-motor só detecta discordância (concordância não é precisão). T7: sem metas prometidas. Mantido: desconhecido ≠ ausente; nenhum resultado automático vira Gabarito. |
| R-B06 | Oficina PDF ↔ partitura, correção, aprovar/rejeitar/reabrir | alterado | E01 (núcleo), E05 (divergências) | D1 prioriza esta fatia. D8: "Versão". T8: aprovação ligada ao hash da Versão. D13: letra/cifra/dedilhado só exibidos em E01. Navegação por divergências e fila por risco dependem de reconhecimento (E05). Correção por MIDI em E04. |
| R-E01 | MusicXML: parser ampliado, preservação, matriz por elemento | alterado | E01 (núcleo), E03 (ampliação) | SUP: modelo de documento híbrido (IdNota estável + faixa de bytes de origem + filhos desconhecidos preservados; só o compasso editado é regenerado), a validar em spike com corpus CC0 antes de travar o ADR. Matriz por elemento mantida; não declarar conformidade integral sem corpus. |
| R-E02 | Modelo editável independente de GUI, comando tipado, undo/redo, autosave | alterado | E01 | SUP: serviço de domínio de edição no núcleo, sem egui (seam principal). Diagnóstico mostrou a tela monolítica e layout acoplado ao egui. Undo na 1ª fatia por snapshot do compasso afetado. Lote atômico e diff musical completos em E06. |
| R-E03 | Editor egui com SMuFL/Bravura, layout, hit test, seleção, zoom | alterado | E01 (spike + notas), E03 (completo) | SUP: layout próprio sobre egui com SMuFL/Bravura (OFL-1.1); Verovio só como oráculo de comparação em processo separado, nunca linkado; fecha o spike da ADR-0007 na E01. Camada geométrica própria com adaptador egui. D2: escopo v1 piano em duas pautas + lead sheet; multi-partes depois. |
| R-E04 | Perfil de atalhos tipo MuseScore | alterado | E01 (mínimo), E03 (completo) | D14: seta ↑/↓ = semitom; Alt+Shift+seta = pela escala; remapeáveis. SUP: durações 1–9 (perfil MuseScore 4); redo Ctrl+Shift+Z; Ctrl+K continua paleta da casca e cifra vai para Ctrl+Shift+K; Ctrl+. (Pânico) reservado; Esc conforme prioridade definida pela casca. |
| R-E05 | Entrada REVAS: step-time, acorde, re-pitch, tempo real com quantização | mantido | E04 | Sem alteração de conteúdo; reposicionado após o editor completo. Todo critério com teclado físico é NÃO VERIFICADO sem hardware. Pedal só após calibração que prove eventos. |
| R-E06 | Letra, cifra, dedilhado, texto livre | alterado | E01 (exibir/preservar), E03 (editar) | D13: mostrar e preservar na 1ª fatia; editar na seguinte. Distinção solfejo × letra mantida. |
| R-L01 | Revisão utilizável vira Exercício; comparação MIDI determinística | alterado | E01 (praticar a Versão), E04/E07 (demais) | Termo: "Versão" (D8). E01 reaproveita os Exercícios e a prática existentes; Hipótese só em modo sinalizado. Corpus de 5 partituras aprovadas e métricas completas permanecem como critério do ciclo de lição (após E03). |
| R-L02 | Histórico por perfil/Versão, relatório de compassos difíceis, planos adaptativos | adiado | E07 | Depende do professor inteligente e da aprovação humana de planos; versões simplificadas/transpostas preservam a fonte (T8). |
| R-P01 | Produção musical (trilhas MIDI, piano roll, exportação) | adiado | E09 | D roadmap: E09 só pesquisa/spike; sem promessa de escopo. |
| R-A01 | Serviços de domínio compartilhados por UI, CLI, MCP e plugins; contrato de worker | alterado | E01 (serviço de documento), E05 (contrato de worker) | Mínimo de módulos: nenhuma fragmentação em muitos crates sem necessidade comprovada (nomes e limites revisáveis). Núcleo puro sem GUI, I/O ou LLM; um único dono da escrita no Acervo. SUP: MCP em binário separado. |
| R-A02 | MCP local stdio, resources/tools, escrita por patch com dry-run | alterado | E06 | D11: MCP primeiro (Chico usa Claude Code ou outro cliente); chat dentro do app depois (E07). SUP: binário separado, runtime assíncrono confinado a ele, SDK rmcp oficial com versão fixada (3.x.y), somente leitura primeiro. Escrita só após leitura aceita: dry-run, diff, aprovação humana, bloqueio otimista. |
| R-A03 | Harness própria com provedores, ferramentas, memória, custo, cofre | alterado | E07 | D6: Claude, OpenAI, Gemini e Ollama desde a fase do professor inteligente. D18: teto US$ 10/mês. D3: consentimento por lote. SUP: camada própria fina (trait de provedor sobre cliente HTTP síncrono já usado); biblioteca de terceiros como plano B. Chaves no cofre do SO. |
| R-A04 | Plugins Luau (e Python isolado), manifesto, assinatura | alterado | E08 | D15: v1 só plugins assinados pelo titular. SUP: Luau sem aleatoriedade não semeada e limite por contagem de interrupções (determinístico); Python recusa executar sem isolamento de rede no Linux de referência, aviso visível no macOS. WASM adiado. |
| R-A05 | Voz opcional (Piper), entrada de fala a pesquisar | adiado | pós-E07 | Piper existente permanece (processo separado, GPL); entrada de fala só pesquisa. LLM indisponível nunca bloqueia edição, treino, gravação ou reprodução (T1). |
| R-N01 | Local/offline, privado × público, limites de segurança | alterado | todas | Acrescenta T3 (D3) e T2. Limites de PDF/XML/MXL (XXE, zip bomb, caminho fora do escopo), processos sem shell, logs sanitizados mantidos. |
| R-N02 | Gates de latência, orçamentos de CPU/memória, metas de busca e edição | mantido | todas | Metas de busca (p95 ≤ 250 ms em 10 mil registros) e edição (≤ 100 ms em 100 compassos) continuam "metas propostas a calibrar", sem rebaixar em silêncio; baseline de benchmark antes de otimizar. Medição sob carga é NÃO VERIFICADA sem o PC de referência. |
| R-N03 | Linux referência, macOS pessoal, build reproduzível, PT-BR | alterado | todas | D9/D10 (código aberto, MIT OR Apache-2.0); inventário de licenças e verificação de dependências na CI antes de entrarem rmcp, provedores e OMR. Sem atualização automática não consentida. |

---

## Roadmap reordenado (D roadmap)

| Fase | Nome | Entrega demonstrável | Requisitos | Depende de |
|---|---|---|---|---|
| E00 | Diagnóstico e decisões | Pesquisas, decisões do titular, esta especificação, ADR-0015, backlog | todos | n/a |
| E01 | Oficina de correção | Corrigir uma partitura já convertida lado a lado com o PDF, salvar Versão, praticar | B06, E01–E04 (parcial), E06 (exibir), L01 (parcial), A01 (serviço de documento) | E00 |
| E02 | Acervo | Importar em lote, cópia por hash, busca, Versões | B01, B02, N01 | E01 |
| E03 | Editor completo | Letra, cifra, dedilhado, ligaduras, tercinas, vozes, repetições, atalhos MuseScore | E01, E03, E04, E06 | E01 |
| E04 | REVAS no editor | Step-time, acorde, trocar altura, gravação com quantização não destrutiva | E05, N02 | E03 |
| E05 | Reconhecimento | PDF vira Hipótese por motores abertos em processos; divergências na Oficina; benchmark público | B03, B04, B05 | E01, E02 |
| E06 | MCP | Leitura (cliente independente) e depois escrita com dry-run, diff e aprovação | A01, A02 | E01, E02 |
| E07 | Professor inteligente | Chat com 4 provedores, teto US$ 10/mês, consentimento por lote | A03, L02 | E06 |
| E08 | Plugins | Luau assinado; Python isolado | A04 | E05, E07 |
| E09 | Produção musical | Somente pesquisa e spike | P01 | E04 |

Regras do roadmap: uma fase por vez; cada fase termina com demonstração e aceite humano do titular (aceite não é inferido de CI verde); o spike de MCP somente leitura pode ocorrer dentro da E01 sem romper Gates, mas a entrega do MCP é a E06. Nomes E00–E09 não se confundem com F00–F14 da especificação completa v1; em conflito sobre edição de partitura, prevalece esta (ADR-0015, que supera a restrição da ADR-0007 sobre edição pós-F13).

---

## Histórias de usuário

Formato: "Como <ator>, quero <recurso>, para <benefício>" — [R-ID, fase].

### Oficina de correção (E01)

1. Como revisor, quero abrir uma partitura já convertida (MusicXML) e ver o PDF original ao lado, para conferir cada compasso contra a fonte. [R-B06, E01]
2. Como revisor, quero que o PDF e a partitura rolem e deem zoom em conjunto por compasso, para não procurar o ponto correspondente à mão. [R-B06, E01]
3. Como revisor, quero clicar numa nota e vê-la selecionada, para saber exatamente o que vou alterar. [R-E03, E01]
4. Como revisor, quero navegar entre notas com as setas ← e →, e trocar de voz/pauta com Tab, para corrigir sem mouse. [R-E03, R-E04, E01]
5. Como revisor, quero subir ou descer a altura de uma nota em semitom com ↑/↓, para corrigir erros de leitura rapidamente. [R-E04, D14, E01]
6. Como revisor, quero subir ou descer a nota pela escala com Alt+Shift+↑/↓, para respeitar a armadura. [R-E04, D14, E01]
7. Como revisor, quero mudar a nota em uma oitava com Ctrl+↑/↓, para corrigir oitava errada. [R-E04, E01]
8. Como revisor, quero escolher a duração da nota com as teclas 1 a 9 e acrescentar ponto, para corrigir figuras mal lidas. [R-E04, SUP: durações, E01]
9. Como revisor, quero que uma alteração de duração que estoure o compasso seja recusada com mensagem clara e nada mude, para não corromper a partitura. [R-E02, E01]
10. Como revisor, quero inserir uma nota ou pausa numa posição do compasso, para recuperar um símbolo que o OMR perdeu. [R-E02, E01]
11. Como revisor, quero remover uma nota (vira pausa de mesma duração), para eliminar símbolos que o OMR inventou sem desalinhar o compasso. [R-E02, E01]
12. Como revisor, quero adicionar uma altura a uma nota existente (acorde) com Shift+letra, e retirar uma altura do acorde, para corrigir acordes incompletos. [R-E02, R-E04, E01]
13. Como revisor, quero desfazer (Ctrl+Z) e refazer (Ctrl+Shift+Z) quantas edições eu fizer, para experimentar sem medo. [R-E02, E01]
14. Como revisor, quero que o trabalho em andamento seja salvo sozinho em intervalos curtos e recuperado se o app fechar de repente, para nunca perder correções. [R-E02, E01]
15. Como revisor, quero salvar o que fiz como uma nova Versão com uma nota opcional de motivo, para guardar o ponto de correção. [R-B06, T8, D8, E01]
16. Como revisor, quero ver a lista de Versões da partitura (original, Hipótese, correções) com data e autor, para voltar a qualquer uma. [R-B06, D8, E01]
17. Como titular, quero a garantia de que a Hipótese e o arquivo original nunca são alterados, para sempre poder comparar com o que a máquina gerou. [T8, R-B06, E01]
18. Como revisor, quero comparar duas Versões e ver quais compassos mudaram, para conferir o que corrigi. [R-E02, R-B06, E01]
19. Como revisor, quero ver letra, cifra e dedilhado que já estavam no arquivo mesmo sem poder editá-los ainda, para ter o contexto completo ao corrigir as notas. [R-E06, D13, E01]
20. Como revisor, quero que letra, cifra, dedilhado, dinâmica e demais elementos que o editor não conhece sobrevivam intactos ao salvar, para não perder nada silenciosamente. [R-E01, R-E06, D13, E01]
21. Como revisor, quero ser avisado, numa lista visível, de quais elementos o editor ainda não suporta nesta partitura, para saber o que não será renderizado nem editado. [R-E01, E01]
22. Como revisor, quero aprovar uma Versão (ou rejeitá-la, ou reabrir) com motivo, para decidir quando ela vale como conteúdo de ensino; a aprovação fica ligada ao hash dessa Versão. [R-B06, T8, E01]
23. Como revisor, quero que editar uma Versão aprovada crie uma nova Versão Em revisão e que a aprovação anterior não valha para ela, para não ensinar conteúdo não conferido. [R-B06, T8, E01]
24. Como aluno, quero praticar a Versão salva (tocar, esperar, nota a nota, loops, andamento) com os Exercícios existentes, para estudar a música já corrigida. [R-L01, E01]
25. Como aluno, quero que uma Versão não aprovada só apareça em modo claramente sinalizado como Hipótese, para não confundir com o Gabarito oficial. [R-L01, T8, E01]
26. Como revisor, quero ouvir o compasso editado, para conferir pelo ouvido o que corrigi. [R-L01, E01]
27. Como revisor, quero que o editor continue respondendo sem travar numa peça de 100 compassos, para ter fluidez; a meta é calibrada após baseline. [R-N02, E01]
28. Como usuário de teclado e leitor de tela, quero operar a Oficina sem mouse, com foco visível e alto contraste, para ter acessibilidade. [R-E03, E01]
29. Como titular, quero que o atalho de Pânico e Ctrl+K da casca continuem funcionando dentro da Oficina, para não perder o controle do app. [T9, R-E04, E01]
30. Como titular, quero remapear qualquer atalho da Oficina, para adaptá-lo ao meu hábito. [R-E04, D14, E01]
31. Como mantenedor, quero um spike que compare o render próprio (Bravura) com Verovio como oráculo, num corpus CC0, para fechar a pendência da ADR-0007 com evidência. [R-E03, SUP: render, E01]
32. Como mantenedor, quero um spike que valide o modelo híbrido de documento em corpus CC0 antes de travar o ADR, para evitar perda de dados em produção. [R-E01, SUP: modelo, E01]
33. Como mantenedor, quero que o aplicativo abra arquivos malformados ou hostis sem travar e mostre erro claro, para segurança. [R-N01, E01]

### Acervo (E02)

34. Como aluno, quero arrastar PDF, MusicXML ou MXL para o app e ter uma cópia imutável no Acervo identificada por SHA-256, para nunca perder a fonte. [R-B01, D4, E02]
35. Como aluno, quero importar uma pasta inteira em lote e ver o progresso e os rejeitados com motivo, para carregar meu material de uma vez. [R-B01, E02]
36. Como aluno, quero que um arquivo já importado (mesmo hash) não seja duplicado, para economizar espaço. [R-B01, E02]
37. Como aluno, quero buscar por título, compositor, letra, tonalidade e tags, com tolerância a acentos, para achar a música rápido. [R-B02, E02]
38. Como aluno, quero que campos desconhecidos apareçam como "desconhecido", não inventados, para confiar nos metadados. [R-B02, E02]
39. Como aluno, quero exportar e restaurar um backup local do Acervo com manifesto e hash, para não perder nada ao trocar de máquina. [R-B02, E02]
40. Como mantenedor, quero migrações do banco com rollback testado, para evoluir o esquema sem perder dados. [R-B02, E02]
41. Como titular, quero que material comprado importado fique só no meu Acervo local, fora de git e CI, para respeitar os direitos. [R-N01, T2, E02]
42. Como aluno, quero ver, por partitura, as Versões e qual está Aprovada, para escolher o que praticar. [R-B02, D8, E02]

### Editor completo (E03)

43. Como revisor, quero editar a letra por sílaba e estrofe, com hífens e extensões, ligada à nota, para corrigir letras do OMR. [R-E06, E03]
44. Como revisor, quero editar cifras (Ctrl+Shift+K) em notação flexível, para lead sheets corretas. [R-E06, SUP: atalhos, E03]
45. Como revisor, quero editar o dedilhado técnico por nota, para estudar com os dedos certos. [R-E06, E03]
46. Como revisor, quero criar e remover ligaduras de valor (T) e de expressão (S), para corrigir fraseado e durações. [R-E01, R-E04, E03]
47. Como revisor, quero criar tercinas (Ctrl+3) e quiálteras aninhadas, para ritmos que o OMR costuma errar. [R-E01, R-E04, E03]
48. Como revisor, quero tratar duas vozes por pauta e movê-las entre pautas, para polifonia de piano. [R-E01, R-E03, E03]
49. Como revisor, quero editar repetições, voltas, segno e coda, para a estrutura correta. [R-E01, E03]
50. Como revisor, quero usar o perfil de atalhos MuseScore (N entrada, Esc saída, A–G, 0 pausa, T, S) e ver os conflitos de remapeamento, para trabalhar no meu fluxo. [R-E04, D14, E03]
51. Como revisor, quero que o salvamento preserve o que o editor ainda não suporta, com lista explícita do não suportado, para não haver perda silenciosa. [R-E01, E03]
52. Como titular, quero escopo v1 em piano de duas pautas e lead sheet, com multi-partes declarado fora, para entregar com qualidade. [D2, E03]

### REVAS no editor (E04)

53. Como tecladista, quero inserir notas tocando no REVAS em modo step-time com a duração escolhida, para entrar a música sem mouse. [R-E05, E04]
54. Como tecladista, quero inserir um acorde tocando as notas dentro de uma janela de tempo configurável, para entrada rápida. [R-E05, E04]
55. Como tecladista, quero trocar a altura de uma nota tocando a certa, mantendo duração, voz e pauta, para corrigir sem redigitar. [R-E05, E04]
56. Como tecladista, quero gravar em tempo real com metrônomo e ver a Gravação quantizada de forma não destrutiva (o MIDI bruto é preservado), para entrar peças inteiras. [R-E05, E04]
57. Como tecladista, quero que desconectar o teclado não deixe nota presa nem perda de edição, para segurança. [R-E05, E04]
58. Como tecladista, quero avançar e confirmar por pedal apenas se a calibração provar os eventos, e sempre poder usar mouse e PC, para flexibilidade sem promessa vazia. [R-E05, E04]
59. Como titular, quero que editor e treino não disputem o mesmo teclado, para não confundir entrada e prática. [R-E05, E04]

### Reconhecimento (E05)

60. Como aluno, quero arrastar um PDF e receber uma Hipótese de MusicXML com a proveniência (motor, versão, página), para começar a revisão. [R-B04, R-B05, E05]
61. Como aluno, quero que o app descubra se o PDF é vetorial, escaneado ou misto e escolha o caminho adequado, para melhor resultado. [R-B04, SUP: OMR, E05]
62. Como revisor, quero ver divergências entre motores por compasso e região do PDF, para ir direto ao que pode estar errado. [R-B05, R-B06, E05]
63. Como titular, quero que concordância entre motores nunca seja apresentada como precisão nem aprovação, para não confiar além do que se pode. [T7, R-B05, E05]
64. Como aluno, quero cancelar e retomar um Job de reconhecimento sem duplicar trabalho nem perder o já feito, para controle. [R-B03, E05]
65. Como aluno, quero que o reconhecimento use pouca CPU para não prejudicar o som e o MIDI, para poder tocar enquanto processa. [R-B03, T10, E05]
66. Como titular, quero que o app funcione sem Audiveris e HOMR instalados, e que, quando empacotados, venham como processos separados com avisos e fontes AGPL, para cumprir a licença. [T6, D16, E05]
67. Como mantenedor, quero um benchmark reproduzível em corpus público (CC0 com MusicXML, PDFs e scans degradados gerados dele), para medir sem usar material comprado. [D5, E05]
68. Como mantenedor, quero que as metas de precisão sejam calibradas após o baseline e publicadas como medições, não como promessas, para honestidade. [T7, E05]
69. Como titular, quero conferir apenas uma amostra do gabarito do benchmark, para economizar meu tempo. [D5, E05]
70. Como titular, quero que reconhecimento por LLM multimodal, quando existir, valha só como Hipótese de recortes consentidos, para respeitar D3. [R-B04, D3, E05]

### MCP (E06)

71. Como usuário do Claude Code, quero que um cliente MCP liste partituras do Acervo e leia compassos, para analisar música por conversa. [R-A02, D11, E06]
72. Como usuário, quero buscar e comparar Versões por MCP, para pedir análises. [R-A02, E06]
73. Como usuário, quero que alterações via MCP apareçam primeiro como simulação com diff, para decidir antes. [R-A02, E06]
74. Como usuário, quero aprovar manualmente cada alteração proposta, e que uma Versão desatualizada seja recusada, para não sobrescrever trabalho. [R-A02, T8, E06]
75. Como mantenedor, quero que o MCP não aceite SQL, shell nem caminho arbitrário, para segurança. [R-A02, E06]
76. Como mantenedor, quero que fechar o MCP não afete o app, para robustez. [R-A02, E06]

### Professor inteligente (E07)

77. Como aluno, quero conversar com um professor que explica minha prática, usando Claude, OpenAI, Gemini ou Ollama, para escolher o provedor. [R-A03, D6, E07]
78. Como aluno, quero que o professor use ferramentas que calculam (compassos difíceis, métricas) e só explique, para respostas confiáveis. [R-A03, R-L02, E07]
79. Como titular, quero ser perguntado antes de qualquer envio de material comprado à nuvem, por lote, e ter o registro do que saiu, para controle. [D3, T3, E07]
80. Como titular, quero que o gasto em nuvem pare ao chegar a US$ 10 no mês, e Ollama local nunca pare, para custo previsível. [D18, T4, E07]
81. Como aluno, quero que instruções escondidas num PDF não executem ações, para segurança. [R-A03, E07]
82. Como aluno, quero que o app continue funcionando sem provedor ou sem rede, para não depender de IA. [T1, R-A05, E07]
83. Como aluno, quero planos de estudo propostos pela IA somente com minha aprovação, para decidir o que praticar. [R-L02, E07]

### Plugins e produção (E08, E09)

84. Como autor, quero plugins Luau assinados pelo titular que propõem patches reversíveis (ex.: dedilhado), para estender o ensino. [R-A04, D15, E08]
85. Como titular, quero que um plugin hostil seja bloqueado e auditado, e que plugin não assinado nunca rode, para segurança. [R-A04, D15, E08]
86. Como autor, quero plugin Python de reconhecimento em processo isolado, recusado se o isolamento de rede não existir no Linux de referência, para segurança. [R-A04, SUP: Python, E08]
87. Como titular, quero uma pesquisa de produção musical (trilhas MIDI, piano roll, exportação) sem compromisso de escopo, para decidir depois. [R-P01, E09]

---

## Decisões de implementação

### Arquitetura geral (valem para todas as fases)

- **Núcleo puro**: o modelo de partitura, o serviço de edição, a validação e a exportação ficam no núcleo, sem interface gráfica, áudio, rede ou LLM. UI, CLI, MCP e plugins são adaptadores finos do mesmo serviço (R-A01). Camada geométrica de layout própria (sem tipos de egui), com adaptador para egui.
- **Menor número de módulos**: nada de crates novos "por organização". Novos limites só onde há motivo concreto: serviço de documento (núcleo), Acervo (um serviço dono da escrita no banco), binário MCP separado, workers de reconhecimento como processos externos.
- **Modelo de documento híbrido (SUP)**: o modelo semântico tipado guarda `IdNota` estável (monotônico, nunca reutilizado, atribuído na leitura, nunca índice), o texto original e, por entidade (parte, compasso, nota), a faixa de bytes de origem e um indicador de "sujo". Na exportação, entidade não suja é copiada byte a byte; entidade suja é regenerada com os campos conhecidos na ordem do esquema, reaproveitando os filhos desconhecidos dela; tudo fora dos compassos (créditos, padrões, lista de partes, identificação, comentários) fica intacto. As faixas só valem para o texto original (patches aplicados uma única vez, de trás para frente). Durações em `Fracao` exata; na exportação `divisions` é o MMC dos denominadores; valor não representável é erro explícito, nunca arredondamento. Persistência dos `IdNota`: usar o `id` do MusicXML quando existir; senão um arquivo lateral. Tudo isso é hipótese a validar no spike de E01 antes de travar o ADR.
- **Forma essencial dos tipos de edição** (ilustrativa): `IdNota` opaco; `Comando` é uma soma tipada (alterar altura por semitons/oitavas/escala, alterar duração com figura e pontos, inserir nota ou pausa em posição de compasso, remover nota, adicionar/retirar altura de acorde, mais comandos de E03 depois); `Resultado` traz o modelo novo, o inverso (ou snapshot do compasso afetado) e a lista de avisos; falha não altera nada.
- **Versão**: estado salvo e imutável, identificado por hash de conteúdo, com Versão-pai, autor, data, nota de motivo e estado de conteúdo (Hipótese, Em revisão, Aprovada, Rejeitada). A Hipótese e a fonte são as primeiras Versões. Aprovação é um atributo da Versão (por hash), nunca do documento em edição.
- **Estados** reaproveitam a máquina existente (Hipótese → Em revisão → Aprovada/Rejeitada, aprovação direta marcada); estados de Job não se misturam com estados de conteúdo.
- **Render**: layout próprio em egui com fonte SMuFL Bravura (OFL-1.1) e hit test por nota; sem paginação na E01 (sistemas e fita como hoje); relayout incremental só se o baseline medir necessidade. Verovio apenas oráculo em processo separado para comparação.
- **Atalhos**: perfil padrão MuseScore, remapeável, com detecção de conflito contra a casca (Ctrl+K, Pânico Ctrl+., Esc). Cifra em Ctrl+Shift+K; redo em Ctrl+Shift+Z; setas ↑/↓ semitom; Alt+Shift+seta pela escala; durações 1–9.
- **Segurança**: limites de tamanho, XXE desligado, proteção contra zip bomb e traversal em MXL; nenhum shell concatenado; processos externos com diretório por Job, timeout e cancelamento; importação nunca executa conteúdo.
- **Rede**: toda saída passa por um único portão de rede (nenhuma funcionalidade abre HTTP por conta própria); consentimento por lote registrado; teto mensal com parada obrigatória; chaves no cofre do SO.

### E01 — Oficina de correção (detalhado)

**Objetivo demonstrável.** O titular abre uma partitura de piano já convertida (duas pautas), com o PDF original ao lado, corrige um erro de altura, um de duração, remove uma nota falsa e insere uma pausa e um acorde, desfaz e refaz, fecha o app sem salvar e recupera o trabalho, salva uma Versão, vê que a Hipótese original continua byte a byte igual, aprova a Versão e a pratica com os Exercícios existentes.

**Dentro do escopo:**
- Abrir uma partitura MusicXML/MXL já presente no repositório de oficina/revisão atual e seu PDF; sem importação em lote (E02).
- Serviço de documento com `IdNota`, origem por faixa de bytes e regeneração só do compasso editado.
- Comandos: alterar altura (semitom, oitava, escala), alterar duração (figura e pontos), inserir nota, inserir pausa, remover nota (vira pausa), adicionar e retirar altura de acorde.
- Validação antes de aplicar: soma das durações da voz igual à do compasso (ou compasso incompleto sinalizado, como em anacruse), altura dentro de limites, voz e pauta existentes.
- Transação com undo/redo por snapshot do compasso afetado; autosave com recuperação após queda.
- Seleção de nota no render, cursor de entrada, navegação por setas e Tab; perfil mínimo de atalhos.
- Painel de PDF ao lado com sincronismo por compasso.
- Salvar como Versão; lista de Versões; diff por compasso; aprovar, rejeitar e reabrir com motivo; aprovação por hash.
- Praticar a Versão: ligar a Versão aprovada (ou Hipótese em modo sinalizado) aos Exercícios e à prática existentes.
- Exibir e preservar letra, cifra, dedilhado, dinâmicas, ornamentos e o que mais houver; lista visível de elementos não suportados.
- Spike do render (Bravura × oráculo Verovio) e spike do modelo híbrido (corpus CC0), ambos antes de travar ADRs.
- Spike opcional de MCP somente leitura (ganchos), sem entregar o MCP.

**Fora da E01:** editar letra, cifra, dedilhado; ligaduras novas; tercinas novas; múltiplas partes; importação no Acervo; qualquer OMR; entrada MIDI no editor (a prática com o teclado já existente continua como está); arrastar com mouse; paginação.

**Critérios de aceite (Given/When/Then):**

| # | Critério |
|---|---|
| A1 | Dado um arquivo MusicXML do corpus de fixtures, quando abro e exporto sem editar, então o resultado é idêntico byte a byte ao original. |
| A2 | Dado um compasso com duas vozes e uma nota de altura errada, quando altero uma altura, então a exportação difere do original apenas nesse compasso e os demais compassos, créditos, lista de partes e elementos desconhecidos permanecem idênticos byte a byte. |
| A3 | Dada uma partitura com letra, cifra e dedilhado, quando edito a altura de uma nota do mesmo compasso e salvo, então letra, cifra e dedilhado desse e dos demais compassos permanecem presentes e associados às mesmas notas. |
| A4 | Dado um compasso 4/4 completo, quando altero a duração de uma nota de modo que a soma estoure o compasso, então o comando é recusado com mensagem, o modelo fica inalterado e o undo não ganha entrada. |
| A5 | Dadas N edições, quando desfaço N vezes, então o modelo exportado é idêntico ao original; quando refaço N vezes, é idêntico ao estado final. |
| A6 | Dada uma partitura com anacruse, duas vozes, tercina e repetição, quando edito uma nota e exporto, então apenas a mudança autorizada e suas consequências declaradas diferem semanticamente (comparação semântica no teste), e nenhuma duração é preenchida sem aviso. |
| A7 | Dado trabalho não salvo, quando o app é encerrado abruptamente e reaberto, então o trabalho é oferecido para recuperação com no máximo o intervalo de autosave perdido. |
| A8 | Dada uma Hipótese aberta, quando salvo duas correções, então existem duas novas Versões encadeadas e a Hipótese continua com o mesmo hash. |
| A9 | Dada uma Versão Aprovada, quando faço uma edição e salvo, então a nova Versão está Em revisão e a aprovação da anterior não vale para ela (hash diferente). |
| A10 | Dadas duas Versões, quando peço o diff, então são listados os compassos alterados com antes e depois. |
| A11 | Dado o atalho Ctrl+. e Ctrl+K, quando a Oficina está em foco, então o Pânico e a paleta da casca disparam como antes. |
| A12 | Dada uma nota selecionada, quando pressiono ↑, então a altura sobe um semitom; com Alt+Shift+↑, sobe um grau da escala; com Ctrl+↑, uma oitava; o resultado é igual ao comando equivalente do serviço. |
| A13 | Dado um MusicXML malformado, com entidade externa, ou um MXL com zip bomb ou caminho fora do escopo, quando abro, então o app mostra erro claro, nada é lido fora do limite e não há crash. |
| A14 | Dada uma Versão Aprovada, quando a pratico, então a prática existente (espera, nota a nota, loop, andamento) funciona com as notas corrigidas; uma Hipótese só pode ser praticada em modo sinalizado. |
| A15 | Dado um elemento sem suporte de renderização na partitura, quando abro, então a lista de não suportados o menciona e o elemento sobrevive ao salvar. |
| A16 | Dado o corpus CC0, quando meço comandos simples numa peça de 100 compassos, então o resultado é registrado como baseline; a meta de 100 ms é "a calibrar", não prometida. |
| A17 | Dada uma Hipótese convertida de um PDF, quando a abro na Oficina, então o PDF original aparece ao lado, com zoom e rolagem, e selecionar um compasso na partitura leva à página correspondente do PDF (por página quando não houver região mapeada). |
| A18 | Dada uma partitura com letra, cifra e dedilhado, quando a abro, então os três aparecem desenhados junto às notas a que pertencem (edição desses textos fica para a E03, D13). |

**Gate de encerramento da E01:** (1) todos os critérios automatizáveis A1–A13, A15 passam (A17–A18 por teste de modelo e captura de tela) em CI no Linux de referência; (2) fmt, clippy com avisos como erro, testes do workspace e build release verdes, com baseline anterior sem regressão (1053 testes verdes, 6 ignorados, na linha de base da E00); (3) code-review independente sem P0/P1; (4) ADR do modelo de documento e ADR de render atualizados com evidência dos spikes; (5) demonstração do titular, com aceite humano explícito e a matriz de rastreabilidade preenchida; (6) itens NÃO VERIFICADOS declarados abaixo não são convertidos em PASS por inferência.

**NÃO VERIFICADO sem hardware na E01:** legibilidade visual do render (julgamento humano, checklist de escuta/visual assinado); som da nota editada em áudio real; prática com o REVAS físico (61 teclas), inclusive latência MIDI→áudio e MIDI→tela com a Oficina aberta; estabilidade sob uso prolongado; comportamento no macOS (evidência separada e sem valor de Gate). O texto de aceite A14 é verificado em CI por simulação; sua versão com teclado real é NÃO VERIFICADO.

### E02 — Acervo (nível de épico)

- Objetivo demonstrável: importar uma pasta mista (PDF, MusicXML, MXL) com deduplicação por hash, achar uma partitura por título e letra com acento, reiniciar o app e ver tudo persistido, restaurar um backup e conferir hashes iguais.
- Decisões: banco SQLite com escada de migrações e rollback; um único serviço dono da escrita; cópia imutável por SHA-256 (D4); metadados desconhecidos nulos; busca de texto completo com normalização de acentos; Versões ligadas à partitura; backup com manifesto. Verificar o conflito de versões do driver SQLite com o armazém existente antes de adotar. Uma única estratégia de migração (sem dois bancos).
- Critérios Given/When/Then: dado um lote de 100 arquivos mistos com duplicatas, quando importo, então há uma cópia por hash e os malformados são rejeitados com motivo sem travar; dada a busca "coração" sobre texto com "coracao", então o resultado aparece; dado um backup, quando restauro em Acervo vazio, então todos os hashes coincidem; dada uma migração, então o rollback restaura o esquema anterior; 10 mil metadados sintéticos, p95 de busca registrado e comparado à meta "a calibrar" de 250 ms.
- Gate: critérios acima em CI; aceite do titular; sem material comprado em fixtures.
- NÃO VERIFICADO sem hardware: desempenho no PC de referência com disco real e Acervo grande; macOS.

### E03 — Editor completo (nível de épico)

- Objetivo: corrigir uma lead sheet e uma peça de piano com letra, cifra, dedilhado, ligaduras, tercinas, vozes e repetições, com perfil de atalhos MuseScore e remapeamento.
- Decisões: novos comandos tipados no mesmo serviço; matriz por elemento MusicXML (importar, renderizar, tocar, editar, exportar) publicada, com "não suportado" explícito; índices de letra e cifra revisados após edição, com órfãos sinalizados; escopo piano duas pautas e lead sheet (D2), multi-partes fora.
- Critérios: dado um compasso com duas estrofes, quando altero uma altura, então ritmo, voz, sílabas e dedilhado são preservados salvo alteração explícita e undo restaura o modelo; dada uma tercina criada, então a soma do compasso fecha e a exportação reimporta igual; dado conflito de atalho remapeado, então é sinalizado; round-trip do corpus crítico sem perda silenciosa.
- Gate: testes de polifonia, anacruse, ligaduras, tercinas, vozes e repetições; aceite do titular.
- NÃO VERIFICADO sem hardware: qualidade tipográfica percebida; leitor de tela real; macOS.

### E04 — REVAS no editor (nível de épico)

- Objetivo: inserir música tocando; trocar altura preservando duração, voz e pauta; gravar com metrônomo e quantizar sem destruir o MIDI bruto.
- Decisões: eventos MIDI viram comandos do mesmo serviço de edição; tratar NoteOn com velocidade 0, note-off, sustain, desconexão, all-notes-off, 61 teclas e notas fora do alcance; roteamento mutuamente exclusivo entre editor e treino; pedal só após calibração; mouse e PC sempre funcionam.
- Critérios (simulados em CI com eventos sintéticos): dado step-time com duração 1/4, quando chegam NoteOn/NoteOff, então a nota é inserida com a duração; dado acorde dentro da janela, então vira um acorde; dado re-pitch, então duração, voz e pauta ficam; dada desconexão, então nenhuma nota fica presa; dada Gravação quantizada, então o MIDI bruto continua recuperável.
- Gate: teste físico no REVAS (61 teclas, 10 notas simultâneas), latência p95 e jitter sob carga, 60 minutos sem crash; sem equipamento o Gate não é aprovado.
- NÃO VERIFICADO sem hardware: **tudo que depende do teclado físico, do pedal e da latência**.

### E05 — Reconhecimento (nível de épico)

- Objetivo: PDF vetorial ou escaneado vira Hipótese com proveniência; divergências aparecem na Oficina; benchmark público reproduzível.
- Decisões: contrato de worker JSON com versão de esquema (identificador do Job, entradas, opções, progresso, candidatos, diagnósticos, proveniência); timeouts, cancelamento, limites, diretório por Job, sem shell; Audiveris (CPU) e o segundo motor candidato (HOMR ou alternativa só se replicada localmente) como processos separados; roteamento vetor × scan; fusão só detecta discordância; fila durável e retomável; sem rede por padrão; nada vira Gabarito automático; PDFtoMusic Pro fora (D17). Primeiro passo: PoC e baseline medido, depois escolha do motor com evidência.
- Corpus: peças com MusicXML de licença verificada (ex.: OpenScore Lieder CC0, com conversão a verificar), PDFs e scans degradados gerados deles, mais engraving próprio de peças em domínio público brasileiro para cobrir letras; scans IMSLP somente com checagem por edição; redistribuir só métricas e scripts; MUSCIMA++ (CC BY-NC-SA) só uso interno não comercial, a confirmar. Sem corpus público verificado de cifra e dedilhado: lacuna declarada.
- Critérios: dado scan ambíguo e dois motores divergentes, então mostrar candidatos com página, região e confiança sem substituir o original; dada falha de um motor, então o outro segue; dado cancelamento e reinício, então sem duplicar nem aprovar arquivo parcial; dado PDF com instrução maliciosa, então tratado como dado. Métricas por estrato publicadas, metas "a calibrar após baseline".
- Gate: contrato de worker testado com motor falso; benchmark rodado e registrado; gates de latência medidos com OMR concorrente; aceite do titular.
- NÃO VERIFICADO sem hardware: latência de MIDI/áudio sob OMR, uso de CPU/GPU no PC de referência, desempenho em macOS, qualidade em material comprado (testado só localmente, nunca em CI).

### E06 — MCP (nível de épico)

- Objetivo: cliente MCP independente lista partituras e Versões, lê compasso, busca, dispara Job local; depois escrita via patch.
- Decisões: binário separado com runtime assíncrono confinado a ele; rmcp oficial, versão fixada; transporte stdio; resources (Acervo, partitura, Versão) e tools (busca, compasso, análise, comparação, status de Job); erros estáveis; descoberta de capacidades. Escrita só em segunda etapa: patch tipado, simulação (dry-run), diff, aprovação humana e bloqueio otimista por hash da Versão; escrita sempre cria nova Versão. Verificação de dependências antes de adotar o SDK.
- Critérios: dado cliente independente, então lista e consulta; dado patch sobre Versão desatualizada, então recusado; dado caminho externo ou argumento inválido, então recusado; dado app fechado ou MCP encerrado, então o outro lado segue.
- Gate: E2E com cliente independente; auditoria registrada; aceite do titular.
- NÃO VERIFICADO sem hardware: o fluxo com o Claude Code do titular até ele testar; macOS.

### E07 — Professor inteligente (nível de épico)

- Objetivo: chat no app com quatro provedores, ferramentas compartilhadas com o MCP, custo controlado.
- Decisões: provedores por camada fina própria; Claude, OpenAI, Gemini, Ollama (D6); ferramentas devolvem evidência calculada pelo Rust, o modelo explica e propõe; contexto mínimo; memória pedagógica por opt-in; limite de passos, tokens, custo e tempo; teto US$ 10/mês com parada (D18); consentimento por lote e registro (D3); logs auditáveis com segredos redigidos; conteúdo de PDF é dado, não instrução; fallback offline.
- Critérios: dados dois provedores (um local ou simulado por contrato, mais um teste real separado), então fluxo completo; dado teto atingido, então chamada recusada; dado prompt injection em PDF, então nenhuma ação executada; dado offline, então edição e treino seguem.
- Gate: 10 cenários pedagógicos com cálculos conferidos pelo Rust; nenhum Gabarito alterado sem aprovação; aceite do titular.
- NÃO VERIFICADO sem hardware: provedores reais pagos sem chaves do titular; Ollama no hardware real; qualidade de voz.

### E08 — Plugins (nível de épico)

- Objetivo: plugin Luau assinado (dedilhado ou adaptação) propondo patch reversível; plugin Python isolado de reconhecimento ou análise.
- Decisões: API semântica de partitura sobre o mesmo serviço; manifesto (id, API, licença, capacidades, hashes, permissões); somente assinados pelo titular (D15); Luau determinístico (SUP); recusa de Python sem isolamento de rede no Linux de referência; quarentena, rollback e teste hostil; WASM adiado.
- Critérios: dado plugin hostil (arquivo fora do escopo, laço infinito), então bloqueado e auditado; versão incompatível rejeitada; plugin não assinado não executa.
- Gate: fixtures de terceiros e API documentada; aceite do titular.
- NÃO VERIFICADO sem hardware: isolamento de rede no kernel do PC de referência e no macOS.

### E09 — Produção musical (somente pesquisa)

- Objetivo: documento de pesquisa e spike descartável (duas trilhas MIDI, gravação, piano roll, exportação MIDI/WAV, relógio único), sem promessa de escopo, VST/CLAP ou DAW; nova especificação só após aceite.
- Gate: relatório aceito pelo titular. NÃO VERIFICADO sem hardware: ensaio de 60 minutos e áudio.

---

## Seams de teste

Princípio: menor número de seams, o mais alto possível; seams existentes têm preferência. Três seams novos e um existente.

| # | Seam | O que atravessa | Fases | Como se testa |
|---|---|---|---|---|
| S1 | **Serviço de documento de partitura**: interface única que abre um MusicXML/MXL, aplica comandos tipados (inclusive de entrada MIDI na E04, de patch na E06 e de plugin na E08), faz undo/redo, valida, gera Versão e exporta. A UI, o MCP e os plugins só chamam este serviço. | MusicXML ↔ modelo ↔ Versão | E01, E03, E04, E06, E08 | Sem GUI. (a) **Golden de round-trip**: abrir e exportar o corpus de fixtures deve ser idêntico byte a byte; edição única deve diferir só no compasso editado; (b) comparação semântica de MusicXML após edição; (c) propriedades: undo N vezes devolve o original; comando inválido não muda nada; (d) fuzz do parser e do MXL (XXE, zip bomb, traversal). |
| S2 | **Serviço do Acervo**: importar, deduplicar, buscar, listar Versões, backup e restauração, migrar. | arquivos ↔ SQLite | E02, E05, E06 | Banco temporário real (sem mock do SQLite); lotes sintéticos de 100 e 10 mil; migração com rollback; backup com hashes. |
| S3 | **Fronteira de processo**: contrato JSON versionado dos workers (reconhecimento, plugins Python) e protocolo MCP em stdio; o mesmo princípio para o contrato de provedor de IA. | app ↔ processo externo | E05, E06, E07, E08 | Worker falso que obedece o contrato (sucesso, falha, lentidão, cancelamento, saída malformada); cliente MCP independente; provedor simulado por contrato mais um teste real separado e opcional. |
| S4 (existente) | **Prática e comparação MIDI determinística** do núcleo atual. | eventos MIDI ↔ Gabarito | E01, E04, E07 | Fixtures de acertos e erros já existentes; E01 só adiciona Versões como entrada. |

Fora dos seams automatizados (verificação humana ou física): render visual e legibilidade, áudio, REVAS e latência.

---

## Decisões de teste

- **Bom teste** verifica comportamento externo através de um dos seams (resultado exportado, Versão salva, resultado de busca, resposta de protocolo), nunca detalhes internos (estruturas privadas, ordem de chamadas). Um teste por critério Given/When/Then, mais propriedades onde cabe (undo/redo, idempotência de importação).
- **TDD** por fatia vertical: o teste falha primeiro (red), depois passa (green), depois refatora; evidência registrada.
- **Corpus de fixtures**: somente licenças verificadas (OpenScore Lieder CC0 já parcialmente presente, fixtures próprias criadas para casos específicos como anacruse, vozes, tercina, repetição, letra, cifra, dedilhado). Material comprado nunca em fixtures (T2).
- **Comparação semântica de MusicXML** (não textual) para edições; textual byte a byte somente para o que não foi editado.
- **Prior art** no repositório: testes do núcleo de partitura (1053 testes verdes na linha de base), máquina de estados de revisão, limites de segurança de XML/MXL com testes, roteiros Luau sandboxados, pacote assinado, `Fracao` racional exata.
- **Baseline antes de meta**: qualquer desempenho (edição em 100 compassos, busca, OMR) é medido e registrado primeiro; metas são calibradas, nunca rebaixadas em silêncio.
- **Gates físicos** (REVAS, latência, 60 minutos, áudio, macOS) não são cobertos por CI: ficam NÃO VERIFICADO até medição humana no PC de referência. Todas as fases listam o que continua NÃO VERIFICADO.
- **Matriz de rastreabilidade** por ticket: R-ID, história, cenário, teste, fixture e licença, evidência, risco e dono; estados PASS/FAIL/NÃO VERIFICADO/N/A.
- **Definição de pronto** da fase segue 04-DEFINICAO-DE-PRONTO: fmt, clippy com avisos como erro, testes do workspace, build release, code-review independente sem P0/P1, auditoria de dependências e licenças, aceite humano.

---

## Fora de escopo

- Edição de múltiplas partes além de piano em duas pautas e lead sheet (D2), até depois da E03.
- Editar letra, cifra e dedilhado na E01 (D13); eles são exibidos e preservados.
- Promessas de precisão de OMR; aprovação ou Gabarito automáticos; fusão como fonte de verdade.
- Uso de PDFtoMusic Pro (D17) e de qualquer ferramenta proprietária paga no benchmark.
- Empacotamento de Audiveris e HOMR (adiado para 2027, D16); instalação automática de motores.
- GPU obrigatória (D12); suporte a macOS como Gate (só teste pessoal).
- Plugins de terceiros não assinados (D15); WASM; plugins nativos arbitrários.
- Produção musical, VST/CLAP, DAW completa (E09 é só pesquisa).
- Busca por contorno melódico e intervalos; entrada de fala; sincronização em nuvem; telemetria; atualização automática.
- Redistribuir material comprado, scans sem checagem de edição ou datasets sem licença verificada (GrandStaff, OLiMPiC e similares até checar).
- Renomear ADRs duplicadas (apenas anotar); sincronizar a matriz de aceite fica em PR separado.

---

## Notas adicionais

- **ADR-0015** (próxima numeração livre) registra: edição de partitura passa a existir e supera a restrição da ADR-0007 pós-F13; modelo de documento híbrido; render próprio com Bravura e Verovio como oráculo. Ficam para ADRs seguintes, na hora certa: contrato de worker, política de nuvem e consentimento, MCP e provedores.
- **Pontos de atenção do diagnóstico** a tratar já na E01: tela monolítica (extrair serviço de documento antes do editor), layout acoplado a egui, seleção hoje só por compasso, ausência de benchmark, deriva documental, Gates físicos NÃO VERIFICADOS (F01/F02/F03/F14).
- **Referência de risco OMR**: a PoC mostrou leitura correta de alturas num piano sintético, mas nenhuma letra nem dedilhado; resultado de uma fixture, não generalizável.
- **Inconsistências entre fontes** a resolver (também listadas no relatório da etapa): (1) D5 fala em gabarito MusicXML do OpenScore Lieder, mas a pesquisa de corpus informa que o corpus está em formato do MuseScore e exige conversão; (2) as pesquisas e o roteiro da 1ª fatia usam "revisão" onde D8 manda "Versão"; (3) D16 manda empacotar AGPL "junto" mas mantém o empacotamento adiado para 2027; (4) a proposta antiga ordena Acervo/OMR antes do editor, invertida por D1; (5) R-B04 previa PDFtoMusic como benchmark opcional, removido por D17; (6) R-L01/R-L02 não têm fase no roteiro reordenado, aqui distribuídos (E01, E04, E07).
- **Perguntas em aberto ao titular** (não bloqueiam a E01): confirmar o uso interno de MUSCIMA++ (licença não comercial); aceitar a conversão por MuseScore para obter MusicXML do corpus CC0; definir cobertura de letras em português por peças de domínio público escolhidas.

### Resoluções do supervisor (2026-11-27)

- **OpenScore Lieder em .mscx:** a conversão para MusicXML usa o MuseScore como ferramenta externa só na preparação do corpus (fora do app e da CI); o MusicXML gerado é CC0 e pode entrar como fixture, com o comando e a versão do MuseScore registrados. Gabarito = esse MusicXML, conferido por amostra (D5).
- **MUSCIMA++ (CC BY-NC-SA, manuscrito):** fora do corpus. O alvo é partitura impressa e o projeto é código aberto (D9).
- **"revisão" nas pesquisas 01 e 03:** são registros históricos; o termo canônico é Versão (D8).
- **D11:** "chat no app depois" = E07.
