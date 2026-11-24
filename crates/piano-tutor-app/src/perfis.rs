//! Perfis de preferências em SQLite (ADR-0010): `~/.local/share/piano-tutor/piano-tutor.sqlite`.
//!
//! - Cada **perfil** guarda as preferências por seção (`som`, `exibicao`, `partitura`, `tema`),
//!   cada uma no texto `chave=valor` que o próprio módulo da seção já lê e escreve
//!   ([`crate::preferencias`]); seção ausente = padrão.
//! - O perfil **De fábrica** existe sempre, não tem linhas e não pode ser alterado, renomeado
//!   nem apagado: escolhê-lo devolve tudo ao padrão. Mudar uma preferência com ele ativo cria
//!   (ou reusa) o perfil "Meu perfil" e passa a gravar nele.
//! - Na primeira abertura, os antigos `preferencias*.txt` viram o perfil "Meu perfil" (os
//!   arquivos ficam onde estão, como cópia de segurança, e deixam de ser gravados).

use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

/// Arquivo do banco, na pasta de dados do usuário.
pub const ARQUIVO_BANCO: &str = "piano-tutor.sqlite";
/// Nome do perfil fixo com os padrões.
pub const NOME_FABRICA: &str = "De fábrica";
/// Nome do perfil criado sozinho na primeira mudança feita sobre o de fábrica.
pub const NOME_PESSOAL: &str = "Meu perfil";
/// Versão do esquema (tabela `meta`, chave `esquema`).
pub const VERSAO_ESQUEMA: i64 = 1;
/// Maior texto aceito numa seção (as seções reais têm poucas centenas de bytes).
const MAX_VALOR: usize = 16 * 1024;

/// Seções de preferências.
pub mod secoes {
    pub const SOM: &str = "som";
    pub const EXIBICAO: &str = "exibicao";
    pub const PARTITURA: &str = "partitura";
    pub const TEMA: &str = "tema";
}

/// Arquivos de texto antigos importados na primeira abertura: (seção, arquivo).
pub const LEGADO: [(&str, &str); 3] = [
    (secoes::SOM, "preferencias.txt"),
    (secoes::EXIBICAO, "preferencias-exibicao.txt"),
    (secoes::PARTITURA, "preferencias-partitura.txt"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Perfil {
    pub id: i64,
    pub nome: String,
    pub de_fabrica: bool,
}

/// O que uma gravação fez com os perfis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gravacao {
    /// Gravou no perfil ativo.
    NoAtivo,
    /// O ativo era o de fábrica: criou ou reusou "Meu perfil", ativou e gravou nele.
    CriouPessoal,
    /// O valor já era esse; nada mudou.
    SemMudanca,
}

pub type Resultado<T> = Result<T, String>;

fn erro(e: rusqlite::Error) -> String {
    format!("banco de preferências: {e}")
}

/// Conexão com o banco de perfis.
pub struct BancoPreferencias {
    conn: Connection,
}

impl std::fmt::Debug for BancoPreferencias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BancoPreferencias").finish_non_exhaustive()
    }
}

