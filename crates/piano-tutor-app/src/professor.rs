//! O professor que fala durante um Exercício (M4, etapa 6): escolhe a fala certa do Roteiro para
//! cada momento (`antes`, `passo`, `acerto`, `erro`, `depois`), diz pela [`Voz`] e guarda o texto
//! dito para a tela mostrar (acessibilidade: quem não ouve, lê).
//!
//! - só falas **aprovadas** são ditas ([`falas_para_dizer`]); hipótese nunca vira voz;
//! - havendo várias falas para o mesmo momento, sorteia uma que **não seja a mesma** dita da
//!   última vez ([`escolher_sem_repetir`]);
//! - o interruptor "Professor fala" é uma preferência gravada (`preferencias-professor.txt`);
//!   desligado, o professor não diz nem mostra nada do Roteiro;
//! - sem voz instalada ([`VozMuda`](crate::professor_falante::VozMuda)) o texto aparece do mesmo
//!   jeito, cada fala por um tempo proporcional ao tamanho, e [`Professor::aviso_de_voz`] traz
//!   o COMO_INSTALAR_A_VOZ.
//!
//! O professor não toca nada nem sabe de partitura: quem observa o Exercício chama
//! [`Professor::dizer`] com o momento.

use crate::professor_falante::{
    ErroVoz, EstadoVoz, LICAO_DE_FABRICA, Momento, Roteiro, Voz, falas_para_dizer,
    id_de_exercicio_de_fabrica, roteiro_de_fabrica,
};
use piano_tutor_core::chamada_resposta::estimar_duracao_da_fala_s;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Preferências do professor, na pasta de dados (arquivo próprio, como as da partitura).
pub const ARQUIVO_PREFERENCIAS_PROFESSOR: &str = "preferencias-professor.txt";

/// Escolhe uma fala entre `candidatas`, evitando repetir `ultima` quando há outra opção.
/// `sorteio` é qualquer número (o chamador sorteia); vazio devolve `None`.
pub fn escolher_sem_repetir<'a>(
    candidatas: &[&'a str],
    ultima: Option<&str>,
    sorteio: u64,
) -> Option<&'a str> {
    let diferentes: Vec<&'a str> = candidatas
        .iter()
        .copied()
        .filter(|c| Some(*c) != ultima)
        .collect();
    let pool = if diferentes.is_empty() {
        candidatas
    } else {
        &diferentes
    };
    if pool.is_empty() {
        return None;
    }
    pool.get((sorteio % pool.len() as u64) as usize).copied()
}

#[derive(Clone, Debug)]
struct Contexto {
    licao: String,
    exercicio: String,
    roteiro: Option<Roteiro>,
}

/// O professor falante do app. Veja o módulo.
pub struct Professor {
    voz: Box<dyn Voz>,
    ligado: bool,
    arquivo: Option<PathBuf>,
    contexto: Option<Contexto>,
    fila: VecDeque<String>,
    legenda: Option<String>,
    ultimas: Vec<(Momento, Option<usize>, String)>,
    semente: u64,
    agora_s: f64,
    /// Voz muda: instante até o qual a fala atual fica na tela antes da próxima.
    livre_em_s: f64,
    erro_de_voz: Option<ErroVoz>,
}

impl std::fmt::Debug for Professor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Professor")
            .field("ligado", &self.ligado)
            .field("legenda", &self.legenda)
            .finish_non_exhaustive()
    }
}

