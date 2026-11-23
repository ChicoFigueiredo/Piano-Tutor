//! Roda de pitch bend virtual: a roda do mouse sobre o teclado na tela curva a afinação, como a
//! roda de bend de um teclado físico (ou como esticar a corda de uma guitarra).
//!
//! Lógica pura, sem egui e sem relógio próprio: quem chama informa o tempo em segundos
//! (monotônico) a cada [`RodaBend::girar`] e [`RodaBend::atualizar`], o que torna a mola
//! determinística e testável.
//!
//! - **Valor:** inteiro centrado em zero, `-8191..=8191` (o mesmo do `Acao::PitchBend` do motor;
//!   a faixa padrão do GM é de ±2 semitons, então 4096 = 1 semitom).
//! - **Passo:** cada "entalhe" da roda soma [`PASSO_POR_ENTALHE`] (1/3 de semitom): três
//!   entalhes ≈ 1 semitom e cerca de 6 entalhes chegam ao fundo de curso.
//! - **Mola:** sem girar por [`ESPERA_MOLA_S`] (120 ms), o valor volta ao centro em
//!   [`DURACAO_MOLA_S`] (200 ms), com suavização (*smoothstep*).

/// Maior valor absoluto da roda (a faixa MIDI vai de -8192 a 8191; o curso é simétrico).
pub const BEND_MAX: f32 = 8191.0;
/// Unidades de bend por semitom na faixa padrão do GM (±2 semitons em 8192).
pub const UNIDADES_POR_SEMITOM: f32 = 4096.0;
/// Quanto um entalhe da roda do mouse curva: 1/3 de semitom.
pub const PASSO_POR_ENTALHE: f32 = UNIDADES_POR_SEMITOM / 3.0;
/// Pontos de rolagem do egui que valem um entalhe (o egui usa 40 pontos por linha da roda).
pub const PONTOS_POR_ENTALHE: f32 = 40.0;
/// Tempo parado até a mola começar a voltar.
pub const ESPERA_MOLA_S: f64 = 0.120;
/// Duração da volta ao centro.
pub const DURACAO_MOLA_S: f64 = 0.200;

/// Estado da roda virtual.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RodaBend {
    /// Curvatura atual, `-BEND_MAX..=BEND_MAX`.
    valor: f32,
    /// Instante (s) do último giro.
    ultimo_giro_s: f64,
    /// Valor em que a volta começou (`None` enquanto a mola não disparou).
    retorno_de: Option<f32>,
}

impl RodaBend {
    pub const fn nova() -> Self {
        Self {
            valor: 0.0,
            ultimo_giro_s: 0.0,
            retorno_de: None,
        }
    }

    /// Gira a roda `entalhes` (positivo = para cima = afinação sobe) no instante `agora_s`.
    /// Acumula até o fundo de curso e cancela uma volta em andamento. Valores não finitos são
    /// ignorados.
    pub fn girar(&mut self, entalhes: f32, agora_s: f64) {
        if !entalhes.is_finite() || entalhes == 0.0 {
            return;
        }
        self.valor = (self.valor + entalhes * PASSO_POR_ENTALHE).clamp(-BEND_MAX, BEND_MAX);
        self.ultimo_giro_s = agora_s;
        self.retorno_de = None;
    }

    /// Avança a mola até `agora_s` e devolve o bend inteiro (`-8191..=8191`) para enviar ao som.
    pub fn atualizar(&mut self, agora_s: f64) -> i16 {
        if self.valor != 0.0 {
            let inicio = self.ultimo_giro_s + ESPERA_MOLA_S;
            if agora_s >= inicio {
                let de = *self.retorno_de.get_or_insert(self.valor);
                let p = ((agora_s - inicio) / DURACAO_MOLA_S).clamp(0.0, 1.0) as f32;
                let suave = p * p * (3.0 - 2.0 * p);
                self.valor = if p >= 1.0 { 0.0 } else { de * (1.0 - suave) };
            }
        }
        self.valor_atual()
    }

    /// Valor inteiro atual, sem avançar o tempo.
    pub fn valor_atual(&self) -> i16 {
        self.valor.round() as i16
    }

    /// `true` quando está exatamente no centro (a UI pode parar de pedir quadros).
    pub fn centrada(&self) -> bool {
        self.valor == 0.0
    }

    /// Volta ao centro na hora, sem mola (perda de foco, pânico, troca de tela).
    pub fn zerar(&mut self) {
        self.valor = 0.0;
        self.retorno_de = None;
    }

    /// Curvatura em semitons (faixa padrão ±2).
    pub fn semitons(&self) -> f32 {
        self.valor / UNIDADES_POR_SEMITOM
    }

    /// Posição de -1,0 a 1,0 para desenhar o indicador.
    pub fn fracao(&self) -> f32 {
        (self.valor / BEND_MAX).clamp(-1.0, 1.0)
    }
}

