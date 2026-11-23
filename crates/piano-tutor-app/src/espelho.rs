//! Estado e regras da tela Espelho (F02, #34), sem janela: tudo aqui é testável por unidade.
//!
//! O estado das teclas e os nomes vêm do núcleo (`piano_tutor_core::teclado`). Este módulo
//! acrescenta o que é do app: geometria do desenho, hit-test, a nota atual, o teclado do PC
//! como fonte de **simulação** e o aviso de que a fonte não é o teclado físico.
//!
//! Ponto de entrada do gerenciador MIDI: [`EstadoEspelho::aplicar_evento`]. Os eventos da
//! simulação saem por [`EstadoEspelho::drenar_simulados`] para também soarem.

use crate::dedos::NomenclaturaDedos;
use crate::roda_bend::RodaBend;
use bancada_casca::egui::{Key, Rect};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::teclado::{
    AcaoTecla, ConfigNomes, CorTecla, ErroTeclado, EstadoTeclas, EventoTecla, FonteEventos, Idioma,
    ModoSimulacao, OrigemEventos, Teclado61, nome_sem_oitava, rotulo,
};

/// Aviso permanente enquanto a fonte dos eventos for simulação.
pub const AVISO_SIMULACAO: &str = "SIMULAÇÃO — não é o teclado físico";

/// Afinação usada para a frequência mostrada (Lá4).
pub const LA4_HZ: f64 = 440.0;

/// Largura de uma tecla preta, em larguras de tecla branca.
pub const LARGURA_PRETA: f32 = 0.6;
/// Altura da tecla branca, em larguras de tecla branca.
pub const ALTURA_BRANCA: f32 = 5.5;
/// Altura da tecla preta, em larguras de tecla branca.
pub const ALTURA_PRETA: f32 = 3.4;
/// Folga nas pontas do desenho (uma preta pode ser a primeira ou a última tecla).
pub const MARGEM: f32 = 0.3;

pub const ZOOM_MIN: f32 = 0.5;
pub const ZOOM_MAX: f32 = 2.5;

// ---------------------------------------------------------------------------------------------
// Geometria
// ---------------------------------------------------------------------------------------------

/// Retângulo de uma tecla em unidades de "largura de tecla branca", origem no canto superior
/// esquerdo da primeira tecla branca (sem a [`MARGEM`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometriaTecla {
    pub indice: usize,
    pub nota: NotaMidi,
    pub cor: CorTecla,
    pub x: f32,
    pub largura: f32,
    pub altura: f32,
}

impl GeometriaTecla {
    pub fn centro_x(&self) -> f32 {
        self.x + self.largura / 2.0
    }

    fn contem(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.largura && y >= 0.0 && y < self.altura
    }
}

/// Geometria das 61 teclas, da mais grave à mais aguda. As pretas ficam centradas na divisa
/// entre as duas brancas vizinhas.
pub fn geometria(teclado: Teclado61) -> Vec<GeometriaTecla> {
    let mut brancas = 0.0_f32;
    teclado
        .teclas()
        .map(|t| match t.cor {
            CorTecla::Branca => {
                let g = GeometriaTecla {
                    indice: t.indice,
                    nota: t.nota,
                    cor: t.cor,
                    x: brancas,
                    largura: 1.0,
                    altura: ALTURA_BRANCA,
                };
                brancas += 1.0;
                g
            }
            CorTecla::Preta => GeometriaTecla {
                indice: t.indice,
                nota: t.nota,
                cor: t.cor,
                x: brancas - LARGURA_PRETA / 2.0,
                largura: LARGURA_PRETA,
                altura: ALTURA_PRETA,
            },
        })
        .collect()
}

/// Largura total do desenho, em larguras de branca, incluindo a margem das duas pontas.
pub fn largura_total(teclado: Teclado61) -> f32 {
    teclado.quantidade_brancas() as f32 + 2.0 * MARGEM
}

/// Tecla sob o ponto `(x, y)` (em unidades de branca, relativo à origem da geometria).
/// As pretas ficam por cima das brancas, como no instrumento.
pub fn tecla_em(geo: &[GeometriaTecla], x: f32, y: f32) -> Option<usize> {
    let achar = |cor: CorTecla| {
        geo.iter()
            .find(|g| g.cor == cor && g.contem(x, y))
            .map(|g| g.indice)
    };
    achar(CorTecla::Preta).or_else(|| achar(CorTecla::Branca))
}

// ---------------------------------------------------------------------------------------------
// Cores (independentes do tema: o teclado é um objeto, não uma superfície do app)
// ---------------------------------------------------------------------------------------------

pub type Rgb = [u8; 3];

pub const BRANCA: Rgb = [244, 241, 232];
pub const PRETA: Rgb = [23, 23, 28];
pub const BRANCA_PRESSIONADA: Rgb = [31, 95, 208];
pub const PRETA_PRESSIONADA: Rgb = [106, 167, 255];
pub const CONTORNO_BRANCA_PRESSIONADA: Rgb = [0, 0, 0];
pub const CONTORNO_PRETA_PRESSIONADA: Rgb = [11, 27, 58];

