//! Praticar a Versão e ouvir o compasso editado (ticket 25, A14).
//!
//! A prática existente (`EstadoPartitura`: Nota a nota, Tempo, Espera tocar, loops, andamento)
//! continua **a mesma**; este módulo só decide **qual documento ela recebe** e com que rótulo:
//!
//! - A prática lê as notas da **Versão por hash** ([`Versoes::abrir`] confere o conteúdo contra o
//!   SHA-256 e [`EstadoPartitura::abrir_xml`] passa o texto pelo serviço de documento). Nunca lê
//!   pelo nome do arquivo e nunca pelo `estado.toml`.
//! - A aprovação vem **só** de [`Versoes::aprovacao_de`] (estado Aprovada **e** registro
//!   `[[decisao]]`; nada aprova sozinho). Versão aprovada = [`ModoDaPratica::Gabarito`].
//! - Versão **não aprovada** (Hipótese, Em revisão, ou "Aprovada" escrita à mão sem registro) só
//!   abre com `permitir_hipotese` e **sempre sinalizada**: [`ModoDaPratica::Hipotese`], com o
//!   rótulo "HIPÓTESE ... não é o Gabarito oficial" no nome do documento e em
//!   [`PraticaDaVersao::rotulo`]. Sem a permissão, é recusada ([`ErroDaPratica::NaoAprovada`]).
//!   Versão **Rejeitada** nunca abre (a mesma regra dos Exercícios de Curso).
//! - A sessão fica **ligada ao hash**. [`PraticaDaVersao::conferir`] recusa (e limpa a prática,
//!   com [`PraticaDaVersao::sincronizar`]) quando o documento aberto deixou de ser a Versão:
//!   outra Versão aberta, outro arquivo, qualquer tentativa de edição depois de abrir, a Versão
//!   adulterada, ou a aprovação mudada (reaberta ou rejeitada).
//! - [`plano_do_compasso`] e [`ouvir_compasso`]: o revisor ouve só o compasso editado, pelo
//!   serviço de som existente (`ServicoSom::demonstrar`). O teste valida o **plano**; o som real
//!   e o REVAS físico são **NÃO VERIFICADOS**.
//! - [`DecisorPorHash`]: o mesmo critério (aprovação por hash do conteúdo) para o importador de
//!   Exercícios de Curso, no lugar do `estado` de `partituras/<id>.estado.toml`.
//!
//! Nada de LLM no caminho da prática, e nenhum MIDI real é aberto aqui: a entrada de notas é a
//! de sempre (`EstadoPartitura::nota_tocada`).

use crate::revisao::{Estado, rotulo_do_estado};
use crate::som::{EstadoSom, ServicoSom};
use crate::tela_partitura::{EstadoPartitura, NotaAgendada};
use crate::versoes::ARQUIVO_DO_INDICE;
use crate::versoes::{Aprovacao, ErroVersoes, Versao, Versoes, hash_curto};
use piano_tutor_core::NotaMidi;
use piano_tutor_curso::EstadoConteudo;
use std::fmt;
use std::time::Duration;

/// Antes do primeiro som do compasso (margem para o agendamento chegar ao motor).
pub const ATRASO_DO_COMPASSO: Duration = Duration::from_millis(60);

/// Como a Versão é praticada.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModoDaPratica {
    /// Versão aprovada por um revisor (por hash, com registro): serve de Gabarito.
    Gabarito(Aprovacao),
    /// Versão **não aprovada**, praticada só como Hipótese e sempre sinalizada. Guarda o estado
    /// que a Versão tem no índice (Hipótese ou Em revisão; "Aprovada" sem registro cai aqui).
    Hipotese(Estado),
}

impl ModoDaPratica {
    /// Só a Versão aprovada é Gabarito oficial; a Hipótese **nunca** é.
    pub const fn gabarito_oficial(&self) -> bool {
        matches!(self, Self::Gabarito(_))
    }

    /// Rótulo curto, para o nome do documento e o selo da tela.
    pub fn rotulo(&self) -> String {
        match self {
            Self::Gabarito(a) => format!("Gabarito (aprovado por {})", a.revisor),
            Self::Hipotese(e) => format!(
                "HIPÓTESE ({}), não é o Gabarito oficial",
                rotulo_do_estado(*e)
            ),
        }
    }
}

/// Por que a Versão não pôde ser praticada (ou a sessão deixou de valer).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroDaPratica {
    /// Problema nas Versões (inexistente, adulterada, disco).
    Versoes(ErroVersoes),
    /// Versão não aprovada e a prática de Hipótese não foi permitida.
    NaoAprovada(Estado),
    /// Versão rejeitada pelo revisor: nunca é praticada.
    Rejeitada,
    /// O serviço de documento não abriu o conteúdo.
    Documento(String),
    /// A sessão não é mais desta Versão (o motivo vai no texto).
    Invalidada(String),
}

