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

## Segunda revisão (2026-11-28)

**Revisor:** qa-aceite, segunda passada independente (sem participação nas correções). **Base:** `claude/project-thread-w8l1yp` em `3f1da94` (G2), diff `3b3042e..HEAD` (29 arquivos, ~2600 linhas). Produção não foi alterada: as sondas foram testes temporários, copiados para `docs/evolucao-musical/e01/sondas-revisao/` (fora de qualquer crate, não compilam) e removidos das árvores dos crates.

### 1. Veredito

**O item "Revisão de código independente sem achados P0/P1" do gate NÃO está atendido.** Há 1 achado P1 novo (N1), reproduzido por teste, que reabre por outro caminho o mesmo dano do R1 (perda do trabalho de uma sessão que caiu). Nenhum P0 novo. As correções de R1, R2, R3, R4, R6, R8, R10, G1, G2 e o bloqueio de edição **resistiram** aos cenários novos que tentei, com as ressalvas das seções 2 e 3: o P1 está numa borda do R1 que o bloqueio não cobre (Recuperar com uma Versão aberta em leitura), não no que as correções disseram cobrir.

| Id | Sev. | Resumo | Reproduzido |
| --- | --- | --- | --- |
| N1 | **P1** | "Recuperar" com uma Versão aberta em leitura instala o rascunho como documento "(leitura)"; "Voltar à edição" o descarta, o banner some e o autosave sobrescreve o rascunho da sessão que caiu | sim (sonda) |
| N2 | P2 | Recuperar (ou Descartar) que falha libera a edição mas deixa o autosave mudo na sessão inteira, sem aviso | sim (sonda) |
| N3 | P2 | Exportar sobre um arquivo escolhido na lista de fontes (`escolher`) não é protegido: `arquivo_aberto` fica `None` | sim (sonda) |
| N4 | P3 | "Sobrescrever" por rename substitui arquivo somente leitura e troca link simbólico por arquivo comum (mudança de comportamento) | sim (sonda; ver ressalva de root) |
| N5 | P3 | G2: "o inverso nunca é recusado" é falso para acorde com altura repetida (uníssono, enarmônico) | sim (sonda) |
| N6 | P3 | G1: `>` em valor de atributo do `<accidental>` faz `MudarAltura` e o inverso serem recusados; o inverso normaliza espaços da marca de abertura | sim (sonda) |
| N7 | P3 | `AdicionarAltura` e `InserirNota` aceitam `acidente` livre sem `grafia_valida`: marcação entra no XML (só por API, inalcançável pela UI) | sim (sonda) |
| N8 | P3 | `DefinirAltura` para a mesma altura, só com o nome, em acidente com atributos: `alterou = true` e entrada de histórico sem mudança | sim (sonda) |
| N9 | P3 | Observações menores (mensagem enganosa, `metadata` por quadro, linhagem do recuperado sem `.ids`) | sim / leitura |

### 2. Cada correção, atacada com cenário novo

