# Revisão de código independente da E01 (gate, ticket 27)

**Data:** 2026-11-28. **Revisor:** qa-aceite (agente de QA, sem participação na implementação).
**Base da revisão:** `claude/project-thread-w8l1yp` em `c8b93f9` (merge fast-forward do ticket 26), contra `git merge-base origin/main HEAD` = `222b513`.

## 1. Veredito

**O item "Revisão de código independente sem achados P0/P1" do gate NÃO está atendido.** Há 1 achado P0 e 2 P1, todos reproduzidos por teste. Os três são na tela Partitura/Versões (camada de aplicação), não no núcleo de documento: o núcleo (`piano-tutor-core::documento`, `xml_seguro`, `mxl`) não teve achado P0/P1.

| Id | Sev. | Resumo | Reproduzido |
| --- | --- | --- | --- |
| R1 | **P0** | Salvar Versão apaga o rascunho de recuperação que ainda espera decisão do usuário; a tela continua oferecendo "Recuperar", que então falha. Perda do trabalho de uma sessão que caiu. | sim (teste) |
| R2 | **P1** | "Exportar" com "Sobrescrever" aceita como destino o próprio arquivo da Hipótese e o reescreve (não atômico). Quebra "a Hipótese e o arquivo original nunca são alterados" (A8, ticket 21). | sim (teste) |
| R3 | **P1** | Salvar Versão grava sempre `pai` = ponta da cadeia, mesmo quando o documento foi aberto da Hipótese e não descende dela. A linhagem registrada é falsa, e o critério "abrir Versão antiga como novo ponto de partida" está marcado [x] com "UI pendente". | sim (teste) |

Isto **não** é CI verde: a suíte inteira está verde (1530 passaram, 0 falharam, 11 ignorados) e os três achados passam por ela, porque nenhum teste cobre esses cenários.

O aceite humano do titular segue pendente e não é inferido deste relatório.

## 2. Método e comandos (o que foi de fato executado)

- `git merge --ff-only c8b93f9...` (ponta de `claude/project-thread-w8l1yp`; o `git merge FETCH_HEAD` com dois refs no fetch seria ambíguo). Sem push.
- `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace --offline`: exit 0, **1530 passaram, 0 falharam, 11 ignorados**, 0 warnings. (Baseline do gate: 1053 verdes e 6 ignorados; a diferença é de testes acrescentados e de testes `#[ignore]` de benchmark/exaustivos.)
- `cargo fmt --all --check`: sem diferenças.
- **Não executados por mim:** `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --release`, `cargo deny`. O item 2 do gate segue sem evidência minha.
- Leitura de código (todo o diff desde `222b513`, `.rs`) dos alvos pedidos, com atenção a: corretude, segurança, atomicidade, pânicos em código de produção.
- Sondas temporárias (não entram no commit; ver apêndice): 3 testes na tela (R1, R2, R3), 5 + 1 sondas no crate `piano-tutor-fixtures` (ids hostis, números gigantes, entidades, voz dupla, escape), e um **fuzz misto** de todos os comandos de edição.
- Auditoria critério → evidência: para cada ticket 05–26, li os critérios [x] e extraí os identificadores de teste citados. Script de conferência: dos 89 identificadores citados, todos os nomes de teste resolvem para uma `fn` existente; os 11 restantes são módulos, campos ou exemplos (`medir_seguranca` existe em `crates/piano-tutor-fixtures/examples/`). Isso prova que a citação existe, **não** que o teste prova o critério: para isso li o código e o teste nas áreas marcadas na seção 4.

## 3. Achados

### R1 (P0): Salvar Versão destrói o rascunho de recuperação pendente

**Onde:** `crates/piano-tutor-app/src/tela_partitura.rs:1544-1546` (`pode_salvar_versao` ignora `rascunho_pendente`), `:1846-1915` (`salvar_versao`), `:1900` (chama `ao_salvar_versao`), `crates/piano-tutor-app/src/autosave.rs:391-405` (`descartar` apaga a pasta de recuperação). Relacionado: `tela_partitura.rs:1501` (`autosave_tick` fica quieto com a decisão pendente).

