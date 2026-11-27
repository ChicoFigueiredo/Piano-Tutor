# 11: Segurança de XML e MXL na abertura da Oficina (A13)

**Fase:** E01

**R-IDs/critérios:** R-N01, T2; A13; história implícita de segurança (Segurança nas Decisões de implementação)

**Seam:** S1 (fuzz do parser e do MXL)

**Modelo sugerido:** Sonnet / alto (Opus revisa o resultado)

**What to build:** Abrir um MusicXML malformado, com entidade externa, ou um MXL com zip bomb, caminho fora do escopo, nomes absolutos ou muitos arquivos, mostra um erro claro na Oficina, não lê nada fora do limite e não derruba o app. Isto vale também para a nova rota do serviço de documento, que não pode ter afrouxado nenhum limite existente.

**Blocked by:** 07 (Corpus de fixtures da E01 e harness de round-trip e comparação semântica), 10 (Abrir uma partitura na Oficina e exportar idêntico (serviço de documento, 1ª fatia))

**Status:** feito, aguarda revisão

- [x] A13: para cada fixture maliciosa do ticket 07 (XML malformado, entidade externa/XXE, entidade expandida em cadeia, MXL com zip bomb, MXL com caminho fora do escopo, MXL com nomes absolutos ou com `..`), o serviço devolve um erro tipado e a tela mostra mensagem em PT-BR; nada é lido fora do limite e o processo não cai (teste automatizado, sem pânico). (feito: `crates/piano-tutor-fixtures/tests/seguranca_xml_mxl.rs`; fixtures XML novas em `gerador::xml_de_teste`; erro tipado `ErroServico`, mensagem em PT-BR conferida por teste)
- [x] Os limites existentes (tamanho do texto, nós XML, partes, compassos, notas, tamanho de MXL) continuam aplicados na rota do serviço e são cobertos por teste que ultrapassa cada um. (feito: um teste por limite na rota do serviço; ver `docs/seguranca/LIMITES-XML-MXL.md`)
- [x] O tratamento de DTD é revisto e justificado: se a configuração atual permite DTD, o ticket prova por teste que nenhuma entidade externa é resolvida e documenta por quê; se possível desliga-se o DTD, sem perder os arquivos do corpus. (feito: DTD mantido ligado porque o corpus A1 16/16 tem `DOCTYPE`; entidade externa nunca resolvida, provada com arquivo real; achado e corrigido: explosão quadrática de entidades, `xml_seguro::verificar_entidades`)
- [x] Um alvo de fuzz (ou teste de mutação por proptest) do parser e do leitor de MXL existe, roda por tempo limitado na CI e por mais tempo localmente; entradas que causaram falha viram regressão fixa. (feito: `tests/fuzz_xml_mxl.rs`, mutação determinística, ~5 s na CI, `PIANO_TUTOR_FUZZ_SEGUNDOS` para mais; regressões em `tests/regressoes_xml_mxl.rs`; sem `proptest` no projeto. Rodada local de 120 s em release: 1,88 milhão de mutações, 0 falhas)
- [x] Memória e tempo no pior caso são limitados e medidos (registro da medição); nenhum shell ou processo externo é usado na abertura. (feito: exemplo `medir_seguranca`, tabela em `docs/seguranca/LIMITES-XML-MXL.md`; pior caso aceito ~0,5 s e ~200 MiB; nenhum shell/processo externo na abertura. Medido só nesta máquina, hardware de referência NÃO VERIFICADO)
- [x] Mensagens de erro e logs não vazam caminho completo do usuário nem conteúdo do arquivo além do necessário. (feito: erros do analisador traduzidos, nomes truncados em 48 caracteres, leitura de arquivo no app mostra só o nome do arquivo, nunca a pasta)