| Item | Resultado | O que tentei (e o que aconteceu) |
| --- | --- | --- |
| R1 + bloqueio de edição | **PASS parcial** (N1, N2) | Com rascunho pendente, 11 de 11 entradas de edição recusadas e documento intacto (altura, duração, ponto, inserir nota, inserir pausa, remover, acorde letra/intervalo/retirar, desfazer, refazer); salvar recusado; "Descartar" devolve a edição e o autosave volta a gravar. **Falha:** abrir uma Versão em leitura com o banner pendente e clicar "Recuperar" (N1); "Recuperar" que falha (N2). |
| R2 | **PASS** com N3, N4 | Recusados: atalho de **pasta** para `partituras/`, caminho relativo (`ode.musicxml` e `./ode.musicxml` com cwd na pasta, `partituras/ode.musicxml`, `partituras/../partituras/ode.musicxml`, sem extensão), link simbólico para objeto de Versão, destino depois de "Continuar a partir desta Versão". Link físico de um objeto de Versão fora de `.versoes/` é aceito, mas o rename troca só o nome: o inode do objeto não mudou. Maiúsculas (`ODE.musicxml`, `ode.MUSICXML`): em Linux é outro arquivo e foi aceito (certo); em disco que ignora caixa a proteção depende do inode (macOS, Unix) ou do `canonicalize` (Windows): **NÃO VERIFICADO**. |
| R3 | **PASS** | Três sessões (S1 salva v1; S2 reabre a Hipótese e salva v2; S3 "Continuar a partir de v1" salva v3 e depois v4; S4 continua de v2 e salva v5; S5 continua da Hipótese): `v1.pai = v2.pai = Hipótese`, `v3.pai = v1`, `v4.pai = v3`, `v5.pai = v2`, filha da Hipótese a partir da Hipótese. v3 contém a edição de v1 e não a de v2. Continuar a partir de uma leitura aberta devolve edição e a filha sai de v2; continuar de objeto adulterado vira aviso e não troca o documento. Recuperado de rascunho com `.ids` herda a linhagem (pai = v1). |
| R4 | **PASS** | `proximo` exato no teto e a 1, 5, 2^20-1, 2^20 e 2^20+1 abaixo dele: abre; editar duração que cria ids recusa com "contador no limite" nas quatro primeiras folgas e passa a partir de 2^20; nunca pânico; a tabela regravada volta a ser aceita. |
| R6 | **PASS** | Em 525 edições aplicadas sobre as 16 peças (fuzz misto grande) a tabela gerada por `tabela_ids_para` foi aceita por `de_texto` em 100 % dos casos e reabrir com ela devolve exatamente os mesmos ids (0 rejeições, 0 divergências): a regra "id repetido/chave repetida" não rejeita arquivo legítimo. |
| R8 | **PASS por leitura** | `sincronizar_diretorio` depois do `rename` e do `hard_link`, objeto sincronizado antes de o índice o citar. Durabilidade em queda de energia: **NÃO VERIFICADA** (nenhum teste pode provar). |
| R10 | **PASS** | Rodei a versão grande (40 edições por peça): 525 edições, 2669 recusas, verde. |
| G1 | **PASS** com N6, N8 | 9 formas de atributo (aspas simples e duplas, aspas dentro de aspas, `&amp;`, espaços duplos, quebra de linha, conteúdo com espaços): 5 combinações de `MudarAltura` cada; o inverso aplicado como comando restaura byte a byte em todas, exceto o espaço atípico da marca de abertura e o espaço em volta do nome (normalizados; N6) e o `>` no valor (recusado; N6). O desfazer real (snapshot) restaurou byte a byte sempre. |
| G2 | **PASS** com N5 | No fuzz misto grande o inverso de cada edição, aplicado como comando sobre o documento reaberto: **0 recusados em 525**, 350 restauram o texto byte a byte, 50 restauram a partitura (modelo musical) e 125 não restauram a partitura de forma idêntica (106 de `RemoverNota`/`RetirarAltura`, as ressalvas declaradas: principal volta como membro, pausas sobras não reagrupadas; 19 de `InserirNota`/`InserirPausa`/`AlternarPonto`/`MudarDuracao`, não aprofundados, ver seção 4). Falso para altura repetida no acorde (N5). |

### 3. Achados

