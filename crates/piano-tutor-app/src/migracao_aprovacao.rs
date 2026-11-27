//! Migração única das aprovações antigas (`<id>.estado.toml` = `aprovada`) para o índice de
//! Versões (ticket 23, dívida registrada pelo supervisor).
//!
//! Desde o ticket 25 só vale a aprovação **por hash**, registrada no índice (`<id>.versoes/`).
//! Partituras aprovadas antes disso só têm o `estado.toml`; sem este passo voltariam a Hipótese.
//!
//! Para cada partitura da Oficina cujo `estado.toml` diz `aprovada` e que **ainda não tem índice**:
//! registra a origem (a Hipótese = o arquivo `<id>.musicxml` de agora) e uma `[[decisao]]` de
//! aprovação direta (`direto = true`, revisor "migração", motivo "migração do estado.toml") para o
//! hash ATUAL do arquivo.
//!
//! Limite conhecido: se o MusicXML mudou depois da aprovação antiga, não há como saber. A
//! aprovação migrada vale para o conteúdo de agora, o que o titular vê no aviso ao abrir a
//! Oficina (que lista tudo o que foi migrado). Decisão documentada no ticket 23.
//!
//! Garantias: idempotente (com índice, não faz nada); nunca altera `<id>.musicxml` nem
//! `<id>.estado.toml`; índice ilegível é relatado e deixado como está; sem arquivo MusicXML
//! legível a partitura não migra (relatada). Roda ao abrir a Oficina.

use crate::revisao::{Estado, Oficina};
use crate::versoes::{Versoes, agora_utc, hash_do_conteudo};

/// Revisor gravado nas decisões migradas.
pub const REVISOR_DA_MIGRACAO: &str = "migração";
/// Motivo gravado nas decisões migradas.
pub const MOTIVO_DA_MIGRACAO: &str = "migração do estado.toml";

/// O que a migração fez numa Oficina.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelatorioDaMigracao {
    /// Ids migrados, em ordem.
    pub migradas: Vec<String>,
    /// Ids que não puderam migrar, com o motivo.
    pub problemas: Vec<(String, String)>,
}

impl RelatorioDaMigracao {
    pub fn vazio(&self) -> bool {
        self.migradas.is_empty() && self.problemas.is_empty()
    }

    pub fn juntar(&mut self, outro: Self) {
        self.migradas.extend(outro.migradas);
        self.problemas.extend(outro.problemas);
    }

    /// Aviso legível para o titular; `None` se nada aconteceu.
    pub fn aviso(&self) -> Option<String> {
        if self.vazio() {
            return None;
        }
        let mut t = String::new();
        if !self.migradas.is_empty() {
            t.push_str(&format!(
                "Aprovações antigas migradas para as Versões ({}): {}. A aprovação vale para o \
                 conteúdo atual de cada arquivo (revisor \"{REVISOR_DA_MIGRACAO}\", aprovação \
                 direta). Se algum arquivo mudou depois de aprovado, reabra-o.",
                self.migradas.len(),
                self.migradas.join(", ")
            ));
        }
        if !self.problemas.is_empty() {
            if !t.is_empty() {
                t.push(' ');
            }
            t.push_str("Não migraram: ");
            let partes: Vec<String> = self
                .problemas
                .iter()
                .map(|(id, m)| format!("{id} ({m})"))
                .collect();
            t.push_str(&partes.join("; "));
            t.push('.');
        }
        Some(t)
    }
}

/// Migra as aprovações antigas da `oficina` (veja o módulo). Nunca falha: o que der errado vai
/// para `problemas`.
pub fn migrar_aprovacoes(oficina: &Oficina) -> RelatorioDaMigracao {
    migrar_em(oficina, &agora_utc())
}

