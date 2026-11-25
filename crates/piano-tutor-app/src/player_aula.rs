//! Player de Aula (ADR-0011): a lógica pura que liga o relógio do vídeo da Aula ao Alinhamento
//! do Exercício, decide o que o teclado mostra e o que o Modo de acompanhamento faz quando o
//! aluno toca junto. A tela (`tela_player_aula`) só desenha e repassa entradas; o vídeo em si
//! fica em `video_embutido`.
//!
//! Tempos: `t_video` é o instante no vídeo da Aula; `t_audio = t_video - offset_aula_s` é o
//! instante no áudio do Exercício, que é o eixo do Alinhamento.

use piano_tutor_curso::{Alinhamento, NotaAlinhada};
use serde::{Deserialize, Serialize};

/// Janela (s) em que uma nota do professor ainda conta como "soando agora" no teclado.
pub const JANELA_SOANDO_S: f64 = 0.35;
/// Tolerância (s) do Segue no tempo para contar acerto.
pub const TOLERANCIA_SEGUE_S: f64 = 0.30;
/// Dois toques de "voltar" dentro deste intervalo (s) vão ao trecho anterior.
pub const DUPLO_VOLTAR_S: f64 = 0.6;

/// Como o player reage enquanto o aluno toca junto (glossário: Modo de acompanhamento).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModoAcompanhamento {
    /// Para em cada nota até o aluno tocar a certa.
    #[default]
    Espera,
    /// Não para; pontua acerto e erro no tempo.
    SegueNoTempo,
    /// O professor toca o trecho, o vídeo para no fim, o aluno repete; só avança se acertar.
    ChamadaEResposta,
}

impl ModoAcompanhamento {
    pub const TODOS: [Self; 3] = [Self::Espera, Self::SegueNoTempo, Self::ChamadaEResposta];

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Espera => "Espera",
            Self::SegueNoTempo => "Segue no tempo",
            Self::ChamadaEResposta => "Chamada e resposta",
        }
    }

    pub fn explicacao(self) -> &'static str {
        match self {
            Self::Espera => "O vídeo para em cada nota até você tocar a certa",
            Self::SegueNoTempo => "O vídeo não para; cada nota sua conta como acerto ou erro",
            Self::ChamadaEResposta => {
                "O professor toca o trecho, o vídeo para e você repete; só avança se acertar"
            }
        }
    }

    pub fn proximo(self) -> Self {
        match self {
            Self::Espera => Self::SegueNoTempo,
            Self::SegueNoTempo => Self::ChamadaEResposta,
            Self::ChamadaEResposta => Self::Espera,
        }
    }

    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Espera => "espera",
            Self::SegueNoTempo => "segue",
            Self::ChamadaEResposta => "chamada",
        }
    }

    pub fn de_texto(t: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|m| m.como_texto() == t.trim())
    }
}

/// Um Exercício da Lição visto como trecho da linha do tempo da Aula.
#[derive(Clone, Debug, PartialEq)]
pub struct Trecho {
    pub exercicio_id: String,
    pub titulo: String,
    pub inicio_s: f64,
    pub fim_s: f64,
}

/// O trecho em que `t_video` cai; entre trechos, o último que já começou (ou o primeiro).
pub fn trecho_no_instante(trechos: &[Trecho], t_video: f64) -> Option<usize> {
    if trechos.is_empty() {
        return None;
    }
    let mut melhor: Option<usize> = None;
    for (i, t) in trechos.iter().enumerate() {
        if t.inicio_s <= t_video + 1e-6 {
            match melhor {
                Some(j) if trechos[j].inicio_s > t.inicio_s => {}
                _ => melhor = Some(i),
            }
        }
    }
    melhor.or(Some(0))
}

/// O que a tela deve fazer com o vídeo depois de um passo do player.
#[derive(Clone, Debug, PartialEq)]
pub enum Comando {
    Pausar,
    Continuar,
    Buscar(f64),
}