fn luminancia(c: Rgb) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// Razão de contraste WCAG 2 entre duas cores (1 a 21).
pub fn contraste(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminancia(a), luminancia(b));
    let (claro, escuro) = if la >= lb { (la, lb) } else { (lb, la) };
    (claro + 0.05) / (escuro + 0.05)
}

// ---------------------------------------------------------------------------------------------
// Teclado do PC (simulação)
// ---------------------------------------------------------------------------------------------

/// Teclas do PC que tocam: `(tecla, semitons acima do Dó da oitava-base, letra mostrada)`.
/// Fileira A S D F G H J K L ; = brancas; W E T Y U O P = pretas.
pub const MAPA_PC: [(Key, u8, &str); 17] = [
    (Key::A, 0, "A"),
    (Key::W, 1, "W"),
    (Key::S, 2, "S"),
    (Key::E, 3, "E"),
    (Key::D, 4, "D"),
    (Key::F, 5, "F"),
    (Key::T, 6, "T"),
    (Key::G, 7, "G"),
    (Key::Y, 8, "Y"),
    (Key::H, 9, "H"),
    (Key::U, 10, "U"),
    (Key::J, 11, "J"),
    (Key::K, 12, "K"),
    (Key::O, 13, "O"),
    (Key::L, 14, "L"),
    (Key::P, 15, "P"),
    (Key::Semicolon, 16, ";"),
];

/// Dó (múltiplo de 12) da oitava-base do teclado do PC no início: o Dó central.
pub const BASE_PC_INICIAL: u8 = 60;

// ---------------------------------------------------------------------------------------------
// Estado
// ---------------------------------------------------------------------------------------------

/// O que o painel "nota atual" mostra.
#[derive(Clone, Debug, PartialEq)]
pub struct InfoNota {
    pub nota: NotaMidi,
    /// Nome conforme a configuração, sem oitava ("Dó", "C").
    pub nome: &'static str,
    /// Nome em PT-BR e internacional, sem oitava, para mostrar os dois.
    pub nome_pt: &'static str,
    pub nome_internacional: &'static str,
    /// Rótulo com a oitava pela convenção escolhida ("Dó4", "C3").
    pub rotulo: String,
    pub midi: u8,
    pub oitava: i8,
    pub frequencia_hz: f64,
    /// `true` se a tecla está pressionada agora; `false` se é a última nota, já solta.
    pub pressionada: bool,
}

/// O que o último desenho produziu; serve de conferência nos testes e de ponte para o
/// mouse (onde o teclado ficou na tela).
#[derive(Clone, Debug, PartialEq)]
pub struct ResumoDesenho {
    /// Retângulo (px) da área das teclas; a origem da geometria é `min + MARGEM * largura`.
    pub area_teclas: Rect,
    pub largura_branca: f32,
    /// Índices das teclas desenhadas como "ligadas" (cor + contorno + marca).
    pub destacadas: Vec<usize>,
    /// Quantas teclas foram desenhadas.
    pub desenhadas: usize,
    /// O aviso de simulação foi desenhado neste quadro.
    pub aviso_simulacao_visivel: bool,
}

/// Estado da tela Espelho.
#[derive(Debug)]
pub struct EstadoEspelho {
    teclas: EstadoTeclas,
    simulacao: ModoSimulacao,
    pub nomes: ConfigNomes,
    zoom: f32,
    fonte: OrigemEventos,
    faixa_calibrada: bool,
    /// Notas ligadas na ordem em que foram pressionadas.
    ordem: Vec<NotaMidi>,
    ultima_solta: Option<NotaMidi>,
    nota_do_mouse: Option<NotaMidi>,
    notas_do_pc: Vec<(Key, NotaMidi)>,
    base_pc: u8,
    erro: Option<String>,
    /// Eventos da simulação já aplicados, à espera do app levá-los ao som.
    simulados: Vec<EventoTecla>,
    pub ultimo_desenho: Option<ResumoDesenho>,
    /// Teclas-guia (acorde, escala ou exercício a tocar), marcadas em verde.
    pub guias: Vec<NotaMidi>,
    /// Dedo de cada tecla-guia informado pela tela (o dedilhado de uma Lição). Vazio: o
    /// próprio teclado resolve o dedo pelas guias ([`crate::dedos::inferir`]).
    dedos: Vec<(NotaMidi, u8)>,
    /// Como o dedo aparece dentro da bolinha (números ou letras); vem das preferências.
    pub nomenclatura: NomenclaturaDedos,
    /// Ponto de divisão do som (mão esquerda em azul abaixo dele), copiado do serviço de som.
    pub divisao: Option<u8>,
    /// Roda de pitch bend virtual (roda do mouse sobre o teclado). O app a leva ao som.
    pub roda_bend: RodaBend,
}

impl Default for EstadoEspelho {
    fn default() -> Self {
        Self::novo()
    }
}

