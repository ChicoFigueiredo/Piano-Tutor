//! Dados do usuário (F14-5): versão dos dados, backup antes de migrar e rollback.
//!
//! A pasta de dados (`~/.local/share/piano-tutor`) guarda preferências, progresso e
//! SoundFonts. Este módulo, em código puro de arquivos (sem janela, áudio nem MIDI):
//!
//! 1. lê/grava `versao-dados.txt` (arquivo ausente = versão 0, dados anteriores ao módulo);
//! 2. antes de **qualquer** migração copia a pasta inteira (menos `backups/`) para
//!    `backups/<versao>-<AAAAMMDD-HHMMSS>/`, e só então aplica as migrações em ordem;
//! 3. se uma migração falha, devolve a pasta ao estado anterior a partir do backup;
//! 4. mantém no máximo [`MAX_BACKUPS`] backups (remove os mais antigos, nunca o recém-criado);
//! 5. permite restaurar um backup ([`restaurar_backup`]), com backup do estado atual antes.
//!
//! Limites assumidos: links simbólicos são **ignorados** (nem copiados nem seguidos); a cópia
//! é completa, inclusive SoundFonts grandes; nada é verificado contra corrupção de disco.
//! Chame [`preparar`] uma vez na inicialização, antes de abrir qualquer arquivo de dados.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Arquivo com a versão dos dados, na raiz da pasta de dados.
pub const ARQUIVO_VERSAO: &str = "versao-dados.txt";
/// Subpasta dos backups (nunca é copiada para dentro de um backup).
pub const PASTA_BACKUPS: &str = "backups";
/// SoundFonts baixados (~150 MB cada): ficam fora dos backups e o rollback não os apaga,
/// porque dá para baixar de novo.
pub const PASTA_SOUNDFONTS: &str = "soundfonts";
/// Versão de dados que esta versão do app entende.
pub const VERSAO_ATUAL: u32 = 1;
/// Quantos backups ficam guardados.
pub const MAX_BACKUPS: usize = 5;

/// Passo de migração de `de` para `para`, aplicado sobre a pasta de dados.
#[derive(Clone, Copy)]
pub struct Migracao {
    pub de: u32,
    pub para: u32,
    pub descricao: &'static str,
    pub aplicar: fn(&Path) -> Result<(), String>,
}

/// Migrações conhecidas, em ordem. Cada uma começa onde a anterior termina.
pub const MIGRACOES: &[Migracao] = &[Migracao {
    de: 0,
    para: 1,
    descricao: "v0 -> v1: passa a registrar a versão dos dados (nenhum arquivo é alterado)",
    aplicar: |_| Ok(()),
}];

/// O que [`preparar`] fez.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelatorioDados {
    pub versao_antes: u32,
    pub versao_depois: u32,
    /// Descrição de cada migração aplicada, em ordem.
    pub migracoes_aplicadas: Vec<String>,
    /// Backup feito antes de migrar (`None` se não havia o que migrar ou o que guardar).
    pub backup: Option<PathBuf>,
    /// Nomes dos backups antigos removidos pelo limite.
    pub backups_removidos: Vec<String>,
}

impl RelatorioDados {
    /// Uma linha para log.
    pub fn resumo(&self) -> String {
        if self.migracoes_aplicadas.is_empty() {
            return format!("dados na versão {}: nada a migrar", self.versao_depois);
        }
        format!(
            "dados migrados da versão {} para a {} ({} passo(s)); backup: {}",
            self.versao_antes,
            self.versao_depois,
            self.migracoes_aplicadas.len(),
            self.backup.as_ref().map_or_else(
                || "nenhum (pasta vazia)".to_owned(),
                |b| b.display().to_string()
            )
        )
    }
}

/// Um backup guardado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Backup {
    /// Nome da pasta, `<versao>-<AAAAMMDD-HHMMSS>` (com `-n` se houve colisão).
    pub nome: String,
    /// Versão dos dados no momento do backup.
    pub versao: u32,
    pub caminho: PathBuf,
}

/// Prepara a pasta de dados: cria, faz backup e migra até [`VERSAO_ATUAL`].
///
/// Erro (sem alterar nada, ou com a pasta devolvida ao estado anterior) quando: o arquivo de
/// versão é ilegível; a versão é mais nova que a do app; falta uma migração; o backup falha; ou
/// uma migração falha (a mensagem diz se a restauração deu certo).
pub fn preparar(pasta: &Path) -> Result<RelatorioDados, String> {
    preparar_com(pasta, agora_unix(), MIGRACOES, MAX_BACKUPS)
}