/// O dedilhado de um Exercício (Hipótese ou Aprovado), indexado pela nota do Alinhamento.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Dedilhado {
    #[serde(default = "formato_um")]
    pub formato_versao: u32,
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub metodo: String,
    #[serde(default)]
    pub notas: Vec<DedoDaNota>,
}

fn formato_um() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct DedoDaNota {
    pub indice: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi: Option<u8>,
    pub dedo: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mao: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confianca: Option<f64>,
    /// Quem pôs este dedo: ausente = a fábrica; `"revisor"` = corrigido à mão no player.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origem: Option<String>,
}

impl Dedilhado {
    pub fn de_json(bytes: &[u8]) -> Result<Self, String> {
        let d: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if d.notas.iter().any(|n| !(1..=5).contains(&n.dedo)) {
            return Err("dedo fora de 1 a 5 no dedilhado".to_owned());
        }
        Ok(d)
    }

    /// Corrige (ou acrescenta) o dedo da nota `indice` à mão: o arquivo volta a ser Hipótese.
    pub fn definir(&mut self, indice: u32, midi: u8, dedo: u8) {
        let dedo = dedo.clamp(1, 5);
        match self.notas.iter_mut().find(|n| n.indice == indice) {
            Some(n) => {
                n.dedo = dedo;
                n.midi = Some(midi);
                n.confianca = Some(1.0);
                n.origem = Some("revisor".into());
            }
            None => {
                self.notas.push(DedoDaNota {
                    indice,
                    midi: Some(midi),
                    dedo,
                    mao: None,
                    confianca: Some(1.0),
                    origem: Some("revisor".into()),
                });
                self.notas.sort_by_key(|n| n.indice);
            }
        }
        self.estado = "hipotese".into();
        if self.metodo.is_empty() {
            self.metodo = "revisor".into();
        }
        self.formato_versao = 1;
    }

    /// De onde veio o dedo da nota: `"revisor"`, `"fábrica"` ou nada.
    pub fn origem(&self, indice: u32) -> Option<&str> {
        self.notas
            .iter()
            .find(|n| n.indice == indice)
            .map(|n| n.origem.as_deref().unwrap_or("fábrica"))
    }

    pub fn dedo(&self, indice: u32) -> Option<u8> {
        self.notas
            .iter()
            .find(|n| n.indice == indice)
            .map(|n| n.dedo)
    }
}

/// O que o teclado mostra num instante: as notas do professor e o dedo de cada uma.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Guias {
    /// (midi, dedo, índice no Alinhamento)
    pub notas: Vec<(u8, Option<u8>, u32)>,
    /// Verdadeiro quando é a próxima nota (ainda não soou), não a que está soando.
    pub antecipada: bool,
}

/// As notas que soam em `t_audio` (dentro da janela); se nenhuma, a próxima a soar.
pub fn guias_no_instante(al: &Alinhamento, dedos: Option<&Dedilhado>, t_audio: f64) -> Guias {
    let dedo = |n: &NotaAlinhada| dedos.and_then(|d| d.dedo(n.indice));
    let soando: Vec<_> = al
        .notas
        .iter()
        .filter(|n| n.t_audio_s <= t_audio + 1e-6 && t_audio - n.t_audio_s < JANELA_SOANDO_S)
        .collect();
    if let Some(ultimo) = soando.iter().map(|n| n.t_audio_s).reduce(f64::max) {
        // Só o ataque mais recente (um acorde tem vários com o mesmo instante).
        let notas = soando
            .iter()
            .filter(|n| (n.t_audio_s - ultimo).abs() < 0.03)
            .map(|n| (n.midi, dedo(n), n.indice))
            .collect();
        return Guias {
            notas,
            antecipada: false,
        };
    }
    let proxima = al
        .notas
        .iter()
        .filter(|n| n.t_audio_s > t_audio)
        .map(|n| n.t_audio_s)
        .reduce(f64::min);
    match proxima {
        Some(t) => Guias {
            notas: al
                .notas
                .iter()
                .filter(|n| (n.t_audio_s - t).abs() < 0.03)
                .map(|n| (n.midi, dedo(n), n.indice))
                .collect(),
            antecipada: true,
        },
        None => Guias::default(),
    }
}