**N1 (P1): "Recuperar" com uma Versão aberta em leitura perde o trabalho recuperado.**
Onde: `crates/piano-tutor-app/src/tela_partitura.rs:1448-1478` (`recuperar_rascunho` não olha `self.leitura`: instala o documento recuperado com `instalar_documento` e deixa `self.leitura` com o `guardado` antigo), `:2183-2194` (`voltar_da_leitura` instala o `guardado`), `:3613-3640` (o banner e os botões "Recuperar/Descartar" e "Abrir" ficam ativos em leitura), `:1448` e `crates/piano-tutor-app/src/autosave.rs:369-381` (depois de `recuperar` o autosave volta a gravar na mesma pasta).
Cenário (todas as ações são botões da tela):
1. O app cai com edições não salvas e o autosave já gravou o rascunho. O usuário reabre a partitura: banner "Recuperar / Descartar".
2. Para decidir, o usuário clica "Abrir" numa Versão da lista (nada o impede; é o gesto natural de comparar).
3. Clica "Recuperar". Aviso: "Trabalho recuperado". A tela passa a mostrar o trabalho recuperado com o nome `ode.musicxml (leitura)`; o banner some (`rascunho_pendente = None`).
4. Editar é recusado ("Versão aberta para leitura"). O usuário clica "Voltar à edição": o documento volta a ser o do arquivo, **sem** o trabalho; o aviso de recuperado e o banner não existem mais.
5. Faz uma edição. No intervalo do autosave, `recuperado = true` + `decisao_pendente = false` deixam o autosave gravar por cima do `rascunho.musicxml.rec` da sessão que caiu; ou, antes disso, "Salvar como Versão" apaga a pasta de recuperação.
Impacto: perda irrecuperável do trabalho da sessão que caiu, o mesmo dano do R1; passos a mais e ordem incomum, por isso P1 e não P0 (promova a P0 se o titular achar o caminho plausível).
Sonda: `zz_revisao2_app_tela_partitura.rs::zz_r1_abrir_versao_em_leitura_e_recuperar`. Saída: `em_leitura=true nome=ode.musicxml (leitura) doc_e_o_rascunho=true pendente=false`; depois de voltar `doc_e_o_rascunho=false modificado=false banner=false rascunho_em_disco=true`; depois de uma edição e um tick `autosave sobrescreveu o rascunho da sessao que caiu: true`; e `salvar depois de voltar: ok=true rascunho_em_disco=false`.
Por que a suíte não pega: nenhum teste abre uma Versão com o banner pendente (`testes_revisao.rs` e `testes_continuar.rs` só cobrem edição, salvar e continuar).
Correção sugerida (não aplicada): recusar `recuperar_rascunho` (e desativar o botão com dica) enquanto `self.leitura.is_some()`, ou sair da leitura antes de instalar o recuperado; e tratar `abrir_versao` com rascunho pendente como o `continuar` já trata.

**N2 (P2): Recuperar que falha libera a edição e deixa o autosave mudo.**
Onde: `autosave.rs:369-381` (`recuperar` só zera `decisao_pendente` no sucesso), `autosave.rs:391-397` (`descartar` só depois de apagar), `tela_partitura.rs:1473-1475` (no erro, `rascunho_pendente = None` mesmo assim), `autosave.rs:407` (`devido` devolve `false` com a decisão pendente).
Cenário: o rascunho existe com hash e tamanho íntegros mas não abre como MusicXML (por exemplo, gravado por outra versão do app). "Recuperar" mostra "Não foi possível recuperar: o rascunho não abre", o banner some, a edição é liberada, e o autosave **nunca mais grava nesta sessão**; o rascunho ruim fica em disco e é reoferecido na abertura seguinte, enquanto o trabalho novo não tem proteção. O texto do ticket 24 ("sem edição durante a decisão, nada fica sem proteção") não vale neste ramo. O mesmo vale, **por leitura do código e não reproduzido**, para "Descartar" que falha ao apagar (`descartar` retorna antes de zerar `decisao_pendente`, e a tela zera `rascunho_pendente` assim mesmo).
Sonda: `zz_r1_recuperar_que_falha_deixa_autosave_mudo` (`rascunho em disco continua o lixo=true`).
Correção sugerida: zerar `decisao_pendente` quando a decisão termina em erro (ou manter o banner e a edição bloqueada com o erro visível).

