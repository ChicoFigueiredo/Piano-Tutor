//! Calibração do teclado físico (F01): mede a nota mais grave e a mais aguda, confere as 61
//! teclas uma a uma, testa velocity e pedal, e grava um perfil. Nada é presumido: o que o
//! dispositivo não transmitir aparece como "não suportado pelo dispositivo".
//!
//! Lógica pura (sem janela): recebe [`EventoMidi`] e expõe o estado para a tela.

use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::EventoMidi;
use piano_tutor_core::teclado::{ConfigNomes, TECLAS, Teclado61, rotulo};
use std::fmt::Write as _;
use std::path::Path;

/// Etapas, na ordem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Etapa {
    Grave,
    Aguda,
    Cobertura,
    Velocidade,
    Pedal,
    Bend,
    Concluida,
}

impl Etapa {
    pub const fn numero(self) -> usize {
        match self {
            Etapa::Grave => 1,
            Etapa::Aguda => 2,
            Etapa::Cobertura => 3,
            Etapa::Velocidade => 4,
            Etapa::Pedal => 5,
            Etapa::Bend => 6,
            Etapa::Concluida => 7,
        }
    }
}

/// Recurso do dispositivo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Suporte {
    NaoTestado,
    Sim,
    NaoSuportado,
}

impl Suporte {
    pub const fn texto(self) -> &'static str {
        match self {
            Suporte::NaoTestado => "não testado",
            Suporte::Sim => "suportado",
            Suporte::NaoSuportado => "não suportado pelo dispositivo",
        }
    }

    fn chave(self) -> &'static str {
        match self {
            Suporte::NaoTestado => "nao-testado",
            Suporte::Sim => "sim",
            Suporte::NaoSuportado => "nao",
        }
    }

    fn de_chave(s: &str) -> Option<Self> {
        match s {
            "nao-testado" => Some(Suporte::NaoTestado),
            "sim" => Some(Suporte::Sim),
            "nao" => Some(Suporte::NaoSuportado),
            _ => None,
        }
    }
}

/// Resultado salvo da calibração.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerfilTeclado {
    pub dispositivo: String,
    pub grave: u8,
    pub velocity: Suporte,
    pub velocity_faixa: Option<(u8, u8)>,
    pub pedal: Suporte,
    /// Roda de pitch bend: se transmite e o curso medido.
    pub bend: Suporte,
    pub bend_curso: Option<CursoBend>,
    /// Teclas conferidas (de 61) e quantas mandaram liberação.
    pub conferidas: usize,
    pub liberacoes: usize,
}

/// Curso medido da roda de pitch bend (valores centrados em zero, `-8192..=8191`). Rodas
/// baratas ficam descentradas, chiam em repouso e não chegam ao fundo; o curso corrige isso.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CursoBend {
    /// Valor em repouso (roda solta).
    pub centro: i16,
    /// Menor e maior valor alcançados.
    pub minimo: i16,
    pub maximo: i16,
    /// Distância do centro tratada como repouso (absorve o chiado).
    pub zona_morta: i16,
}

/// Zona morta mínima: 2 passos da resolução de 7 bits que o REVAS usa (128 unidades).
pub const ZONA_MORTA_MIN: i16 = 256;

impl CursoBend {
    /// Bend corrigido: zona morta em volta do centro e cada lado esticado até o fundo de curso
    /// MIDI. Um lado que não se afastou do centro além da zona morta fica sempre em zero.
    pub fn normalizar(&self, valor: i16) -> i16 {
        let d = i32::from(valor) - i32::from(self.centro);
        let zona = i32::from(self.zona_morta);
        if d.abs() <= zona {
            return 0;
        }
        let (curso, alvo) = if d > 0 {
            (i32::from(self.maximo) - i32::from(self.centro) - zona, 8191)
        } else {
            (i32::from(self.centro) - i32::from(self.minimo) - zona, 8192)
        };
        if curso <= 0 {
            return 0;
        }
        let v = ((d.abs() - zona).min(curso) * alvo + curso / 2) / curso;
        (if d > 0 { v } else { -v }).clamp(-8192, 8191) as i16
    }

    fn para_texto(&self) -> String {
        format!(
            "{},{},{},{}",
            self.centro, self.minimo, self.maximo, self.zona_morta
        )
    }