impl EstadoEspelho {
    /// Estado inicial: faixa padrão de testes/simulação (**não calibrada**) e fonte simulação.
    pub fn novo() -> Self {
        Self::com_teclado(Teclado61::padrao_para_testes_e_simulacao(), false)
    }

    /// Estado com uma faixa vinda da calibração (F01). Ainda sem chamador real.
    pub fn calibrado(teclado: Teclado61) -> Self {
        Self::com_teclado(teclado, true)
    }

    fn com_teclado(teclado: Teclado61, calibrada: bool) -> Self {
        Self {
            teclas: EstadoTeclas::novo(teclado),
            simulacao: ModoSimulacao::novo(),
            nomes: ConfigNomes::default(),
            zoom: 1.0,
            fonte: OrigemEventos::Simulacao,
            faixa_calibrada: calibrada,
            ordem: Vec::new(),
            ultima_solta: None,
            nota_do_mouse: None,
            notas_do_pc: Vec::new(),
            base_pc: BASE_PC_INICIAL,
            erro: None,
            simulados: Vec::new(),
            ultimo_desenho: None,
            guias: Vec::new(),
            dedos: Vec::new(),
            nomenclatura: NomenclaturaDedos::default(),
            divisao: None,
            roda_bend: RodaBend::nova(),
        }
    }

    /// Teclas-guia a destacar. O dedo de cada uma o teclado resolve sozinho (acorde, escala),
    /// salvo quando a tela informa o dedilhado (Lição): então vale o informado.
    pub fn definir_guias(&mut self, guias: Vec<NotaMidi>, dedos_informados: Vec<(NotaMidi, u8)>) {
        self.guias = guias;
        self.dedos = dedos_informados;
    }

    /// Tira as guias e o dedilhado.
    pub fn limpar_guias(&mut self) {
        self.guias.clear();
        self.dedos.clear();
    }

    /// Dedo de cada tecla-guia: o informado pela tela ou o resolvido pelo teclado. A mão é a
    /// esquerda quando todas as guias ficam abaixo do ponto de divisão; senão, a direita.
    pub fn dedos_das_guias(&self, divisao: Option<u8>) -> Vec<(NotaMidi, u8)> {
        if !self.dedos.is_empty() {
            return self.dedos.clone();
        }
        let esquerda = divisao
            .is_some_and(|d| !self.guias.is_empty() && self.guias.iter().all(|n| n.numero() < d));
        crate::dedos::inferir(&self.guias, esquerda)
    }

    pub fn teclado(&self) -> Teclado61 {
        self.teclas.teclado()
    }

    pub fn teclas(&self) -> &EstadoTeclas {
        &self.teclas
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn definir_zoom(&mut self, zoom: f32) {
        self.zoom = if zoom.is_finite() {
            zoom.clamp(ZOOM_MIN, ZOOM_MAX)
        } else {
            1.0
        };
    }

    pub fn fonte(&self) -> OrigemEventos {
        self.fonte
    }

    pub fn faixa_calibrada(&self) -> bool {
        self.faixa_calibrada
    }

    pub fn base_pc(&self) -> u8 {
        self.base_pc
    }

    /// Último erro não fatal (nota fora da faixa), para mostrar na tela.
    pub fn erro(&self) -> Option<&str> {
        self.erro.as_deref()
    }

    /// Aviso permanente: só existe enquanto a fonte dos eventos for simulação.
    pub fn aviso_simulacao(&self) -> Option<&'static str> {
        (self.fonte == OrigemEventos::Simulacao).then_some(AVISO_SIMULACAO)
    }

    /// Aviso de faixa: a faixa padrão de testes não representa nenhum teclado medido.
    pub fn aviso_faixa(&self) -> Option<String> {
        if self.faixa_calibrada {
            return None;
        }
        let t = self.teclado();
        Some(format!(
            "Faixa não calibrada: usando a faixa padrão de teste (MIDI {} a {}). \
             A calibração real é da fase F01.",
            t.nota_mais_grave(),
            t.nota_mais_aguda(),
        ))
    }

    // --- Ponto de entrada de qualquer fonte (o gerenciador MIDI chamará isto) ---------------

    /// Aplica um evento de qualquer fonte. A origem do evento define a fonte mostrada na tela:
    /// um evento de dispositivo físico apaga o aviso de simulação; um de simulação o traz de
    /// volta. Nota fora da faixa é reportada e o estado das demais não muda.
    pub fn aplicar_evento(&mut self, evento: EventoTecla) -> Result<(), ErroTeclado> {
        self.fonte = evento.origem;
        match self.teclas.aplicar(evento) {
            Ok(()) => {
                self.erro = None;
                self.registrar(evento);
                Ok(())
            }
            Err(e) => {
                self.erro = Some(self.texto_do_erro(e));
                Err(e)
            }
        }
    }

    /// Apaga tudo (dispositivo desconectado, troca de tela etc.).
    pub fn limpar_tudo(&mut self) {
        self.teclas.limpar_tudo();
        self.ordem.clear();
        self.nota_do_mouse = None;
        self.notas_do_pc.clear();
    }

