//! Instalação atômica em `<dados>/cursos/<id>/` e listagem dos cursos instalados.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::chaves::ChavePublica;
use crate::erro::ErroPacote;
use crate::leitura::PacoteVerificado;
use crate::modelo::{Contagem, Manifesto};
use crate::{ARQUIVO_ASSINATURA, ARQUIVO_MANIFESTO, PASTAS_RESERVADAS};

/// Curso já instalado em `<dados>/cursos/<id>/`.
#[derive(Debug, Clone)]
pub struct CursoInstalado {
    pub id: String,
    pub titulo: String,
    pub versao: String,
    pub autor: String,
    pub licenciado: String,
    pub chave_publica: ChavePublica,
    /// `false` se a assinatura guardada não confere mais com o manifesto instalado.
    pub assinatura_valida: bool,
    pub contagem: Contagem,
    pub pasta: PathBuf,
}

/// Instala um pacote já verificado em `<pasta_dados>/cursos/<id>/` e devolve essa pasta.
///
/// Extrai primeiro para uma pasta temporária ao lado e só então troca (rename), então uma falha
/// no meio nunca deixa o curso pela metade; a versão anterior só é substituída porque a nova
/// já foi verificada (um [`PacoteVerificado`] não existe sem isso). Se o curso instalado foi
/// assinado por outra chave, recusa com [`ErroPacote::AutorDiferente`].
pub fn instalar(pacote: &PacoteVerificado, pasta_dados: &Path) -> Result<PathBuf, ErroPacote> {
    let id = &pacote.manifesto.id;
    // Reforço: o id vira nome de pasta.
    if !crate::id_valido(id) || PASTAS_RESERVADAS.contains(&id.as_str()) {
        return Err(ErroPacote::IdInvalido {
            tipo: "curso",
            id: id.clone(),
        });
    }
    let cursos = pasta_dados.join("cursos");
    fs::create_dir_all(&cursos)
        .map_err(|e| ErroPacote::Io(format!("{}: {e}", cursos.display())))?;
    let destino = cursos.join(id);

    if let Some(chave_instalada) = chave_do_instalado(&destino)
        && chave_instalada != pacote.chave_publica
    {
        return Err(ErroPacote::AutorDiferente { curso: id.clone() });
    }

    let sufixo = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let temporaria = cursos.join(format!(".instalando-{id}-{sufixo}"));
    if let Err(e) = extrair(pacote, &temporaria) {
        let _ = fs::remove_dir_all(&temporaria);
        return Err(e);
    }

    if destino.exists() {
        let antiga = cursos.join(format!(".substituido-{id}-{sufixo}"));
        let io = |e: std::io::Error| ErroPacote::Io(format!("{}: {e}", destino.display()));
        if let Err(e) = fs::rename(&destino, &antiga) {
            let _ = fs::remove_dir_all(&temporaria);
            return Err(io(e));
        }
        if let Err(e) = fs::rename(&temporaria, &destino) {
            // Desfaz: devolve a versão anterior.
            let _ = fs::rename(&antiga, &destino);
            let _ = fs::remove_dir_all(&temporaria);
            return Err(io(e));
        }
        let _ = fs::remove_dir_all(&antiga);
    } else if let Err(e) = fs::rename(&temporaria, &destino) {
        let _ = fs::remove_dir_all(&temporaria);
        return Err(ErroPacote::Io(format!("{}: {e}", destino.display())));
    }
    Ok(destino)
}

fn extrair(pacote: &PacoteVerificado, pasta: &Path) -> Result<(), ErroPacote> {
    let io = |e: std::io::Error| ErroPacote::Io(format!("{}: {e}", pasta.display()));
    fs::create_dir(pasta).map_err(io)?;
    fs::write(pasta.join(ARQUIVO_MANIFESTO), &pacote.manifesto_bytes).map_err(io)?;
    fs::write(pasta.join(ARQUIVO_ASSINATURA), pacote.assinatura).map_err(io)?;
    for (caminho, dados) in &pacote.arquivos {
        let destino = pasta.join(caminho);
        if let Some(pai) = destino.parent() {
            fs::create_dir_all(pai).map_err(io)?;
        }
        fs::write(&destino, dados).map_err(io)?;
    }
    Ok(())
}

/// Chave que assinou o curso instalado em `pasta`, se estiver íntegro (assinatura confere).
fn chave_do_instalado(pasta: &Path) -> Option<ChavePublica> {
    ler_instalado(pasta)
        .filter(|l| l.assinatura_valida)
        .map(|l| l.chave)
}

/// Manifesto de uma pasta instalada, com a chave que declara e se a assinatura guardada confere.
struct Instalado {
    manifesto: Manifesto,
    chave: ChavePublica,
    assinatura_valida: bool,
}

fn ler_instalado(pasta: &Path) -> Option<Instalado> {
    let bytes = fs::read(pasta.join(ARQUIVO_MANIFESTO)).ok()?;
    let manifesto = Manifesto::de_toml(std::str::from_utf8(&bytes).ok()?).ok()?;
    let chave = ChavePublica::de_hex(&manifesto.assinatura.chave_publica).ok()?;
    let assinatura: Option<[u8; 64]> = fs::read(pasta.join(ARQUIVO_ASSINATURA))
        .ok()
        .and_then(|a| a.as_slice().try_into().ok());
    let assinatura_valida = assinatura.is_some_and(|a| chave.verificar(&bytes, &a));
    Some(Instalado {
        manifesto,
        chave,
        assinatura_valida,
    })
}

/// Lista os cursos de `<pasta_dados>/cursos/` (ignora pastas ocultas, `progresso`,
/// `chaves-confiaveis` e pastas sem manifesto legível), em ordem de id.
pub fn cursos_instalados(pasta_dados: &Path) -> Result<Vec<CursoInstalado>, ErroPacote> {
    let cursos = pasta_dados.join("cursos");
    let entradas = match fs::read_dir(&cursos) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(ErroPacote::Io(format!("{}: {e}", cursos.display()))),
    };
    let mut lista = Vec::new();
    for entrada in entradas.filter_map(Result::ok) {
        let nome = entrada.file_name().to_string_lossy().into_owned();
        if nome.starts_with('.')
            || PASTAS_RESERVADAS.contains(&nome.as_str())
            || !entrada.path().is_dir()
        {
            continue;
        }
        let Some(Instalado {
            manifesto: m,
            chave,
            assinatura_valida: valida,
        }) = ler_instalado(&entrada.path())
        else {
            continue;
        };
        lista.push(CursoInstalado {
            contagem: m.contagem(),
            id: m.id,
            titulo: m.titulo,
            versao: m.versao,
            autor: m.autor.nome,
            licenciado: m.licenciado.nome,
            chave_publica: chave,
            assinatura_valida: valida,
            pasta: entrada.path(),
        });
    }
    lista.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(lista)
}