impl fmt::Display for ErroDaPratica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Versoes(e) => write!(f, "{e}"),
            Self::NaoAprovada(e) => write!(
                f,
                "Esta Versão está {} e não é o Gabarito oficial. Ligue «Usar conteúdo não \
                 revisado» para praticá-la como Hipótese.",
                rotulo_do_estado(*e)
            ),
            Self::Rejeitada => write!(
                f,
                "Esta Versão foi rejeitada na revisão e não pode ser praticada. Reabra-a para \
                 revisar."
            ),
            Self::Documento(e) => write!(f, "não foi possível abrir a Versão: {e}"),
            Self::Invalidada(m) => write!(f, "a prática desta Versão não vale mais: {m}"),
        }
    }
}

impl std::error::Error for ErroDaPratica {}

impl From<ErroVersoes> for ErroDaPratica {
    fn from(e: ErroVersoes) -> Self {
        Self::Versoes(e)
    }
}

/// O modo de prática da Versão `v`: aprovada por hash (com registro) é Gabarito; rejeitada é
/// recusada; o resto é Hipótese. A aprovação é perguntada **ao índice, por hash**.
fn modo_da_versao(versoes: &Versoes, v: &Versao) -> Result<ModoDaPratica, ErroDaPratica> {
    if let Some(a) = versoes.aprovacao_de(&v.hash)? {
        return Ok(ModoDaPratica::Gabarito(a));
    }
    if v.estado == Estado::Rejeitada {
        return Err(ErroDaPratica::Rejeitada);
    }
    Ok(ModoDaPratica::Hipotese(v.estado))
}

/// Uma Versão aberta para praticar: a sessão guarda o **hash** e o modo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PraticaDaVersao {
    hash: String,
    modo: ModoDaPratica,
    nome: String,
    notas: usize,
}

impl PraticaDaVersao {
    /// Abre a Versão `hash` em `p` (substitui o documento e **limpa qualquer prática anterior**).
    /// Aprovada = Gabarito; não aprovada só com `permitir_hipotese` (e sinalizada); rejeitada
    /// nunca. Em caso de erro, `p` não muda.
    pub fn abrir(
        versoes: &Versoes,
        hash: &str,
        permitir_hipotese: bool,
        p: &mut EstadoPartitura,
    ) -> Result<Self, ErroDaPratica> {
        let aberta = versoes.abrir(hash)?;
        let modo = modo_da_versao(versoes, &aberta.versao)?;
        if let ModoDaPratica::Hipotese(e) = &modo
            && !permitir_hipotese
        {
            return Err(ErroDaPratica::NaoAprovada(*e));
        }
        let nome = format!("{} · Versão {}", modo.rotulo(), hash_curto(hash));
        p.abrir_xml(&nome, aberta.conteudo)
            .map_err(ErroDaPratica::Documento)?;
        Ok(Self {
            hash: hash.to_owned(),
            modo,
            nome,
            notas: p.total_de_notas(),
        })
    }

    /// Hash (SHA-256) da Versão em prática.
    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub const fn modo(&self) -> &ModoDaPratica {
        &self.modo
    }

    /// Só a Versão aprovada é Gabarito oficial.
    pub const fn gabarito_oficial(&self) -> bool {
        self.modo.gabarito_oficial()
    }

    /// Rótulo para a tela (nunca chama a Hipótese de Gabarito).
    pub fn rotulo(&self) -> String {
        self.modo.rotulo()
    }

    /// A sessão ainda é desta Versão? Confere, nesta ordem: o documento aberto em `p` (mesmo
    /// nome, mesmas notas e nenhuma tentativa de edição depois de abrir), a Versão contra o hash
    /// (adulterada falha) e o modo (aprovação reaberta ou rejeitada muda o modo).
    pub fn conferir(&self, versoes: &Versoes, p: &EstadoPartitura) -> Result<(), ErroDaPratica> {
        if p.nome() != self.nome || p.total_de_notas() != self.notas {
            return Err(ErroDaPratica::Invalidada(format!(
                "outro documento foi aberto no lugar da Versão {}",
                hash_curto(&self.hash)
            )));
        }
        if p.ultima_edicao().is_some() {
            return Err(ErroDaPratica::Invalidada(format!(
                "a partitura foi mexida depois de abrir a Versão {}; abra a Versão de novo",
                hash_curto(&self.hash)
            )));
        }
        let aberta = versoes.abrir(&self.hash)?;
        let modo = modo_da_versao(versoes, &aberta.versao).map_err(|e| match e {
            ErroDaPratica::Rejeitada => {
                ErroDaPratica::Invalidada("a Versão foi rejeitada depois de aberta".into())
            }
            outro => outro,
        })?;
        if modo != self.modo {
            return Err(ErroDaPratica::Invalidada(format!(
                "a revisão da Versão {} mudou depois de aberta; abra a Versão de novo",
                hash_curto(&self.hash)
            )));
        }
        Ok(())
    }