    fn registrar(&mut self, evento: EventoTecla) {
        self.ordem.retain(|n| *n != evento.nota);
        match evento.acao {
            AcaoTecla::Ligar => self.ordem.push(evento.nota),
            AcaoTecla::Desligar => self.ultima_solta = Some(evento.nota),
        }
    }

    fn texto_do_erro(&self, e: ErroTeclado) -> String {
        match e {
            ErroTeclado::ForaDaFaixa { nota } => {
                let t = self.teclado();
                format!(
                    "Nota MIDI {nota} fora da faixa do teclado ({}–{}); ignorada.",
                    t.nota_mais_grave(),
                    t.nota_mais_aguda()
                )
            }
            ErroTeclado::CalibracaoInvalida { grave } => {
                format!("Calibração inválida (nota mais grave {grave}).")
            }
        }
    }

    // --- Simulação ---------------------------------------------------------------------------

    fn simular(&mut self, acao: AcaoTecla, nota: NotaMidi) {
        match acao {
            AcaoTecla::Ligar => self.simulacao.pressionar(nota),
            AcaoTecla::Desligar => self.simulacao.soltar(nota),
        }
        while let Some(ev) = self.simulacao.proximo_evento() {
            // O erro já fica em `self.erro`; nota fora da faixa não altera nada (nem soa).
            if self.aplicar_evento(ev).is_ok() {
                self.simulados.push(ev);
            }
        }
    }

    /// Entrega (e esvazia) os eventos da simulação aplicados desde a última chamada, na
    /// ordem, para o app tocá-los no motor de áudio.
    pub fn drenar_simulados(&mut self) -> Vec<EventoTecla> {
        std::mem::take(&mut self.simulados)
    }

    pub fn pressionar_simulado(&mut self, nota: NotaMidi) {
        self.simular(AcaoTecla::Ligar, nota);
    }

    pub fn soltar_simulado(&mut self, nota: NotaMidi) {
        self.simular(AcaoTecla::Desligar, nota);
    }

    /// Informa qual tecla está sob o mouse com o botão primário apertado (`None` = nenhuma,
    /// ou botão solto). Arrastar de uma tecla a outra solta a primeira e liga a segunda.
    pub fn atualizar_mouse(&mut self, alvo: Option<usize>) {
        let nova = alvo.and_then(|i| self.teclado().nota_de(i));
        if nova == self.nota_do_mouse {
            return;
        }
        if let Some(antiga) = self.nota_do_mouse.take() {
            self.soltar_simulado(antiga);
        }
        if let Some(n) = nova {
            self.nota_do_mouse = Some(n);
            self.pressionar_simulado(n);
        }
    }

    /// Nota que a tecla do PC toca na oitava-base atual (`None` se não é tecla do mapa ou se a
    /// nota sai de 0..=127).
    pub fn nota_da_tecla_pc(&self, tecla: Key) -> Option<NotaMidi> {
        let (_, semitons, _) = MAPA_PC.iter().find(|(k, _, _)| *k == tecla)?;
        NotaMidi::nova(self.base_pc.checked_add(*semitons)?)
    }

    /// Letra do PC que toca a nota na oitava-base atual, se houver.
    pub fn letra_pc_da_nota(&self, nota: NotaMidi) -> Option<&'static str> {
        let desloc = nota.numero().checked_sub(self.base_pc)?;
        MAPA_PC
            .iter()
            .find(|(_, s, _)| *s == desloc)
            .map(|(_, _, letra)| *letra)
    }

    /// Tecla do PC apertada (sem repetição automática). Z/X descem/sobem a oitava-base.
    pub fn pc_apertou(&mut self, tecla: Key) {
        match tecla {
            Key::Z => self.base_pc = self.base_pc.saturating_sub(12).max(12),
            Key::X => self.base_pc = (self.base_pc + 12).min(108),
            _ => {
                if self.notas_do_pc.iter().any(|(k, _)| *k == tecla) {
                    return;
                }
                if let Some(nota) = self.nota_da_tecla_pc(tecla) {
                    self.notas_do_pc.push((tecla, nota));
                    self.pressionar_simulado(nota);
                }
            }
        }
    }

    /// Tecla do PC solta: solta a nota que ela ligou (mesmo que a oitava-base tenha mudado).
    pub fn pc_soltou(&mut self, tecla: Key) {
        if let Some(i) = self.notas_do_pc.iter().position(|(k, _)| *k == tecla) {
            let (_, nota) = self.notas_do_pc.remove(i);
            self.soltar_simulado(nota);
        }
    }

    /// Solta tudo o que a simulação (mouse e teclado do PC) segura: perda de foco da janela ou
    /// saída da tela. Notas de outras fontes ficam como estão.
    pub fn liberar_simulacao(&mut self) {
        // Perda de foco ou saída da tela: o bend volta ao centro na hora.
        self.roda_bend.zerar();
        if let Some(n) = self.nota_do_mouse.take() {
            self.soltar_simulado(n);
        }
        for (_, n) in std::mem::take(&mut self.notas_do_pc) {
            self.soltar_simulado(n);
        }
    }

    // --- Leitura para a tela -----------------------------------------------------------------

    /// Nota que o painel mostra: a última ainda pressionada, ou, se nada está pressionado, a
    /// última que foi solta.
    pub fn nota_atual(&self) -> Option<InfoNota> {
        let (nota, pressionada) = match self.ordem.last() {
            Some(n) => (*n, true),
            None => (self.ultima_solta?, false),
        };
        let pt = ConfigNomes {
            idioma: Idioma::PtBr,
            ..self.nomes
        };
        let en = ConfigNomes {
            idioma: Idioma::Internacional,
            ..self.nomes
        };
        Some(InfoNota {
            nota,
            nome: nome_sem_oitava(nota, self.nomes),
            nome_pt: nome_sem_oitava(nota, pt),
            nome_internacional: nome_sem_oitava(nota, en),
            rotulo: rotulo(nota, self.nomes),
            midi: nota.numero(),
            oitava: self.nomes.convencao.oitava(nota),
            frequencia_hz: nota.frequencia_hz(LA4_HZ).unwrap_or(0.0),
            pressionada,
        })
    }

    /// Rótulos das notas ligadas, em ordem de altura (acordes inclusive).
    pub fn rotulos_ativos(&self) -> Vec<String> {
        self.teclas
            .notas_ligadas()
            .map(|n| rotulo(n, self.nomes))
            .collect()
    }
}