    fn de_texto(v: &str) -> Option<Self> {
        let mut it = v.split(',').map(|x| x.trim().parse::<i16>());
        let c = Self {
            centro: it.next()?.ok()?,
            minimo: it.next()?.ok()?,
            maximo: it.next()?.ok()?,
            zona_morta: it.next()?.ok()?,
        };
        (it.next().is_none() && c.minimo <= c.centro && c.centro <= c.maximo && c.zona_morta >= 0)
            .then_some(c)
    }
}

impl PerfilTeclado {
    pub fn teclado(&self) -> Option<Teclado61> {
        Teclado61::calibrado(self.grave).ok()
    }

    pub fn aguda(&self) -> u8 {
        self.grave.saturating_add((TECLAS - 1) as u8)
    }

    pub fn para_texto(&self) -> String {
        let mut s = String::from("# Piano Tutor: perfil do teclado v1\n");
        let _ = writeln!(s, "dispositivo={}", self.dispositivo.replace('\n', " "));
        let _ = writeln!(s, "grave={}", self.grave);
        let _ = writeln!(s, "velocity={}", self.velocity.chave());
        if let Some((a, b)) = self.velocity_faixa {
            let _ = writeln!(s, "velocity_faixa={a}-{b}");
        }
        let _ = writeln!(s, "pedal={}", self.pedal.chave());
        let _ = writeln!(s, "bend={}", self.bend.chave());
        if let Some(c) = self.bend_curso {
            let _ = writeln!(s, "bend_curso={}", c.para_texto());
        }
        let _ = writeln!(s, "conferidas={}", self.conferidas);
        let _ = writeln!(s, "liberacoes={}", self.liberacoes);
        s
    }

    pub fn de_texto(t: &str) -> Option<Self> {
        let mut p = PerfilTeclado {
            dispositivo: String::new(),
            grave: 0,
            velocity: Suporte::NaoTestado,
            velocity_faixa: None,
            pedal: Suporte::NaoTestado,
            bend: Suporte::NaoTestado,
            bend_curso: None,
            conferidas: 0,
            liberacoes: 0,
        };
        let mut tem_grave = false;
        for l in t.lines() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let (k, v) = l.split_once('=')?;
            match k {
                "dispositivo" => p.dispositivo = v.to_owned(),
                "grave" => {
                    p.grave = v.parse().ok()?;
                    tem_grave = true;
                }
                "velocity" => p.velocity = Suporte::de_chave(v)?,
                "velocity_faixa" => {
                    let (a, b) = v.split_once('-')?;
                    p.velocity_faixa = Some((a.parse().ok()?, b.parse().ok()?));
                }
                "pedal" => p.pedal = Suporte::de_chave(v)?,
                "bend" => p.bend = Suporte::de_chave(v)?,
                "bend_curso" => p.bend_curso = CursoBend::de_texto(v),
                "conferidas" => p.conferidas = v.parse().ok()?,
                "liberacoes" => p.liberacoes = v.parse().ok()?,
                _ => {}
            }
        }
        (tem_grave && p.teclado().is_some()).then_some(p)
    }

    /// Bend do teclado físico corrigido pelo curso calibrado (sem calibração, inalterado).
    pub fn bend_corrigido(&self, valor: i16) -> i16 {
        self.bend_curso.map_or(valor, |c| c.normalizar(valor))
    }

    /// Lê o perfil salvo (ausente ou inválido = `None`).
    pub fn ler(caminho: &Path) -> Option<Self> {
        let t = std::fs::read_to_string(caminho).ok()?;
        if t.len() > 16 * 1024 {
            return None;
        }
        Self::de_texto(&t)
    }
}

/// Estado do assistente de calibração.
#[derive(Clone, Debug)]
pub struct Calibracao {
    pub etapa: Etapa,
    pub grave: Option<u8>,
    pub aguda: Option<u8>,
    /// Aviso da etapa atual (faixa errada, nota fora da faixa…).
    pub aviso: Option<String>,
    pub dispositivo: String,
    cobertas: [bool; 128],
    liberadas: [bool; 128],
    vel_min: Option<u8>,
    vel_max: Option<u8>,
    notas_velocidade: usize,
    pedal_visto: bool,
    pub pedal: Suporte,
    pub ultima_nota: Option<u8>,
    pub bend: Suporte,
    /// Valores de bend recebidos na etapa do bend, em ordem (até [`MAX_AMOSTRAS_BEND`]).
    amostras_bend: Vec<i16>,
}