**N3 (P2): Exportar sobre o arquivo escolhido na lista de fontes não é protegido.**
Onde: `tela_partitura.rs:1348-1370` (`escolher` abre por `DocumentoPartitura::de_arquivo` e `carregar_documento`, que zera `arquivo_aberto`, `:547`; só `abrir_caminho` o preenche, `:1381`) e `:2632-2648` (`exportar` protege só `arquivo_aberto` e a Hipótese vinculada).
Cenário: uma partitura solta em `<assets>/partituras/solta.musicxml` aparece na lista de fontes; o usuário a escolhe, edita, digita o mesmo caminho em "Exportar" com "Sobrescrever": o original é reescrito (`Ok`, `original_mudou=true`); o mesmo teste por `abrir_caminho` recusa. A Hipótese de um item da Oficina continua protegida (por `e_hipotese_da_oficina`); o que escapa é o arquivo solto, contra a frase "o arquivo aberto" do ticket 21 e do R2.
Sonda: `zz_r2_escolher_arquivo_solto_da_pasta_de_assets_sobrescreve_o_original`.
Correção sugerida: gravar `arquivo_aberto` também em `escolher` quando `f.caminho` é `Some`.

**N4 (P3): "Sobrescrever" por rename muda o comportamento.** `documento_partitura.rs:394-410` passou a `gravar_atomico` (tmp + rename). Efeito: um arquivo `0444` é substituído (antes, um usuário comum receberia "permissão negada" ao abrir para escrita) e o novo arquivo sai com o modo do `umask` (`0644`); um link simbólico escolhido como destino vira arquivo comum e o alvo não muda (antes escrevia através do link). Sonda: `zz_r2_sobrescrever_arquivo_somente_leitura_e_atalho` (`Ok`, `conteudo_mudou=true modo_depois=644`; `atalho_ainda_e_link=false real_mudou=false`). **Ressalva:** a sessão roda como root, então "antes falhava" é raciocínio sobre o código antigo, não teste como usuário comum. É decisão de desenho aceitável (atomicidade vale mais), mas convém registrá-la.

**N5 (P3): o inverso de `RemoverNota` em acorde com altura repetida é recusado.** `inserir_remover.rs:926-941` monta `AdicionarAltura{altura, nota: resto}`; `acorde.rs:300-308` recusa por `AlturaJaNoAcorde` (compara pelo MIDI). Acorde `Dó4 + Dó4` (uníssono) ou `Dó♯4 + Ré♭4` (enarmônico): remover qualquer membro dá `inverso` recusado: «Dó 4» já está no acorde (as quatro combinações). A garantia "o inverso nunca é recusado" da linha G2 da matriz é verdadeira no corpus (0 recusas em 525 edições) e falsa em geral. O desfazer real usa snapshot e não é afetado. Sonda: `zz_g2_acorde_com_unissono_e_enarmonico`. Corrigir a frase da matriz ou aceitar o duplicado no inverso.

**N6 (P3): G1, bordas do atributo do `<accidental>`.** `edicao.rs:116-119` (`grafia_valida` recusa `<` e `>` nos atributos). `>` em valor de atributo é XML válido: `<accidental font-family="a>b">natural</accidental>` abre, mas `MudarAltura` que mantém o acidente e o inverso de qualquer edição são recusados («altura inválida: acidente 'natural font-family="a>b"'»). Antes do G1 só o conteúdo contava e a edição passava. Também: o inverso reescreve `<accidental   cautionary="yes">` como `<accidental cautionary="yes">` e ` natural ` como `natural` (a restauração byte a byte vem do desfazer por snapshot, como já diz a nota do G2). Sonda: `zz_g1_acidentes_com_atributos_exoticos`. Improvável em arquivo real.

**N7 (P3): `acidente` livre sem validação (continuação do R7).** `acorde.rs:314-317` (`Some(t) => Some(t.to_owned())`) e `inserir_remover.rs` (`no_de_nota`) passam o texto a `leaf_acidente` (`edicao.rs:87-96`), que o põe na marca de abertura. `AdicionarAltura{acidente: Some("sharp a=\"1\"><injetado/")}` é **aceito** e o `<injetado/>` entra no texto exportado, que reabre sem erro; `DefinirAltura` e `definir_acidente` validam com `grafia_valida`, estes dois caminhos não. Alcançável só por API: a UI passa `None` ou a grafia vinda do próprio documento. Sonda: `zz_injecao_de_marcacao_pelo_acidente_em_inserir_nota_e_acorde` (a parte de `InserirNota` caiu numa recusa por posição ocupada, então só `AdicionarAltura` foi provado).