**Cenário:**
1. O app cai (ou é fechado à força) com edições não salvas; o autosave já tinha gravado `rascunho.musicxml.rec`.
2. O usuário reabre a partitura. A tela mostra o banner "Recuperar / Descartar" (`rascunho_pendente = Some`), e o documento aberto é o do arquivo (sem as edições da sessão que caiu).
3. O usuário ignora o banner, faz outra edição (nada impede) e clica em "Salvar como Versão".
4. `salvar_versao` salva o estado atual, e na sequência chama `ao_salvar_versao()`, que apaga o rascunho **da sessão anterior**: "O trabalho virou Versão" (comentário em `:1898`) é falso, porque a Versão salva é a do documento aberto, que não contém o trabalho do rascunho.
5. O banner continua visível (`rascunho_pendente` não é limpo) e "Recuperar" responde "Não foi possível recuperar: não há rascunho de recuperação".

**Impacto:** perda irrecuperável do trabalho de uma sessão que caiu, exatamente o que A7/ticket 24 existem para evitar. Enquanto o banner está pendente o `autosave_tick` não grava, então as edições feitas nesse intervalo também não têm proteção.

**Teste que reproduz** (apêndice A.1, `zz_sonda_recuperacao_pendente_e_salvar_versao`). Saída observada:

```
SONDA salvar_versao com recuperacao pendente: true
SONDA arquivo do rascunho existe depois de salvar: false
SONDA banner de recuperacao ainda oferecido: true
SONDA recuperar depois: aviso = Some("Não foi possível recuperar: não há rascunho de recuperação")
```

**Por que a suíte não pega:** o teste `salvar_versao_apaga_o_rascunho_e_descartar_nao_toca_originais_nem_versoes` (`autosave.rs:683`) parte de um rascunho do próprio documento aberto, nunca de um rascunho pendente de outra sessão.

**Correção sugerida (não aplicada, fora do escopo):** recusar `salvar_versao` (ou exigir decisão) enquanto `rascunho_pendente.is_some()`; só chamar `ao_salvar_versao` quando o rascunho for do documento salvo (`recuperado` ou escrito nesta sessão); limpar `rascunho_pendente` junto.

### R2 (P1): Exportar sobre o arquivo da Hipótese

**Onde:** `crates/piano-tutor-app/src/tela_partitura.rs:2591-2599` (`exportar`), `crates/piano-tutor-app/src/documento_partitura.rs:127-143` (`destino_de_exportacao` só valida a extensão) e `:367-377` (`gravar`: `OpenOptions` com `truncate`, escrita direta, sem tmp+rename).

**Cenário:** na tela Partitura de um item da Oficina, o usuário digita no campo de exportação o caminho do próprio `partituras/<id>.musicxml` (ou o `<id>.musicxml` aberto), marca "Sobrescrever" e exporta. O arquivo da Hipótese é truncado e reescrito com o documento editado, junto com o `.ids` lateral (`documento_partitura.rs:~390`). Não há alerta (`alerta_de_originais` = `None`) e nenhuma Versão guarda o que havia antes.

**Impacto:** (a) viola a promessa do ticket 21 / A8, "a Hipótese e o arquivo original nunca são alterados", que só vale para o caminho Salvar Versão; (b) perda do conteúdo original se a gravação cair no meio (truncate seguido de escrita, sem arquivo temporário); (c) a aprovação por hash do item deixa de valer (direção segura), e um item legado `aprovada` sem índice pode ser migrado para o **novo** hash, se a exportação ocorrer antes da primeira abertura da Oficina após a atualização (janela estreita; `migracao_aprovacao.rs`, decisão registrada no ticket 23).

**Teste que reproduz** (apêndice A.3, `zz_sonda_exportar_por_cima_da_hipotese`). Saída:

```
SONDA exportar sobre a Hipótese: ok=true hipotese_mudou=true
SONDA alerta de originais: None
SONDA alerta depois de salvar versao: None
SONDA lista: ["Hipótese"]
```

**Correção sugerida:** recusar destino igual (por `canonicalize`) ao arquivo aberto, ao da Hipótese de qualquer item da Oficina e a objetos de Versões; gravar por tmp+rename.

