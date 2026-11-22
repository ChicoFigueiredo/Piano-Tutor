# Arquitetura técnica — Piano Tutor

## Separação estrutural

- **Aplicativo desktop (Rust)**: a interface será implementada por meio de `bancada-casca`, crate privada do repositório [bancada](https://github.com/ChicoFigueiredo/bancada), por dependência fixada em commit. Não copiar a casca nem depender do Bevy sem necessidade.
- **Captura MIDI**: `midir` (validar a conectividade USB real do REVAS KB-330 na F01). Normalizar `NoteOn` com velocidade 0 como desligamento da nota; suportar múltiplas notas e desconexão.
- **Áudio em tempo real**: avaliar `rustysynth` para SoundFont SF2 e `cpal` para saída de áudio; medir latência total, nunca usar o loop de UI para síntese. Avaliar `Kira` para amostras, metrônomo e pads sem criar dois relógios concorrentes.
- **Música e formatos**: MIDI para eventos de execução, MusicXML/MXL para partitura, PDF para exportação e WAV para áudio; registrar claramente perdas de informação e quantização.
- **Dados locais**: SQLite para exercícios, sessões, resultados e metadados (não embutir vídeo ou bancos de áudio como blobs no banco).
- **IA offline opcional (Python)**: worker isolado por subprocesso/IPC com contratos JSON versionados, logs e limites. Considerar Whisper/faster-whisper, Transkun, Basic Pitch, YourMT3, Music21, MediaPipe, OpenCV. MuScriptor depende de validação de licença dos pesos.
- **Lua/Luau opcional**: usar `mlua` para regras limitadas e determinísticas de exercício, **não** como substituto de modelos Python.

## Princípios

1. A IA nunca participa do caminho crítico entre tecla MIDI e áudio.
2. Uma única fonte temporal controla metrônomo, playback e batidas; usar timestamps monotônicos.
3. Separar eventos originais de MIDI das interpretações quantizadas e das partituras.
4. Todo resultado de IA possui proveniência, confiança e revisão humana antes de ser gabarito.
5. Aplicativo funciona sem rede; dados de videoaulas ficam locais por padrão.
6. A equipe implementa por fases F00 a F14 e não aceita mock como prova física.

Referência detalhada: [especificação integral](../specs/ESPECIFICACAO-COMPLETA-v1.md).