/// Formata a frequência em PT-BR ("261,63 Hz").
pub fn texto_frequencia(hz: f64) -> String {
    format!("{hz:.2} Hz").replace('.', ",")
}

#[cfg(test)]
mod testes {
    use super::*;
    use piano_tutor_core::teclado::{Acidente, ConvencaoOitava};

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota válida")
    }

    #[test]
    fn o_teclado_resolve_o_dedo_das_guias_e_respeita_o_informado() {
        let mut e = EstadoEspelho::novo();
        e.definir_guias(vec![n(64), n(67), n(72)], Vec::new());
        let d: Vec<u8> = e.dedos_das_guias(None).iter().map(|x| x.1).collect();
        assert_eq!(d, vec![1, 2, 5], "1ª inversão, mão direita");
        let d: Vec<u8> = e.dedos_das_guias(Some(80)).iter().map(|x| x.1).collect();
        assert_eq!(d, vec![5, 3, 1], "abaixo da divisão: mão esquerda");
        e.definir_guias(vec![n(60)], vec![(n(60), 2)]);
        assert_eq!(e.dedos_das_guias(None), vec![(n(60), 2)]);
        e.limpar_guias();
        assert!(e.dedos_das_guias(None).is_empty());
    }

    fn padrao() -> Teclado61 {
        Teclado61::padrao_para_testes_e_simulacao()
    }

    #[test]
    fn geometria_tem_61_teclas_36_brancas_e_25_pretas_em_ordem() {
        let g = geometria(padrao());
        assert_eq!(g.len(), 61);
        assert_eq!(g.iter().filter(|t| t.cor == CorTecla::Branca).count(), 36);
        assert_eq!(g.iter().filter(|t| t.cor == CorTecla::Preta).count(), 25);
        for (i, t) in g.iter().enumerate() {
            assert_eq!(t.indice, i);
            assert_eq!(usize::from(t.nota.numero()), 36 + i);
        }
    }

    #[test]
    fn brancas_sao_contiguas_e_pretas_ficam_na_divisa_das_vizinhas() {
        let g = geometria(padrao());
        let brancas: Vec<&GeometriaTecla> =
            g.iter().filter(|t| t.cor == CorTecla::Branca).collect();
        for (k, b) in brancas.iter().enumerate() {
            assert_eq!(b.x, k as f32);
            assert_eq!(b.largura, 1.0);
        }
        for p in g.iter().filter(|t| t.cor == CorTecla::Preta) {
            let antes = g[p.indice - 1];
            let depois = g[p.indice + 1];
            assert_eq!(antes.cor, CorTecla::Branca, "{}", p.nota.numero());
            assert_eq!(depois.cor, CorTecla::Branca, "{}", p.nota.numero());
            let divisa = antes.x + 1.0;
            assert_eq!(depois.x, divisa);
            assert!((p.centro_x() - divisa).abs() < 1e-6);
            assert!(p.altura < antes.altura);
            assert!(p.largura < antes.largura);
        }
    }

    #[test]
    fn padrao_de_grupos_pretos_2_3_confere_com_o_piano() {
        // Entre Dó e Si há 5 pretas: Dó♯ Ré♯ | Fá♯ Sol♯ Lá♯ (sem preta entre Mi-Fá e Si-Dó).
        let g = geometria(padrao());
        let classes_pretas: Vec<u8> = g
            .iter()
            .filter(|t| t.cor == CorTecla::Preta)
            .take(5)
            .map(|t| t.nota.numero() % 12)
            .collect();
        assert_eq!(classes_pretas, vec![1, 3, 6, 8, 10]);
        // Sem preta entre Mi (4) e Fá (5): brancas coladas.
        for t in g.iter().filter(|t| t.nota.numero() % 12 == 5) {
            let mi = g[t.indice - 1];
            assert_eq!(mi.cor, CorTecla::Branca);
            assert_eq!(mi.x + 1.0, t.x);
        }
    }

    #[test]
    fn hit_test_acha_cada_uma_das_61_teclas_no_proprio_centro() {
        let g = geometria(padrao());
        for t in &g {
            let y = match t.cor {
                CorTecla::Preta => t.altura * 0.5,
                // abaixo das pretas, para não cair numa preta
                CorTecla::Branca => ALTURA_PRETA + (t.altura - ALTURA_PRETA) * 0.5,
            };
            assert_eq!(
                tecla_em(&g, t.centro_x(), y),
                Some(t.indice),
                "nota {}",
                t.nota.numero()
            );
        }
    }

    #[test]
    fn hit_test_preta_tem_prioridade_e_fora_nao_acha_nada() {
        let g = geometria(padrao());
        // Dó♯ (índice 1) fica na divisa x=1; perto do topo pega a preta, embaixo a branca.
        assert_eq!(tecla_em(&g, 1.0, 1.0), Some(1));
        assert_eq!(tecla_em(&g, 0.9, ALTURA_PRETA + 0.5), Some(0));
        assert_eq!(tecla_em(&g, -0.1, 1.0), None);
        assert_eq!(tecla_em(&g, 100.0, 1.0), None);
        assert_eq!(tecla_em(&g, 1.0, ALTURA_BRANCA + 0.1), None);
        assert_eq!(tecla_em(&g, 1.0, -0.1), None);
    }

    #[test]
    fn faixa_que_comeca_ou_termina_em_preta_cabe_na_margem() {
        // grave = 37 (Dó♯2): a primeira tecla é preta.
        let t = Teclado61::calibrado(37).expect("faixa válida");
        let g = geometria(t);
        assert_eq!(g[0].cor, CorTecla::Preta);
        assert!(g[0].x >= -MARGEM - 1e-6);
        let ultima = g.last().expect("61 teclas");
        assert!(ultima.x + ultima.largura <= largura_total(t) - MARGEM + 1e-6);
    }

    #[test]
    fn do_central_dentro_da_faixa_padrao_e_na_branca_certa() {
        let t = padrao();
        let i = t.indice_do_central().expect("60 está em 36..=96");
        assert_eq!(geometria(t)[i].nota.numero(), 60);
        let fora = Teclado61::calibrado(67).expect("faixa válida");
        assert_eq!(fora.indice_do_central(), None);
    }

    #[test]
    fn cores_de_tecla_ligada_tem_contraste_de_pelo_menos_3_para_1() {
        assert!(contraste(BRANCA_PRESSIONADA, BRANCA) >= 3.0);
        assert!(contraste(PRETA_PRESSIONADA, PRETA) >= 3.0);
        assert!(contraste(CONTORNO_BRANCA_PRESSIONADA, BRANCA) >= 3.0);
        assert!(contraste(CONTORNO_PRETA_PRESSIONADA, PRETA_PRESSIONADA) >= 3.0);
        assert!(contraste(BRANCA, PRETA) > 10.0);
    }

    #[test]
    fn comeca_em_simulacao_sem_calibracao_e_sem_nota_atual() {
        let e = EstadoEspelho::novo();
        assert_eq!(
            e.aviso_simulacao(),
            Some("SIMULAÇÃO — não é o teclado físico")
        );
        assert!(!e.faixa_calibrada());
        let aviso = e.aviso_faixa().expect("faixa não calibrada");
        assert!(aviso.contains("não calibrada"), "{aviso}");
        assert!(aviso.contains("36"), "{aviso}");
        assert!(e.nota_atual().is_none());
        assert_eq!(e.teclas().quantidade(), 0);
        assert!(EstadoEspelho::calibrado(padrao()).aviso_faixa().is_none());
    }

    #[test]
    fn evento_fisico_apaga_o_aviso_e_evento_simulado_o_traz_de_volta() {
        let mut e = EstadoEspelho::novo();
        e.aplicar_evento(EventoTecla {
            origem: OrigemEventos::DispositivoFisico,
            acao: AcaoTecla::Ligar,
            nota: n(60),
        })
        .expect("na faixa");
        assert_eq!(e.fonte(), OrigemEventos::DispositivoFisico);
        assert_eq!(e.aviso_simulacao(), None);
        e.pressionar_simulado(n(62));
        assert!(e.aviso_simulacao().is_some());
        assert!(e.teclas().esta_ligada(n(60)), "a nota física segue ligada");
    }

    #[test]
    fn aplicar_evento_liga_desliga_e_mantem_as_outras() {
        let mut e = EstadoEspelho::novo();
        for x in 48..58 {
            e.pressionar_simulado(n(x));
        }
        assert_eq!(e.teclas().quantidade(), 10);
        e.soltar_simulado(n(52));
        assert_eq!(e.teclas().quantidade(), 9);
        assert!(!e.teclas().esta_ligada(n(52)));
        assert!(e.teclas().esta_ligada(n(51)) && e.teclas().esta_ligada(n(53)));
    }

    #[test]
    fn nota_fora_da_faixa_e_reportada_sem_mexer_nas_outras() {
        let mut e = EstadoEspelho::novo();
        e.pressionar_simulado(n(60));
        e.pressionar_simulado(n(20));
        assert_eq!(e.teclas().quantidade(), 1);
        let msg = e.erro().expect("erro registrado");
        assert!(msg.contains("20") && msg.contains("36"), "{msg}");
        e.pressionar_simulado(n(61));
        assert!(e.erro().is_none());
    }

    #[test]
    fn nota_atual_segue_a_ultima_pressionada_e_cai_para_a_anterior_ao_soltar() {
        let mut e = EstadoEspelho::novo();
        e.pressionar_simulado(n(60));
        e.pressionar_simulado(n(64));
        let a = e.nota_atual().expect("há nota");
        assert_eq!((a.midi, a.pressionada), (64, true));
        e.soltar_simulado(n(64));
        let b = e.nota_atual().expect("há nota");
        assert_eq!((b.midi, b.pressionada), (60, true));
        e.soltar_simulado(n(60));
        let c = e.nota_atual().expect("última solta");
        assert_eq!((c.midi, c.pressionada), (60, false));
    }

    #[test]
    fn info_da_nota_traz_nome_midi_oitava_e_frequencia() {
        let mut e = EstadoEspelho::novo();
        e.pressionar_simulado(n(60));
        let i = e.nota_atual().expect("há nota");
        assert_eq!(i.rotulo, "Dó4");
        assert_eq!((i.nome, i.nome_pt, i.nome_internacional), ("Dó", "Dó", "C"));
        assert_eq!((i.midi, i.oitava), (60, 4));
        assert!((i.frequencia_hz - 261.625565).abs() < 1e-3);
        assert_eq!(texto_frequencia(i.frequencia_hz), "261,63 Hz");
    }

    #[test]
    fn config_de_nomes_muda_o_rotulo_mas_nunca_o_numero_midi() {
        let mut e = EstadoEspelho::novo();
        e.pressionar_simulado(n(61));
        assert_eq!(e.nota_atual().expect("nota").rotulo, "Dó♯4");
        e.nomes.convencao = ConvencaoOitava::Do3;
        e.nomes.idioma = Idioma::Internacional;
        e.nomes.acidente = Acidente::Bemol;
        let i = e.nota_atual().expect("nota");
        assert_eq!(i.rotulo, "D♭3");
        assert_eq!((i.midi, i.oitava), (61, 3));
        assert_eq!((i.nome_pt, i.nome_internacional), ("Ré♭", "D♭"));
        assert!(e.teclas().esta_ligada(n(61)));
    }

    #[test]
    fn rotulos_ativos_listam_o_acorde_em_ordem_de_altura() {
        let mut e = EstadoEspelho::novo();
        for x in [67, 60, 64] {
            e.pressionar_simulado(n(x));
        }
        assert_eq!(e.rotulos_ativos(), vec!["Dó4", "Mi4", "Sol4"]);
    }

    #[test]
    fn mouse_arrastar_solta_a_anterior_e_liga_a_nova() {
        let mut e = EstadoEspelho::novo();
        let t = e.teclado();
        let i60 = t.indice_de(n(60)).expect("na faixa");
        e.atualizar_mouse(Some(i60));
        assert!(e.teclas().esta_ligada(n(60)));
        e.atualizar_mouse(Some(i60 + 2));
        assert!(!e.teclas().esta_ligada(n(60)));
        assert!(e.teclas().esta_ligada(n(62)));
        e.atualizar_mouse(None);
        assert_eq!(e.teclas().quantidade(), 0);
    }

    #[test]
    fn mapa_do_pc_tem_brancas_e_pretas_corretas_na_oitava_base() {
        let e = EstadoEspelho::novo();
        let esperado = [
            (Key::A, 60),
            (Key::W, 61),
            (Key::S, 62),
            (Key::E, 63),
            (Key::D, 64),
            (Key::F, 65),
            (Key::T, 66),
            (Key::G, 67),
            (Key::Y, 68),
            (Key::H, 69),
            (Key::U, 70),
            (Key::J, 71),
            (Key::K, 72),
            (Key::O, 73),
            (Key::L, 74),
            (Key::P, 75),
            (Key::Semicolon, 76),
        ];
        assert_eq!(esperado.len(), MAPA_PC.len());
        for (k, midi) in esperado {
            let nota = e.nota_da_tecla_pc(k).expect("mapeada");
            assert_eq!(nota.numero(), midi, "{k:?}");
            let cor = piano_tutor_core::teclado::cor_da_nota(nota);
            let letra_preta = matches!(
                k,
                Key::W | Key::E | Key::T | Key::Y | Key::U | Key::O | Key::P
            );
            assert_eq!(cor == CorTecla::Preta, letra_preta, "{k:?}");
            assert!(e.letra_pc_da_nota(nota).is_some());
        }
        assert_eq!(e.nota_da_tecla_pc(Key::Q), None);
    }

    #[test]
    fn pc_liga_solta_e_ignora_repeticao_da_mesma_tecla() {
        let mut e = EstadoEspelho::novo();
        e.pc_apertou(Key::A);
        e.pc_apertou(Key::A);
        assert_eq!(e.teclas().quantidade(), 1);
        e.pc_apertou(Key::D);
        e.pc_soltou(Key::A);
        assert!(!e.teclas().esta_ligada(n(60)));
        assert!(e.teclas().esta_ligada(n(64)));
        e.pc_soltou(Key::A);
        assert_eq!(e.teclas().quantidade(), 1);
    }

    #[test]
    fn z_e_x_mudam_a_oitava_e_a_tecla_solta_a_nota_que_ligou() {
        let mut e = EstadoEspelho::novo();
        e.pc_apertou(Key::A);
        e.pc_apertou(Key::X);
        assert_eq!(e.base_pc(), 72);
        e.pc_soltou(Key::A);
        assert_eq!(
            e.teclas().quantidade(),
            0,
            "soltou a nota de 60, não a de 72"
        );
        for _ in 0..20 {
            e.pc_apertou(Key::Z);
        }
        assert_eq!(e.base_pc(), 12);
        for _ in 0..20 {
            e.pc_apertou(Key::X);
        }
        assert_eq!(e.base_pc(), 108);
    }

    #[test]
    fn pc_fora_da_faixa_e_do_midi_nao_liga_nada_nem_quebra() {
        let mut e = EstadoEspelho::novo();
        for _ in 0..20 {
            e.pc_apertou(Key::X);
        }
        e.pc_apertou(Key::Semicolon); // 108 + 16 = 124: MIDI válido, fora da faixa do teclado
        e.pc_apertou(Key::P); // 123
        assert_eq!(e.teclas().quantidade(), 0);
        assert!(e.erro().is_some());
    }

    #[test]
    fn liberar_simulacao_solta_mouse_e_pc_mas_preserva_nota_fisica() {
        let mut e = EstadoEspelho::novo();
        e.aplicar_evento(EventoTecla {
            origem: OrigemEventos::DispositivoFisico,
            acao: AcaoTecla::Ligar,
            nota: n(48),
        })
        .expect("na faixa");
        e.pc_apertou(Key::A);
        e.atualizar_mouse(Some(10));
        assert_eq!(e.teclas().quantidade(), 3);
        e.liberar_simulacao();
        assert_eq!(e.teclas().quantidade(), 1);
        assert!(e.teclas().esta_ligada(n(48)));
    }

    #[test]
    fn limpar_tudo_apaga_estado_e_ordem_mas_lembra_a_ultima_nota_solta() {
        let mut e = EstadoEspelho::novo();
        e.pressionar_simulado(n(60));
        e.soltar_simulado(n(60));
        e.pressionar_simulado(n(62));
        e.limpar_tudo();
        assert_eq!(e.teclas().quantidade(), 0);
        let i = e.nota_atual().expect("última solta");
        assert!(!i.pressionada);
    }

    #[test]
    fn simulacao_entrega_os_eventos_aplicados_para_o_som_e_ignora_fora_da_faixa() {
        use piano_tutor_core::teclado::OrigemEventos;
        let mut e = EstadoEspelho::novo();
        e.pressionar_simulado(n(60));
        e.pressionar_simulado(n(10)); // fora da faixa 36–96: nem liga nem soa
        e.soltar_simulado(n(60));
        let ev = e.drenar_simulados();
        let resumo: Vec<(AcaoTecla, u8)> = ev.iter().map(|x| (x.acao, x.nota.numero())).collect();
        assert_eq!(
            resumo,
            vec![(AcaoTecla::Ligar, 60), (AcaoTecla::Desligar, 60)]
        );
        assert!(ev.iter().all(|x| x.origem == OrigemEventos::Simulacao));
        assert!(e.drenar_simulados().is_empty(), "drenar esvazia");
        // Evento físico não entra na fila da simulação (o app já o leva ao som).
        let fisico = EventoTecla {
            origem: OrigemEventos::DispositivoFisico,
            acao: AcaoTecla::Ligar,
            nota: n(62),
        };
        assert!(e.aplicar_evento(fisico).is_ok());
        assert!(e.drenar_simulados().is_empty());
    }

    #[test]
    fn zoom_e_limitado_e_ignora_valor_invalido() {
        let mut e = EstadoEspelho::novo();
        e.definir_zoom(10.0);
        assert_eq!(e.zoom(), ZOOM_MAX);
        e.definir_zoom(0.01);
        assert_eq!(e.zoom(), ZOOM_MIN);
        e.definir_zoom(f32::NAN);
        assert_eq!(e.zoom(), 1.0);
    }
}