/// Amostras guardadas na etapa do bend (a roda manda dezenas por segundo).
const MAX_AMOSTRAS_BEND: usize = 20_000;
/// Últimas amostras usadas para achar o repouso quando o usuário confirma.
const AMOSTRAS_REPOUSO: usize = 24;

impl Default for Calibracao {
    fn default() -> Self {
        Self::nova("")
    }
}

impl Calibracao {
    pub fn nova(dispositivo: &str) -> Self {
        Self {
            etapa: Etapa::Grave,
            grave: None,
            aguda: None,
            aviso: None,
            dispositivo: dispositivo.to_owned(),
            cobertas: [false; 128],
            liberadas: [false; 128],
            vel_min: None,
            vel_max: None,
            notas_velocidade: 0,
            pedal_visto: false,
            pedal: Suporte::NaoTestado,
            ultima_nota: None,
            bend: Suporte::NaoTestado,
            amostras_bend: Vec::new(),
        }
    }

    /// Recomeça do zero.
    pub fn reiniciar(&mut self) {
        *self = Self::nova(&self.dispositivo);
    }

    /// Volta para medir a mais grave de novo.
    pub fn refazer_faixa(&mut self) {
        self.etapa = Etapa::Grave;
        self.grave = None;
        self.aguda = None;
        self.aviso = None;
    }

    /// Recebe um evento do teclado físico.
    pub fn evento(&mut self, ev: &EventoMidi) {
        match *ev {
            EventoMidi::NotaLigada {
                nota, velocidade, ..
            } => self.nota_ligada(nota, velocidade),
            EventoMidi::NotaDesligada { nota, .. } => {
                if self.etapa == Etapa::Cobertura && self.na_faixa(nota.numero()) {
                    self.liberadas[usize::from(nota.numero())] = true;
                }
            }
            EventoMidi::PitchBend { valor, .. } => {
                if self.etapa == Etapa::Bend && self.amostras_bend.len() < MAX_AMOSTRAS_BEND {
                    self.amostras_bend.push(valor);
                    self.bend = Suporte::Sim;
                }
            }
            EventoMidi::ControleMudou { controle: 64, .. } => {
                self.pedal_visto = true;
                if self.etapa == Etapa::Pedal {
                    self.pedal = Suporte::Sim;
                }
            }
            _ => {}
        }
    }

    fn nota_ligada(&mut self, nota: NotaMidi, velocidade: u8) {
        let n = nota.numero();
        self.ultima_nota = Some(n);
        match self.etapa {
            Etapa::Grave => {
                self.grave = Some(n);
                self.aviso = None;
                self.etapa = Etapa::Aguda;
            }
            Etapa::Aguda => {
                let Some(g) = self.grave else {
                    self.etapa = Etapa::Grave;
                    return;
                };
                if n <= g {
                    self.aviso = Some(format!(
                        "A nota {n} não é mais aguda que a mais grave medida ({g}). Toque a \
                         tecla mais à direita do teclado."
                    ));
                    return;
                }
                self.aguda = Some(n);
                let teclas = usize::from(n - g) + 1;
                if teclas == TECLAS {
                    self.aviso = None;
                    self.etapa = Etapa::Cobertura;
                } else {
                    self.aviso = Some(format!(
                        "Medi {teclas} teclas (MIDI {g} a {n}), mas o teclado tem {TECLAS}. \
                         Confira se a oitava/transposição do teclado está no padrão e se você \
                         tocou as teclas das pontas. Toque a mais aguda de novo ou refaça a \
                         mais grave."
                    ));
                }
            }
            Etapa::Cobertura => {
                if self.na_faixa(n) {
                    self.cobertas[usize::from(n)] = true;
                    self.aviso = None;
                } else {
                    self.aviso = Some(format!(
                        "A nota MIDI {n} está fora da faixa medida. A oitava do teclado mudou?"
                    ));
                }
            }
            Etapa::Velocidade => {
                self.notas_velocidade += 1;
                self.vel_min = Some(self.vel_min.map_or(velocidade, |m| m.min(velocidade)));
                self.vel_max = Some(self.vel_max.map_or(velocidade, |m| m.max(velocidade)));
            }
            Etapa::Pedal | Etapa::Bend | Etapa::Concluida => {}
        }
    }

    fn na_faixa(&self, n: u8) -> bool {
        matches!((self.grave, self.aguda), (Some(g), Some(a)) if (g..=a).contains(&n))
    }