**N8 (P3): `DefinirAltura` sem mudança conta como mudança.** `edicao.rs:244-250`: `acidente_atual` agora é a grafia completa (`natural cautionary="yes"`) e a comparação com o nome puro (`natural`) falha; `alterou = true`, texto igual, `entradas_de_desfazer 0 → 1`. Só por comando direto com nome puro; o inverso de `MudarAltura` usa a grafia completa. Sonda: `zz_g1_definir_altura_igual_com_nome_so_conta_como_mudanca`.

**N9 (P3, observações):** (a) exportar para pasta inexistente responde "...No such file or directory (os error 2) (marque "Sobrescrever" para trocar um arquivo)", dica enganosa (`documento_partitura.rs:~398`); (b) `pdf_lateral.rs:171-176` chama `std::fs::metadata` a cada quadro enquanto o painel está visível (custo pequeno, no thread da interface); (c) recuperado **sem** o `.ids` do rascunho cai na Hipótese como pai (`pai == Hipótese`, medido), que é verdadeiro porém não o mais próximo (com `.ids`, pai = v1); (d) `salvar` com `JaExiste` não atualiza a base do documento (decisão aceitável, não é falsa linhagem: o histórico de edição é o que conta).

### 4. O que correu bem e o que não aprofundei

- Fuzz misto grande com verificações novas (round trip dos ids, inverso como comando): 525 edições, 2669 recusas, **0** tabelas rejeitadas, **0** divergências de ids, **0** inversos recusados.
- `AlternarPonto`, `MudarDuracao`, `InserirPausa` e `InserirNota` têm 19 casos em que o inverso aplicado como comando não reproduz a partitura exatamente (agrupamento das pausas); não aprofundei e não classifiquei: o desfazer por snapshot é byte a byte nos mesmos 525 passos (já verificado pelo fuzz do repositório) e a UI não aplica o `inverso` como comando.
- `pdf_lateral.rs` (567 linhas, R5): lido por inteiro; sem pânico, sem escrita em disco, rolagem só quando a textura é a da página; a cópia de `pdf_lateral` por `mem::take` é devolvida antes do corpo da tela.

### 5. Comandos executados (e resultado)

- `git fetch` + `git merge --ff-only origin/claude/project-thread-w8l1yp` (ponta `3f1da94`). Sem push.
- `CARGO_PROFILE_DEV_DEBUG=0 cargo test --offline -p piano-tutor-app --lib zz_ -- --nocapture --test-threads=1`: 15 sondas, todas rodaram; as saídas citadas acima são dessas execuções (as sondas imprimem, não afirmam; os resultados que reproduzem achado estão em N1 a N5).
- `CARGO_PROFILE_DEV_DEBUG=0 cargo test --offline -p piano-tutor-fixtures --test zz_sonda2_exoticos -- --nocapture --test-threads=1`: 5 sondas.
- `CARGO_PROFILE_DEV_DEBUG=0 cargo test --offline -p piano-tutor-fixtures --test zz_sonda2_fuzz -- --ignored --nocapture mista_em_todo_o_corpus_grande`: 525 edições, 2669 recusas (23 s).
- Árvore limpa (sem as sondas): `cargo test --workspace --offline -- --skip zz_`: exit 0, **1574 passaram, 0 falharam, 12 ignorados**. `cargo fmt --all --check`: sem diferenças. `cargo clippy --workspace --all-targets --offline -- -D warnings`: exit 0, sem avisos.
- **Não executados por mim:** `cargo build --release`, `cargo deny`, os três exaustivos em release (`cargo test --release -p piano-tutor-fixtures -- --ignored`; a matriz registra 27,8 s, 378,9 s e 116,9 s). Verde na suíte padrão não prova o item 2 inteiro.

### 6. NÃO VERIFICADO

