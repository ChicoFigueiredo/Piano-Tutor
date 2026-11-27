# 21: Salvar como nova Versão, com nota de motivo, e listar as Versões

**Fase:** E01

**R-IDs/critérios:** R-B06, R-E02, T8, D8; A8; histórias 15, 16, 17

**Seam:** S1

**Modelo sugerido:** Sonnet / alto (Opus revisa o formato de armazenamento)

**What to build:** O revisor salva o que fez como uma nova Versão, com nota opcional de motivo; cada Versão é imutável, identificada pelo hash do conteúdo, e ligada à Versão-pai, com autor e data. A lista de Versões mostra a Hipótese e a fonte como as primeiras, e as correções em sequência. A Hipótese e o arquivo original nunca são alterados. Nesta fase as Versões vivem no repositório de oficina; o Acervo (E02) as assume depois.

**Blocked by:** 14 (Selecionar nota e mudar a altura com seta (semitom, oitava, escala))

**Status:** ready-for-agent

- [ ] A8: dada uma Hipótese aberta, salvar duas correções cria duas Versões encadeadas (pai→filho) e o hash da Hipótese permanece idêntico antes e depois (teste).
- [ ] Salvar nunca sobrescreve arquivo de Versão existente; salvar sem mudanças não cria Versão vazia (ou pede confirmação, decisão registrada).
- [ ] Cada Versão registra hash de conteúdo, hash do pai, autor, data, nota de motivo e estado de conteúdo (nova Versão de edição nasce Em revisão); estados reaproveitam a máquina existente.
- [ ] A lista de Versões mostra autor, data, nota e estado, e abrir uma Versão antiga a carrega em modo leitura ou como novo ponto de partida (decisão registrada).
- [ ] Gravação atômica (arquivo temporário e renomear); queda no meio da gravação deixa a Versão anterior íntegra (teste de interrupção simulada).
- [ ] O arquivo original (PDF e MusicXML de origem) é verificado por hash após qualquer ação da Oficina.
- [ ] Material comprado nunca é copiado para fora da pasta de dados do usuário; o formato de armazenamento está documentado para a migração futura ao Acervo.

**Notas:** Pergunta aberta: guardar as Versões no formato atual do repositório de oficina (estado em TOML + arquivos) ou já preparando o esquema do Acervo. Por padrão, o formato atual com histórico estendido.