fn agora_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn erro_io(o: &str, p: &Path, e: &std::io::Error) -> String {
    format!("{o} {}: {e}", p.display())
}

/// Versão gravada na pasta: 0 se o arquivo não existe.
pub fn ler_versao(pasta: &Path) -> Result<u32, String> {
    let arq = pasta.join(ARQUIVO_VERSAO);
    match fs::read_to_string(&arq) {
        Ok(t) => t
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("{} ilegível (conteúdo: {:?})", arq.display(), t.trim())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(erro_io("não consegui ler", &arq, &e)),
    }
}

/// Grava a versão de forma atômica (arquivo temporário + renomear).
pub fn gravar_versao(pasta: &Path, versao: u32) -> Result<(), String> {
    let arq = pasta.join(ARQUIVO_VERSAO);
    let tmp = pasta.join(format!("{ARQUIVO_VERSAO}.tmp"));
    fs::write(&tmp, format!("{versao}\n")).map_err(|e| erro_io("não consegui gravar", &tmp, &e))?;
    fs::rename(&tmp, &arq).map_err(|e| erro_io("não consegui renomear para", &arq, &e))
}

fn preparar_com(
    pasta: &Path,
    agora: u64,
    migracoes: &[Migracao],
    max_backups: usize,
) -> Result<RelatorioDados, String> {
    fs::create_dir_all(pasta).map_err(|e| erro_io("não consegui criar", pasta, &e))?;
    let alvo = migracoes.last().map_or(0, |m| m.para);
    let antes = ler_versao(pasta)?;
    if antes > alvo {
        return Err(format!(
            "os dados estão na versão {antes}, mais nova que a {alvo} que este app entende; \
             atualize o app ou restaure um backup em {}",
            pasta.join(PASTA_BACKUPS).display()
        ));
    }
    let mut relatorio = RelatorioDados {
        versao_antes: antes,
        versao_depois: antes,
        ..RelatorioDados::default()
    };
    if antes == alvo {
        return Ok(relatorio);
    }
    // Plano completo antes de mexer em qualquer coisa.
    let mut plano: Vec<Migracao> = Vec::new();
    let mut v = antes;
    while v < alvo {
        let m = migracoes
            .iter()
            .find(|m| m.de == v && m.para > v)
            .ok_or_else(|| format!("não há migração a partir da versão {v}"))?;
        plano.push(*m);
        v = m.para;
    }

    let backup = if tem_conteudo(pasta)? {
        Some(fazer_backup(pasta, antes, agora)?)
    } else {
        None
    };
    for m in plano {
        let resultado = (m.aplicar)(pasta).and_then(|()| gravar_versao(pasta, m.para));
        if let Err(e) = resultado {
            let destino = match &backup {
                Some(b) => restaurar_de(pasta, Some(b)),
                None => restaurar_de(pasta, None),
            };
            return Err(match (destino, &backup) {
                (Ok(()), Some(b)) => format!(
                    "migração \"{}\" falhou: {e}; dados restaurados do backup {}",
                    m.descricao,
                    b.display()
                ),
                (Ok(()), None) => format!(
                    "migração \"{}\" falhou: {e}; pasta devolvida ao estado vazio",
                    m.descricao
                ),
                (Err(r), b) => format!(
                    "migração \"{}\" falhou: {e}; ATENÇÃO, a restauração também falhou ({r}); \
                     backup: {}",
                    m.descricao,
                    b.as_ref()
                        .map_or_else(|| "nenhum".to_owned(), |b| b.display().to_string())
                ),
            });
        }
        relatorio.migracoes_aplicadas.push(m.descricao.to_owned());
        relatorio.versao_depois = m.para;
    }
    relatorio.backups_removidos = limitar_backups(pasta, max_backups)?;
    relatorio.backup = backup;
    Ok(relatorio)
}

/// Há algo na pasta além de `backups/`? (Só a pasta vazia dispensa o backup.)
fn tem_conteudo(pasta: &Path) -> Result<bool, String> {
    let it = fs::read_dir(pasta).map_err(|e| erro_io("não consegui listar", pasta, &e))?;
    for e in it {
        let e = e.map_err(|e| erro_io("não consegui listar", pasta, &e))?;
        if e.file_name() != PASTA_BACKUPS {
            return Ok(true);
        }
    }
    Ok(false)
}