macOS e Windows (comparação de caixa em disco que ignora caixa, `canonicalize`, `fsync` de diretório, `hard_link` em FAT/exFAT com a volta por `rename`); "antes falhava" do N4 como usuário comum; durabilidade em queda de energia (R8); janela real, clique, teclado e leitor de tela para o banner, a lista de Versões e o botão "Continuar a partir desta Versão"; desempenho; REVAS, áudio e latência.

### 7. Estado do item "sem P0/P1" e do gate

- Item "revisão independente sem P0/P1": **NÃO atendido** (N1, P1). Passa a atendido quando N1 for corrigido, com teste que reproduza (abrir Versão com rascunho pendente, Recuperar, Voltar), e nova rodada de revisão; N2 e N3 (P2) devem ser corrigidos ou aceitos por escrito pelo titular antes do aceite.
- Os demais itens do gate (3 a 7 do relatório da primeira revisão) não foram reauditados aqui.
- O aceite humano do titular segue pendente e não é inferido deste relatório.

### Apêndice. Sondas da segunda revisão

Em `docs/evolucao-musical/e01/sondas-revisao/` (fora da compilação, como as da primeira revisão):

- `zz_revisao2_app_tela_partitura.rs`: 15 sondas da tela (R1, R2, R3; N1 a N4). Para rodar, copiar para `crates/piano-tutor-app/src/tela_partitura/zz_sondas.rs`, acrescentar `#[cfg(test)] mod zz_sondas;` a `tela_partitura.rs` e rodar o comando acima (só Unix: usa `std::os::unix::fs::symlink`; `zz_r2_caminho_relativo_e_maiusculas` muda o cwd, por isso `--test-threads=1`).
- `zz_revisao2_fixtures_exoticos.rs`: G1, G2, injeção de marcação e R4 no teto (N5 a N8), em `crates/piano-tutor-fixtures/tests/zz_sonda2_exoticos.rs`.
- `zz_revisao2_fixtures_fuzz_inverso.rs`: cópia de `documento_fuzz_misto.rs` com o round trip dos ids (R6) e o inverso aplicado como comando (G1/G2), em `crates/piano-tutor-fixtures/tests/zz_sonda2_fuzz.rs`; só a versão grande está ativa (`--ignored`).

## Correções da segunda revisão (2026-11-28, N1 a N4 e N9)

Cada correção nasceu de um teste que falhava (red registrado) e depois passou. Estado: **corrigido, aguarda nova rodada de re-revisão independente** (a seção "Segunda revisão" acima descreve o código anterior e não foi reescrita). N5 a N8 (núcleo) estão com outra equipe e **não foram tocados**. Os testes novos ficam em `crates/piano-tutor-app/src/tela_partitura/testes_revisao2.rs` (12) e em `tela_partitura/pdf_lateral.rs` (1); red: 9 dos 13 falhavam, os outros 4 já passavam (documentam o que não pode regredir). Evidência final no fim desta seção.