/// Os instantes de ataque (t_audio) do Alinhamento, em ordem, sem repetir os de um acorde.
pub fn ataques(al: &Alinhamento) -> Vec<f64> {
    let mut v: Vec<f64> = al.notas.iter().map(|n| n.t_audio_s).collect();
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 0.03);
    v
}

/// O ataque anterior a `t_audio` (com folga, para dois toques seguidos andarem de nota em nota).
pub fn nota_anterior(al: &Alinhamento, t_audio: f64) -> Option<f64> {
    ataques(al).into_iter().rev().find(|&a| a < t_audio - 0.05)
}

/// O ataque seguinte a `t_audio`.
pub fn proxima_nota(al: &Alinhamento, t_audio: f64) -> Option<f64> {
    ataques(al).into_iter().find(|&a| a > t_audio + 0.05)
}

/// Estado do acompanhamento (o que o aluno já tocou no trecho).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Acompanhamento {
    pub modo: ModoAcompanhamento,
    /// Em Espera: o índice da nota que o vídeo aguarda.
    pub esperando: Option<u32>,
    /// Notas do Alinhamento que o aluno já tocou certo (Espera) neste trecho.
    pub cumpridas: Vec<u32>,
    /// Em Chamada e resposta: quantas notas do trecho o aluno já repetiu na ordem.
    pub repetidas: usize,
    /// Em Chamada e resposta: o vídeo chegou ao fim do trecho e espera a repetição.
    pub aguardando_resposta: bool,
    pub acertos: u32,
    pub erros: u32,
    ultimo_voltar: Option<f64>,
}

impl Acompanhamento {
    pub fn trocar_modo(&mut self, modo: ModoAcompanhamento) {
        *self = Self {
            modo,
            ..Self::default()
        };
    }

    /// Zera o que foi tocado (ao voltar ou ao trocar de trecho), mantendo o modo.
    pub fn recomecar(&mut self) {
        let modo = self.modo;
        let ultimo = self.ultimo_voltar;
        *self = Self {
            modo,
            ultimo_voltar: ultimo,
            ..Self::default()
        };
    }

    /// Um passo por quadro: o vídeo está em `t_video`, tocando (`!pausado`). Devolve o que fazer
    /// com o vídeo.
    pub fn passo(
        &mut self,
        al: &Alinhamento,
        trecho: &Trecho,
        t_video: f64,
        pausado: bool,
    ) -> Option<Comando> {
        let t_audio = t_video - al.offset_aula_s;
        match self.modo {
            ModoAcompanhamento::Espera => {
                if self.esperando.is_some() {
                    return (!pausado).then_some(Comando::Pausar);
                }
                if pausado || t_video < trecho.inicio_s || t_video > trecho.fim_s {
                    return None;
                }
                // A primeira nota já alcançada que o aluno ainda não tocou.
                let alcancada = al
                    .notas
                    .iter()
                    .filter(|n| n.t_audio_s <= t_audio && !self.cumpridas.contains(&n.indice))
                    .filter(|n| n.t_audio_s + al.offset_aula_s >= trecho.inicio_s - 0.05)
                    .min_by(|a, b| a.t_audio_s.total_cmp(&b.t_audio_s))?;
                self.esperando = Some(alcancada.indice);
                Some(Comando::Pausar)
            }
            ModoAcompanhamento::SegueNoTempo => None,
            ModoAcompanhamento::ChamadaEResposta => {
                if self.aguardando_resposta {
                    return (!pausado).then_some(Comando::Pausar);
                }
                if !pausado && t_video >= trecho.fim_s && t_video < trecho.fim_s + 1.0 {
                    self.aguardando_resposta = true;
                    self.repetidas = 0;
                    return Some(Comando::Pausar);
                }
                None
            }
        }
    }