### R3 (P1): `pai` da Versão é sempre a ponta, não a base do documento

**Onde:** `crates/piano-tutor-app/src/tela_partitura.rs:1862-1864` (passa `pai = None`), `crates/piano-tutor-app/src/versoes.rs:911-922` (`None` vira `indice.versoes.last()`). O mesmo `salvar_versao` já conhece a verdadeira base do documento (`d.base_de_ids()` → `derivada_de`, `:1857-1860`), mas não a usa para o `pai`.

**Cenário:** sessão 1 edita e salva a Versão v1. Sessão 2 abre o arquivo da Hipótese de novo (a Oficina sempre abre `partituras/<id>.musicxml`; "Abrir" uma Versão é só leitura), edita outra nota e salva v2. O índice grava `pai(v2) = v1`, mas o conteúdo de v2 **não contém** a edição de v1. O diff v1→v2 mostra a correção de v1 como se o revisor a tivesse desfeito, e a árvore de Versões afirma uma linhagem que o conteúdo nega. O critério "ligada à Versão-pai" do ticket 21 fica formalmente cumprido com dado errado.

**Relação com o ticket 21:** o critério "abrir uma Versão antiga a carrega em modo leitura ou como novo ponto de partida" está marcado [x] com a anotação "UI pendente" (`backlog/evolucao/21-salvar-versao-e-lista.md:20`). Não existe na tela caminho para editar a partir de uma Versão (busca por "ponto de partida" em `tela_partitura.rs` e `tela_partitura/` sem resultado), então o único jeito de continuar um trabalho é reabrir a Hipótese, que é exatamente o cenário acima. Critério marcado [x] com parte da funcionalidade ausente: conta como P1 de aceite.

**Teste que reproduz** (apêndice A.2, `zz_sonda_sessao_dois_nao_parte_da_ultima_versao`). Saída:

```
SONDA v2.pai == v1: true
SONDA v1 compasso 1 midis: [Some(65), Some(64), Some(65), Some(67), Some(48), Some(43)]
SONDA v2 compasso 1 midis: [Some(64), Some(63), Some(65), Some(67), Some(48), Some(43)]
```

(v2 não tem o Fá4 inicial de v1: parte da Hipótese, mas declara v1 como pai.)

**Correção sugerida:** o documento deve guardar o hash de partida (já existe `base_de_ids`) e `salvar_versao` passar `Some(base)`; ou implementar "Editar a partir desta Versão" e reabrir o critério até existir.

### P2

- **R4 (P2): contador de ids estoura com `.ids` hostil.** `crates/piano-tutor-core/src/documento/ids.rs:77-80` (`self.proximo += 1`) e `crates/piano-tutor-core/src/documento/editar_duracao.rs:~143` (`self.documento.ids.proximo += novos`). `TabelaIds::de_texto` aceita `proximo=18446744073709551615` (a validação em `ids.rs:140` só rejeita ids >= proximo). Reproduzido: `abrir_texto_com_ids` com esse arquivo lateral dá `attempt to add with overflow` em `TabelaIds::alocar` (pânico no perfil de testes). Em release o overflow é silencioso por padrão (contador dá a volta; **não testado** como ficaria o resultado). Alcançável por um `.ids` ao lado do MusicXML ou em `objetos/<hash>.ids`. Correção: `checked_add` com erro `ErroDocumento` e teto de `proximo` na leitura.
- **R5 (P2): o roteiro de demonstração do gate não cabe numa tela.** O gate pede "abrir com PDF ao lado, corrigir altura e duração...". O PDF sincronizado só existe na aba Revisão (`tela_revisao.rs:812 sincronizar_pdf`, `:1396 desenhar_vista`, vista **somente leitura**); a edição só existe na aba Partitura, que não mostra PDF (`grep` de `pdf_sincronizado`/`sincronizar_pdf` fora de `revisao.rs` e `tela_revisao.rs`: nenhum uso). O ticket 21 já registra a pendência "não há botão 'editar este item' entre as abas". A17 está atendido (por teste de modelo, na Revisão); o roteiro como escrito exige alternar abas e perde o "ao lado". Decidir antes da demonstração: ajustar o roteiro ou ligar o PDF à aba Partitura.

