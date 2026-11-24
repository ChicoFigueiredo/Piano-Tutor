//! Lógica da CLI `piano-tutor-curso` (separada do `main` para ser testada).

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{
    ChavePrivada, Confianca, ErroPacote, EstadoConteudo, PacoteVerificado, chaves_confiaveis,
    cursos_instalados, empacotar, gerar_par, instalar, ler_chaves_da_pasta, ler_pacote_de_arquivo,
};

pub const AJUDA: &str = "\
uso: piano-tutor-curso <comando> [argumentos]

comandos:
  chave-gerar <pasta-saida>
      grava curso.chave (privada, permissão 0600) e curso.pub (pública)
  empacotar <pasta-origem> --chave <arquivo> -o <saida.ptcurso>
      recalcula os hashes, assina e gera o pacote
  verificar <arquivo.ptcurso> [--confiaveis <pasta>] [--dados <pasta>]
      confere assinatura e arquivos; mostra autor, chave e contagens
  instalar <arquivo.ptcurso> [--dados <pasta>]
      instala em <dados>/cursos/<id>/ (padrão de --dados: pasta de dados do usuário/piano-tutor)
  listar [--dados <pasta>]
      lista os cursos instalados";

/// Erro da CLI: uso incorreto (código 2) ou falha na execução (código 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroCli {
    Uso(String),
    Falha(String),
}

impl fmt::Display for ErroCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uso(m) | Self::Falha(m) => f.write_str(m),
        }
    }
}

impl From<ErroPacote> for ErroCli {
    fn from(e: ErroPacote) -> Self {
        Self::Falha(e.to_string())
    }
}

impl From<std::io::Error> for ErroCli {
    fn from(e: std::io::Error) -> Self {
        Self::Falha(format!("erro de saída: {e}"))
    }
}

/// Executa a CLI com `args` (sem o nome do programa), escrevendo o resultado em `saida`.
pub fn executar(args: &[String], saida: &mut dyn Write) -> Result<(), ErroCli> {
    let Some((comando, resto)) = args.split_first() else {
        return Err(ErroCli::Uso("faltou o comando".into()));
    };
    match comando.as_str() {
        "-h" | "--help" | "ajuda" => {
            writeln!(saida, "{AJUDA}")?;
            Ok(())
        }
        "chave-gerar" => chave_gerar(resto, saida),
        "empacotar" => cmd_empacotar(resto, saida),
        "verificar" => verificar(resto, saida),
        "instalar" => cmd_instalar(resto, saida),
        "listar" => listar(resto, saida),
        outro => Err(ErroCli::Uso(format!("comando desconhecido: {outro:?}"))),
    }
}

/// Argumentos já separados em posicionais e opções com valor.
struct Argumentos {
    posicionais: Vec<String>,
    opcoes: Vec<(String, String)>,
}

impl Argumentos {
    /// `validas` lista as opções aceitas, cada uma com seus apelidos.
    fn ler(args: &[String], validas: &[&[&str]]) -> Result<Self, ErroCli> {
        let mut posicionais = Vec::new();
        let mut opcoes = Vec::new();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if a.starts_with('-') && a.len() > 1 {
                let Some(grupo) = validas.iter().find(|g| g.contains(&a.as_str())) else {
                    return Err(ErroCli::Uso(format!("opção desconhecida: {a}")));
                };
                let valor = it
                    .next()
                    .ok_or_else(|| ErroCli::Uso(format!("a opção {a} precisa de um valor")))?;
                opcoes.push((grupo[0].to_string(), valor.clone()));
            } else {
                posicionais.push(a.clone());
            }
        }
        Ok(Self {
            posicionais,
            opcoes,
        })
    }

    fn opcao(&self, nome: &str) -> Option<&str> {
        self.opcoes
            .iter()
            .rev()
            .find(|(n, _)| n == nome)
            .map(|(_, v)| v.as_str())
    }

    fn exigir_posicionais(&self, quantos: usize, uso: &str) -> Result<(), ErroCli> {
        if self.posicionais.len() == quantos {
            Ok(())
        } else {
            Err(ErroCli::Uso(format!("uso: piano-tutor-curso {uso}")))
        }
    }
}

fn pasta_dados(a: &Argumentos) -> Result<PathBuf, ErroCli> {
    match a.opcao("--dados") {
        Some(p) => Ok(PathBuf::from(p)),
        None => dirs::data_dir()
            .map(|d| d.join("piano-tutor"))
            .ok_or_else(|| {
                ErroCli::Falha(
                    "não achei a pasta de dados do usuário; informe --dados <pasta>".into(),
                )
            }),
    }
}

fn chave_gerar(args: &[String], saida: &mut dyn Write) -> Result<(), ErroCli> {
    let a = Argumentos::ler(args, &[])?;
    a.exigir_posicionais(1, "chave-gerar <pasta-saida>")?;
    let pasta = Path::new(&a.posicionais[0]);
    std::fs::create_dir_all(pasta).map_err(ErroPacote::from)?;
    let (privada, publica) = gerar_par();
    let caminho_privada = pasta.join("curso.chave");
    let caminho_publica = pasta.join("curso.pub");
    privada.gravar(&caminho_privada)?;
    publica.gravar(&caminho_publica)?;
    writeln!(
        saida,
        "chave privada: {} (guarde fora do repositório; permissão 0600)",
        caminho_privada.display()
    )?;
    writeln!(saida, "chave pública: {}", caminho_publica.display())?;
    writeln!(saida, "chave curta:   {}", publica.curta())?;
    Ok(())
}