impl BancoPreferencias {
    /// Abre (ou cria) o banco em `caminho`.
    pub fn abrir(caminho: &Path) -> Resultado<Self> {
        if let Some(p) = caminho.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("banco de preferências: {e}"))?;
        }
        Self::preparar(Connection::open(caminho).map_err(erro)?)
    }

    /// Banco só na memória (testes e quando não há pasta de dados).
    pub fn em_memoria() -> Resultado<Self> {
        Self::preparar(Connection::open_in_memory().map_err(erro)?)
    }

    fn preparar(conn: Connection) -> Resultado<Self> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS meta (chave TEXT PRIMARY KEY, valor TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS perfis (
                 id INTEGER PRIMARY KEY,
                 nome TEXT NOT NULL UNIQUE,
                 de_fabrica INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS preferencias (
                 perfil INTEGER NOT NULL REFERENCES perfis(id) ON DELETE CASCADE,
                 secao TEXT NOT NULL,
                 valor TEXT NOT NULL,
                 PRIMARY KEY (perfil, secao)
             );",
        )
        .map_err(erro)?;
        let banco = Self { conn };
        let tem_fabrica: Option<i64> = banco
            .conn
            .query_row("SELECT id FROM perfis WHERE de_fabrica = 1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(erro)?;
        if tem_fabrica.is_none() {
            banco
                .conn
                .execute(
                    "INSERT INTO perfis (nome, de_fabrica) VALUES (?1, 1)",
                    params![NOME_FABRICA],
                )
                .map_err(erro)?;
        }
        banco.definir_meta("esquema", &VERSAO_ESQUEMA.to_string())?;
        Ok(banco)
    }

    fn meta(&self, chave: &str) -> Resultado<Option<String>> {
        self.conn
            .query_row(
                "SELECT valor FROM meta WHERE chave = ?1",
                params![chave],
                |r| r.get(0),
            )
            .optional()
            .map_err(erro)
    }

    fn definir_meta(&self, chave: &str, valor: &str) -> Resultado<()> {
        self.conn
            .execute(
                "INSERT INTO meta (chave, valor) VALUES (?1, ?2)
                 ON CONFLICT(chave) DO UPDATE SET valor = excluded.valor",
                params![chave, valor],
            )
            .map(|_| ())
            .map_err(erro)
    }

    /// Todos os perfis, o de fábrica primeiro e os demais por nome.
    pub fn perfis(&self) -> Resultado<Vec<Perfil>> {
        let mut st = self
            .conn
            .prepare("SELECT id, nome, de_fabrica FROM perfis ORDER BY de_fabrica DESC, nome")
            .map_err(erro)?;
        st.query_map([], |r| {
            Ok(Perfil {
                id: r.get(0)?,
                nome: r.get(1)?,
                de_fabrica: r.get::<_, i64>(2)? == 1,
            })
        })
        .map_err(erro)?
        .collect::<Result<_, _>>()
        .map_err(erro)
    }

    fn perfil(&self, id: i64) -> Resultado<Option<Perfil>> {
        Ok(self.perfis()?.into_iter().find(|p| p.id == id))
    }

    fn fabrica(&self) -> Resultado<Perfil> {
        self.perfis()?
            .into_iter()
            .find(|p| p.de_fabrica)
            .ok_or_else(|| "banco de preferências: perfil de fábrica ausente".to_owned())
    }

    /// Perfil em uso (o de fábrica se nada foi escolhido ou o escolhido sumiu).
    pub fn ativo(&self) -> Resultado<Perfil> {
        let id = self
            .meta("perfil_ativo")?
            .and_then(|v| v.parse::<i64>().ok());
        match id.map(|i| self.perfil(i)).transpose()?.flatten() {
            Some(p) => Ok(p),
            None => self.fabrica(),
        }
    }

    pub fn ativar(&self, id: i64) -> Resultado<Perfil> {
        let p = self
            .perfil(id)?
            .ok_or_else(|| format!("perfil {id} não existe"))?;
        self.definir_meta("perfil_ativo", &id.to_string())?;
        Ok(p)
    }

    fn nome_valido(nome: &str) -> Resultado<String> {
        let n = nome.trim();
        if n.is_empty() || n.chars().count() > 60 {
            return Err("Dê ao perfil um nome de 1 a 60 caracteres.".to_owned());
        }
        Ok(n.to_owned())
    }

    /// Cria um perfil novo com as preferências de `copiar_de` (o de fábrica = tudo no padrão).
    pub fn criar(&self, nome: &str, copiar_de: i64) -> Resultado<Perfil> {
        let nome = Self::nome_valido(nome)?;
        if self.perfis()?.iter().any(|p| p.nome == nome) {
            return Err(format!("Já existe um perfil chamado \"{nome}\"."));
        }
        self.conn
            .execute("INSERT INTO perfis (nome) VALUES (?1)", params![nome])
            .map_err(erro)?;
        let id = self.conn.last_insert_rowid();
        self.conn
            .execute(
                "INSERT INTO preferencias (perfil, secao, valor)
                 SELECT ?1, secao, valor FROM preferencias WHERE perfil = ?2",
                params![id, copiar_de],
            )
            .map_err(erro)?;
        Ok(Perfil {
            id,
            nome,
            de_fabrica: false,
        })
    }

    pub fn renomear(&self, id: i64, nome: &str) -> Resultado<()> {
        let nome = Self::nome_valido(nome)?;
        let p = self
            .perfil(id)?
            .ok_or_else(|| format!("perfil {id} não existe"))?;
        if p.de_fabrica {
            return Err("O perfil de fábrica não pode ser renomeado.".to_owned());
        }
        if self.perfis()?.iter().any(|o| o.nome == nome && o.id != id) {
            return Err(format!("Já existe um perfil chamado \"{nome}\"."));
        }
        self.conn
            .execute(
                "UPDATE perfis SET nome = ?1 WHERE id = ?2",
                params![nome, id],
            )
            .map(|_| ())
            .map_err(erro)
    }

    /// Apaga um perfil (nunca o de fábrica). Se era o ativo, o de fábrica passa a valer.
    pub fn apagar(&self, id: i64) -> Resultado<()> {
        let p = self
            .perfil(id)?
            .ok_or_else(|| format!("perfil {id} não existe"))?;
        if p.de_fabrica {
            return Err("O perfil de fábrica não pode ser apagado.".to_owned());
        }
        let era_ativo = self.ativo()?.id == id;
        self.conn
            .execute("DELETE FROM perfis WHERE id = ?1", params![id])
            .map_err(erro)?;
        if era_ativo {
            let f = self.fabrica()?;
            self.definir_meta("perfil_ativo", &f.id.to_string())?;
        }
        Ok(())
    }

    /// Texto da seção no perfil ativo (`None` = padrão).
    pub fn ler(&self, secao: &str) -> Resultado<Option<String>> {
        let ativo = self.ativo()?;
        self.conn
            .query_row(
                "SELECT valor FROM preferencias WHERE perfil = ?1 AND secao = ?2",
                params![ativo.id, secao],
                |r| r.get(0),
            )
            .optional()
            .map_err(erro)
    }

    /// Grava a seção no perfil ativo. Sobre o de fábrica, grava em "Meu perfil" (criado se
    /// preciso, com os padrões) e o ativa.
    pub fn gravar(&self, secao: &str, valor: &str) -> Resultado<Gravacao> {
        if valor.len() > MAX_VALOR {
            return Err(format!("preferência \"{secao}\" grande demais"));
        }
        if self.ler(secao)?.as_deref() == Some(valor) {
            return Ok(Gravacao::SemMudanca);
        }
        let mut ativo = self.ativo()?;
        let mut resultado = Gravacao::NoAtivo;
        if ativo.de_fabrica {
            let existente = self
                .perfis()?
                .into_iter()
                .find(|p| !p.de_fabrica && p.nome == NOME_PESSOAL);
            ativo = match existente {
                Some(p) => p,
                None => self.criar(NOME_PESSOAL, ativo.id)?,
            };
            // "Meu perfil" reusado parte do padrão, como o de fábrica de onde se partiu.
            self.conn
                .execute(
                    "DELETE FROM preferencias WHERE perfil = ?1",
                    params![ativo.id],
                )
                .map_err(erro)?;
            self.ativar(ativo.id)?;
            resultado = Gravacao::CriouPessoal;
        }
        self.conn
            .execute(
                "INSERT INTO preferencias (perfil, secao, valor) VALUES (?1, ?2, ?3)
                 ON CONFLICT(perfil, secao) DO UPDATE SET valor = excluded.valor",
                params![ativo.id, secao, valor],
            )
            .map_err(erro)?;
        Ok(resultado)
    }

    /// Primeira abertura: se ainda só existe o de fábrica e há `preferencias*.txt` antigos em
    /// `pasta`, cria "Meu perfil" com eles e o ativa. Devolve quantas seções importou.
    pub fn importar_legado(&self, pasta: &Path) -> Resultado<usize> {
        if self.meta("legado_importado")?.is_some() {
            return Ok(0);
        }
        let mut n = 0;
        if self.perfis()?.iter().all(|p| p.de_fabrica) {
            for (secao, arquivo) in LEGADO {
                let Ok(t) = std::fs::read_to_string(pasta.join(arquivo)) else {
                    continue;
                };
                if t.len() <= MAX_VALOR {
                    self.gravar(secao, &t)?;
                    n += 1;
                }
            }
        }
        self.definir_meta("legado_importado", "1")?;
        Ok(n)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn de_fabrica_existe_e_e_o_ativo_inicial_sem_preferencias() {
        let b = BancoPreferencias::em_memoria().expect("banco");
        let a = b.ativo().expect("ativo");
        assert!(a.de_fabrica);
        assert_eq!(a.nome, NOME_FABRICA);
        assert_eq!(b.ler(secoes::SOM).expect("ler"), None);
    }

    #[test]
    fn mudar_sobre_o_de_fabrica_cria_meu_perfil_e_o_de_fabrica_fica_intacto() {
        let b = BancoPreferencias::em_memoria().expect("banco");
        let fabrica = b.ativo().expect("ativo");
        assert_eq!(
            b.gravar(secoes::SOM, "volume=0.5\n").expect("gravar"),
            Gravacao::CriouPessoal
        );
        let ativo = b.ativo().expect("ativo");
        assert_eq!(ativo.nome, NOME_PESSOAL);
        assert_eq!(
            b.gravar(secoes::SOM, "volume=0.6\n").expect("gravar"),
            Gravacao::NoAtivo
        );
        assert_eq!(
            b.gravar(secoes::SOM, "volume=0.6\n").expect("gravar"),
            Gravacao::SemMudanca
        );
        b.ativar(fabrica.id).expect("ativar");
        assert_eq!(b.ler(secoes::SOM).expect("ler"), None, "fábrica = padrão");
        b.ativar(ativo.id).expect("ativar");
        assert_eq!(
            b.ler(secoes::SOM).expect("ler").as_deref(),
            Some("volume=0.6\n")
        );
    }

    #[test]
    fn criar_copia_renomear_e_apagar_sem_tocar_no_de_fabrica() {
        let b = BancoPreferencias::em_memoria().expect("banco");
        let fabrica = b.ativo().expect("ativo");
        b.gravar(secoes::TEMA, "noite").expect("gravar");
        let meu = b.ativo().expect("ativo");
        let aula = b.criar("Aula", meu.id).expect("criar");
        b.ativar(aula.id).expect("ativar");
        assert_eq!(b.ler(secoes::TEMA).expect("ler").as_deref(), Some("noite"));
        assert!(b.criar("Aula", meu.id).is_err(), "nome repetido");
        assert!(b.criar("   ", meu.id).is_err(), "nome vazio");
        b.renomear(aula.id, "Estudo").expect("renomear");
        assert!(b.renomear(fabrica.id, "X").is_err());
        assert!(b.apagar(fabrica.id).is_err());
        b.apagar(aula.id).expect("apagar");
        assert!(
            b.ativo().expect("ativo").de_fabrica,
            "apagar o ativo volta ao de fábrica"
        );
        let nomes: Vec<String> = b
            .perfis()
            .expect("perfis")
            .into_iter()
            .map(|p| p.nome)
            .collect();
        assert_eq!(
            nomes,
            vec![NOME_FABRICA.to_owned(), NOME_PESSOAL.to_owned()]
        );
    }

    #[test]
    fn importa_os_txt_antigos_uma_vez_e_persiste_no_arquivo() {
        let pasta = std::env::temp_dir().join(format!("pt-perfis-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).expect("pasta");
        std::fs::write(pasta.join("preferencias.txt"), "volume=0.3\n").expect("txt");
        let caminho = pasta.join(ARQUIVO_BANCO);
        {
            let b = BancoPreferencias::abrir(&caminho).expect("banco");
            assert_eq!(b.importar_legado(&pasta).expect("importar"), 1);
            assert_eq!(
                b.importar_legado(&pasta).expect("importar"),
                0,
                "só uma vez"
            );
        }
        let b = BancoPreferencias::abrir(&caminho).expect("reabrir");
        assert_eq!(b.ativo().expect("ativo").nome, NOME_PESSOAL);
        assert_eq!(
            b.ler(secoes::SOM).expect("ler").as_deref(),
            Some("volume=0.3\n")
        );
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
