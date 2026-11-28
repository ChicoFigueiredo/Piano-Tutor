# 30: Épico E04: REVAS no editor

**Fase:** E04

**R-IDs/critérios:** R-E05, R-N02, T10

**Seam:** S1 (comandos de entrada MIDI), S4

**Modelo sugerido:** Opus / alto para o to-tickets do épico; os tickets filhos recebem modelo e effort próprios quando forem criados

**What to build:** **Épico (não quebrar agora).** Objetivo demonstrável: inserir música tocando, trocar altura preservando duração, voz e pauta, e gravar com metrônomo e quantizar sem destruir o MIDI bruto.

**Decisões já tomadas na especificação:** eventos MIDI viram comandos do mesmo serviço de edição; tratar NoteOn com velocidade 0, note-off, sustain, desconexão, all-notes-off, 61 teclas e notas fora do alcance; roteamento mutuamente exclusivo entre editor e treino; pedal só após calibração que prove eventos; mouse e PC sempre funcionam.

Este ticket é um marcador de fase: quando seus bloqueios fecharem, o orquestrador roda `to-tickets` sobre ele e gera as fatias verticais (prefactors primeiro) antes de qualquer implementação.

**Blocked by:** 29 (Épico E03: Editor completo)

**Status:** ready-for-agent

- [ ] Dado step-time com duração 1/4, NoteOn/NoteOff inserem a nota com a duração.
- [ ] Dado acorde dentro da janela, ele vira acorde.
- [ ] Dado re-pitch, duração, voz e pauta ficam.
- [ ] Dada desconexão, nenhuma nota fica presa.
- [ ] Dada Gravação quantizada, o MIDI bruto continua recuperável (tudo isso em CI por eventos sintéticos).
- [ ] Gate da fase: teste físico no REVAS (61 teclas, 10 notas simultâneas), latência p95 e jitter sob carga, 60 minutos sem crash; sem o equipamento o Gate não é aprovado.
- [ ] NÃO VERIFICADO sem hardware: tudo que depende do teclado físico, do pedal e da latência.
- [ ] Este épico só é considerado pronto quando seus tickets filhos (a criar) estiverem aceitos e o titular registrar o aceite humano da fase.