fn cmd_empacotar(args: &[String], saida: &mut dyn Write) -> Result<(), ErroCli> {
    let a = Argumentos::ler(args, &[&["--chave"], &["-o", "--saida"]])?;
    let uso = "empacotar <pasta-origem> --chave <arquivo> -o <saida.ptcurso>";
    a.exigir_posicionais(1, uso)?;
    let chave = a
        .opcao("--chave")
        .ok_or_else(|| ErroCli::Uso(format!("faltou --chave; uso: {uso}")))?;
    let destino = a
        .opcao("-o")
        .ok_or_else(|| ErroCli::Uso(format!("faltou -o; uso: {uso}")))?;
    let chave = ChavePrivada::ler(Path::new(chave))?;
    let bytes = empacotar(Path::new(&a.posicionais[0]), &chave)?;
    std::fs::write(destino, &bytes).map_err(|e| ErroPacote::Io(format!("{destino}: {e}")))?;
    writeln!(saida, "pacote gravado: {destino} ({} bytes)", bytes.len())?;
    writeln!(saida, "chave: {}", chave.publica().curta())?;
    Ok(())
}

fn verificar(args: &[String], saida: &mut dyn Write) -> Result<(), ErroCli> {
    let a = Argumentos::ler(args, &[&["--confiaveis"], &["--dados"]])?;
    a.exigir_posicionais(1, "verificar <arquivo.ptcurso> [--confiaveis <pasta>]")?;
    let pacote = ler_pacote_de_arquivo(Path::new(&a.posicionais[0]))?;
    let confiaveis = match a.opcao("--confiaveis") {
        Some(p) => ler_chaves_da_pasta(Path::new(p)),
        None => chaves_confiaveis(&pasta_dados(&a)?),
    };
    descrever(&pacote, pacote.confianca(&confiaveis), saida)
}

fn descrever(
    pacote: &PacoteVerificado,
    confianca: Confianca,
    saida: &mut dyn Write,
) -> Result<(), ErroCli> {
    let m = &pacote.manifesto;
    let c = pacote.contagem();
    writeln!(
        saida,
        "Curso:       {} ({}) versão {}",
        m.titulo, m.id, m.versao
    )?;
    writeln!(saida, "Autor:       {}", m.autor.nome)?;
    writeln!(saida, "Licenciado:  {}", m.licenciado.nome)?;
    if !m.licenciado.licenca.is_empty() {
        writeln!(saida, "Licença:     {}", m.licenciado.licenca)?;
    }
    writeln!(saida, "Chave:       {}", pacote.chave_publica.curta())?;
    writeln!(
        saida,
        "Assinatura:  válida; {} arquivos conferidos",
        m.arquivos.len()
    )?;
    let texto = match confianca {
        Confianca::Confiavel => "confiável",
        Confianca::Desconhecida => "desconhecida (chave fora da lista de confiáveis)",
    };
    writeln!(saida, "Confiança:   {texto}")?;
    writeln!(
        saida,
        "Seções: {}  Lições: {}  Exercícios: {}",
        c.secoes, c.licoes, c.exercicios
    )?;
    for estado in EstadoConteudo::TODOS {
        writeln!(saida, "  {:<11} {}", estado.rotulo(), c.por_estado(estado))?;
    }
    if let Some(r) = &pacote.roteiro {
        writeln!(
            saida,
            "Roteiro:     {} falas, {} aprovadas",
            r.falas.len(),
            r.falas_aprovadas().count()
        )?;
    }
    Ok(())
}

fn cmd_instalar(args: &[String], saida: &mut dyn Write) -> Result<(), ErroCli> {
    let a = Argumentos::ler(args, &[&["--dados"]])?;
    a.exigir_posicionais(1, "instalar <arquivo.ptcurso> [--dados <pasta>]")?;
    let dados = pasta_dados(&a)?;
    let pacote = ler_pacote_de_arquivo(Path::new(&a.posicionais[0]))?;
    let confianca = pacote.confianca(&chaves_confiaveis(&dados));
    let destino = instalar(&pacote, &dados)?;
    writeln!(
        saida,
        "instalado: {} ({}) em {}",
        pacote.manifesto.titulo,
        pacote.manifesto.versao,
        destino.display()
    )?;
    if confianca == Confianca::Desconhecida {
        writeln!(
            saida,
            "aviso: a chave {} não está entre as confiáveis; para confiar nela, grave o .pub em {}",
            pacote.chave_publica.curta(),
            dados.join("cursos").join("chaves-confiaveis").display()
        )?;
    }
    Ok(())
}

fn listar(args: &[String], saida: &mut dyn Write) -> Result<(), ErroCli> {
    let a = Argumentos::ler(args, &[&["--dados"]])?;
    a.exigir_posicionais(0, "listar [--dados <pasta>]")?;
    let dados = pasta_dados(&a)?;
    let cursos = cursos_instalados(&dados)?;
    if cursos.is_empty() {
        writeln!(
            saida,
            "nenhum curso instalado em {}",
            dados.join("cursos").display()
        )?;
        return Ok(());
    }
    let confiaveis = chaves_confiaveis(&dados);
    for c in cursos {
        let confianca = if confiaveis.contains(&c.chave_publica) {
            "confiável"
        } else {
            "desconhecida"
        };
        let integridade = if c.assinatura_valida {
            ""
        } else {
            "  [ASSINATURA NÃO CONFERE]"
        };
        writeln!(
            saida,
            "{}  {}  v{}  autor: {}  chave {} ({confianca})  {} exercícios ({} aprovados){integridade}",
            c.id,
            c.titulo,
            c.versao,
            c.autor,
            c.chave_publica.curta(),
            c.contagem.exercicios,
            c.contagem.por_estado(EstadoConteudo::Aprovada)
        )?;
    }
    Ok(())
}