    /// O aluno tocou `midi` com o vídeo em `t_video`. Devolve o que fazer com o vídeo.
    pub fn nota_tocada(
        &mut self,
        al: &Alinhamento,
        trecho: &Trecho,
        proximo: Option<&Trecho>,
        t_video: f64,
        midi: u8,
    ) -> Option<Comando> {
        let t_audio = t_video - al.offset_aula_s;
        match self.modo {
            ModoAcompanhamento::Espera => {
                let i = self.esperando?;
                let alvo = al.notas.iter().find(|n| n.indice == i)?;
                // Num acorde, qualquer nota do mesmo ataque serve; o acorde inteiro libera.
                let mesmo_ataque: Vec<_> = al
                    .notas
                    .iter()
                    .filter(|n| (n.t_audio_s - alvo.t_audio_s).abs() < 0.03)
                    .collect();
                match mesmo_ataque.iter().find(|n| n.midi == midi) {
                    Some(n) => {
                        self.acertos += 1;
                        self.cumpridas.push(n.indice);
                        if mesmo_ataque
                            .iter()
                            .all(|n| self.cumpridas.contains(&n.indice))
                        {
                            self.esperando = None;
                            return Some(Comando::Continuar);
                        }
                        None
                    }
                    None => {
                        self.erros += 1;
                        None
                    }
                }
            }
            ModoAcompanhamento::SegueNoTempo => {
                let perto = al
                    .notas
                    .iter()
                    .any(|n| n.midi == midi && (n.t_audio_s - t_audio).abs() <= TOLERANCIA_SEGUE_S);
                if perto {
                    self.acertos += 1;
                } else {
                    self.erros += 1;
                }
                None
            }
            ModoAcompanhamento::ChamadaEResposta => {
                if !self.aguardando_resposta {
                    return None;
                }
                let sequencia = notas_do_trecho(al, trecho);
                let esperada = sequencia.get(self.repetidas)?;
                if esperada.midi == midi {
                    self.repetidas += 1;
                    self.acertos += 1;
                    if self.repetidas >= sequencia.len() {
                        self.aguardando_resposta = false;
                        self.repetidas = 0;
                        return Some(match proximo {
                            Some(p) => Comando::Buscar(p.inicio_s),
                            None => Comando::Continuar,
                        });
                    }
                } else {
                    // Errou: recomeça a repetição do trecho.
                    self.erros += 1;
                    self.repetidas = 0;
                }
                None
            }
        }
    }

    /// "Voltar": ao início do trecho atual; dois toques seguidos, ao trecho anterior.
    pub fn voltar(&mut self, trechos: &[Trecho], t_video: f64, agora_s: f64) -> Option<Comando> {
        let duplo = self
            .ultimo_voltar
            .is_some_and(|u| agora_s - u >= 0.0 && agora_s - u <= DUPLO_VOLTAR_S);
        self.ultimo_voltar = Some(agora_s);
        let atual = trecho_no_instante(trechos, t_video)?;
        let alvo = if duplo {
            atual.saturating_sub(1)
        } else {
            atual
        };
        self.recomecar();
        // Recomeçar zera o último toque; guarda de novo para um terceiro toque seguir voltando.
        self.ultimo_voltar = Some(agora_s);
        Some(Comando::Buscar(trechos[alvo].inicio_s))
    }
}

/// As notas do Alinhamento que caem dentro do trecho, na ordem de ataque.
pub fn notas_do_trecho<'a>(al: &'a Alinhamento, trecho: &Trecho) -> Vec<&'a NotaAlinhada> {
    let mut v: Vec<_> = al
        .notas
        .iter()
        .filter(|n| {
            let t = n.t_audio_s + al.offset_aula_s;
            t >= trecho.inicio_s - 0.05 && t <= trecho.fim_s + 0.05
        })
        .collect();
    v.sort_by(|a, b| {
        a.t_audio_s
            .total_cmp(&b.t_audio_s)
            .then(a.indice.cmp(&b.indice))
    });
    v
}