### P3

- **R6:** ids duplicados no `.ids` colapsam em silêncio. Reproduzido: 4 chaves de posição com o mesmo `IdNota(1)` abrem sem erro e o índice fica com 1 nota (`total_de_notas = 1` para 4 notas); só a última é endereçável. Editar, remover, desfazer e exportar não corromperam o texto (exportar devolveu 869 bytes), mas 3 notas ficam inalcançáveis por id. Rejeitar duplicatas em `de_texto`.
- **R7:** `leaf()` não escapa o conteúdo (`edicao.rs:81-83`) e `no_de_nota` o usa para voz/pauta (`inserir_remover.rs:315-319,330-332`). Hoje inalcançável: pauta não numérica é recusada na validação ("a pauta «a&b» não é um número") e a voz só passa por `leaf` quando o modelo não tem `<voice>` e `voz != "1"`. Com voz `a&lt;b` o nó é clonado e a exportação reabre e confere. Defesa em profundidade: escapar em `leaf`.
- **R8:** `gravar_atomico` (`revisao.rs:168-182`) faz `sync_all` do arquivo, mas não do diretório depois do `rename`. Em queda de energia o índice pode voltar ao estado anterior (íntegro, mas sem a última Versão/decisão). Durabilidade, não corrupção.
- **R9:** `com_trava` (`versoes.rs:767`) usa trava bloqueante e é chamado na thread da interface (salvar, decidir). Se outro processo segurar `indice.lock`, a janela congela sem timeout. Inferido da leitura; **não reproduzido**.
- **R10:** o teste A5 aleatório (`crates/piano-tutor-fixtures/tests/documento_historico.rs`) só sorteia `MudarAltura`. Os outros comandos dependem de testes unitários por ticket. O fuzz misto desta revisão fecha a lacuna como evidência (seção 5) mas não está no repositório; recomendo adotá-lo como teste.
- **R11:** trilha de testes: o ticket 25 afirma "nenhum teste existente foi editado" (verdade no commit `c898648`), mas o commit seguinte `3e2da8a` (ticket 23, CLI por hash) renomeou e reescreveu `importa_empacota_e_le_com_os_estados_do_estado_toml` para `..._por_hash`. A mudança é legítima e documentada no ticket 23 (a CLI deixou de olhar o `estado.toml`), mas o texto do 25 ficou desatualizado em relação ao estado final.

## 4. Auditoria critério → evidência (tickets 05–26)

Legenda: **PASS** = critérios [x] com testes citados que existem e passam, e (quando "código lido") o código e o teste conferem o critério; **PASS*** = só ticket + nomes de teste + suíte verde, sem leitura profunda; **FAIL** = critério [x] contradito pelos achados; **UNVERIFIED** = sem prova que sustente a afirmação; **NÃO VERIFICADO** = depende de hardware, áudio, janela real ou humano, mantido assim.