fn migrar_em(oficina: &Oficina, em: &str) -> RelatorioDaMigracao {
    let mut r = RelatorioDaMigracao::default();
    for item in oficina.partituras() {
        if item.problema.is_some() || item.estado != Estado::Aprovada {
            continue;
        }
        let v = Versoes::da_partitura(&item);
        match v.indice() {
            Ok(Some(_)) => continue,
            Ok(None) => {}
            Err(e) => {
                r.problemas.push((item.id, e.to_string()));
                continue;
            }
        }
        let Ok(bytes) = std::fs::read(&item.musicxml) else {
            r.problemas
                .push((item.id, "sem arquivo MusicXML".to_owned()));
            continue;
        };
        let hash = hash_do_conteudo(&bytes);
        let feito = v.registrar_origem(REVISOR_DA_MIGRACAO, em).and_then(|_| {
            v.aprovar_direto(&hash, Some(MOTIVO_DA_MIGRACAO), REVISOR_DA_MIGRACAO, em)
        });
        match feito {
            Ok(_) => r.migradas.push(item.id),
            Err(e) => r.problemas.push((item.id, e.to_string())),
        }
    }
    r
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::revisao::ItemPartitura;
    use crate::versoes::{ARQUIVO_DO_INDICE, Aprovacao};
    use std::fs;
    use std::path::{Path, PathBuf};

    const XML: &str = "<score-partwise><!-- migracao --></score-partwise>";

    fn oficina(d: &Path) -> Oficina {
        let raiz = d.join("oficina");
        fs::create_dir_all(raiz.join("partituras")).expect("cria");
        Oficina {
            tutor: "t".into(),
            raiz,
        }
    }

    fn partitura(o: &Oficina, id: &str, estado: &str, xml: Option<&str>) {
        let p = o.pasta_de_partituras();
        fs::write(
            p.join(format!("{id}.estado.toml")),
            format!("estado = \"{estado}\"\ntitulo = \"{id}\"\n"),
        )
        .expect("estado");
        if let Some(x) = xml {
            fs::write(p.join(format!("{id}.musicxml")), x).expect("xml");
        }
    }

    fn indice_de(o: &Oficina, id: &str) -> PathBuf {
        o.pasta_de_partituras()
            .join(format!("{id}.versoes"))
            .join(ARQUIVO_DO_INDICE)
    }

    fn aprovacao(o: &Oficina, id: &str) -> Option<Aprovacao> {
        let item = ItemPartitura::ler(o, id);
        let v = Versoes::da_partitura(&item);
        v.aprovacao_do_arquivo(&item.musicxml).expect("lê")
    }

    #[test]
    fn migra_aprovada_sem_indice_para_o_hash_atual_sem_tocar_nos_arquivos() {
        let d = tempfile::tempdir().expect("t");
        let o = oficina(d.path());
        partitura(&o, "a", "aprovada", Some(XML));
        partitura(&o, "b", "hipotese", Some(XML));
        partitura(&o, "c", "em_revisao", Some(XML));
        let estado_antes = fs::read(o.pasta_de_partituras().join("a.estado.toml")).expect("l");

        let r = migrar_em(&o, "2026-11-28T10:00:00Z");
        assert_eq!(r.migradas, vec!["a".to_owned()]);
        assert!(r.problemas.is_empty());

        let ap = aprovacao(&o, "a").expect("aprovada por hash");
        assert_eq!(ap.hash, hash_do_conteudo(XML.as_bytes()));
        assert_eq!(ap.revisor, REVISOR_DA_MIGRACAO);
        assert_eq!(ap.motivo, MOTIVO_DA_MIGRACAO);
        assert!(ap.direto);
        assert!(!indice_de(&o, "b").exists());
        assert!(!indice_de(&o, "c").exists());
        assert_eq!(
            fs::read_to_string(o.pasta_de_partituras().join("a.musicxml")).expect("l"),
            XML
        );
        assert_eq!(
            fs::read(o.pasta_de_partituras().join("a.estado.toml")).expect("l"),
            estado_antes
        );
        let aviso = r.aviso().expect("aviso");
        assert!(
            aviso.contains("migradas") && aviso.contains(": a."),
            "{aviso}"
        );
    }

    #[test]
    fn e_idempotente_segunda_vez_nao_muda_nada() {
        let d = tempfile::tempdir().expect("t");
        let o = oficina(d.path());
        partitura(&o, "a", "aprovada", Some(XML));
        migrar_em(&o, "2026-11-28T10:00:00Z");
        let antes = fs::read(indice_de(&o, "a")).expect("l");
        let r = migrar_em(&o, "2026-12-01T10:00:00Z");
        assert!(r.vazio());
        assert!(r.aviso().is_none());
        assert_eq!(fs::read(indice_de(&o, "a")).expect("l"), antes);
    }

    #[test]
    fn nao_desfaz_reabertura_de_quem_ja_tem_indice() {
        let d = tempfile::tempdir().expect("t");
        let o = oficina(d.path());
        partitura(&o, "a", "aprovada", Some(XML));
        migrar_em(&o, "2026-11-28T10:00:00Z");
        let item = ItemPartitura::ler(&o, "a");
        let v = Versoes::da_partitura(&item);
        v.reabrir(
            &hash_do_conteudo(XML.as_bytes()),
            Some("revisar"),
            "ana",
            "2026-11-29T10:00:00Z",
        )
        .expect("reabre");
        let r = migrar_em(&o, "2026-12-01T10:00:00Z");
        assert!(r.vazio());
        assert!(aprovacao(&o, "a").is_none());
    }

    #[test]
    fn sem_musicxml_ou_com_indice_ilegivel_relata_e_nao_cria_nem_apaga() {
        let d = tempfile::tempdir().expect("t");
        let o = oficina(d.path());
        partitura(&o, "sem", "aprovada", None);
        partitura(&o, "ruim", "aprovada", Some(XML));
        let pasta = o.pasta_de_partituras().join("ruim.versoes");
        fs::create_dir_all(&pasta).expect("cria");
        fs::write(pasta.join(ARQUIVO_DO_INDICE), "isto não é toml [[").expect("w");
        let r = migrar_em(&o, "2026-11-28T10:00:00Z");
        assert!(r.migradas.is_empty());
        assert_eq!(r.problemas.len(), 2);
        assert!(!indice_de(&o, "sem").exists());
        assert_eq!(
            fs::read_to_string(pasta.join(ARQUIVO_DO_INDICE)).expect("l"),
            "isto não é toml [["
        );
        assert!(r.aviso().expect("aviso").contains("Não migraram"));
    }

    #[test]
    fn arquivo_mudado_depois_nao_herda_a_aprovacao() {
        let d = tempfile::tempdir().expect("t");
        let o = oficina(d.path());
        partitura(&o, "a", "aprovada", Some(XML));
        migrar_em(&o, "2026-11-28T10:00:00Z");
        fs::write(o.pasta_de_partituras().join("a.musicxml"), "<outro/>").expect("w");
        assert!(aprovacao(&o, "a").is_none());
    }
}