impl Professor {
    /// Lê a preferência de `pasta_dados` (padrão: fala ligada).
    pub fn novo(voz: Box<dyn Voz>, pasta_dados: Option<&Path>) -> Self {
        let arquivo = pasta_dados.map(|p| p.join(ARQUIVO_PREFERENCIAS_PROFESSOR));
        let ligado = arquivo
            .as_deref()
            .and_then(|a| std::fs::read_to_string(a).ok())
            .is_none_or(|t| ler_fala_ligada(&t));
        let semente = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9_7F4A_7C15, |d| d.as_nanos() as u64)
            | 1;
        Self {
            voz,
            ligado,
            arquivo,
            contexto: None,
            fila: VecDeque::new(),
            legenda: None,
            ultimas: Vec::new(),
            semente,
            agora_s: 0.0,
            livre_em_s: 0.0,
            erro_de_voz: None,
        }
    }

    /// Professor sem voz e sem preferência gravada (testes e telas sem pasta de dados).
    pub fn mudo() -> Self {
        Self::novo(Box::new(crate::professor_falante::VozMuda::nova()), None)
    }

    pub fn ligado(&self) -> bool {
        self.ligado
    }

    /// Liga ou desliga o professor e grava a preferência. Desligado, corta a fala em curso.
    pub fn definir_ligado(&mut self, ligado: bool) {
        if self.ligado == ligado {
            return;
        }
        self.ligado = ligado;
        if !ligado {
            self.parar();
            self.legenda = None;
        }
        if let Some(a) = &self.arquivo {
            if let Some(p) = a.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            let _ = std::fs::write(
                a,
                format!(
                    "# Piano Tutor: preferências do professor v1\nfala={}\n",
                    u8::from(ligado)
                ),
            );
        }
    }

    /// Fixa o sorteio (testes).
    pub fn definir_semente(&mut self, semente: u64) {
        self.semente = semente | 1;
    }

    fn sortear(&mut self) -> u64 {
        // xorshift64*: suficiente para variar a fala, sem dependência.
        let mut x = self.semente;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.semente = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11
    }

    // --- contexto ---

    /// Abre um Exercício: guarda de onde vêm as falas, corta o que estava dizendo e diz o `antes`.
    pub fn abrir(&mut self, licao: &str, exercicio: &str, roteiro: Option<Roteiro>) {
        self.parar();
        self.legenda = None;
        self.ultimas.clear();
        self.contexto = Some(Contexto {
            licao: licao.to_owned(),
            exercicio: exercicio.to_owned(),
            roteiro,
        });
        self.dizer(Momento::Antes, None, false);
    }

    /// Abre o exercício de fábrica `n` (1 a 10) com o Roteiro embutido no app.
    pub fn abrir_fabrica(&mut self, n: u8) {
        self.abrir(
            LICAO_DE_FABRICA,
            &id_de_exercicio_de_fabrica(n),
            roteiro_de_fabrica().ok(),
        );
    }

    /// Larga o Exercício atual (ao sair da tela).
    pub fn fechar(&mut self) {
        self.parar();
        self.contexto = None;
        self.legenda = None;
    }

    /// Há alguma fala aprovada neste Exercício (em qualquer momento)?
    pub fn tem_falas_aprovadas(&self) -> bool {
        let Some(c) = &self.contexto else {
            return false;
        };
        c.roteiro.as_ref().is_some_and(|r| {
            r.falas_aprovadas().any(|f| {
                f.licao == c.licao && f.exercicio.as_deref().is_none_or(|e| e == c.exercicio)
            })
        })
    }

    // --- falar ---

    /// Pede a fala do `momento` (e do `passo`, se for o caso). Devolve `true` se havia fala
    /// aprovada e ela entrou na fila. Com `so_se_livre`, não enfileira enquanto o professor
    /// ainda fala (para comentários de erro não se acumularem).
    pub fn dizer(&mut self, momento: Momento, passo: Option<usize>, so_se_livre: bool) -> bool {
        if !self.ligado || (so_se_livre && !self.livre()) {
            return false;
        }
        let candidatas: Vec<String> = {
            let Some(c) = &self.contexto else {
                return false;
            };
            let Some(roteiro) = &c.roteiro else {
                return false;
            };
            falas_para_dizer(roteiro, &c.licao, Some(&c.exercicio), momento, passo)
                .iter()
                .map(|f| f.texto.trim().to_owned())
                .filter(|t| !t.is_empty())
                .collect()
        };
        if candidatas.is_empty() {
            return false;
        }
        let ultima = self
            .ultimas
            .iter()
            .find(|(m, p, _)| *m == momento && *p == passo)
            .map(|(_, _, t)| t.clone());
        let sorteio = self.sortear();
        let refs: Vec<&str> = candidatas.iter().map(String::as_str).collect();
        let Some(texto) =
            escolher_sem_repetir(&refs, ultima.as_deref(), sorteio).map(str::to_owned)
        else {
            return false;
        };
        match self
            .ultimas
            .iter_mut()
            .find(|(m, p, _)| *m == momento && *p == passo)
        {
            Some(e) => e.2.clone_from(&texto),
            None => self.ultimas.push((momento, passo, texto.clone())),
        }
        self.fila.push_back(texto);
        true
    }

    /// Diz `texto` já (um anúncio do modo Chamada e resposta), cortando a fala anterior e a
    /// fila. Devolve `true` se uma **voz** vai dizê-lo (aí o fim da fala é avisado por
    /// [`Professor::falando`]); `false` se o texto só aparece na tela.
    pub fn anunciar(&mut self, texto: &str) -> bool {
        self.fila.clear();
        if !self.ligado {
            return false;
        }
        let com_voz = self.voz_pronta();
        self.dizer_agora(texto);
        com_voz && self.voz.falando()
    }

    fn dizer_agora(&mut self, texto: &str) {
        let texto = texto.trim();
        if texto.is_empty() {
            return;
        }
        self.legenda = Some(texto.to_owned());
        self.livre_em_s = if self.voz_pronta() {
            0.0
        } else {
            self.agora_s + estimar_duracao_da_fala_s(texto)
        };
        if let Err(e) = self.voz.falar(texto) {
            self.erro_de_voz = Some(e);
        }
    }

    /// Um passo por quadro: guarda o instante, recolhe erro de voz e diz a próxima da fila
    /// quando o professor terminou a anterior.
    pub fn atualizar(&mut self, agora_s: f64) {
        if agora_s.is_finite() {
            self.agora_s = agora_s;
        }
        if let Some(e) = self.voz.tomar_erro() {
            self.erro_de_voz = Some(e);
        }
        if !self.ligado {
            self.fila.clear();
            return;
        }
        if self.ocioso()
            && let Some(texto) = self.fila.pop_front()
        {
            self.dizer_agora(&texto);
        }
    }

    fn voz_pronta(&self) -> bool {
        self.voz.estado() == EstadoVoz::Pronta
    }

    /// A voz (ou o tempo da legenda, se for muda) terminou a fala anterior.
    fn ocioso(&self) -> bool {
        !self.voz.falando() && self.agora_s >= self.livre_em_s
    }

    /// Ocioso e sem nada na fila.
    fn livre(&self) -> bool {
        self.fila.is_empty() && self.ocioso()
    }

    /// A voz ainda está dizendo algo (síntese ou reprodução).
    pub fn falando(&self) -> bool {
        self.voz.falando()
    }

    /// Corta a fala em curso e esvazia a fila.
    pub fn parar(&mut self) {
        self.fila.clear();
        self.voz.parar();
        self.livre_em_s = 0.0;
    }

    // --- tela ---

    /// Última fala dita, para mostrar na tela.
    pub fn legenda(&self) -> Option<&str> {
        self.legenda.as_deref()
    }

    /// Aviso para o aluno quando não há voz (ou a voz falhou): COMO_INSTALAR_A_VOZ. Só com o
    /// professor ligado.
    pub fn aviso_de_voz(&self) -> Option<String> {
        if !self.ligado {
            return None;
        }
        if let EstadoVoz::Indisponivel(m) = self.voz.estado() {
            return Some(m);
        }
        self.erro_de_voz.as_ref().map(ToString::to_string)
    }

    /// Há voz de verdade (não a muda) e o professor está ligado.
    pub fn com_voz(&self) -> bool {
        self.ligado && self.voz_pronta()
    }
}

