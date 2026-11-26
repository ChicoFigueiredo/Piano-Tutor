# ADR-0014 — macOS como plataforma de teste pessoal

**Status:** PROPOSTA — 2026-11-26 · **Complementa:** [ADR-0004](ADR-0004-somente-linux.md) (não a substitui)

O titular quer praticar também no **Mac Home** (MacBook Pro 15" 2016, Intel i7-6820HQ, macOS Sequoia via OpenCore Legacy Patcher), em outro cômodo, com **todas as funcionalidades iguais às do Linux**. O macOS passa a ser uma **plataforma de teste pessoal**: o Linux no cerebro continua sendo a referência, e só ele conta para os gates de hardware (F01, F02, F03, F14) e para a homologação.

Decisões tomadas com o titular:

- **Entrega:** um `Piano Tutor.app` com assinatura ad-hoc, **sem notarização**, sem `.dmg` e sem loja. Na primeira abertura usa-se botão direito > Abrir. Isso não é empacotamento de distribuição e não contraria a regra de não empacotar em 2026.
- **Arquitetura:** binário **universal** (x86_64 + arm64), para rodar no Mac Home (Intel) e em Macs Apple Silicon.
- **Onde compilar:** no próprio Mac Home, acessado por `ssh mac` a partir do cerebro. A CI de macOS só entra depois de os builds locais estarem verificados.
- **Dependências do sistema:** Homebrew no Mac Home, com `mpv` (a `libmpv` para o vídeo embutido do Player de Aula e o `mpv` para o áudio do professor).
- **Voz do professor:** o mesmo **piper** e o mesmo modelo `.onnx` do cerebro, para a voz soar igual nas duas máquinas.
- **Revisão:** MuseScore instalado pelo Homebrew; o app o procura também dentro do `.app` do MuseScore.
- **Conteúdo:** o curso inteiro é copiado para o Mac, para uso pessoal e local, como no cerebro ([ADR-0008](ADR-0008-aulas-somente-locais.md)). O perfil e o progresso (SQLite, [ADR-0010](ADR-0010-preferencias-em-sqlite-com-perfis.md)) são copiados do cerebro **uma vez**. Depois cada máquina segue separada, sem sincronização.

**Consequências:** o código deve procurar a `libmpv` pelo nome do macOS (`libmpv.2.dylib`, nas pastas do Homebrew) e não só por `libmpv.so.2`. Caminhos de programas externos (piper, MuseScore) ganham as pastas do Homebrew e dos `.app`. Um teste no Mac **não** vale como evidência de gate. Promover o macOS a plataforma oficial exige ADR nova.