// --- Cópia -----------------------------------------------------------------------------------

/// Copia `origem` para `destino` (criado), recursivamente. Ignora links simbólicos e, na raiz,
/// as entradas em `ignorar_na_raiz`.
fn copiar_pasta(origem: &Path, destino: &Path, ignorar_na_raiz: &[&str]) -> Result<(), String> {
    fs::create_dir_all(destino).map_err(|e| erro_io("não consegui criar", destino, &e))?;
    let it = fs::read_dir(origem).map_err(|e| erro_io("não consegui listar", origem, &e))?;
    for e in it {
        let e = e.map_err(|e| erro_io("não consegui listar", origem, &e))?;
        let nome = e.file_name();
        if ignorar_na_raiz.iter().any(|i| nome == *i) {
            continue;
        }
        let tipo = e
            .file_type()
            .map_err(|e2| erro_io("não consegui ler o tipo de", &e.path(), &e2))?;
        let (de, para) = (e.path(), destino.join(&nome));
        if tipo.is_dir() {
            copiar_pasta(&de, &para, &[])?;
        } else if tipo.is_file() {
            fs::copy(&de, &para).map_err(|e| erro_io("não consegui copiar", &de, &e))?;
        }
    }
    Ok(())
}

fn pasta_backups(pasta: &Path) -> PathBuf {
    pasta.join(PASTA_BACKUPS)
}

/// Copia a pasta inteira (menos `backups/`) para `backups/<versao>-<data>/`. A cópia vai
/// primeiro para uma pasta temporária e só depois é renomeada, então um backup incompleto
/// nunca aparece na lista.
fn fazer_backup(pasta: &Path, versao: u32, agora: u64) -> Result<PathBuf, String> {
    let raiz = pasta_backups(pasta);
    fs::create_dir_all(&raiz).map_err(|e| erro_io("não consegui criar", &raiz, &e))?;
    let base = format!("{versao}-{}", data_utc(agora));
    let mut nome = base.clone();
    let mut n = 2;
    while raiz.join(&nome).exists() {
        nome = format!("{base}-{n}");
        n += 1;
    }
    let tmp = raiz.join(format!(".tmp-{nome}"));
    let _ = fs::remove_dir_all(&tmp);
    if let Err(e) = copiar_pasta(pasta, &tmp, &[PASTA_BACKUPS, PASTA_SOUNDFONTS]) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(format!("backup não concluído, nada foi migrado: {e}"));
    }
    let final_ = raiz.join(&nome);
    fs::rename(&tmp, &final_).map_err(|e| {
        let _ = fs::remove_dir_all(&tmp);
        erro_io("não consegui concluir o backup em", &final_, &e)
    })?;
    Ok(final_)
}

/// Nomes de backup, do mais novo ao mais antigo.
pub fn listar_backups(pasta: &Path) -> Vec<Backup> {
    let Ok(it) = fs::read_dir(pasta_backups(pasta)) else {
        return Vec::new();
    };
    let mut v: Vec<Backup> = it
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let nome = e.file_name().into_string().ok()?;
            if nome.starts_with('.') {
                return None;
            }
            let (versao, _) = nome.split_once('-')?;
            Some(Backup {
                versao: versao.parse().ok()?,
                caminho: e.path(),
                nome,
            })
        })
        .collect();
    // A data (AAAAMMDD-HHMMSS[-n]) ordena como texto; a versão no prefixo não entra na chave.
    v.sort_by(|a, b| chave_de_data(&b.nome).cmp(chave_de_data(&a.nome)));
    v
}

fn chave_de_data(nome: &str) -> &str {
    nome.split_once('-').map_or(nome, |(_, resto)| resto)
}

/// Remove os backups mais antigos além de `max` (no mínimo 1 é sempre mantido).
fn limitar_backups(pasta: &Path, max: usize) -> Result<Vec<String>, String> {
    let max = max.max(1);
    let mut removidos = Vec::new();
    for b in listar_backups(pasta).into_iter().skip(max) {
        fs::remove_dir_all(&b.caminho)
            .map_err(|e| erro_io("não consegui remover o backup antigo", &b.caminho, &e))?;
        removidos.push(b.nome);
    }
    Ok(removidos)
}

// --- Restauração ---------------------------------------------------------------------------------

