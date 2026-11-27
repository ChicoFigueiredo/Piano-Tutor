# Limites e política de segurança na abertura de MusicXML e MXL (ticket 11, A13)

Vale para toda a rota de abrir partitura: `DocumentoAberto::abrir_texto` e `abrir_bytes`
(`piano_tutor_core::documento`), que passam por `Partitura::de_musicxml`, `Documento::abrir` e
`extrair_musicxml`. A política de XML vive num lugar só, `piano_tutor_core::xml_seguro`.

## Limites (todos com teste que os ultrapassa)

| Limite | Valor | Constante | Erro |
| --- | --- | --- | --- |
| Tamanho do texto MusicXML | 8 MiB | `partitura::TAMANHO_MAXIMO_BYTES` | `GrandeDemais` |
| Tamanho do `.mxl` (compactado) | 16 MiB | `mxl::MXL_TAMANHO_MAXIMO_BYTES` | `GrandeDemais` |
| Entradas no zip | 128 | `mxl::MXL_MAX_ENTRADAS` | `LimiteExcedido("número de entradas do zip")` |
| `container.xml` descompactado | 64 KiB | `mxl::MXL_MAX_CONTAINER_BYTES` | `LimiteExcedido` |
| Partitura descompactada (a única entrada lida além do container) | 8 MiB, medido por leitor com teto, sem confiar no cabeçalho | `partitura::TAMANHO_MAXIMO_BYTES` | `LimiteExcedido` |
| Razão descompactado/compactado | 200 (piso de 512 B compactados) | `mxl::MXL_TAXA_MAXIMA` | `LimiteExcedido("taxa de compressão suspeita")` |
| Nós XML | 2.000.000 | `partitura::MAX_NOS_XML` | `LimiteExcedido("número de nós XML")` |
| Profundidade de elementos | 128 | `xml_seguro::MAX_PROFUNDIDADE_XML` | `LimiteExcedido("profundidade do XML")` |
| Texto produzido por entidades do DTD (soma de todas as referências) | 1 MiB | `xml_seguro::MAX_EXPANSAO_ENTIDADES_BYTES` | `LimiteExcedido("expansão de entidades")` |
| Partes / compassos por parte / notas | 64 / 20.000 / 200.000 | `MAX_PARTES`, `MAX_COMPASSOS`, `MAX_NOTAS` | `LimiteExcedido` |

Só o `container.xml` e o arquivo principal são descompactados; as demais entradas nunca são lidas
nem gravadas (nada é extraído para o disco). Nomes de entrada com `..`, caminho absoluto, `\`,
unidade (`C:`) ou NUL recusam o arquivo. Entrada criptografada ou com compressão que não seja
armazenada/deflate é recusada. Nenhum shell, processo externo, rede ou gravação em disco é usado
na abertura (a leitura do arquivo pelo app tem teto de tamanho também na leitura, não só na
checagem).

## DTD: ligado, com provas (decisão do ticket 11)

`allow_dtd` continua `true` porque o MusicXML oficial traz
`<!DOCTYPE score-partwise PUBLIC ... "http://www.musicxml.org/dtds/partwise.dtd">` e o corpus da
E01 (A1, 16/16) inclui arquivos assim; desligar recusaria partituras legítimas. Em troca:

- **Entidade externa nunca é resolvida.** `ParsingOptions::entity_resolver` é `None` por
  construção (`xml_seguro::opcoes`); sem resolvedor, `roxmltree` descarta `SYSTEM`/`PUBLIC` (`file://`,
  `http://`, entidade de parâmetro) e qualquer referência vira erro "entidade não declarada ou
  externa". Provas: `xml_seguro::testes::entidade_externa_nunca_e_resolvida_nem_lida` e
  `xxe_nao_le_arquivo_algum_do_disco` (arquivo real com marcador único; o marcador não aparece
  em lugar nenhum). O identificador externo do `DOCTYPE` (`PUBLIC ... "http://..."`,
  `SYSTEM "file://..."`) é lido como texto e ignorado.
- **Billion laughs:** barrado pelo detector de laço do `roxmltree` (profundidade 10, 255 referências
  por referência) e também pelo orçamento de expansão.