    /// Como [`conferir`](Self::conferir), mas **encerra a prática em `p`** quando a sessão deixou
    /// de valer (o resumo da prática anterior some). A tela chama a cada quadro ou a cada ação.
    pub fn sincronizar(
        &self,
        versoes: &Versoes,
        p: &mut EstadoPartitura,
    ) -> Result<(), ErroDaPratica> {
        let r = self.conferir(versoes, p);
        if r.is_err() {
            p.limpar_pratica();
        }
        r
    }

    /// Começa a prática (no modo, na tolerância e no andamento já configurados em `p`) **só se a
    /// sessão ainda é desta Versão**.
    pub fn iniciar(
        &self,
        versoes: &Versoes,
        p: &mut EstadoPartitura,
        agora_ns: u64,
    ) -> Result<(), ErroDaPratica> {
        self.sincronizar(versoes, p)?;
        p.iniciar_pratica(agora_ns);
        Ok(())
    }
}

// --- Ouvir o compasso editado -----------------------------------------------------------------

fn ns(segundos: f64) -> u64 {
    if segundos.is_finite() && segundos > 0.0 {
        (segundos * 1e9).round() as u64
    } else {
        0
    }
}

/// Plano de reprodução **só do compasso** `compasso` (índice a partir de 0, a ordem escrita), no
/// andamento atual de `p`, com a primeira nota em `t0_ns`. Lê as notas do documento aberto (com
/// as edições): é o que o revisor confere de ouvido. Vazio se o compasso não tem notas. Mesma
/// regra de duração do plano da partitura (96%, mínimo 40 ms).
pub fn plano_do_compasso(p: &EstadoPartitura, compasso: usize, t0_ns: u64) -> Vec<NotaAgendada> {
    let doc: Vec<_> = p
        .esperados_para_chamada(None)
        .into_iter()
        .filter(|e| e.compasso == compasso)
        .collect();
    let base = doc.iter().map(|e| e.inicio_s).fold(f64::INFINITY, f64::min);
    let mut v: Vec<NotaAgendada> = doc
        .iter()
        .map(|e| NotaAgendada {
            t_ns: t0_ns.saturating_add(ns(e.inicio_s - base)),
            dur_ns: ns((e.duracao_s * 0.96).max(0.04)),
            midi: e.midi,
        })
        .collect();
    v.sort_by_key(|n| (n.t_ns, n.midi));
    v
}

/// Toca o compasso `compasso` pelo serviço de som existente. Devolve quantas notas agendou.
/// Recusa (com mensagem) durante a reprodução ou a prática, sem som pronto, ou se o compasso não
/// tem notas. Som real: **NÃO VERIFICADO** (os testes usam o motor falso).
pub fn ouvir_compasso(
    p: &EstadoPartitura,
    som: &mut ServicoSom,
    compasso: usize,
) -> Result<usize, String> {
    if p.tocando() || p.praticando() {
        return Err("Pare a reprodução ou a prática antes de ouvir o compasso.".into());
    }
    if som.estado() != EstadoSom::Pronto {
        return Err("O som ainda não está pronto (aba Timbres).".into());
    }
    let t0 = som
        .agora_ns()
        .saturating_add(ns(ATRASO_DO_COMPASSO.as_secs_f64()));
    let plano = plano_do_compasso(p, compasso, t0);
    if plano.is_empty() {
        return Err(format!("O compasso {} não tem notas.", compasso + 1));
    }
    for n in &plano {
        if let Some(nota) = NotaMidi::nova(n.midi) {
            som.demonstrar(&[nota], n.t_ns, Duration::from_nanos(n.dur_ns));
        }
    }
    Ok(plano.len())
}

// --- Exercícios: o estado vem do hash ---------------------------------------------------------

/// Decisor de estado do importador de Exercícios: o estado do Exercício é o da **Versão cujo
/// hash é o do conteúdo** de `partituras/<id>.musicxml`, nunca o do `estado.toml`. A regra mora
/// em `piano-tutor-curso` (a CLI `importar` usa a mesma, sem duplicar): Aprovada só com o
/// registro de decisão do revisor; conteúdo sem Versão registrada fica sem decisão.
pub use piano_tutor_curso::DecisorPorHash;