/// Troca o conteúdo da pasta (menos `backups/`) pelo de `origem`; `None` = deixa a pasta
/// vazia. A cópia acontece antes de apagar qualquer coisa: se ela falhar, nada mudou.
fn restaurar_de(pasta: &Path, origem: Option<&Path>) -> Result<(), String> {
    let raiz = pasta_backups(pasta);
    let staging = raiz.join(".restaurando");
    let _ = fs::remove_dir_all(&staging);
    if let Some(o) = origem {
        fs::create_dir_all(&raiz).map_err(|e| erro_io("não consegui criar", &raiz, &e))?;
        if let Err(e) = copiar_pasta(o, &staging, &[]) {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
    }
    let it = fs::read_dir(pasta).map_err(|e| erro_io("não consegui listar", pasta, &e))?;
    for e in it {
        let e = e.map_err(|e| erro_io("não consegui listar", pasta, &e))?;
        if e.file_name() == PASTA_BACKUPS || e.file_name() == PASTA_SOUNDFONTS {
            continue;
        }
        let p = e.path();
        let eh_pasta = e.file_type().is_ok_and(|t| t.is_dir());
        let r = if eh_pasta {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        };
        r.map_err(|e| erro_io("não consegui remover", &p, &e))?;
    }
    if origem.is_some() {
        let it =
            fs::read_dir(&staging).map_err(|e| erro_io("não consegui listar", &staging, &e))?;
        for e in it {
            let e = e.map_err(|e| erro_io("não consegui listar", &staging, &e))?;
            let para = pasta.join(e.file_name());
            fs::rename(e.path(), &para)
                .map_err(|e| erro_io("não consegui restaurar", &para, &e))?;
        }
        let _ = fs::remove_dir_all(&staging);
    }
    Ok(())
}

/// Rollback: restaura o backup `nome` (de [`listar_backups`]). Antes disso faz um backup do
/// estado atual (que pode ser restaurado depois, desfazendo o rollback). Devolve a versão dos
/// dados restaurados. Depois do rollback, um app mais novo migra de novo na próxima
/// inicialização; para ficar na versão antiga é preciso usar o app daquela versão.
pub fn restaurar_backup(pasta: &Path, nome: &str) -> Result<u32, String> {
    restaurar_backup_com(pasta, nome, agora_unix(), MAX_BACKUPS)
}

fn restaurar_backup_com(pasta: &Path, nome: &str, agora: u64, max: usize) -> Result<u32, String> {
    let b = listar_backups(pasta)
        .into_iter()
        .find(|b| b.nome == nome)
        .ok_or_else(|| format!("backup \"{nome}\" não encontrado"))?;
    let atual = ler_versao(pasta).unwrap_or(0);
    let seguranca = if tem_conteudo(pasta)? {
        Some(fazer_backup(pasta, atual, agora)?)
    } else {
        None
    };
    restaurar_de(pasta, Some(&b.caminho)).map_err(|e| {
        format!(
            "restauração falhou: {e}; backup do estado anterior: {}",
            seguranca
                .as_ref()
                .map_or_else(|| "nenhum".to_owned(), |s| s.display().to_string())
        )
    })?;
    // O backup restaurado e o de segurança nunca são removidos por este limite.
    let _ = limitar_backups_preservando(pasta, max, &b.nome);
    ler_versao(pasta)
}

fn limitar_backups_preservando(pasta: &Path, max: usize, manter: &str) -> Result<(), String> {
    let max = max.max(1);
    for b in listar_backups(pasta).into_iter().skip(max) {
        if b.nome != manter {
            fs::remove_dir_all(&b.caminho)
                .map_err(|e| erro_io("não consegui remover o backup antigo", &b.caminho, &e))?;
        }
    }
    Ok(())
}

// --- Data ------------------------------------------------------------------------------------------

/// `AAAAMMDD-HHMMSS` em UTC a partir de segundos Unix (sem dependência de calendário).
fn data_utc(segundos: u64) -> String {
    let dias = (segundos / 86_400) as i64;
    let resto = segundos % 86_400;
    // Dias desde 1970-01-01 -> data civil (algoritmo de Howard Hinnant).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + i64::from(mes <= 2);
    format!(
        "{ano:04}{mes:02}{dia:02}-{:02}{:02}{:02}",
        resto / 3_600,
        resto % 3_600 / 60,
        resto % 60
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Pasta temporária única, removida ao sair do teste.
    struct Temp(PathBuf);

    impl Temp {
        fn nova(rotulo: &str) -> Self {
            static SEQ: AtomicUsize = AtomicUsize::new(0);
            let n = SEQ.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!(
                "piano-tutor-dados-{}-{n}-{rotulo}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).expect("criar pasta temporária");
            Temp(p)
        }

        fn dados(&self) -> PathBuf {
            self.0.join("dados")
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Conteúdo (caminho relativo -> bytes; pastas = None), sem `backups/`.
    fn foto(pasta: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
        fn rec(base: &Path, atual: &Path, m: &mut BTreeMap<String, Option<Vec<u8>>>) {
            for e in fs::read_dir(atual).expect("listar") {
                let e = e.expect("entrada");
                let rel = e
                    .path()
                    .strip_prefix(base)
                    .expect("prefixo")
                    .to_string_lossy()
                    .into_owned();
                if rel == PASTA_BACKUPS {
                    continue;
                }
                if e.file_type().expect("tipo").is_dir() {
                    m.insert(rel, None);
                    rec(base, &e.path(), m);
                } else {
                    m.insert(rel, Some(fs::read(e.path()).expect("ler")));
                }
            }
        }
        let mut m = BTreeMap::new();
        rec(pasta, pasta, &mut m);
        m
    }

    fn dados_legados(pasta: &Path) {
        fs::create_dir_all(pasta.join("soundfonts")).expect("criar");
        fs::write(pasta.join("preferencias.txt"), "volume=0.8\ntimbre=2\n").expect("gravar");
        fs::write(pasta.join("progresso.txt"), "licao1=ok\n").expect("gravar");
        fs::write(pasta.join("soundfonts/exemplo.sf2"), [0u8, 1, 2, 3, 255]).expect("gravar");
    }

    const T0: u64 = 1_700_000_000; // 2023-11-14 22:13:20 UTC

    fn ok(_: &Path) -> Result<(), String> {
        Ok(())
    }

    fn mig(de: u32, para: u32, aplicar: fn(&Path) -> Result<(), String>) -> Migracao {
        Migracao {
            de,
            para,
            descricao: "teste",
            aplicar,
        }
    }

    #[test]
    fn data_utc_de_valores_conhecidos() {
        assert_eq!(data_utc(0), "19700101-000000");
        assert_eq!(data_utc(1_000_000_000), "20010909-014640");
        assert_eq!(data_utc(T0), "20231114-221320");
        // 29 de fevereiro de ano bissexto.
        assert_eq!(data_utc(1_709_210_096), "20240229-123456");
    }

    #[test]
    fn migracoes_conhecidas_formam_uma_cadeia_ate_a_versao_atual() {
        let mut v = 0;
        for m in MIGRACOES {
            assert_eq!(m.de, v, "{}", m.descricao);
            assert!(m.para > m.de);
            v = m.para;
        }
        assert_eq!(v, VERSAO_ATUAL);
    }

    #[test]
    fn pasta_inexistente_e_criada_na_versao_atual_sem_backup() {
        let t = Temp::nova("nova");
        let pasta = t.dados();
        let r = preparar_com(&pasta, T0, MIGRACOES, MAX_BACKUPS).expect("preparar");
        assert_eq!((r.versao_antes, r.versao_depois), (0, VERSAO_ATUAL));
        assert_eq!(r.migracoes_aplicadas.len(), 1);
        assert_eq!(r.backup, None, "não há o que guardar numa pasta vazia");
        assert_eq!(ler_versao(&pasta), Ok(VERSAO_ATUAL));
        assert!(listar_backups(&pasta).is_empty());
    }

    #[test]
    fn dados_legados_ganham_backup_e_continuam_intactos() {
        let t = Temp::nova("legado");
        let pasta = t.dados();
        dados_legados(&pasta);
        let antes = foto(&pasta);
        let r = preparar_com(&pasta, T0, MIGRACOES, MAX_BACKUPS).expect("preparar");
        assert_eq!(r.versao_depois, VERSAO_ATUAL);
        let b = r.backup.expect("backup feito");
        assert_eq!(
            b,
            pasta.join("backups").join("0-20231114-221320"),
            "backups/<versao>-<data>"
        );
        // Backup idêntico ao original (sem arquivo de versão, que não existia), menos os
        // SoundFonts, que ficam fora dos backups.
        let mut sem_sf2 = antes.clone();
        sem_sf2.retain(|k, _| !k.starts_with(PASTA_SOUNDFONTS));
        assert_eq!(foto(&b), sem_sf2);
        // Dados originais preservados; só o arquivo de versão é novo.
        let mut depois = foto(&pasta);
        assert_eq!(depois.remove(ARQUIVO_VERSAO), Some(Some(b"1\n".to_vec())));
        assert_eq!(depois, antes);
    }

    #[test]
    fn segunda_chamada_nao_migra_nem_faz_backup() {
        let t = Temp::nova("idempotente");
        let pasta = t.dados();
        dados_legados(&pasta);
        preparar_com(&pasta, T0, MIGRACOES, MAX_BACKUPS).expect("1ª");
        let r = preparar_com(&pasta, T0 + 60, MIGRACOES, MAX_BACKUPS).expect("2ª");
        assert!(r.migracoes_aplicadas.is_empty());
        assert_eq!(r.backup, None);
        assert_eq!(listar_backups(&pasta).len(), 1);
        assert!(r.resumo().contains("nada a migrar"));
    }

    #[test]
    fn backup_nao_copia_a_propria_pasta_de_backups() {
        let t = Temp::nova("sem-recursao");
        let pasta = t.dados();
        dados_legados(&pasta);
        let m01 = [mig(0, 1, ok)];
        let m012 = [mig(0, 1, ok), mig(1, 2, ok)];
        preparar_com(&pasta, T0, &m01, 5).expect("v1");
        let r = preparar_com(&pasta, T0 + 10, &m012, 5).expect("v2");
        let b2 = r.backup.expect("backup");
        assert!(!b2.join(PASTA_BACKUPS).exists(), "backups dentro de backup");
        assert_eq!(listar_backups(&pasta).len(), 2);
        // O backup da v1 tem o arquivo de versão "1".
        assert_eq!(
            fs::read_to_string(b2.join(ARQUIVO_VERSAO)).expect("versão"),
            "1\n"
        );
    }

    #[test]
    fn limite_de_backups_remove_os_mais_antigos() {
        let t = Temp::nova("limite");
        let pasta = t.dados();
        dados_legados(&pasta);
        let todas = [
            mig(0, 1, ok),
            mig(1, 2, ok),
            mig(2, 3, ok),
            mig(3, 4, ok),
            mig(4, 5, ok),
        ];
        // Cada chamada conhece uma migração a mais: uma subida de versão por chamada.
        let mut removidos = Vec::new();
        for (i, k) in (1..=5usize).enumerate() {
            let r = preparar_com(&pasta, T0 + 100 * i as u64, &todas[..k], 2).expect("preparar");
            removidos.extend(r.backups_removidos);
        }
        // Ficam os dois mais novos: os feitos antes de ir para a v5 (versão 4) e para a v4 (3).
        let versoes: Vec<u32> = listar_backups(&pasta).iter().map(|b| b.versao).collect();
        assert_eq!(versoes, [4, 3]);
        assert_eq!(removidos.len(), 3, "{removidos:?}");
        // Limite 0 nunca apaga o backup recém-criado.
        let r = preparar_com(
            &pasta,
            T0 + 900,
            &[
                mig(0, 1, ok),
                mig(1, 2, ok),
                mig(2, 3, ok),
                mig(3, 4, ok),
                mig(4, 5, ok),
                mig(5, 6, ok),
            ],
            0,
        )
        .expect("limite 0");
        assert_eq!(listar_backups(&pasta).len(), 1);
        assert!(r.backup.expect("backup").exists());
    }

    fn escreve_e_apaga(p: &Path) -> Result<(), String> {
        fs::write(p.join("preferencias.txt"), "volume=0.1\n").map_err(|e| e.to_string())?;
        fs::remove_file(p.join("progresso.txt")).map_err(|e| e.to_string())?;
        fs::write(p.join("novo.txt"), "x").map_err(|e| e.to_string())
    }

    fn falha(_: &Path) -> Result<(), String> {
        Err("disco cheio simulado".to_owned())
    }

    #[test]
    fn falha_no_meio_da_migracao_devolve_o_estado_anterior() {
        let t = Temp::nova("falha");
        let pasta = t.dados();
        dados_legados(&pasta);
        // Já na v1; a v1->v2 altera arquivos e a v2->v3 falha.
        preparar_com(&pasta, T0, &[mig(0, 1, ok)], 5).expect("v1");
        let antes = foto(&pasta);
        assert_eq!(ler_versao(&pasta), Ok(1));

        let cadeia = [mig(0, 1, ok), mig(1, 2, escreve_e_apaga), mig(2, 3, falha)];
        let e = preparar_com(&pasta, T0 + 60, &cadeia, 5).expect_err("deve falhar");
        assert!(e.contains("disco cheio simulado"), "{e}");
        assert!(e.contains("dados restaurados do backup"), "{e}");
        // Estado idêntico ao de antes, inclusive versão 1 e arquivos apagados de volta.
        assert_eq!(foto(&pasta), antes);
        assert_eq!(ler_versao(&pasta), Ok(1));
        assert!(!pasta.join("novo.txt").exists());
        // O backup continua disponível e sem restos temporários.
        let bs = listar_backups(&pasta);
        assert_eq!(bs.len(), 2, "o da v0 e o da tentativa");
        assert!(!pasta.join("backups/.restaurando").exists());
    }

    #[test]
    fn falha_na_primeira_migracao_de_dados_legados_volta_a_versao_zero() {
        let t = Temp::nova("falha0");
        let pasta = t.dados();
        dados_legados(&pasta);
        let antes = foto(&pasta);
        let e = preparar_com(
            &pasta,
            T0,
            &[mig(0, 1, escreve_e_apaga), mig(1, 2, falha)],
            5,
        )
        .expect_err("falha");
        assert!(e.contains("restaurados"), "{e}");
        assert_eq!(foto(&pasta), antes);
        assert_eq!(ler_versao(&pasta), Ok(0), "sem arquivo de versão");
        assert!(!pasta.join(ARQUIVO_VERSAO).exists());
    }

    #[test]
    fn falha_em_pasta_vazia_devolve_pasta_vazia() {
        let t = Temp::nova("falha-vazia");
        let pasta = t.dados();
        fn deixa_lixo(p: &Path) -> Result<(), String> {
            fs::write(p.join("lixo.txt"), "x").map_err(|e| e.to_string())?;
            Err("quebrou".to_owned())
        }
        let e = preparar_com(&pasta, T0, &[mig(0, 1, deixa_lixo)], 5).expect_err("falha");
        assert!(e.contains("estado vazio"), "{e}");
        assert!(foto(&pasta).is_empty());
    }

    #[test]
    fn versao_mais_nova_ou_arquivo_ilegivel_nao_alteram_nada() {
        let t = Temp::nova("versoes");
        let pasta = t.dados();
        dados_legados(&pasta);
        fs::write(pasta.join(ARQUIVO_VERSAO), "99\n").expect("gravar");
        let antes = foto(&pasta);
        let e = preparar_com(&pasta, T0, MIGRACOES, 5).expect_err("versão futura");
        assert!(e.contains("mais nova"), "{e}");
        assert_eq!(foto(&pasta), antes);
        assert!(listar_backups(&pasta).is_empty());

        fs::write(pasta.join(ARQUIVO_VERSAO), "abc").expect("gravar");
        let antes = foto(&pasta);
        let e = preparar_com(&pasta, T0, MIGRACOES, 5).expect_err("ilegível");
        assert!(e.contains("ilegível"), "{e}");
        assert_eq!(foto(&pasta), antes);
        assert!(listar_backups(&pasta).is_empty());
    }

    #[test]
    fn migracao_ausente_na_cadeia_e_recusada_antes_de_qualquer_mudanca() {
        let t = Temp::nova("buraco");
        let pasta = t.dados();
        dados_legados(&pasta);
        let antes = foto(&pasta);
        let e = preparar_com(&pasta, T0, &[mig(1, 2, ok)], 5).expect_err("sem 0->1");
        assert!(e.contains("não há migração"), "{e}");
        assert_eq!(foto(&pasta), antes);
        assert!(listar_backups(&pasta).is_empty());
    }

    #[test]
    fn rollback_restaura_um_backup_e_guarda_o_estado_atual() {
        let t = Temp::nova("rollback");
        let pasta = t.dados();
        dados_legados(&pasta);
        let original = foto(&pasta);
        preparar_com(&pasta, T0, &[mig(0, 1, escreve_e_apaga)], 5).expect("v1");
        let migrado = foto(&pasta);
        assert_ne!(migrado, original);
        let nome = listar_backups(&pasta)[0].nome.clone();
        assert_eq!(nome, "0-20231114-221320");

        let v = restaurar_backup_com(&pasta, &nome, T0 + 500, 5).expect("rollback");
        assert_eq!(v, 0, "dados voltam à versão do backup");
        assert_eq!(foto(&pasta), original);
        // O estado migrado ficou guardado e pode ser restaurado (desfaz o rollback).
        let bs = listar_backups(&pasta);
        assert_eq!(bs.len(), 2);
        let seg = bs
            .iter()
            .find(|b| b.versao == 1)
            .expect("backup de segurança");
        let v = restaurar_backup_com(&pasta, &seg.nome.clone(), T0 + 900, 5).expect("desfazer");
        assert_eq!(v, 1);
        assert_eq!(foto(&pasta), migrado);
    }

    #[test]
    fn rollback_recusa_nomes_inexistentes_e_caminhos_fora_dos_backups() {
        let t = Temp::nova("rollback-nomes");
        let pasta = t.dados();
        dados_legados(&pasta);
        preparar_com(&pasta, T0, MIGRACOES, 5).expect("v1");
        let antes = foto(&pasta);
        for nome in [
            "nao-existe",
            "../soundfonts",
            "..",
            "",
            "0-20231114-221320/..",
            "/etc",
        ] {
            assert!(
                restaurar_backup_com(&pasta, nome, T0, 5).is_err(),
                "{nome:?}"
            );
        }
        assert_eq!(foto(&pasta), antes);
        assert_eq!(
            listar_backups(&pasta).len(),
            1,
            "sem backup de segurança à toa"
        );
    }

    #[test]
    fn backup_incompleto_nao_aparece_na_lista() {
        let t = Temp::nova("incompleto");
        let pasta = t.dados();
        fs::create_dir_all(pasta.join("backups/.tmp-0-20231114-221320")).expect("criar");
        fs::create_dir_all(pasta.join("backups/0-20231114-221320")).expect("criar");
        fs::create_dir_all(pasta.join("backups/qualquer-coisa")).expect("criar");
        let nomes: Vec<String> = listar_backups(&pasta).into_iter().map(|b| b.nome).collect();
        assert_eq!(nomes, ["0-20231114-221320"]);
    }

    #[cfg(unix)]
    #[test]
    fn links_simbolicos_nao_sao_seguidos_nem_copiados() {
        let t = Temp::nova("links");
        let pasta = t.dados();
        dados_legados(&pasta);
        let fora = t.0.join("fora.txt");
        fs::write(&fora, "segredo").expect("gravar");
        std::os::unix::fs::symlink(&fora, pasta.join("atalho.txt")).expect("link");
        std::os::unix::fs::symlink(&t.0, pasta.join("laco")).expect("link para pasta");
        let r = preparar_com(&pasta, T0, MIGRACOES, 5).expect("preparar");
        let b = r.backup.expect("backup");
        assert!(!b.join("atalho.txt").exists());
        assert!(!b.join("laco").exists());
        assert!(b.join("preferencias.txt").exists());
    }

    #[test]
    fn preparar_publico_funciona_numa_pasta_temporaria() {
        let t = Temp::nova("publico");
        let pasta = t.dados();
        dados_legados(&pasta);
        let r = preparar(&pasta).expect("preparar");
        assert_eq!(r.versao_depois, VERSAO_ATUAL);
        assert!(
            r.backup
                .as_ref()
                .is_some_and(|b| b.starts_with(pasta.join(PASTA_BACKUPS)))
        );
        assert!(r.resumo().contains("migrados"));
    }

    #[test]
    fn soundfonts_ficam_fora_do_backup_e_sobrevivem_ao_rollback() {
        let d = std::env::temp_dir().join(format!("pt-sf-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join(PASTA_SOUNDFONTS)).expect("pasta");
        fs::write(d.join(PASTA_SOUNDFONTS).join("x.sf2"), b"sf2").expect("sf2");
        fs::write(d.join("preferencias.txt"), b"a").expect("prefs");
        let r = preparar(&d).expect("preparar");
        let b = r.backup.expect("houve backup");
        assert!(b.join("preferencias.txt").is_file());
        assert!(!b.join(PASTA_SOUNDFONTS).exists(), "SF2 não entra no backup");
        restaurar_de(&d, Some(&b)).expect("restaurar");
        assert!(d.join(PASTA_SOUNDFONTS).join("x.sf2").is_file(), "SF2 continua");
        let _ = fs::remove_dir_all(&d);
    }
}
