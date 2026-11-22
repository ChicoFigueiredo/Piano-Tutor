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
            Etapa::Concluida => 6,
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
    /// Teclas conferidas (de 61) e quantas mandaram liberação.
    pub conferidas: usize,
    pub liberacoes: usize,
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
                "conferidas" => p.conferidas = v.parse().ok()?,
                "liberacoes" => p.liberacoes = v.parse().ok()?,
                _ => {}
            }
        }
        (tem_grave && p.teclado().is_some()).then_some(p)
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
}

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
            Etapa::Pedal | Etapa::Concluida => {}
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
            Some((a, b)) if self.notas_velocidade >= 3 && b.saturating_sub(a) >= 10 => {
                Suporte::Sim
            }
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
            Etapa::Pedal | Etapa::Concluida => Etapa::Concluida,
        };
    }

    /// "Não tenho pedal / ele não respondeu".
    pub fn sem_pedal(&mut self) {
        self.pedal = Suporte::NaoSuportado;
        self.etapa = Etapa::Concluida;
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
        let _ = writeln!(s, "- Pedal de sustain (CC64): {}\n", p.pedal.texto());
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
        assert!(k.aviso.as_deref().unwrap_or("").contains("não é mais aguda"));
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