    /// Notas da faixa e se cada uma foi conferida.
    pub fn mapa(&self) -> Vec<(u8, bool, bool)> {
        let (Some(g), Some(a)) = (self.grave, self.aguda) else {
            return Vec::new();
        };
        (g..=a)
            .map(|n| {
                let i = usize::from(n);
                (n, self.cobertas[i], self.liberadas[i])
            })
            .collect()
    }

    pub fn conferidas(&self) -> usize {
        self.mapa().iter().filter(|(_, c, _)| *c).count()
    }

    pub fn liberacoes(&self) -> usize {
        self.mapa().iter().filter(|(_, _, l)| *l).count()
    }

    /// Faixa de velocity observada.
    pub fn faixa_velocidade(&self) -> Option<(u8, u8)> {
        Some((self.vel_min?, self.vel_max?))
    }

    pub fn notas_velocidade(&self) -> usize {
        self.notas_velocidade
    }

    /// Veredito de velocity: variou pelo menos 10 em 3 ou mais notas.
    pub fn suporte_velocity(&self) -> Suporte {
        match self.faixa_velocidade() {
            Some((a, b)) if self.notas_velocidade >= 3 && b.saturating_sub(a) >= 10 => Suporte::Sim,
            Some(_) if self.notas_velocidade >= 3 => Suporte::NaoSuportado,
            _ => Suporte::NaoTestado,
        }
    }

    /// Avança para a próxima etapa (botão "Continuar").
    pub fn avancar(&mut self) {
        self.aviso = None;
        self.etapa = match self.etapa {
            Etapa::Grave | Etapa::Aguda => self.etapa,
            Etapa::Cobertura => Etapa::Velocidade,
            Etapa::Velocidade => {
                if self.pedal_visto {
                    self.pedal = Suporte::Sim;
                }
                Etapa::Pedal
            }
            Etapa::Pedal => Etapa::Bend,
            Etapa::Bend | Etapa::Concluida => Etapa::Concluida,
        };
    }

    /// "Não tenho pedal / ele não respondeu".
    pub fn sem_pedal(&mut self) {
        self.pedal = Suporte::NaoSuportado;
        self.etapa = Etapa::Bend;
    }

    /// "O teclado não tem roda de bend / ela não respondeu".
    pub fn sem_bend(&mut self) {
        self.bend = Suporte::NaoSuportado;
        self.amostras_bend.clear();
        self.etapa = Etapa::Concluida;
    }

    /// Quantos valores de bend chegaram na etapa do bend.
    pub fn amostras_bend(&self) -> usize {
        self.amostras_bend.len()
    }

    /// Curso medido até agora: repouso = média das últimas amostras (a roda solta), zona morta
    /// = o dobro do chiado em repouso (no mínimo [`ZONA_MORTA_MIN`]).
    pub fn curso_bend(&self) -> Option<CursoBend> {
        let a = &self.amostras_bend;
        if a.is_empty() {
            return None;
        }
        let repouso = &a[a.len().saturating_sub(AMOSTRAS_REPOUSO)..];
        let soma: i32 = repouso.iter().map(|&v| i32::from(v)).sum();
        let centro = (soma / repouso.len() as i32) as i16;
        let chiado = repouso
            .iter()
            .map(|&v| (i32::from(v) - i32::from(centro)).abs())
            .max()
            .unwrap_or(0);
        let minimo = *a.iter().min()?;
        let maximo = *a.iter().max()?;
        Some(CursoBend {
            centro,
            minimo: minimo.min(centro),
            maximo: maximo.max(centro),
            zona_morta: (chiado * 2).clamp(i32::from(ZONA_MORTA_MIN), 2048) as i16,
        })
    }

    /// O perfil resultante (só depois de medir a faixa de 61 teclas).
    pub fn perfil(&self) -> Option<PerfilTeclado> {
        let g = self.grave?;
        let a = self.aguda?;
        if usize::from(a - g) + 1 != TECLAS {
            return None;
        }
        Some(PerfilTeclado {
            dispositivo: self.dispositivo.clone(),
            grave: g,
            velocity: self.suporte_velocity(),
            velocity_faixa: self.faixa_velocidade(),
            pedal: self.pedal,
            bend: self.bend,
            bend_curso: if self.bend == Suporte::Sim {
                self.curso_bend()
            } else {
                None
            },
            conferidas: self.conferidas(),
            liberacoes: self.liberacoes(),
        })
    }