fn ler_fala_ligada(texto: &str) -> bool {
    texto
        .lines()
        .filter_map(|l| l.trim().split_once('='))
        .find(|(k, _)| k.trim() == "fala")
        .is_none_or(|(_, v)| !matches!(v.trim(), "0" | "false"))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::professor_falante::{VozFalsa, VozMuda, ler_roteiro};

    const ROTEIRO: &str = r#"
formato_versao = 1

[[falas]]
id = "a1"
licao = "l1"
exercicio = "ex"
momento = "antes"
texto = "Vamos começar."
estado = "aprovada"

[[falas]]
id = "a2"
licao = "l1"
exercicio = "ex"
momento = "antes"
texto = "Esta é hipótese."
estado = "hipotese"

[[falas]]
id = "c1"
licao = "l1"
momento = "acerto"
texto = "Muito bem."
estado = "aprovada"

[[falas]]
id = "c2"
licao = "l1"
momento = "acerto"
texto = "Isso mesmo."
estado = "aprovada"

[[falas]]
id = "c3"
licao = "l1"
momento = "acerto"
texto = "Perfeito."
estado = "aprovada"

[[falas]]
id = "p1"
licao = "l1"
exercicio = "ex"
momento = "passo"
passo = 2
texto = "Agora o Mi."
estado = "aprovada"
"#;

    fn professor_falso() -> (Professor, VozFalsa) {
        let voz = VozFalsa::nova();
        let mut p = Professor::novo(Box::new(voz.clone()), None);
        p.definir_semente(7);
        (p, voz)
    }

    fn abrir_teste(p: &mut Professor) {
        p.abrir("l1", "ex", Some(ler_roteiro(ROTEIRO).expect("roteiro")));
    }

    #[test]
    fn escolhe_sem_repetir_a_ultima_quando_ha_outra() {
        assert_eq!(escolher_sem_repetir(&[], None, 3), None);
        assert_eq!(escolher_sem_repetir(&["a"], Some("a"), 9), Some("a"));
        for sorteio in 0..20 {
            assert_eq!(
                escolher_sem_repetir(&["a", "b"], Some("a"), sorteio),
                Some("b")
            );
            let x = escolher_sem_repetir(&["a", "b", "c"], Some("b"), sorteio);
            assert!(matches!(x, Some("a" | "c")), "{x:?}");
        }
        // Sem última, qualquer uma; o sorteio varre todas.
        let todas: std::collections::BTreeSet<_> = (0..6)
            .filter_map(|s| escolher_sem_repetir(&["a", "b", "c"], None, s))
            .collect();
        assert_eq!(todas.len(), 3);
    }

    #[test]
    fn so_diz_falas_aprovadas_e_mostra_a_legenda() {
        let (mut p, voz) = professor_falso();
        abrir_teste(&mut p);
        p.atualizar(0.0);
        assert_eq!(voz.pedidos(), ["Vamos começar."], "a hipótese não fala");
        assert_eq!(p.legenda(), Some("Vamos começar."));
        assert!(p.tem_falas_aprovadas());
        assert_eq!(p.aviso_de_voz(), None);
    }

    #[test]
    fn nunca_repete_a_mesma_fala_em_seguida_no_mesmo_momento() {
        let (mut p, voz) = professor_falso();
        abrir_teste(&mut p);
        let mut t = 0.0;
        for _ in 0..30 {
            t += 1.0;
            voz.terminar_a_fala();
            p.atualizar(t);
            assert!(p.dizer(Momento::Acerto, None, false));
            voz.terminar_a_fala();
            p.atualizar(t + 0.5);
        }
        let ditas: Vec<String> = voz
            .pedidos()
            .into_iter()
            .filter(|t| t != "Vamos começar.")
            .collect();
        assert_eq!(ditas.len(), 30);
        for par in ditas.windows(2) {
            assert_ne!(par[0], par[1], "repetiu em seguida");
        }
        let distintas: std::collections::BTreeSet<_> = ditas.iter().collect();
        assert_eq!(distintas.len(), 3, "as três aparecem: {distintas:?}");
    }

    #[test]
    fn passo_so_fala_no_numero_certo_e_sem_fala_nao_diz_nada() {
        let (mut p, voz) = professor_falso();
        abrir_teste(&mut p);
        p.atualizar(1.0);
        voz.terminar_a_fala();
        assert!(!p.dizer(Momento::Passo, Some(1), false));
        assert!(!p.dizer(Momento::Erro, None, false));
        assert!(p.dizer(Momento::Passo, Some(2), false));
        p.atualizar(2.0);
        assert_eq!(
            voz.pedidos().last().map(String::as_str),
            Some("Agora o Mi.")
        );
    }

    #[test]
    fn fila_espera_a_voz_terminar_e_so_se_livre_nao_acumula() {
        let (mut p, voz) = professor_falso();
        abrir_teste(&mut p);
        p.atualizar(0.0);
        assert!(voz.falando());
        // Ainda falando o "antes": a de acerto espera na fila.
        assert!(p.dizer(Momento::Acerto, None, false));
        p.atualizar(0.1);
        assert_eq!(voz.pedidos().len(), 1);
        // Comentário opcional (erro) não entra na fila enquanto está ocupado.
        assert!(!p.dizer(Momento::Acerto, None, true));
        voz.terminar_a_fala();
        p.atualizar(0.2);
        assert_eq!(voz.pedidos().len(), 2);
        assert!(p.falando());
    }

    #[test]
    fn interruptor_desligado_cala_e_a_preferencia_vai_e_volta() {
        let pasta = tempfile::tempdir().expect("pasta");
        let voz = VozFalsa::nova();
        let mut p = Professor::novo(Box::new(voz.clone()), Some(pasta.path()));
        assert!(p.ligado(), "padrão: ligado");
        abrir_teste(&mut p);
        p.atualizar(0.0);
        assert_eq!(voz.pedidos().len(), 1);
        p.definir_ligado(false);
        assert_eq!(voz.vezes_que_parou(), 2, "abrir corta e desligar corta");
        assert_eq!(p.legenda(), None);
        assert!(!p.dizer(Momento::Acerto, None, false));
        assert!(!p.anunciar("Ouça e repita."));
        p.atualizar(1.0);
        assert_eq!(voz.pedidos().len(), 1, "nada novo");
        assert_eq!(p.aviso_de_voz(), None, "desligado, sem aviso");
        // Preferência gravada: outro professor nasce desligado.
        let outro = Professor::novo(Box::new(VozFalsa::nova()), Some(pasta.path()));
        assert!(!outro.ligado());
        let mut outro = outro;
        outro.definir_ligado(true);
        assert!(Professor::novo(Box::new(VozFalsa::nova()), Some(pasta.path())).ligado());
    }

    #[test]
    fn sem_voz_o_texto_aparece_e_o_aviso_diz_como_instalar() {
        let mut p = Professor::novo(Box::new(VozMuda::nova()), None);
        abrir_teste(&mut p);
        p.atualizar(0.0);
        assert_eq!(p.legenda(), Some("Vamos começar."));
        let aviso = p.aviso_de_voz().expect("aviso");
        assert!(aviso.contains("Piper"), "{aviso}");
        assert!(!p.com_voz());
        // A voz muda segura cada fala na tela pelo tempo estimado antes da próxima.
        assert!(p.dizer(Momento::Acerto, None, false));
        p.atualizar(0.5);
        assert_eq!(p.legenda(), Some("Vamos começar."));
        p.atualizar(10.0);
        assert!(matches!(
            p.legenda(),
            Some("Muito bem." | "Isso mesmo." | "Perfeito.")
        ));
        // Anúncio sem voz: só texto na tela, e quem chama usa a estimativa.
        assert!(!p.anunciar("Frase 1. Ouça e repita."));
        assert_eq!(p.legenda(), Some("Frase 1. Ouça e repita."));
    }

    #[test]
    fn anuncio_com_voz_corta_a_fila_e_diz_na_hora() {
        let (mut p, voz) = professor_falso();
        abrir_teste(&mut p);
        p.atualizar(0.0);
        assert!(p.dizer(Momento::Acerto, None, false));
        assert!(p.anunciar("Frase 2. Ouça e repita."));
        p.atualizar(1.0);
        assert_eq!(
            voz.pedidos(),
            ["Vamos começar.", "Frase 2. Ouça e repita."],
            "a fila foi descartada"
        );
        assert!(p.com_voz());
    }
}