| Ticket | Critérios | Resultado | Base e ressalvas |
| --- | --- | --- | --- |
| 05 prefactor documento | 5/5 [x] | PASS* | suíte baseline verde; sem mudança de comportamento verificada por testes existentes |
| 06 prefactor geometria | 5 [x], **1 [ ]** | **UNVERIFIED** (1 aberto) | "sem aumento mensurável no layout" segue aberto no ticket, corretamente. A baseline do 26 dá o valor absoluto (0,15 a 0,17 ms) mas não compara antes/depois: "nenhum aumento" continua NÃO COMPROVADO |
| 07 fixtures e harness golden | 6/6 | PASS* | nomes citados existem |
| 08 spike modelo | 8/8 | PASS* | A1/A2 por prova; A6 parcial, como o ticket declara |
| 09 spike render | 7/7 | PASS* | Verovio opcional e NÃO VERIFICADO na CI, declarado |
| 10 abrir/exportar idêntico | 7/7 | PASS (código lido) | A1/A2; 7 testes citados existem; fuzz misto confirma exportação reabre e confere |
| 11 segurança XML/MXL | 6/6 | PASS (código lido) | A13: DTD permitido, entidade externa nunca resolvida, orçamento de expansão 1 MiB, nós/profundidade 128, zip com nome seguro, `take` e razão de compressão. Todo parse de produção passa por `xml_seguro::analisar` (exceções: testes, `diff.rs` sobre trechos do próprio documento, `container.xml` com DTD desligado). Medição de tempo/memória só nesta máquina |
| 12 selecionar e navegar | 7/7 | PASS (A11 lido) | `partitura_em_foco_navega_sem_atrapalhar_panico_nem_paleta` (`app.rs`) é teste real. Leitor de tela e clique de mouse físico NÃO VERIFICADOS |
| 13 atalhos remapeáveis | 6/6 | PASS* | só ticket, nomes de teste e suíte; `ATALHOS_DA_CASCA`/`reservado()` apenas vistos |
| 14 mudar altura | 8/8 | PASS (código lido) | som real NÃO VERIFICADO |
| 15 transação/undo/redo | 6/6 | PASS com ressalva R10 | A4/A5: `historico.rs` troca instantâneos; desfazer tudo = original byte a byte (fuzz misto, 16 peças) |
| 16 duração | 7/7 | PASS (código lido) | recusa sem rastro verificada no fuzz (2669 recusas, nenhuma mexeu no documento) |
| 17 inserir/remover | 7/7 | PASS (código lido) | lacunas aceitas pelo supervisor (voz só de pausas não se alcança por clique) |
| 18 acorde | 6/6 | PASS (código lido) | tupla não transferida: pendência declarada |
| 19 letra/cifra/dedilhado | 6/6 | PASS* (A18 por teste de modelo) | legibilidade humana, GPU, HiDPI NÃO VERIFICADOS |
| 20 PDF ao lado | 6/6 | PASS* (A17 na Revisão) | ver R5; captura de tela NÃO VERIFICADA |
| 21 salvar Versão e lista | 7/7 | **FAIL** (R2, R3) | critério "novo ponto de partida" [x] com "UI pendente"; "Hipótese nunca alterada" não vale para Exportar |
| 22 diff | 6/6 | PASS* (`diff.rs` do núcleo lido; `versoes_diff.rs` e a tela não reauditados) | A10; a falsa linhagem de R3 contamina o resultado quando o pai é errado |
| 23 aprovar por hash | 6/6 | PASS com ressalva | regra única `decisao_que_aprova` (estado `aprovada` **e** última decisão `para = aprovada`) usada por app, importador e CLI; migração aprova o hash **atual** (decisão documentada); 2 processos reais, Windows, macOS NÃO VERIFICADOS |
| 24 autosave e recuperação | 6/6 | **FAIL** (R1) | A7 cobre queda e recuperação do documento aberto, não com rascunho pendente de outra sessão |
| 25 praticar a Versão | 6/6 | PASS (código lido) | A14 por simulação; REVAS 61 teclas, latências e som real NÃO VERIFICADOS (declarado no próprio ticket) |
| 26 baseline | 5/5 | PASS (registro) | A16 é baseline, não meta; mede o serviço e a conferência em disco, **não** o desenho egui nem o caminho de tela. PC de referência NÃO VERIFICADO |

## 5. Pontos verificados sem achado

