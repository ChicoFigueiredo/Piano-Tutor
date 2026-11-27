# 25: Praticar a Versão com os Exercícios existentes, e ouvir o compasso editado (A14)

**Fase:** E01

**R-IDs/critérios:** R-L01 (parcial), T8; A14; histórias 24, 25, 26

**Seam:** S4 (entrada nova: Versão); S1

**Modelo sugerido:** Opus / médio (contrato entre a Versão e a prática existente)

**What to build:** O aluno pratica a Versão salva (tocar, esperar, nota a nota, loops, andamento) com os Exercícios e a prática existentes, usando as notas corrigidas. Uma Versão Aprovada pratica normalmente; uma Hipótese só pode ser praticada em modo claramente sinalizado. O revisor também ouve o compasso editado para conferir de ouvido.

**Blocked by:** 23 (Aprovar, rejeitar e reabrir uma Versão com motivo, ligadas ao hash (A9))

**Status:** feito, aguarda revisão (módulo `pratica_versao.rs` e API para a tela; os botões Praticar Versão e Ouvir compasso entram com a ligação da tela, que outra equipe mexe)

- [x] A14: com uma Versão Aprovada, a prática existente funciona com as notas corrigidas (teste por simulação de eventos MIDI sobre as fixtures de acertos e erros já existentes, S4). (teste `a14_versao_aprovada_pratica_com_as_notas_corrigidas`: Ode com a primeira nota corrigida para Fá4, 94 de 94 certas e 1 extra, como a fixture da Ode; tocar o Mi4 da Hipótese não acerta o passo)
- [x] Uma Hipótese ou Versão não aprovada só abre em modo sinalizado como Hipótese; nunca aparece como Gabarito oficial (teste do rótulo e do bloqueio). (testes `hipotese_e_em_revisao_so_abrem_sinalizadas_...`, `aprovada_so_com_registro_...`, `rejeitada_nunca_abre_...`)
- [x] A prática lê as notas do serviço de documento da Versão por hash; mudar a Versão invalida a sessão de prática anterior (teste). (testes `abrir_outra_versao_invalida_a_sessao_anterior`, `reabrir_ou_rejeitar_a_versao_invalida_...`, `editar_a_partitura_depois_de_abrir_invalida_a_sessao`, `versao_adulterada_no_disco_e_recusada`)
- [x] Ouvir o compasso editado toca as notas do compasso pelo serviço de som existente; o teste valida o plano de reprodução gerado; o som real e o REVAS físico são NÃO VERIFICADOS. (testes `plano_do_compasso_...` e `ouvir_o_compasso_toca_pelo_servico_de_som_existente`, este com o motor falso: o som real segue NÃO VERIFICADO)
- [x] Todos os testes de prática existentes continuam verdes sem mudança de assertivas. (`cargo test --workspace`: 1380 passaram, 0 falharam; nenhum teste existente foi editado)
- [x] Prática com o REVAS físico (61 teclas), latência MIDI→áudio/MIDI→tela com a Oficina aberta e estabilidade prolongada ficam registrados como NÃO VERIFICADO no relatório do ticket.

**Notas:** O texto de aceite A14 é verificado por simulação em CI; a versão com teclado real depende do PC de referência.

**Implementação (2026-11-28):** `crates/piano-tutor-app/src/pratica_versao.rs` (novo), `importar_com` e `DecideEstado` em `crates/piano-tutor-curso/src/importacao.rs`. Decisões:
- API para a tela: `PraticaDaVersao::abrir(versoes, hash, permitir_hipotese, &mut EstadoPartitura)`, `.iniciar(...)`, `.conferir(...)`, `.sincronizar(...)`, `.rotulo()`, `.gabarito_oficial()`; `plano_do_compasso(&EstadoPartitura, compasso, t0_ns)` e `ouvir_compasso(&EstadoPartitura, &mut ServicoSom, compasso)`. Nenhuma mudança em `tela_partitura.rs`, `comando.rs` nem nos módulos de prática: a prática existente (Nota a nota, Tempo, Espera tocar, loops, andamento) recebe só o documento da Versão por `abrir_xml`.
- Aprovada (estado Aprovada **e** registro `[[decisao]]`) = Gabarito. Não aprovada (Hipótese, Em revisão, ou "aprovada" escrita à mão sem registro) só abre com `permitir_hipotese` e sempre com o selo "HIPÓTESE (...), não é o Gabarito oficial" no nome do documento. Rejeitada nunca abre (mesma regra dos Exercícios de Curso).
- A sessão guarda o hash. `conferir` recusa e `sincronizar` limpa a prática quando o documento aberto deixou de ser a Versão: outro documento aberto (nome e número de notas), qualquer tentativa de edição depois de abrir (`ultima_edicao` preenchida; conservador, sem mexer na tela), Versão adulterada no disco, aprovação reaberta ou rejeitada.
- Ouvir o compasso usa `esperados_para_chamada` (notas do documento aberto, com as edições) filtradas pelo compasso e o `ServicoSom::demonstrar` existente; recusa durante reprodução ou prática, sem som pronto ou compasso vazio.
- Exercício: `DecideEstado` é o ponto de extensão do importador; `EstadoDoArquivo` é o legado (`importar` não mudou de comportamento) e `DecisorPorHash` (app) olha o SHA-256 do conteúdo de `partituras/<id>.musicxml`. Aprovação de uma Versão corrigida só vale para o Exercício cujo arquivo tem aquele conteúdo. O `carregar_exercicio` de Curso instalado segue lendo o `estado` do manifesto assinado, com SHA-256 conferido por arquivo.
- Mutação conferida: trocar a consulta por hash pelo `estado` bruto do índice e remover a checagem do documento aberto fazem testes falharem.

**NÃO VERIFICADO:** prática com o REVAS físico (61 teclas), latência MIDI para áudio e MIDI para tela com a Oficina aberta, estabilidade prolongada e o som real do compasso (os testes usam o motor falso e MIDI simulado). Pendente de ligação: botões de Praticar Versão, selo na tela e Ouvir compasso em `tela_partitura.rs`.