    /// Relatório `rev-as-calibracao.md` com a tabela tecla ↔ MIDI medida.
    pub fn relatorio(&self, data: &str) -> Option<String> {
        let p = self.perfil()?;
        let nomes = ConfigNomes::default();
        let mut s = String::new();
        let _ = writeln!(s, "# Calibração do teclado\n");
        let _ = writeln!(s, "- Data: {data}");
        let _ = writeln!(s, "- Dispositivo: {}", vazio_ou(&p.dispositivo));
        let _ = writeln!(
            s,
            "- Faixa medida: MIDI {} a {} ({} teclas)",
            p.grave,
            p.aguda(),
            TECLAS
        );
        let _ = writeln!(s, "- Teclas conferidas: {} de {TECLAS}", p.conferidas);
        let _ = writeln!(s, "- Liberação (NoteOff) recebida: {} teclas", p.liberacoes);
        let _ = writeln!(
            s,
            "- Velocity: {}{}",
            p.velocity.texto(),
            p.velocity_faixa
                .map(|(a, b)| format!(" (observado {a} a {b})"))
                .unwrap_or_default()
        );
        let _ = writeln!(s, "- Pedal de sustain (CC64): {}", p.pedal.texto());
        let _ = writeln!(
            s,
            "- Roda de pitch bend: {}{}\n",
            p.bend.texto(),
            p.bend_curso
                .map(|c| format!(
                    " (repouso {}, curso {} a {}, zona morta ±{})",
                    c.centro, c.minimo, c.maximo, c.zona_morta
                ))
                .unwrap_or_default()
        );
        let _ = writeln!(s, "| Tecla | MIDI | Nota | Conferida | Liberação |");
        let _ = writeln!(s, "|---:|---:|---|---|---|");
        for (i, (n, c, l)) in self.mapa().into_iter().enumerate() {
            let nome = NotaMidi::nova(n)
                .map(|x| rotulo(x, nomes))
                .unwrap_or_default();
            let _ = writeln!(
                s,
                "| {} | {n} | {nome} | {} | {} |",
                i + 1,
                if c { "sim" } else { "NÃO VERIFICADO" },
                if l { "sim" } else { "não recebida" }
            );
        }
        Some(s)
    }
}

fn vazio_ou(s: &str) -> &str {
    if s.is_empty() { "(não informado)" } else { s }
}

#[cfg(test)]
mod testes {

    fn bend(valor: i16) -> EventoMidi {
        EventoMidi::PitchBend {
            canal: piano_tutor_core::midi::CanalMidi::novo(0).expect("canal"),
            valor,
            timestamp_monotonico_ns: 0,
        }
    }

    #[test]
    fn curso_do_bend_descentrado_vira_repouso_zero_e_fundo_de_curso() {
        let mut k = Calibracao::nova("REVAS");
        k.etapa = Etapa::Bend;
        // Roda como a do REVAS: vai a -3072 e +2048, e solta chia em volta de -2600.
        for v in [-1000, -3072, -2000, 0, 2048, 500] {
            k.evento(&bend(v));
        }
        for i in 0..30 {
            k.evento(&bend(if i % 2 == 0 { -2560 } else { -2688 }));
        }
        let c = k.curso_bend().expect("curso");
        assert_eq!((c.minimo, c.maximo), (-3072, 2048));
        assert!((c.centro + 2624).abs() <= 1, "{c:?}");
        assert_eq!(c.zona_morta, ZONA_MORTA_MIN);
        assert_eq!(c.normalizar(-2600), 0, "repouso com chiado é zero");
        assert_eq!(c.normalizar(2048), 8191);
        assert_eq!(c.normalizar(-3072), -8192);
        let meio = c.normalizar(-200);
        assert!(meio > 1000 && meio < 8191, "{meio}");
        k.avancar();
        assert_eq!(k.etapa, Etapa::Concluida);
        let p = k.perfil();
        assert!(p.is_none() || p.as_ref().is_some_and(|p| p.bend == Suporte::Sim));
    }

    #[test]
    fn curso_do_bend_vai_e_volta_no_texto_do_perfil() {
        let c = CursoBend {
            centro: -2600,
            minimo: -3072,
            maximo: 2048,
            zona_morta: 256,
        };
        assert_eq!(CursoBend::de_texto(&c.para_texto()), Some(c));
        assert_eq!(CursoBend::de_texto("1,2"), None);
        assert_eq!(
            CursoBend::de_texto("5,10,20,1"),
            None,
            "mínimo acima do centro"
        );
    }