- **Fuzz misto** (`zz_revisao_mista`, apêndice A.4): para cada uma das 16 peças do corpus, até 40 edições sorteadas entre altura (semitom/grau/oitava), duração, ponto, remover, retirar altura, acrescentar letra/intervalo, inserir nota e pausa. Verifica, a cada edição: a exportação reabre, `conferir_exportacao` passa, recusa e "sem mudança" não alteram o retrato (texto, notas, modelo e ids) nem a pilha; depois, desfazer tudo = texto original e retrato inicial em cada passo; refazer tudo = estado final. **Resultado: 1 passou, 0 falhou; 525 edições aplicadas, 2669 recusas, 15 s.** Isso é evidência desta revisão, fora do repositório.
- **Números gigantes** (`duration` 2^62, `divisions` 2^40 e 1e9+7, `beats` 1e9 ou 0, `beat-type` 0, `fifths` 100, oitava 9 e -1, `alter` 2.5): sem pânico; `divisions` fora do limite é recusado; nos demais o ciclo editar-desfazer-exportar volta ao original idêntico. (Aceitar `beats=0` e `fifths=100` sem recusa é leniência, não corrupção.)
- **Entidade interna** em `<step>` e `<duration>`: edição, exportação e conferência passam, sem `&p;` residual no resultado.
- **Voz 2 com `backup`:** alongar nota da voz 1 não toma a pausa da voz 2 (exportação íntegra).
- **Versões:** objeto antes do índice (`versoes.rs:937-942`), índice por tmp+rename com `sync_all` (`revisao.rs:168`), `hard_link` sem substituir (`:1171`), trava `indice.lock`, nome do hash validado (`hash_valido`) antes de montar caminho (sem traversal por id), `converter` valida o índice lido.
- **Aprovação por hash:** `Indice::aprovacao` (`versoes.rs:526`) exige as duas condições; estado escrito à mão sem decisão não vale; `DecisorPorHash` lê o hash do arquivo de agora (arquivo alterado perde a aprovação).
- **Autosave:** gravação em thread, tmp com pid e `limpar_temporarios`, cabeçalho com SHA-256 e tamanho; rascunho adulterado é recusado.

## 6. NÃO VERIFICADO (mantido, sem conversão por inferência)

Legibilidade visual do render e dos textos (letra, cifra, dedilhado), GPU real, HiDPI; som em áudio real; REVAS 61 teclas, latência MIDI→áudio e MIDI→tela, estabilidade prolongada; macOS e Windows; leitor de tela; clique de mouse físico; desempenho no PC de referência e sob áudio; "nenhum aumento" do layout do ticket 06; exportações reais de MuseScore/Finale/Sibelius/OMR; dois processos reais disputando `indice.lock` (testado só com threads); clippy com `-D warnings` e build release (não executados por esta revisão).

## 7. Itens do gate 27 ainda abertos (além dos achados)

- Item 3: bloqueado por R1, R2, R3 até serem corrigidos e re-revisados.
- Item 4: `docs/decisoes/ADR-0015-edicao-nativa-e-plataforma-musical.md` **não referencia** a ADR-0017 nem a ADR-0018 (0 ocorrências de "0017" ou "0018").
- Item 5: a matriz **por ticket** (R-ID, história, critério, teste, fixture e licença, evidência, risco, dono) não existe; só há `docs/evolucao-musical/e01/MATRIZ-POR-ELEMENTO.md`, que é outra matriz.
- Itens 1 e 2: A1–A13 e A15 passam na suíte nesta máquina de nuvem; fmt ok; clippy e release não confirmados por mim.
- Item 6 (roteiro de demonstração): ver R5.
- Item 7: aceite humano do titular (nome e data) ou o estado "aguardando aceite".

## Apêndice. Sondas (ficam fora do repositório)

Sondas temporárias, removidas antes do commit. A.1 a A.3 foram inseridas dentro de `tela_partitura::testes` (usam os auxiliares privados `com_oficina`, `editar_uma_nota`, `tick_ate_gravar`, `arquivo_do_rascunho`) e rodadas com `CARGO_PROFILE_DEV_DEBUG=0 cargo test --offline -p piano-tutor-app --lib zz_sonda -- --nocapture --test-threads=1` (3 passaram). `tela_partitura.rs` foi restaurado por `git checkout` depois.

**A.1 (R1)**

```rust
let (_d, mut e, xml) = com_oficina();
editar_uma_nota(&mut e);
e.autosave_tick(0);
let iv = e.config_autosave.intervalo_ms;
tick_ate_gravar(&mut e, iv);
drop(e);                                   // "queda"
let mut novo = EstadoPartitura::novo(None);
novo.abrir_caminho(&xml.to_string_lossy());
assert!(novo.rascunho_pendente().is_some());
novo.navegar(Comando::Seguinte);
novo.navegar(Comando::Seguinte);
novo.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Abaixo);
let r = novo.salvar_versao();              // Ok
assert!(!arquivo_do_rascunho(&xml).exists());      // rascunho apagado
assert!(novo.rascunho_pendente().is_some());       // banner ainda oferecido
novo.recuperar_rascunho();                 // "não há rascunho de recuperação"
```