- **Explosão quadrática (achado deste ticket):** uma entidade de 100 KB referenciada 2.000 vezes
  expandia 200 MB em ~0,5 s e 200 MB de memória (e, em arquivo de 8 MiB, dezenas de GB), porque o
  detector não conta referências no nível zero. `xml_seguro::verificar_entidades` percorre o prólogo
  (espelhando a gramática do `roxmltree`), soma o tamanho expandido de cada referência a entidade
  declarada e recusa acima de 1 MiB, **antes** de analisar. Declaração que o verificador não entende
  é recusada (falha fechada).
- O `container.xml` do `.mxl` é analisado com DTD desligado: qualquer `DOCTYPE` é recusado.

## Mensagens e logs

Erros do analisador são traduzidos para português (`xml_seguro::traduzir`) com linha e coluna, sem
reproduzir conteúdo do arquivo; nomes de entidade, de entrada de zip e de `rootfile` são cortados
em 48 caracteres e sem caracteres de controle. Erros de leitura do arquivo no app mostram só o
nome do arquivo, nunca a pasta do usuário, e o motivo vem em português (não o texto do sistema).
Nenhum `Debug` de conteúdo de arquivo é gravado em log por esta rota.

## Testes

- `crates/piano-tutor-fixtures/tests/seguranca_xml_mxl.rs`: A13 (fixtures maliciosas do ticket 07,
  mais as XML de `gerador::xml_de_teste`), cada limite ultrapassado pela rota do serviço, XXE com
  arquivo real, mensagens em português.
- `crates/piano-tutor-core/src/xml_seguro.rs` (módulo `testes`): política de DTD, orçamento de
  expansão (fronteira exata), profundidade, mensagens.
- `crates/piano-tutor-fixtures/tests/fuzz_xml_mxl.rs`: mutação determinística (xorshift, semente
  fixa) de MusicXML e `.mxl`; propriedades: sem pânico, tempo por entrada limitado e, se abre,
  exportar devolve o original (A1). Na CI: ~5 s. Mais tempo:
  `PIANO_TUTOR_FUZZ_SEGUNDOS=600 cargo test --release -p piano-tutor-fixtures --test fuzz_xml_mxl`.
  Falhas vão para `target/tmp/fuzz-falhas/` e entram em `regressoes_xml_mxl.rs` (ou em
  `tests/regressoes/`). Não há `proptest` no projeto; não foi adicionada dependência.

## Medição (Linux x86_64, 4 CPUs, release; `cargo run --release -p piano-tutor-fixtures --example medir_seguranca`)

Pico de memória é o `VmHWM` do processo (a linha de base de ~60 MiB é a geração das entradas de
teste dentro do próprio processo de medição). Tempo é só a abertura.

| Caso | Tempo | Memória acima da base | Resultado |
| --- | --- | --- | --- |
| Maior partitura aceita (199.000 notas, ~4 MB de XML) | 542 ms | ~200 MiB | abre |
| 1.990.000 elementos `<a/>` (quase o limite de nós) | 363 ms | ~110 MiB | recusa (sem partes) |
| 2.000.001 elementos (limite de nós) | 134 ms | ~110 MiB | recusa (limite de nós) |
| 100 mil elementos aninhados | 7 ms | desprezível | recusa (profundidade) |
| Billion laughs; explosão quadrática (texto e atributo); entidade cíclica | < 1 ms | desprezível | recusa (expansão) |
| `.mxl` com 24 MiB de espaços (zip bomb); entradas demais; `..`; absoluto | < 1 ms | desprezível | recusa |
| 16 MiB de lixo como `.mxl` | < 1 ms | desprezível | recusa (não é zip) |

Pior caso medido: ~0,5 s e ~200 MiB acima da base, para uma partitura legítima no limite de notas.
Observação: a mesma partitura sintética (pausas idênticas repetidas) dentro de um `.mxl` passa de
200x de compressão e é recusada pelo limite de razão; partitura real comprime algo como 5x a 20x.
Esta medição é de uma máquina de desenvolvimento, não do hardware de referência do titular:
**NÃO VERIFICADO** fora daqui.

## Fora do escopo desta rota (declarado)

`piano-tutor-fixtures` (`harness.rs`, `leitura.rs`) ainda chama `roxmltree` com `allow_dtd: true`
diretamente, mas só sobre o corpus versionado, em testes; nenhum crate de produção depende dele.