    #[test]
    fn sem_roda_de_bend_conclui_como_nao_suportado() {
        let mut k = Calibracao::nova("X");
        k.etapa = Etapa::Bend;
        k.sem_bend();
        assert_eq!(k.etapa, Etapa::Concluida);
        assert_eq!(k.bend, Suporte::NaoSuportado);
        assert_eq!(k.curso_bend(), None);
    }

    use super::*;
    use piano_tutor_core::midi::CanalMidi;

    fn c() -> CanalMidi {
        CanalMidi::novo(0).expect("canal")
    }
    fn on(n: u8, v: u8) -> EventoMidi {
        EventoMidi::NotaLigada {
            canal: c(),
            nota: NotaMidi::nova(n).expect("nota"),
            velocidade: v,
            timestamp_monotonico_ns: 0,
        }
    }
    fn off(n: u8) -> EventoMidi {
        EventoMidi::NotaDesligada {
            canal: c(),
            nota: NotaMidi::nova(n).expect("nota"),
            velocidade: 0,
            timestamp_monotonico_ns: 0,
        }
    }

    #[test]
    fn fluxo_completo_com_61_teclas_velocity_e_pedal() {
        let mut k = Calibracao::nova("KB-330");
        k.evento(&on(36, 100));
        assert_eq!(k.etapa, Etapa::Aguda);
        k.evento(&on(96, 100));
        assert_eq!(k.etapa, Etapa::Cobertura);
        for n in 36..=96 {
            k.evento(&on(n, 100));
            k.evento(&off(n));
        }
        assert_eq!(k.conferidas(), 61);
        assert_eq!(k.liberacoes(), 61);
        k.avancar();
        assert_eq!(k.etapa, Etapa::Velocidade);
        for v in [20, 64, 120] {
            k.evento(&on(60, v));
        }
        assert_eq!(k.suporte_velocity(), Suporte::Sim);
        k.avancar();
        assert_eq!(k.etapa, Etapa::Pedal);
        k.evento(&EventoMidi::ControleMudou {
            canal: c(),
            controle: 64,
            valor: 127,
            timestamp_monotonico_ns: 0,
        });
        assert_eq!(k.pedal, Suporte::Sim);
        k.avancar();
        let p = k.perfil().expect("perfil");
        assert_eq!(p.grave, 36);
        assert_eq!(p.aguda(), 96);
        assert_eq!(PerfilTeclado::de_texto(&p.para_texto()), Some(p));
        let r = k.relatorio("2026-11-22").expect("relatório");
        assert!(r.contains("| 61 | 96 |"));
        assert!(!r.contains("NÃO VERIFICADO"));
    }

    #[test]
    fn faixa_errada_avisa_e_nao_gera_perfil() {
        let mut k = Calibracao::nova("");
        k.evento(&on(48, 100));
        k.evento(&on(96, 100));
        assert_eq!(k.etapa, Etapa::Aguda);
        assert!(k.aviso.as_deref().unwrap_or("").contains("49 teclas"));
        assert!(k.perfil().is_none());
        k.evento(&on(40, 100));
        assert!(
            k.aviso
                .as_deref()
                .unwrap_or("")
                .contains("não é mais aguda")
        );
        k.refazer_faixa();
        k.evento(&on(36, 100));
        k.evento(&on(96, 100));
        assert_eq!(k.etapa, Etapa::Cobertura);
    }

    #[test]
    fn velocity_constante_e_sem_pedal_viram_nao_suportado() {
        let mut k = Calibracao::nova("");
        k.evento(&on(36, 100));
        k.evento(&on(96, 100));
        k.evento(&on(60, 100));
        k.avancar();
        for _ in 0..4 {
            k.evento(&on(60, 100));
        }
        assert_eq!(k.suporte_velocity(), Suporte::NaoSuportado);
        k.avancar();
        k.sem_pedal();
        let p = k.perfil().expect("perfil");
        assert_eq!(p.pedal, Suporte::NaoSuportado);
        assert_eq!(p.conferidas, 1);
        let r = k.relatorio("hoje").expect("relatório");
        assert!(r.contains("NÃO VERIFICADO"));
        assert!(r.contains("não suportado pelo dispositivo"));
    }

    #[test]
    fn perfil_invalido_nao_carrega() {
        assert_eq!(PerfilTeclado::de_texto("grave=100"), None);
        assert_eq!(PerfilTeclado::de_texto("lixo"), None);
        assert_eq!(PerfilTeclado::de_texto(""), None);
    }
}