#[cfg(test)]
mod testes {
    use super::*;
    use piano_tutor_curso::EstadoConteudo;

    fn nota(indice: u32, midi: u8, t: f64) -> NotaAlinhada {
        NotaAlinhada {
            indice,
            compasso: 1,
            midi,
            t_partitura_s: t,
            t_audio_s: t,
            confianca: 1.0,
        }
    }

    /// Dó-Ré-Mi em 1 s, 2 s e 3 s do áudio; o áudio começa aos 100 s da Aula.
    fn al() -> Alinhamento {
        Alinhamento {
            formato_versao: 1,
            exercicio: "x".into(),
            audio: None,
            estado: EstadoConteudo::Hipotese,
            metodo: "teste".into(),
            offset_aula_s: 100.0,
            notas: vec![nota(0, 60, 1.0), nota(1, 62, 2.0), nota(2, 64, 3.0)],
        }
    }

    fn trecho(id: &str, i: f64, f: f64) -> Trecho {
        Trecho {
            exercicio_id: id.into(),
            titulo: id.into(),
            inicio_s: i,
            fim_s: f,
        }
    }

    #[test]
    fn modo_gira_e_volta_do_texto() {
        let mut m = ModoAcompanhamento::Espera;
        for _ in 0..3 {
            m = m.proximo();
            assert_eq!(ModoAcompanhamento::de_texto(m.como_texto()), Some(m));
        }
        assert_eq!(m, ModoAcompanhamento::Espera);
    }

    #[test]
    fn trecho_no_instante_pega_o_ultimo_que_comecou() {
        let t = [trecho("a", 10.0, 20.0), trecho("b", 30.0, 40.0)];
        assert_eq!(trecho_no_instante(&t, 5.0), Some(0));
        assert_eq!(trecho_no_instante(&t, 15.0), Some(0));
        assert_eq!(trecho_no_instante(&t, 25.0), Some(0));
        assert_eq!(trecho_no_instante(&t, 35.0), Some(1));
        assert_eq!(trecho_no_instante(&[], 35.0), None);
    }

    #[test]
    fn guias_mostram_a_nota_soando_e_antecipam_a_proxima() {
        let a = al();
        let d = Dedilhado {
            estado: "hipotese".into(),
            notas: vec![DedoDaNota {
                indice: 1,
                midi: Some(62),
                dedo: 2,
                mao: None,
                confianca: None,
                origem: None,
            }],
            ..Dedilhado::default()
        };
        let g = guias_no_instante(&a, Some(&d), 2.1);
        assert_eq!(g.notas, vec![(62, Some(2), 1)]);
        assert!(!g.antecipada);
        let g = guias_no_instante(&a, Some(&d), 2.6);
        assert_eq!(g.notas, vec![(64, None, 2)]);
        assert!(g.antecipada);
        assert!(guias_no_instante(&a, None, 9.0).notas.is_empty());
    }