**A.2 (R3)**: sessão 1 edita e salva v1; sessão 2 reabre o arquivo da Oficina, edita outra nota e salva v2; compara `v2.pai == Some(v1.hash)` (verdadeiro) e os `midi` do compasso 1 de v1 e v2 (v2 não contém a edição de v1).

**A.3 (R2)**: `editar_uma_nota(&mut e); e.sobrescrever = true; e.exportar(<caminho do xml aberto>)` devolve `Ok` e o conteúdo do arquivo muda.

**A.4 (fuzz misto)**: `crates/piano-tutor-fixtures/tests/zz_revisao_mista.rs` (~180 linhas; semente fixa, 40 edições por peça, retrato = texto + notas + modelo + ids), executado com `cargo test --offline -p piano-tutor-fixtures --test zz_revisao_mista -- --nocapture`. **A.5**: sondas de ids (`proximo` máximo, duplicados), números gigantes, entidade em nota, voz dupla e escape, no mesmo crate. Os arquivos das sondas ficaram no scratchpad da sessão, que é efêmero; o texto acima é o registro durável. Reproduzir R4 e R6: `TabelaIds::de_texto("piano-tutor-ids v1\nproximo=18446744073709551615\n")` seguido de `DocumentoAberto::abrir_texto_com_ids`; e quatro linhas `pos\tP1\t0\t<n>\t1` com `proximo=10`.

## Correções aplicadas (2026-11-28, depois do relatório)

Cada correção nasceu de um teste que falhava (red registrado) e depois passou. Estado: **corrigido, aguarda re-revisão independente** (o relatório acima descreve o código anterior às correções e não foi reescrito). Evidência: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace --offline`: 1559 passaram, 0 falharam, 12 ignorados (antes: 1530/0/11).

| Id | Correção | Teste (red → green) |
| --- | --- | --- |
| R1 | Salvar Versão fica **bloqueado** enquanto há rascunho de recuperação pendente (dica "recupere ou descarte"); decisão e motivo no ticket 24. | `tela_partitura::testes_revisao::r1_*` (2 de 2 falhavam) |
| R2 | Exportar recusa destino = arquivo aberto, Hipótese (do item e de qualquer item da Oficina), qualquer arquivo de `<id>.versoes/` e o `.ids` deles (caminho canonizado e inode no Unix); grava tmp+rename / tmp+`hard_link`. | `tela_partitura::testes_exportar` (6 de 7 falhavam) |
| R3 | `pai` = Versão de partida do documento (`Versoes::salvar_a_partir_de`); a tela ganhou "Continuar a partir desta Versão". | `testes_revisao::r3_*` (2 de 3 falhavam), `testes_continuar` (7) |
| R4 | Contador com `checked_add` e teto `TETO_DO_CONTADOR_DE_IDS` (2^62); `.ids` com `proximo` acima do teto é tabela inválida e o texto abre sem ela. | `piano-tutor-fixtures/tests/documento_ids_hostis.rs` (5 de 6 falhavam), `documento_partitura::testes::r4_*` |
| R6 | `.ids` com id repetido, chave repetida ou id >= contador (em qualquer ordem das linhas) é tabela inválida. | mesmo arquivo |
| R8 | `gravar_atomico` (e a gravação de objetos de Versão e da exportação sem substituir) faz `fsync` do diretório depois do `rename`/`link` (Unix; no-op no Windows; falha do fsync do diretório não vira erro). | `revisao::testes::r8_*` (não compilavam: a função não existia). Durabilidade em queda de energia em si: NÃO VERIFICADA (o teste só prova que o diretório é aberto e sincronizado) |
| R10 | Fuzz misto adotado, semente fixa `0xDEAD_BEEF_1234_5678`: 8 edições por peça no CI (~3,5 s em depuração, 125 edições e 188 recusas); versão grande (40 por peça, ~16 s) `#[ignore]`. | `documento_fuzz_misto.rs` (já verde: adoção, sem red) |

Não tratados nesta rodada, por pedido: R5 (PDF na aba Partitura, outra equipe), R7, R9, R11 e os itens 3 a 7 do gate 27.