/// Texto do indicador, em pt-BR: `Bend +0,8 st` (st = semitons); no centro, `Bend 0,0 st`.
pub fn texto_bend(semitons: f32) -> String {
    let s = if semitons.abs() < 0.05 { 0.0 } else { semitons };
    let sinal = if s > 0.0 { "+" } else { "" };
    format!("Bend {sinal}{s:.1} st").replace('.', ",")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tres_entalhes_valem_cerca_de_um_semitom() {
        let mut r = RodaBend::nova();
        r.girar(3.0, 0.0);
        assert!((i32::from(r.valor_atual()) - 4096).abs() <= 1);
        assert!((r.semitons() - 1.0).abs() < 0.001);
        r.girar(-3.0, 0.01);
        assert_eq!(r.valor_atual(), 0);
    }

    #[test]
    fn acumula_ate_o_fundo_do_curso_nos_dois_sentidos() {
        let mut r = RodaBend::nova();
        for _ in 0..50 {
            r.girar(1.0, 0.0);
        }
        assert_eq!(r.valor_atual(), 8191);
        assert!((r.fracao() - 1.0).abs() < 1e-6);
        for _ in 0..100 {
            r.girar(-1.0, 0.0);
        }
        assert_eq!(r.valor_atual(), -8191);
        assert!((r.fracao() + 1.0).abs() < 1e-6);
        // Gira muito de uma vez: continua limitado.
        r.girar(1_000.0, 0.0);
        assert_eq!(r.valor_atual(), 8191);
    }

    #[test]
    fn entrada_nao_finita_ou_zero_e_ignorada() {
        let mut r = RodaBend::nova();
        r.girar(f32::NAN, 1.0);
        r.girar(f32::INFINITY, 1.0);
        r.girar(0.0, 1.0);
        assert!(r.centrada());
    }

    #[test]
    fn mola_so_comeca_depois_de_120_ms_parada() {
        let mut r = RodaBend::nova();
        r.girar(3.0, 10.0);
        let v = r.valor_atual();
        assert_eq!(r.atualizar(10.0), v);
        assert_eq!(r.atualizar(10.119), v, "ainda segurando");
        assert_eq!(r.atualizar(10.120), v, "começa agora, ainda sem andar");
        assert!(r.atualizar(10.15) < v, "já voltando");
    }

    #[test]
    fn mola_volta_ao_centro_em_200_ms_de_forma_monotonica() {
        let mut r = RodaBend::nova();
        r.girar(4.0, 0.0);
        let inicio = r.valor_atual();
        let mut anterior = inicio;
        let mut t = 0.12;
        while t < 0.32 {
            let v = r.atualizar(t);
            assert!(v <= anterior && v >= 0, "t={t} v={v} anterior={anterior}");
            anterior = v;
            t += 0.004;
        }
        // No meio da volta (t = 0,22 s) o smoothstep dá exatamente metade.
        let mut m = RodaBend::nova();
        m.girar(4.0, 0.0);
        let meio = m.atualizar(0.22);
        assert!(
            (i32::from(meio) - i32::from(inicio) / 2).abs() <= 1,
            "{meio}"
        );
        assert_eq!(r.atualizar(0.32), 0);
        assert!(r.centrada());
    }

    #[test]
    fn mola_no_negativo_volta_sem_passar_do_zero() {
        let mut r = RodaBend::nova();
        r.girar(-5.0, 0.0);
        let mut t = 0.0;
        while t < 0.5 {
            let v = r.atualizar(t);
            assert!(v <= 0, "t={t} v={v}");
            t += 0.003;
        }
        assert!(r.centrada());
    }

    #[test]
    fn resultado_nao_depende_da_taxa_de_quadros() {
        let mut a = RodaBend::nova();
        let mut b = RodaBend::nova();
        a.girar(3.0, 1.0);
        b.girar(3.0, 1.0);
        for i in 1..=20 {
            a.atualizar(1.0 + f64::from(i) * 0.005);
        }
        // Um único passo grande no mesmo instante final.
        assert_eq!(a.atualizar(1.2), b.atualizar(1.2));
    }

    #[test]
    fn girar_durante_a_volta_cancela_e_acumula_a_partir_do_valor_atual() {
        let mut r = RodaBend::nova();
        r.girar(6.0, 0.0);
        let durante = r.atualizar(0.22);
        assert!(durante > 0 && durante < 8191);
        r.girar(1.0, 0.22);
        let depois = r.valor_atual();
        assert!(depois > durante, "acumulou sobre o valor em volta");
        // A espera de 120 ms recomeça do novo giro.
        assert_eq!(r.atualizar(0.30), depois);
        assert!(r.atualizar(0.40) < depois);
    }

    #[test]
    fn zerar_centra_na_hora() {
        let mut r = RodaBend::nova();
        r.girar(5.0, 0.0);
        r.zerar();
        assert!(r.centrada());
        assert_eq!(r.atualizar(0.0), 0);
    }

    #[test]
    fn texto_em_semitons_com_virgula_e_sinal() {
        assert_eq!(texto_bend(0.0), "Bend 0,0 st");
        assert_eq!(texto_bend(0.02), "Bend 0,0 st");
        assert_eq!(texto_bend(0.8), "Bend +0,8 st");
        assert_eq!(texto_bend(-1.5), "Bend -1,5 st");
        assert_eq!(texto_bend(2.0), "Bend +2,0 st");
    }
}