/// O estado do conteúdo `bytes` segundo as Versões, por hash. `None` = nenhuma Versão tem esse
/// hash. Estado "Aprovada" sem registro de decisão **não** vale: conta como Hipótese. Mesma
/// regra do importador (`piano_tutor_curso::estado_do_conteudo_no_indice`).
pub fn estado_do_conteudo(
    versoes: &Versoes,
    bytes: &[u8],
) -> Result<Option<EstadoConteudo>, ErroVersoes> {
    let indice = versoes.pasta().join(ARQUIVO_DO_INDICE);
    piano_tutor_curso::estado_do_conteudo_no_indice(&indice, bytes)
        .map_err(|e| ErroVersoes::Formato(e.to_string()))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::som::apoio_testes::servico_pronto;
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_audio::{Relogio, RelogioManual};
    use piano_tutor_curso::DecideEstado;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    const ODE: &str =
        include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml");
    const EM: &str = "2026-11-28T10:00:00Z";
    const PRIMEIRA: &str = "<pitch><step>E</step><octave>4</octave></pitch>";
    const CORRIGIDA: &str = "<pitch><step>F</step><octave>4</octave></pitch>";

    struct Cenario {
        _d: tempfile::TempDir,
        oficina: PathBuf,
        v: Versoes,
    }

    /// Oficina com a Hipótese (a Ode como veio do OMR) e Versões ao lado.
    fn cenario() -> Cenario {
        let d = tempfile::tempdir().expect("pasta");
        let oficina = d.path().join("oficina");
        let partituras = oficina.join("partituras");
        fs::create_dir_all(&partituras).expect("cria");
        fs::write(partituras.join("ode.musicxml"), ODE).expect("hipotese");
        let v = Versoes::em(&partituras, "ode", None);
        Cenario { _d: d, oficina, v }
    }

    /// A correção do revisor: a primeira nota (Mi4) vira Fá4 (65).
    fn corrigida() -> String {
        assert!(
            ODE.contains(PRIMEIRA),
            "a fixture tem a primeira nota esperada"
        );
        ODE.replacen(PRIMEIRA, CORRIGIDA, 1)
    }

    fn salvar(c: &Cenario, xml: &str, pai: Option<&str>) -> Versao {
        match c
            .v
            .salvar(pai, xml, Some("corrigi a primeira nota"), "Chico", EM)
        {
            Ok(crate::versoes::ResultadoDoSalvar::Criada(v)) => v,
            outro => panic!("esperava Versão criada, veio {outro:?}"),
        }
    }

    fn aprovada(c: &Cenario, xml: &str) -> Versao {
        let v = salvar(c, xml, None);
        c.v.aprovar(&v.hash, Some("conferi com o PDF"), "Chico", EM)
            .expect("aprova");
        v
    }

    fn nota(n: u8) -> NotaMidi {
        NotaMidi::nova(n).expect("nota válida")
    }

    fn partitura() -> EstadoPartitura {
        EstadoPartitura::novo(None)
    }

    /// Eventos esperados no instante 0 (o primeiro passo).
    fn alturas_do_primeiro_passo(p: &EstadoPartitura) -> Vec<u8> {
        let mut v: Vec<u8> = p
            .esperados_para_chamada(None)
            .iter()
            .filter(|e| e.inicio_s == 0.0)
            .map(|e| e.midi)
            .collect();
        v.sort_unstable();
        v
    }

    /// A14: com uma Versão Aprovada, a prática existente funciona com as notas corrigidas
    /// (simulação de MIDI, sem teclado físico: o REVAS é NÃO VERIFICADO).
    #[test]
    fn a14_versao_aprovada_pratica_com_as_notas_corrigidas() {
        let c = cenario();
        let v = aprovada(&c, &corrigida());
        let mut p = partitura();
        let s = PraticaDaVersao::abrir(&c.v, &v.hash, false, &mut p).expect("abre aprovada");
        assert!(s.gabarito_oficial());
        assert!(s.rotulo().starts_with("Gabarito"), "{}", s.rotulo());
        // O primeiro passo é o corrigido (Fá4 = 65 e o baixo Dó3 = 48), não o da Hipótese (64).
        assert_eq!(alturas_do_primeiro_passo(&p), vec![48, 65]);
        assert_eq!(p.total_de_notas(), 94);

        // Mesma simulação das fixtures de acertos e erros: uma nota fora, depois tudo certo.
        p.modo_pratica = piano_tutor_core::pratica::ModoPratica::NotaANota;
        s.iniciar(&c.v, &mut p, 1_000).expect("inicia");
        assert!(p.praticando());
        p.nota_tocada(true, nota(61), 2_000);
        let mut esperadas = p.esperados_para_chamada(None);
        esperadas.sort_by(|a, b| a.inicio_s.total_cmp(&b.inicio_s));
        for e in &esperadas {
            p.nota_tocada(true, nota(e.midi), 10_000);
        }
        p.atualizar_pratica(20_000);
        assert!(!p.praticando(), "terminou");
        let r = p.resumo_pratica().expect("resumo");
        assert_eq!((r.esperadas, r.acertos, r.faltas, r.extras), (94, 94, 0, 1));

        // Tocar a nota da Hipótese (Mi4, 64) no lugar da corrigida não acerta o passo.
        let mut q = partitura();
        let s = PraticaDaVersao::abrir(&c.v, &v.hash, false, &mut q).expect("abre");
        s.iniciar(&c.v, &mut q, 1_000).expect("inicia");
        q.nota_tocada(true, nota(64), 2_000);
        q.nota_tocada(true, nota(48), 2_000);
        q.atualizar_pratica(3_000);
        assert!(q.praticando(), "o passo corrigido ainda espera o Fá4");
        let r = q.resumo_pratica().expect("resumo");
        assert_eq!((r.acertos, r.extras), (1, 1));
    }

    /// A prática existente (Hipótese embutida, sem Versão) segue igual: o módulo não a toca.
    #[test]
    fn a_pratica_sem_versao_continua_a_mesma() {
        let mut p = partitura();
        p.escolher(3);
        p.iniciar_pratica(1_000);
        assert_eq!(alturas_do_primeiro_passo(&p), vec![48, 64]);
        assert!(p.praticando());
        assert!(!p.nome().contains("Gabarito"));
    }

    #[test]
    fn hipotese_e_em_revisao_so_abrem_sinalizadas_e_nunca_como_gabarito() {
        let c = cenario();
        c.v.registrar_origem("Chico", EM).expect("origem");
        let hipotese = c.v.ponta().expect("ponta").expect("tem").hash;
        let em_revisao = salvar(&c, &corrigida(), None);
        assert_eq!(em_revisao.estado, Estado::EmRevisao);

        for (hash, estado) in [
            (hipotese.clone(), Estado::Hipotese),
            (em_revisao.hash.clone(), Estado::EmRevisao),
        ] {
            // Bloqueio: sem a permissão, não abre; o documento não muda.
            let mut p = partitura();
            let antes = p.nome().to_owned();
            let erro = PraticaDaVersao::abrir(&c.v, &hash, false, &mut p).expect_err("bloqueia");
            assert_eq!(erro, ErroDaPratica::NaoAprovada(estado));
            assert!(
                erro.to_string().contains("não é o Gabarito oficial"),
                "{erro}"
            );
            assert_eq!(p.nome(), antes);

            // Com a permissão: abre sinalizada como Hipótese.
            let s = PraticaDaVersao::abrir(&c.v, &hash, true, &mut p).expect("abre sinalizada");
            assert!(!s.gabarito_oficial());
            assert_eq!(s.modo(), &ModoDaPratica::Hipotese(estado));
            let rotulo = s.rotulo();
            assert!(rotulo.starts_with("HIPÓTESE"), "{rotulo}");
            assert!(rotulo.contains("não é o Gabarito oficial"), "{rotulo}");
            assert!(
                p.nome().starts_with("HIPÓTESE"),
                "o nome mostra o selo: {}",
                p.nome()
            );
            assert!(!p.nome().contains("Gabarito (aprovado"));
        }
    }

    #[test]
    fn aprovada_so_com_registro_estado_a_mao_no_indice_nao_vira_gabarito() {
        let c = cenario();
        let v = salvar(&c, &corrigida(), None);
        // Escreve "aprovada" à mão, sem [[decisao]]: não vale.
        let indice = c.v.pasta().join(crate::versoes::ARQUIVO_DO_INDICE);
        let texto = fs::read_to_string(&indice).expect("lê");
        let adulterado = texto.replacen("estado = \"em_revisao\"", "estado = \"aprovada\"", 1);
        assert_ne!(texto, adulterado);
        fs::write(&indice, adulterado).expect("grava");
        let mut p = partitura();
        let erro = PraticaDaVersao::abrir(&c.v, &v.hash, false, &mut p).expect_err("bloqueia");
        assert!(matches!(erro, ErroDaPratica::NaoAprovada(_)), "{erro:?}");
        let s = PraticaDaVersao::abrir(&c.v, &v.hash, true, &mut p).expect("sinalizada");
        assert!(!s.gabarito_oficial());
    }

    #[test]
    fn rejeitada_nunca_abre_nem_com_a_permissao() {
        let c = cenario();
        let v = salvar(&c, &corrigida(), None);
        c.v.rejeitar(&v.hash, Some("nota errada"), "Chico", EM)
            .expect("rejeita");
        let mut p = partitura();
        for permitir in [false, true] {
            let erro = PraticaDaVersao::abrir(&c.v, &v.hash, permitir, &mut p).expect_err("recusa");
            assert_eq!(erro, ErroDaPratica::Rejeitada);
        }
    }

    #[test]
    fn hash_inexistente_ou_malformado_e_recusado_sem_mexer_na_partitura() {
        let c = cenario();
        let mut p = partitura();
        let antes = p.nome().to_owned();
        for h in ["../../etc/passwd", &"a".repeat(64)] {
            let erro = PraticaDaVersao::abrir(&c.v, h, true, &mut p).expect_err("recusa");
            assert!(matches!(erro, ErroDaPratica::Versoes(_)), "{erro:?}");
        }
        assert_eq!(p.nome(), antes);
    }

    #[test]
    fn versao_adulterada_no_disco_e_recusada() {
        let c = cenario();
        let v = aprovada(&c, &corrigida());
        let objeto =
            c.v.pasta()
                .join(crate::versoes::PASTA_DOS_OBJETOS)
                .join(format!("{}.musicxml", v.hash));
        fs::write(&objeto, ODE).expect("adultera");
        let mut p = partitura();
        let erro = PraticaDaVersao::abrir(&c.v, &v.hash, true, &mut p).expect_err("recusa");
        assert_eq!(
            erro,
            ErroDaPratica::Versoes(ErroVersoes::Corrompida(v.hash))
        );
    }

    /// Mudar a Versão invalida a sessão de prática anterior.
    #[test]
    fn abrir_outra_versao_invalida_a_sessao_anterior() {
        let c = cenario();
        let v1 = aprovada(&c, &corrigida());
        let outra = ODE.replacen(
            PRIMEIRA,
            "<pitch><step>G</step><octave>4</octave></pitch>",
            1,
        );
        let v2 = salvar(&c, &outra, Some(&v1.hash));
        c.v.aprovar(&v2.hash, Some("outra correção"), "Chico", EM)
            .expect("aprova");

        let mut p = partitura();
        let s1 = PraticaDaVersao::abrir(&c.v, &v1.hash, false, &mut p).expect("abre 1");
        s1.iniciar(&c.v, &mut p, 1_000).expect("inicia");
        p.nota_tocada(true, nota(48), 2_000);
        assert!(p.praticando());
        assert!(s1.conferir(&c.v, &p).is_ok());

        // Abre a Versão 2: a prática da 1 some e a sessão antiga não vale mais.
        let s2 = PraticaDaVersao::abrir(&c.v, &v2.hash, false, &mut p).expect("abre 2");
        assert!(!p.praticando());
        assert!(
            p.resumo_pratica().is_none(),
            "o resumo anterior não sobrevive"
        );
        assert_eq!(alturas_do_primeiro_passo(&p), vec![48, 67]);
        assert!(matches!(
            s1.conferir(&c.v, &p),
            Err(ErroDaPratica::Invalidada(_))
        ));
        assert!(matches!(
            s1.iniciar(&c.v, &mut p, 3_000),
            Err(ErroDaPratica::Invalidada(_))
        ));
        assert!(!p.praticando(), "a sessão antiga não inicia nada");
        assert!(s2.conferir(&c.v, &p).is_ok());
        assert_ne!(s1.hash(), s2.hash());
    }

    #[test]
    fn reabrir_ou_rejeitar_a_versao_invalida_a_sessao_e_limpa_a_pratica() {
        let c = cenario();
        let v = aprovada(&c, &corrigida());
        let mut p = partitura();
        let s = PraticaDaVersao::abrir(&c.v, &v.hash, false, &mut p).expect("abre");
        s.iniciar(&c.v, &mut p, 1_000).expect("inicia");
        assert!(p.praticando());
        // O revisor reabre a Versão no meio da prática: o Gabarito deixou de valer.
        c.v.reabrir(&v.hash, Some("duvidei"), "Chico", EM)
            .expect("reabre");
        let erro = s.sincronizar(&c.v, &mut p).expect_err("invalida");
        assert!(matches!(erro, ErroDaPratica::Invalidada(_)), "{erro:?}");
        assert!(!p.praticando());
        assert!(p.resumo_pratica().is_none());
        // E agora ela só abre como Hipótese sinalizada.
        assert!(PraticaDaVersao::abrir(&c.v, &v.hash, false, &mut p).is_err());
        let h = PraticaDaVersao::abrir(&c.v, &v.hash, true, &mut p).expect("hipótese");
        assert_eq!(h.modo(), &ModoDaPratica::Hipotese(Estado::EmRevisao));

        c.v.rejeitar(&v.hash, Some("errada"), "Chico", EM)
            .expect("rejeita");
        assert!(matches!(
            h.sincronizar(&c.v, &mut p),
            Err(ErroDaPratica::Invalidada(_))
        ));
    }

    #[test]
    fn editar_a_partitura_depois_de_abrir_invalida_a_sessao() {
        use crate::selecao_partitura::Comando;
        use piano_tutor_core::documento::{SentidoDaAltura, UnidadeDeAltura};
        let c = cenario();
        let v = aprovada(&c, &corrigida());
        let mut p = partitura();
        let s = PraticaDaVersao::abrir(&c.v, &v.hash, false, &mut p).expect("abre");
        assert!(s.conferir(&c.v, &p).is_ok());
        p.navegar(Comando::Seguinte);
        p.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima);
        assert!(matches!(
            s.iniciar(&c.v, &mut p, 1_000),
            Err(ErroDaPratica::Invalidada(_))
        ));
        assert!(!p.praticando());
    }

    // --- Ouvir o compasso editado ---

    #[test]
    fn plano_do_compasso_so_tem_as_notas_do_compasso_editado() {
        let c = cenario();
        let v = aprovada(&c, &corrigida());
        let mut p = partitura();
        PraticaDaVersao::abrir(&c.v, &v.hash, false, &mut p).expect("abre");
        p.bpm = 120.0;
        let t0 = 5_000_000_000;
        let plano = plano_do_compasso(&p, 0, t0);
        assert!(!plano.is_empty());
        // As notas do compasso 1 (índice 0), com a correção: Fá4 e o baixo Dó3 no tempo 0.
        let esperadas: Vec<_> = p
            .esperados_para_chamada(None)
            .into_iter()
            .filter(|e| e.compasso == 0)
            .collect();
        assert_eq!(plano.len(), esperadas.len());
        assert_eq!(plano[0].t_ns, t0, "a primeira nota toca em t0");
        assert!(plano.iter().filter(|n| n.t_ns == t0).any(|n| n.midi == 65));
        assert!(
            plano.iter().all(|n| n.midi != 64 || n.t_ns > t0),
            "sem o Mi4 antigo no tempo 0"
        );
        // Compasso em 4/4 a 120 bpm = 2 s: nenhuma nota passa da janela do compasso.
        assert!(plano.iter().all(|n| n.t_ns < t0 + 2_000_000_000));
        assert!(plano.windows(2).all(|w| w[0].t_ns <= w[1].t_ns));
        // Duração: 96% da nota (semínima a 120 bpm = 0,5 s), mínimo de 40 ms.
        assert!(plano.iter().all(|n| n.dur_ns >= 40_000_000));
        // Outro compasso começa também em t0, com outras notas.
        let seg = plano_do_compasso(&p, 1, t0);
        assert_eq!(seg[0].t_ns, t0);
        assert_ne!(
            plano.iter().map(|n| n.midi).collect::<Vec<_>>(),
            seg.iter().map(|n| n.midi).collect::<Vec<_>>()
        );
        // Compasso inexistente: plano vazio.
        assert!(plano_do_compasso(&p, 999, t0).is_empty());
    }

    #[test]
    fn ouvir_o_compasso_toca_pelo_servico_de_som_existente() {
        let c = cenario();
        let v = aprovada(&c, &corrigida());
        let mut p = partitura();
        PraticaDaVersao::abrir(&c.v, &v.hash, false, &mut p).expect("abre");
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        motor.renderizar(512, 0);
        let n = ouvir_compasso(&p, &mut som, 0).expect("toca");
        assert!(n > 0);
        motor.renderizar_no_relogio(48_000 * 6);
        let ligadas: Vec<u8> = motor
            .registro()
            .iter()
            .filter_map(|r| match *r {
                RegistroFalso::Ligada { nota, .. } => Some(nota),
                _ => None,
            })
            .collect();
        assert_eq!(ligadas.len(), n);
        assert!(ligadas.contains(&65), "toca a nota corrigida: {ligadas:?}");
        assert!(!ligadas.contains(&64) || ligadas.iter().filter(|m| **m == 64).count() < 2);
        // Compasso sem notas e prática em andamento são recusados com mensagem.
        assert!(ouvir_compasso(&p, &mut som, 999).is_err());
        p.iniciar_pratica(som.agora_ns());
        let msg = ouvir_compasso(&p, &mut som, 0).expect_err("recusa");
        assert!(msg.contains("Pare"), "{msg}");
    }

    #[test]
    fn ouvir_sem_som_pronto_avisa_em_vez_de_calar() {
        let p = partitura();
        let mut som = ServicoSom::inativo(Arc::new(RelogioManual::novo()) as Arc<dyn Relogio>);
        let msg = ouvir_compasso(&p, &mut som, 0).expect_err("sem som");
        assert!(msg.contains("som"), "{msg}");
    }

    // --- Exercício: o estado vem do hash ---

    #[test]
    fn exercicio_consulta_a_aprovacao_por_hash_e_nao_pelo_estado_toml() {
        let c = cenario();
        let v = aprovada(&c, &corrigida());
        let partituras = c.oficina.join("partituras");
        // O estado.toml mente: diz "aprovada" para o que não está aprovada por hash.
        fs::write(
            partituras.join("ode.estado.toml"),
            "id = \"ode\"\nestado = \"aprovada\"\n",
        )
        .expect("estado.toml");
        let decisor = DecisorPorHash;
        let conteudo_aprovado = corrigida();
        let estado = |bytes: &[u8]| {
            decisor
                .estado_do_exercicio(&c.oficina, "ode", Some(bytes))
                .expect("consulta")
        };
        // Mesmo conteúdo da Versão aprovada: aprovada (o nome do arquivo não importa).
        assert_eq!(
            estado(conteudo_aprovado.as_bytes()),
            Some(EstadoConteudo::Aprovada)
        );
        // Alterar o conteúdo muda o hash e a aprovação não vale (sem Versão: sem decisão).
        let alterado = format!("{conteudo_aprovado}<!-- mexi -->");
        assert_eq!(estado(alterado.as_bytes()), None);
        // O estado.toml "aprovada" não aprova a Hipótese original (que tem outro hash).
        c.v.registrar_origem("Chico", EM).expect("origem");
        assert_eq!(estado(ODE.as_bytes()), Some(EstadoConteudo::Hipotese));
        // Reabrir a Versão tira a aprovação; rejeitar vira Rejeitada.
        c.v.reabrir(&v.hash, Some("revisar"), "Chico", EM)
            .expect("reabre");
        assert_eq!(
            estado(conteudo_aprovado.as_bytes()),
            Some(EstadoConteudo::EmRevisao)
        );
        c.v.rejeitar(&v.hash, Some("errada"), "Chico", EM)
            .expect("rejeita");
        assert_eq!(
            estado(conteudo_aprovado.as_bytes()),
            Some(EstadoConteudo::Rejeitada)
        );
        // Arquivo ausente: sem decisão.
        assert_eq!(
            decisor
                .estado_do_exercicio(&c.oficina, "ode", None)
                .expect("consulta"),
            None
        );
    }

    #[test]
    fn estado_aprovada_escrito_a_mao_no_indice_nao_aprova_o_exercicio() {
        let c = cenario();
        let v = salvar(&c, &corrigida(), None);
        let indice = c.v.pasta().join(crate::versoes::ARQUIVO_DO_INDICE);
        let texto = fs::read_to_string(&indice).expect("lê");
        fs::write(
            &indice,
            texto.replacen("estado = \"em_revisao\"", "estado = \"aprovada\"", 1),
        )
        .expect("grava");
        let e = estado_do_conteudo(&c.v, corrigida().as_bytes()).expect("consulta");
        assert_eq!(e, Some(EstadoConteudo::Hipotese));
        let _ = v;
    }

    /// Importação de um Curso a partir da oficina: só entra como Aprovada o Exercício cujo
    /// conteúdo (hash) tem aprovação registrada; o `estado.toml` "aprovada" não conta.
    #[test]
    fn importar_curso_com_o_decisor_por_hash() {
        let c = cenario();
        let partituras = c.oficina.join("partituras");
        // A oficina guarda a Versão corrigida e aprovada como o arquivo do Exercício.
        let xml = corrigida();
        let v = salvar(&c, &xml, None);
        fs::write(partituras.join("ode.musicxml"), &xml).expect("grava");
        fs::write(
            partituras.join("ode.estado.toml"),
            "id = \"ode\"\nestado = \"aprovada\"\nbpm_sugerido = 90.0\n",
        )
        .expect("estado.toml");
        let plano = c._d.path().join("plano.toml");
        fs::write(
            &plano,
            r#"formato_versao = 1
id = "teste.ode"
titulo = "Ode"
versao = "0.1.0"
idioma = "pt-BR"
convencao_oitava = "do3-central"
[autor]
nome = "Autor"
[licenciado]
nome = "Aluno"
licenca = "teste"
[[secoes]]
id = "s01"
numero = 1
titulo = "Seção"
[[secoes.licoes]]
id = "s01-a01"
numero = 1
titulo = "Ode"
tipo = "exercicio"
[[secoes.licoes.exercicios]]
id = "ode"
titulo = "Ode"
modo = "praticar"
mao = "direita"
"#,
        )
        .expect("plano");
        let importa = |destino: &str| {
            piano_tutor_curso::importar_com(
                &plano,
                &c.oficina,
                &c._d.path().join(destino),
                false,
                &DecisorPorHash,
            )
            .expect("importa")
        };
        // Em revisão (sem decisão): o `estado.toml` "aprovada" é ignorado.
        let r = importa("o1");
        assert_eq!(r.contagem.por_estado(EstadoConteudo::EmRevisao), 1);
        assert_eq!(r.contagem.por_estado(EstadoConteudo::Aprovada), 0);
        // Depois da aprovação registrada, o mesmo conteúdo entra Aprovado.
        c.v.aprovar(&v.hash, Some("conferi"), "Chico", EM)
            .expect("aprova");
        let r = importa("o2");
        assert_eq!(r.contagem.por_estado(EstadoConteudo::Aprovada), 1);
        // Mexer no arquivo muda o hash: a aprovação não vale mais (sem Versão com esse hash).
        fs::write(partituras.join("ode.musicxml"), format!("{xml}<!-- x -->")).expect("mexe");
        let r = importa("o3");
        assert_eq!(r.contagem.por_estado(EstadoConteudo::Aprovada), 0);
        assert!(r.avisos.iter().any(|a| a.contains("sem Versão registrada")));
    }
}