| Id | Decisão e correção | Teste (red → green) |
| --- | --- | --- |
| N1 (P1) | **Recuperar sai primeiro do modo leitura** (`voltar_da_leitura`: o documento em edição volta) **e só então instala o rascunho** como documento em edição, modificado, com o nome sem "(leitura)" e o aviso "A leitura da Versão foi encerrada". O nome usado para abrir o rascunho já não é o do documento "(leitura)". O rascunho em disco só some ao virar Versão (`salvar_versao`) ou por "Descartar" explícito; depois de Recuperar o autosave grava o trabalho todo. Abrir uma Versão com o banner pendente continua permitido (é o gesto de comparar) e "Voltar à edição" sem recuperar mantém banner e rascunho. Alternativa descartada: desativar Recuperar em leitura (obriga um passo a mais e o usuário não entende por quê). | `n1_recuperar_com_versao_em_leitura_sai_da_leitura_e_instala_o_rascunho_em_edicao` (falhava: continuava em leitura, `nome` com "(leitura)"); `n1_descartar_em_leitura_...` e `n1_abrir_versao_e_voltar_sem_recuperar_...` (já verdes, guardam o contrato) |
| N2 (P2) | Recuperar que falha **não zera** `rascunho_pendente`: o banner, a edição bloqueada e o salvar bloqueado continuam e o aviso diz "Não foi possível recuperar: ... O rascunho continua guardado. Descarte-o para voltar a editar." Descartar que falha (disco recusa apagar) também mantém o rascunho pendente e o aviso (antes a tela zerava o pendente e o autosave ficava mudo na sessão). Com isso o autosave só fica quieto enquanto o banner está visível: não há mais silêncio sem explicação. Consequência aceita: um rascunho que nunca abre só sai por "Descartar" (ou, na próxima abertura, se o arquivo foi removido). | `n2_recuperar_que_falha_...` (falhava: banner sumia) e `n2_descartar_que_falha_...` (falhava; falha induzida com uma pasta no lugar de `rascunho.ids`, só Unix) |
| N3 (P2) | `carregar_documento(d, origem)` agora **exige** a origem (`Option<PathBuf>`): `abrir_caminho` e `escolher` (arquivo do disco) a informam, texto embutido passa `None`; trocar de fonte sempre troca ou limpa o registro. A exportação protege o arquivo escolhido na lista como já protegia o digitado. | `n3_escolher_arquivo_solto_da_lista_protege_o_original_na_exportacao` (falhava: `Ok` e original reescrito) |
| N4 (P3) | Decisão (a mais segura): **link simbólico como destino é recusado** (com ou sem "Sobrescrever"), com a mensagem "o destino é um atalho (link simbólico)... Escolha o arquivo real"; **destino somente leitura** (ou que o processo não abre para escrita) **é recusado** com "Sobrescrever", sem tocar o arquivo; destino gravável **mantém as permissões** (`gravar_atomico` passa o modo do arquivo existente ao temporário antes do `rename`; vale também para os demais usuários dessa função, que só gravam arquivos próprios). Dono e grupo não são preservados (o `rename` cria o arquivo com o usuário atual): decisão aceita. | `n4_sobrescrever_preserva_as_permissoes_do_destino` (0640 virava 0644), `n4_sobrescrever_recusa_destino_somente_leitura_com_mensagem` e `n4_sobrescrever_recusa_link_simbolico_como_destino` (as três falhavam; só Unix) |
| N9 (a) | Exportar para pasta inexistente diz "a pasta de destino não existe: ..."; a dica de "Sobrescrever" só aparece quando o arquivo já existe (`AlreadyExists`). | `n9_exportar_para_pasta_inexistente_...` (falhava); `n9_dica_de_sobrescrever_...` (já verde) |
| N9 (b) | `pdf_lateral.rs`: o `metadata` por quadro saiu; a página carregada fica em cache e o disco só é lido quando a página pedida muda (a chave passou de `(caminho, mtime)` para o caminho). Efeito aceito: um PNG trocado no disco com a mesma página aberta só é relido ao mudar de página. | `pdf_lateral::testes::pagina_carregada_nao_consulta_o_disco_a_cada_quadro` (falhava: apagar o PNG derrubava a textura no quadro seguinte) |
| N9 (c), (d) | **Não alterados**, registrados como aceitos: (c) o recuperado sem `.ids` ter a Hipótese como pai é verdadeiro e só menos próximo; (d) `salvar` com `JaExiste` não atualizar a base não é linhagem falsa. | n/a |

NÃO VERIFICADO: Windows e macOS (atributo somente leitura, `symlink_metadata`, `rename` sobre arquivo); "antes falhava" do N4 como usuário comum (a suíte roda como root, e por isso a recusa de somente leitura usa o bit de permissão além da tentativa de abrir para escrita); clique real em "Recuperar" com Versão aberta, em janela.

Evidência final (Linux, root): `cargo fmt --all --check` sem diferenças; `cargo clippy --workspace --all-targets --offline -- -D warnings` sem avisos; `cargo test --workspace --offline`: **1587 passaram, 0 falharam, 12 ignorados** (antes: 1574/0/12; +13 testes novos). Não executados aqui: `cargo build --release`, `cargo deny` e os exaustivos em release.
