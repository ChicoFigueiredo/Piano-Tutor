//! "Continuar a partir desta Versão" (revisão independente da E01, achado R3; critério do ticket
//! 21, "abrir Versão antiga ... como novo ponto de partida"). Abrir uma Versão pela lista é
//! somente leitura; aqui a Versão vira o documento em **edição**, e a próxima Versão salva é filha
//! dela (o `pai` vem da base do documento, ver `EstadoPartitura::salvar_versao`). Fica num módulo
//! próprio para não disputar linhas com o resto da tela.

use super::*;

impl EstadoPartitura {
    /// Por que "Continuar a partir desta Versão" não vale agora (`None` se vale).
    pub fn motivo_sem_continuar_da_versao(&self) -> Option<&'static str> {
        if self.versoes.is_none() || self.documento.is_none() {
            return Some(
                "Arquivo solto: só partituras da Oficina (pasta partituras/ com estado.toml) \
                 têm Versões.",
            );
        }
        if self.rascunho_pendente.is_some() {
            return Some(
                "Há trabalho não salvo de uma sessão anterior: recupere ou descarte o rascunho \
                 antes de continuar a partir de outra Versão.",
            );
        }
        // O documento em edição: o aberto, ou o guardado enquanto se lê uma Versão.
        let em_edicao = match &self.leitura {
            Some(l) => l.guardado.as_ref(),
            None => self.documento.as_ref(),
        };
        if em_edicao.is_some_and(DocumentoPartitura::modificado) {
            return Some(
                "Há edições não salvas: salve como Versão (ou desfaça) antes de continuar a partir \
                 de outra Versão.",
            );
        }
        None
    }

    /// Abre a Versão `hash` como o documento em **edição**: a próxima Versão salva é filha dela.
    /// Recusa, sem mudar nada, com edições não salvas ou rascunho pendente (nada se perde em
    /// silêncio); Versão inexistente ou adulterada vira aviso. Não escreve nada em disco.
    pub fn continuar_a_partir_da_versao(&mut self, hash: &str) {
        if let Some(m) = self.motivo_sem_continuar_da_versao() {
            self.aviso_versoes = Some(m.to_owned());
            return;
        }
        let Some(v) = &self.versoes else {
            return;
        };
        let aberta = match v.abrir(hash) {
            Ok(a) => a,
            Err(e) => {
                self.aviso_versoes = Some(format!("Não foi possível abrir a Versão: {e}"));
                self.conferir_originais();
                return;
            }
        };
        let nome = self.nome().trim_end_matches(" (leitura)").to_owned();
        let doc = match DocumentoPartitura::de_versao(
            nome,
            &aberta.conteudo,
            aberta.ids,
            &aberta.versao.hash,
        ) {
            Ok(d) => d,
            Err(e) => {
                self.aviso_versoes = Some(format!("A Versão não pôde ser aberta: {e}"));
                return;
            }
        };
        self.parar_sem_som();
        self.pratica_versao = None;
        self.pedido_hipotese = None;
        self.leitura = None;
        // O rascunho que esta sessão gravou era do documento anterior (sem edição, senão teria
        // recusado): o autosave o apaga como obsoleto no próximo quadro.
        self.recuperado = false;
        self.ultimo_hash_rascunho = None;
        let recusa = recusa_dos_ids(&doc);
        self.instalar_documento(doc);
        let curto = hash_curto(&aberta.versao.hash).to_owned();
        self.aviso_versoes = Some(match recusa {
            Some(m) => format!("Continuando a partir da Versão {curto}. {m}"),
            None => format!(
                "Continuando a partir da Versão {curto}: a próxima Versão salva será filha dela."
            ),
        });
        self.conferir_originais();
    }
}
