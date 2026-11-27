# Conferências do supervisor (amostragem independente)

| Arquivo | Afirmação conferida | Como | Resultado |
|---|---|---|---|
| 04 | rmcp versão atual 3.12.0 | API crates.io, 2026-11-27 | CONFIRMADA (publicada 2026-11-26) |
| 05 | MUSCIMA++ é CC BY-NC-SA 4.0; OpenScore Lieder CC0 | conhecimento prévio coincide; links no 05 | Coerente; reconferir no to-spec |
| 02 | PoC Audiveris 5.11.0 rodou e leu 20/20 alturas, 0 letras, 0 dedos | relato com comandos no 02; API GitHub bloqueada na nuvem, não reconferido | Aceito como evidência de 1 fixture; não generalizar |
| 02 | copisteria/copista-28m Apache-2.0, OMR-NED 11,7% | auto-relatado pelo autor | Tratar como candidato a replicar, não como fato |
| 03 | Ctrl+K já é atalho da casca (paleta) | grep bancada-casca casca.rs:370 (6d296a7) | CONFIRMADA |
| 01 | baseline verde: fmt, clippy -D warnings, 1053 testes ok, 6 ignorados (toolchain 1.98.0) | relato com comandos no 01 | Aceito; reconferir antes do PR de spec |

# Decisões de engenharia tomadas pelo supervisor (reversíveis, sem card)

- Modelo de documento: híbrido (modelo semântico com IdNota estável + faixa de bytes de origem e filhos desconhecidos preservados; só o compasso editado é regenerado). Validar em spike com corpus CC0 antes de travar ADR.
- Render do editor: layout próprio egui + SMuFL/Bravura (OFL-1.1). Verovio só como oráculo de comparação, em processo separado, nunca linkado. Spike de ADR-0007 feito como parte da 1ª fatia.
- Durações 1–9 (perfil MuseScore 4); redo Ctrl+Shift+Z; Ctrl+K continua paleta da casca, cifra vai para Ctrl+Shift+K; Ctrl+. (pânico) reservado; Esc com prioridade definida pela casca. Tudo remapeável.
- Edição salva sempre como nova Versão; Hipótese e original nunca sobrescritos.
- OMR: fusão multi-motor só como detector de discordância, não como fonte de verdade; rotear PDF vetorial vs scan por pdffonts; LLM multimodal nunca primário.
- Metas F1 97–99% da proposta: rebaixadas a "metas a calibrar após baseline" (sem suporte na literatura), sem prometer precisão.
- MCP: binário separado piano-tutor-mcp com tokio confinado e rmcp fixado (=3.x.y); somente leitura primeiro.
- Providers: camada própria fina (trait Provider sobre ureq); genai como plano B.
- Luau de plugins: sem math.random não semeado; limite por contagem de interrupções (determinístico).
- Sandbox Python: no Linux de referência recusa executar sem isolamento de rede; no macOS aviso visível.
- Corpus: scans IMSLP só com checagem por edição; redistribuir apenas métricas e scripts; engraving próprio de peças PT em domínio público para cobrir letras.
- ADR-0009 duplicado: anotar, não renomear (links). Próxima ADR = 0015. MATRIZ-DE-ACEITE sincronizada com progresso.json em PR separado. cargo deny na CI antes de rmcp/providers/OMR.
- Material comprado do titular: só testes locais no cerebro, nunca git/CI; envio à nuvem só com consentimento por lote (decisão do titular, prevalece sobre a recomendação mais restritiva das pesquisas 02/04).
- Ticket 08: arquivo lateral de IdNota amarrado por impressão digital do texto, só a partir do ticket 17 (edição estrutural); até lá ids pela ordem do documento. Mudança de divisions: recusar por padrão (redeclarar no compasso fica para avaliar no ticket 16).
