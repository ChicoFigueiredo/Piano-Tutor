//! Chaves ed25519: geração, hexadecimal, arquivos e lista de chaves confiáveis.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::erro::ErroPacote;

/// Chaves públicas embutidas no app como confiáveis (hexadecimal). Vazia por enquanto:
/// a confiança vem de `<dados>/cursos/chaves-confiaveis/*.pub`.
pub const CHAVES_EMBUTIDAS: &[&str] = &[];

/// Chave pública ed25519 (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChavePublica([u8; 32]);

/// Chave privada ed25519 (semente de 32 bytes). Nunca aparece em `Debug`.
pub struct ChavePrivada(SigningKey);

impl fmt::Debug for ChavePrivada {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChavePrivada(<segredo omitido>)")
    }
}

impl fmt::Debug for ChavePublica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChavePublica({})", self.para_hex())
    }
}

impl fmt::Display for ChavePublica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.para_hex())
    }
}

/// Gera um par de chaves novo.
pub fn gerar_par() -> (ChavePrivada, ChavePublica) {
    let privada = ChavePrivada::gerar();
    let publica = privada.publica();
    (privada, publica)
}

impl ChavePrivada {
    /// Gera uma chave nova com bytes do sistema operacional.
    ///
    /// # Panics
    /// Se o sistema não conseguir fornecer bytes aleatórios (situação irrecuperável).
    pub fn gerar() -> Self {
        let mut semente = [0u8; 32];
        getrandom::fill(&mut semente).expect("o sistema não forneceu bytes aleatórios");
        Self(SigningKey::from_bytes(&semente))
    }

    pub fn publica(&self) -> ChavePublica {
        ChavePublica(self.0.verifying_key().to_bytes())
    }

    /// Assina `mensagem` (ed25519, 64 bytes).
    pub fn assinar(&self, mensagem: &[u8]) -> [u8; 64] {
        self.0.sign(mensagem).to_bytes()
    }

    pub fn para_hex(&self) -> String {
        para_hex(&self.0.to_bytes())
    }

    pub fn de_hex(texto: &str) -> Result<Self, ErroPacote> {
        let bytes = de_hex_32(texto, "chave privada")?;
        Ok(Self(SigningKey::from_bytes(&bytes)))
    }

    /// Grava a chave em `caminho` com permissão 0600 no Unix. Não sobrescreve arquivo existente.
    pub fn gravar(&self, caminho: &Path) -> Result<(), ErroPacote> {
        let mut opcoes = fs::OpenOptions::new();
        opcoes.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opcoes.mode(0o600);
        }
        let mut arquivo = opcoes
            .open(caminho)
            .map_err(|e| ErroPacote::Io(format!("{}: {e}", caminho.display())))?;
        arquivo.write_all(format!("{}\n", self.para_hex()).as_bytes())?;
        Ok(())
    }

    pub fn ler(caminho: &Path) -> Result<Self, ErroPacote> {
        let texto = fs::read_to_string(caminho)
            .map_err(|e| ErroPacote::Io(format!("{}: {e}", caminho.display())))?;
        Self::de_hex(texto.trim())
    }
}

impl ChavePublica {
    pub fn para_hex(&self) -> String {
        para_hex(&self.0)
    }

    pub fn de_hex(texto: &str) -> Result<Self, ErroPacote> {
        let bytes = de_hex_32(texto, "chave pública")?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| {
            ErroPacote::ChaveInvalida("a chave pública não é um ponto válido".into())
        })?;
        Ok(Self(bytes))
    }

    /// Forma curta para mostrar ao usuário: `abcd…1234`.
    pub fn curta(&self) -> String {
        let hex = self.para_hex();
        format!("{}…{}", &hex[..4], &hex[hex.len() - 4..])
    }

    /// Confere a assinatura ed25519 (modo estrito) de `mensagem`.
    pub fn verificar(&self, mensagem: &[u8], assinatura: &[u8; 64]) -> bool {
        let Ok(chave) = VerifyingKey::from_bytes(&self.0) else {
            return false;
        };
        chave
            .verify_strict(mensagem, &Signature::from_bytes(assinatura))
            .is_ok()
    }

    /// Grava a chave em hexadecimal. Não sobrescreve arquivo existente.
    pub fn gravar(&self, caminho: &Path) -> Result<(), ErroPacote> {
        let mut arquivo = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(caminho)
            .map_err(|e| ErroPacote::Io(format!("{}: {e}", caminho.display())))?;
        arquivo.write_all(format!("{}\n", self.para_hex()).as_bytes())?;
        Ok(())
    }

    pub fn ler(caminho: &Path) -> Result<Self, ErroPacote> {
        let texto = fs::read_to_string(caminho)
            .map_err(|e| ErroPacote::Io(format!("{}: {e}", caminho.display())))?;
        Self::de_hex(texto.trim())
    }
}

/// Lê todas as chaves públicas `*.pub` de uma pasta (ordem por nome); arquivos ilegíveis são ignorados.
pub fn ler_chaves_da_pasta(pasta: &Path) -> Vec<ChavePublica> {
    let Ok(entradas) = fs::read_dir(pasta) else {
        return Vec::new();
    };
    let mut arquivos: Vec<_> = entradas
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pub") && p.is_file())
        .collect();
    arquivos.sort();
    arquivos
        .iter()
        .filter_map(|p| ChavePublica::ler(p).ok())
        .collect()
}

/// Chaves confiáveis: as embutidas no app mais as de `<dados>/cursos/chaves-confiaveis/*.pub`.
pub fn chaves_confiaveis(pasta_dados: &Path) -> Vec<ChavePublica> {
    let mut chaves: Vec<ChavePublica> = CHAVES_EMBUTIDAS
        .iter()
        .filter_map(|h| ChavePublica::de_hex(h).ok())
        .collect();
    chaves.extend(ler_chaves_da_pasta(
        &pasta_dados.join("cursos").join("chaves-confiaveis"),
    ));
    chaves
}

/// Hexadecimal minúsculo.
pub fn para_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut saida = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(saida, "{b:02x}");
    }
    saida
}

fn de_hex_32(texto: &str, o_que: &str) -> Result<[u8; 32], ErroPacote> {
    let texto = texto.trim();
    if texto.len() != 64 || !texto.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ErroPacote::ChaveInvalida(format!(
            "{o_que}: esperava 64 dígitos hexadecimais"
        )));
    }
    let mut saida = [0u8; 32];
    for (i, par) in texto.as_bytes().chunks(2).enumerate() {
        let alto = (par[0] as char).to_digit(16).unwrap_or(0);
        let baixo = (par[1] as char).to_digit(16).unwrap_or(0);
        saida[i] = ((alto << 4) | baixo) as u8;
    }
    Ok(saida)
}

/// `true` se `texto` é SHA-256 em hexadecimal minúsculo (64 dígitos).
pub fn e_sha256_hex(texto: &str) -> bool {
    texto.len() == 64
        && texto
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