    #[test]
    fn espera_pausa_na_nota_e_continua_quando_o_aluno_acerta() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = Acompanhamento::default();
        assert_eq!(ac.passo(&a, &tr, 100.8, false), None);
        assert_eq!(ac.passo(&a, &tr, 101.02, false), Some(Comando::Pausar));
        assert_eq!(ac.esperando, Some(0));
        // Errou: continua esperando.
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.02, 61), None);
        assert_eq!(ac.passo(&a, &tr, 101.02, true), None);
        assert_eq!(
            ac.nota_tocada(&a, &tr, None, 101.02, 60),
            Some(Comando::Continuar)
        );
        assert_eq!((ac.acertos, ac.erros), (1, 1));
        // Não volta a parar na mesma nota.
        assert_eq!(ac.passo(&a, &tr, 101.2, false), None);
        assert_eq!(ac.passo(&a, &tr, 102.01, false), Some(Comando::Pausar));
        assert_eq!(ac.esperando, Some(1));
    }

    #[test]
    fn segue_no_tempo_pontua_sem_parar() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = Acompanhamento::default();
        ac.trocar_modo(ModoAcompanhamento::SegueNoTempo);
        assert_eq!(ac.passo(&a, &tr, 101.0, false), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.1, 60), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.1, 64), None);
        assert_eq!((ac.acertos, ac.erros), (1, 1));
    }

    #[test]
    fn chamada_para_no_fim_e_so_avanca_com_a_repeticao_certa() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let prox = trecho("y", 110.0, 120.0);
        let mut ac = Acompanhamento::default();
        ac.trocar_modo(ModoAcompanhamento::ChamadaEResposta);
        assert_eq!(ac.passo(&a, &tr, 102.0, false), None);
        assert_eq!(ac.passo(&a, &tr, 103.52, false), Some(Comando::Pausar));
        assert_eq!(ac.nota_tocada(&a, &tr, Some(&prox), 103.52, 60), None);
        // Errou a segunda: recomeça.
        assert_eq!(ac.nota_tocada(&a, &tr, Some(&prox), 103.52, 64), None);
        assert_eq!(ac.repetidas, 0);
        for m in [60, 62] {
            assert_eq!(ac.nota_tocada(&a, &tr, Some(&prox), 103.52, m), None);
        }
        assert_eq!(
            ac.nota_tocada(&a, &tr, Some(&prox), 103.52, 64),
            Some(Comando::Buscar(110.0))
        );
        assert!(!ac.aguardando_resposta);
    }

    #[test]
    fn voltar_vai_ao_inicio_do_trecho_e_duplo_ao_anterior() {
        let t = [trecho("a", 10.0, 20.0), trecho("b", 30.0, 40.0)];
        let mut ac = Acompanhamento::default();
        assert_eq!(ac.voltar(&t, 35.0, 1.0), Some(Comando::Buscar(30.0)));
        assert_eq!(ac.voltar(&t, 30.0, 1.3), Some(Comando::Buscar(10.0)));
        assert_eq!(ac.voltar(&t, 35.0, 5.0), Some(Comando::Buscar(30.0)));
    }

    #[test]
    fn anda_de_nota_em_nota() {
        let a = al();
        assert_eq!(proxima_nota(&a, 0.0), Some(1.0));
        assert_eq!(proxima_nota(&a, 1.01), Some(2.0));
        assert_eq!(nota_anterior(&a, 2.01), Some(1.0));
        assert_eq!(nota_anterior(&a, 1.0), None);
        assert_eq!(proxima_nota(&a, 3.0), None);
    }

    #[test]
    fn corrigir_dedo_marca_revisor_e_volta_a_hipotese() {
        let mut d = Dedilhado::de_json(br#"{"estado":"aprovada","notas":[{"indice":0,"dedo":1}]}"#)
            .expect("lê");
        assert_eq!(d.origem(0), Some("fábrica"));
        d.definir(0, 60, 2);
        d.definir(3, 65, 4);
        assert_eq!((d.dedo(0), d.dedo(3)), (Some(2), Some(4)));
        assert_eq!(d.origem(0), Some("revisor"));
        assert_eq!(d.estado, "hipotese");
        let volta =
            Dedilhado::de_json(serde_json::to_string(&d).expect("json").as_bytes()).expect("relê");
        assert_eq!(volta, d);
    }

    #[test]
    fn dedilhado_le_json_e_recusa_dedo_invalido() {
        let ok = br#"{"estado":"hipotese","notas":[{"indice":0,"midi":60,"dedo":1,"mao":"direita","confianca":0.9}]}"#;
        assert_eq!(Dedilhado::de_json(ok).expect("lê").dedo(0), Some(1));
        let ruim = br#"{"notas":[{"indice":0,"dedo":7}]}"#;
        assert!(Dedilhado::de_json(ruim).is_err());
    }
}
