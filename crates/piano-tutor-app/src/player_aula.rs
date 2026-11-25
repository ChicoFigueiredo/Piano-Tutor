//! Player de Aula (ADR-0011): a lógica pura que liga o relógio do vídeo da Aula ao Alinhamento
//! do Exercício, decide o que o teclado mostra e o que o Modo de acompanhamento faz quando o
//! aluno toca junto. A tela (`tela_player_aula`) só desenha e repassa entradas; o vídeo em si
//! fica em `video_embutido`.
//!
//! Tempos: `t_video` é o instante no vídeo da Aula; `t_audio = t_video - offset_aula_s` é o
//! instante no áudio do Exercício, que é o eixo do Alinhamento.

use piano_tutor_core::comparador::EventoEsperado;
use piano_tutor_core::pratica::{BATIDAS_RETOMADA, ConfigPratica, ModoPratica, Pratica, Retomada};
use piano_tutor_curso::{Alinhamento, NotaAlinhada};
use serde::{Deserialize, Serialize};

/// Janela (s) em que uma nota do professor ainda conta como "soando agora" no teclado.
pub const JANELA_SOANDO_S: f64 = 0.35;
/// Tolerância (s) do Segue no tempo para contar acerto.
pub const TOLERANCIA_SEGUE_S: f64 = 0.30;
/// Dois toques de "voltar" dentro deste intervalo (s) vão ao trecho anterior.
pub const DUPLO_VOLTAR_S: f64 = 0.6;
/// Espera tocar: quadros (passos) que o player espera o vídeo chegar a um instante buscado antes
/// de desistir e seguir com a posição que o vídeo informa.
const QUADROS_ATE_CHEGAR: u32 = 90;
/// Espera tocar: o vídeo "chegou" a um instante buscado quando está a menos disto (s) dele.
const CHEGADA_S: f64 = 0.6;
/// Espera tocar: duração (s) de uma batida quando nem o andamento do Exercício nem os ataques do
/// trecho dizem nada.
const BATIDA_PADRAO_S: f64 = 0.5;

/// Como o player reage enquanto o aluno toca junto (glossário: Modo de acompanhamento).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModoAcompanhamento {
    /// Para em cada nota até o aluno tocar a certa, sem andamento (nome antigo: "Espera").
    #[default]
    NotaANota,
    /// Anda no tempo do vídeo e só para quando a janela de uma nota passa sem ela.
    EsperaTocar,
    /// Não para; pontua acerto e erro no tempo.
    SegueNoTempo,
    /// O professor toca o trecho, o vídeo para no fim, o aluno repete; só avança se acertar.
    ChamadaEResposta,
}

impl ModoAcompanhamento {
    pub const TODOS: [Self; 4] = [
        Self::NotaANota,
        Self::EsperaTocar,
        Self::SegueNoTempo,
        Self::ChamadaEResposta,
    ];

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::NotaANota => "Nota a nota",
            Self::EsperaTocar => "Espera tocar",
            Self::SegueNoTempo => "Segue no tempo",
            Self::ChamadaEResposta => "Chamada e resposta",
        }
    }

    pub fn explicacao(self) -> &'static str {
        match self {
            Self::NotaANota => "O vídeo para em cada nota até você tocar a certa",
            Self::EsperaTocar => {
                "O vídeo anda no tempo e só para se a nota passar sem você tocá-la; ao acertar, \
                 conta dois tempos e retoma"
            }
            Self::SegueNoTempo => "O vídeo não para; cada nota sua conta como acerto ou erro",
            Self::ChamadaEResposta => {
                "O professor toca o trecho, o vídeo para e você repete; só avança se acertar"
            }
        }
    }

    pub fn proximo(self) -> Self {
        match self {
            Self::NotaANota => Self::EsperaTocar,
            Self::EsperaTocar => Self::SegueNoTempo,
            Self::SegueNoTempo => Self::ChamadaEResposta,
            Self::ChamadaEResposta => Self::NotaANota,
        }
    }

    pub fn como_texto(self) -> &'static str {
        match self {
            Self::NotaANota => "nota_a_nota",
            Self::EsperaTocar => "espera_tocar",
            Self::SegueNoTempo => "segue",
            Self::ChamadaEResposta => "chamada",
        }
    }

    /// Lê o texto; aceita o nome antigo "espera" (= Nota a nota).
    pub fn de_texto(t: &str) -> Option<Self> {
        match t.trim() {
            "espera" => Some(Self::NotaANota),
            t => Self::TODOS.into_iter().find(|m| m.como_texto() == t),
        }
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
    /// Vai ao instante e toca dali.
    Buscar(f64),
    /// Vai ao instante e fica parado nele (a contagem da retomada do Espera tocar).
    BuscarEPausar(f64),
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

/// O que o Espera tocar precisa saber do player (a tela repassa a cada quadro).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigEsperaTocar {
    /// Janela de acerto de cada nota, em ms (no tempo da música, não do relógio de parede).
    pub tolerancia_ms: f64,
    /// Quanto voltar ao retomar depois de uma parada.
    pub retomada: Retomada,
    /// Andamento do Exercício (batidas por minuto), se ele disser; senão a batida é estimada
    /// pelos ataques do trecho ([`batida_do_trecho`]).
    pub bpm: Option<f64>,
    /// Velocidade do vídeo (a contagem de retomada dura mais quando o vídeo está mais lento).
    pub velocidade: f64,
}

impl Default for ConfigEsperaTocar {
    fn default() -> Self {
        Self {
            tolerancia_ms: 150.0,
            retomada: Retomada::Zero,
            bpm: None,
            velocidade: 1.0,
        }
    }
}

/// Placar do Espera tocar no trecho: o do core ([`piano_tutor_core::pratica::Resumo`]), somado ao
/// das práticas que foram refeitas no meio (salto no vídeo, mudança de tolerância ou de retomada).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlacarEspera {
    /// Notas esperadas (das práticas refeitas contam só as que o aluno acertou).
    pub esperadas: u32,
    pub acertos: u32,
    /// Notas tocadas sem par (altura errada, fora da janela, repetidas).
    pub extras: u32,
    /// Vezes que o vídeo parou esperando o aluno.
    pub paradas: u32,
}

impl PlacarEspera {
    /// Acertos sobre as esperadas mais as paradas (cada parada é uma falta a mais, como no core);
    /// 100 se não havia nada esperado.
    pub fn precisao_pct(&self) -> f64 {
        let base = self.esperadas + self.paradas;
        if base == 0 {
            100.0
        } else {
            100.0 * f64::from(self.acertos) / f64::from(base)
        }
    }
}

/// A prática do core que acompanha o trecho atual no Espera tocar.
#[derive(Clone, Debug)]
struct PraticaDoTrecho {
    pratica: Pratica,
    /// A configuração com que ela nasceu (mudou a do player? refaz).
    config: ConfigPratica,
    /// Exercício do trecho a que ela pertence.
    exercicio_id: String,
}

/// Em que pé está o Espera tocar fora da parada em si (que a `Pratica` sabe).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum FaseEspera {
    #[default]
    Andando,
    /// O vídeo foi buscado de fora (transporte, linha do tempo, voltar): espera ele chegar a
    /// `alvo_video` e então refaz a prática dali.
    Chegando { alvo_video: f64, quadros: u32 },
    /// Retomada: vídeo parado no instante da retomada, contando [`BATIDAS_RETOMADA`] tempos até
    /// `fim_s` (relógio da tela); cada batida dura `batida_s` do relógio.
    Contagem { fim_s: f64, batida_s: f64 },
}

/// Estado do acompanhamento (o que o aluno já tocou no trecho).
///
/// No **Espera tocar** o vídeo anda sozinho e quem decide quando parar é uma
/// [`piano_tutor_core::pratica::Pratica`] do core, construída com as notas do Alinhamento do
/// trecho (instantes em `t_audio`). O relógio dela é o do vídeo: só anda quando o vídeo toca.
/// Parar, e retomar com contagem, funciona assim:
///
/// 1. a janela de uma nota vence sem ela: `passo` devolve [`Comando::Pausar`] e o vídeo (com o
///    áudio do professor que vem nele) fica parado;
/// 2. o aluno toca a nota que falta: a prática pede a retomada num instante (conforme
///    [`Retomada`]) e `nota_tocada` devolve [`Comando::BuscarEPausar`] até ele; o vídeo fica parado
///    nesse instante enquanto o player conta [`BATIDAS_RETOMADA`] tempos ("2, 1") pelo relógio da
///    tela (batida do andamento do Exercício ou estimada, na velocidade do vídeo); as notas do
///    aluno na contagem são ignoradas;
/// 3. a contagem acaba: [`Comando::Continuar`], sem nova busca, e o vídeo segue do instante exato.
///
/// Se a parada era o último passo da prática, não há o que retomar: só [`Comando::Continuar`].
#[derive(Clone, Debug, Default)]
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
    /// No Espera tocar: o placar completo (`acertos` e `erros` vêm dele; `erros` soma extras e
    /// paradas).
    pub placar: PlacarEspera,
    ultimo_voltar: Option<f64>,
    config_et: ConfigEsperaTocar,
    /// Último instante do relógio da tela (s), para a contagem da retomada.
    relogio_s: f64,
    et: Option<PraticaDoTrecho>,
    fase_et: FaseEspera,
    /// Placar das práticas que foram refeitas no trecho.
    acumulado_et: PlacarEspera,
    /// A busca que o próprio acompanhamento acabou de pedir (a retomada); não é um salto de fora.
    busca_propria: bool,
    /// O aluno retomou o vídeo à mão enquanto estava parado: desiste da parada.
    liberar_parada: bool,
}

impl Acompanhamento {
    /// O estado vazio de `modo`, guardando só o que não é do trecho (configuração do Espera
    /// tocar, relógio e último toque de "voltar").
    fn limpo(&self, modo: ModoAcompanhamento) -> Self {
        Self {
            modo,
            ultimo_voltar: self.ultimo_voltar,
            config_et: self.config_et,
            relogio_s: self.relogio_s,
            ..Self::default()
        }
    }

    pub fn trocar_modo(&mut self, modo: ModoAcompanhamento) {
        *self = Self {
            ultimo_voltar: None,
            ..self.limpo(modo)
        };
    }

    /// Zera o que foi tocado (ao voltar ou ao trocar de trecho), mantendo o modo.
    pub fn recomecar(&mut self) {
        *self = self.limpo(self.modo);
    }

    // --- Espera tocar: o que a tela repassa e o que ela mostra -------------------------------

    /// A configuração do Espera tocar (a tela repassa a cada quadro; mudou, a prática é refeita
    /// assim que não estiver parada nem contando).
    pub fn definir_espera_tocar(&mut self, c: ConfigEsperaTocar) {
        self.config_et = c;
    }

    /// O relógio da tela, em segundos (qualquer origem monotônica), para a contagem da retomada.
    pub fn definir_relogio(&mut self, agora_s: f64) {
        if agora_s.is_finite() {
            self.relogio_s = agora_s;
        }
    }

    /// Vezes que o vídeo parou esperando o aluno neste trecho.
    pub fn paradas(&self) -> u32 {
        self.placar.paradas
    }

    /// No Espera tocar, o vídeo está parado esperando o aluno tocar o que falta.
    pub fn parado_esperando(&self) -> bool {
        self.modo == ModoAcompanhamento::EsperaTocar
            && self
                .et
                .as_ref()
                .is_some_and(|v| v.pratica.parada().is_some())
    }

    /// Alturas MIDI que faltam tocar na parada, em ordem de altura.
    pub fn notas_que_faltam(&self) -> Vec<u8> {
        let Some(v) = &self.et else {
            return Vec::new();
        };
        let mut m: Vec<u8> = v
            .pratica
            .notas_da_parada()
            .into_iter()
            .map(|i| v.pratica.esperados()[i].midi)
            .collect();
        m.sort_unstable();
        m.dedup();
        m
    }

    /// Na contagem da retomada: quantos tempos faltam (2, depois 1).
    pub fn contagem_retomada(&self) -> Option<u32> {
        let FaseEspera::Contagem { fim_s, batida_s } = self.fase_et else {
            return None;
        };
        let restante = fim_s - self.relogio_s;
        if restante <= 0.0 || batida_s <= 0.0 {
            return None;
        }
        let max = BATIDAS_RETOMADA as u32;
        Some(((restante / batida_s).ceil() as u32).clamp(1, max))
    }

    /// As guias do teclado na parada: as notas que faltam (no lugar das "que estão soando", que a
    /// janela de tolerância larga pode ter deixado para trás).
    pub fn guias_da_parada(&self, al: &Alinhamento, dedos: Option<&Dedilhado>) -> Option<Guias> {
        let v = self.et.as_ref()?;
        v.pratica.parada()?;
        let notas: Vec<_> = v
            .pratica
            .notas_da_parada()
            .into_iter()
            .filter_map(|i| {
                let e = &v.pratica.esperados()[i];
                al.notas
                    .iter()
                    .find(|n| n.midi == e.midi && (n.t_audio_s - e.inicio_s).abs() < 0.03)
                    .map(|n| (n.midi, dedos.and_then(|d| d.dedo(n.indice)), n.indice))
            })
            .collect();
        (!notas.is_empty()).then_some(Guias {
            notas,
            antecipada: false,
        })
    }

    /// A tela executou `c` no vídeo. Um salto pedido de fora (transporte, linha do tempo,
    /// "voltar", Roteiro de Aula) faz o Espera tocar esperar o vídeo chegar e refazer a prática
    /// dali; a busca da própria retomada não conta.
    pub fn ao_executar(&mut self, c: &Comando) {
        let (Comando::Buscar(s) | Comando::BuscarEPausar(s)) = c else {
            return;
        };
        if std::mem::take(&mut self.busca_propria) || self.modo != ModoAcompanhamento::EsperaTocar {
            return;
        }
        self.liberar_parada = false;
        self.fase_et = FaseEspera::Chegando {
            alvo_video: *s,
            quadros: 0,
        };
    }

    /// O aluno retomou o vídeo à mão (Espaço, botão): numa parada, desiste dela; na contagem,
    /// pula o resto da contagem.
    pub fn liberar_manual(&mut self) {
        if self.modo != ModoAcompanhamento::EsperaTocar {
            return;
        }
        match &mut self.fase_et {
            FaseEspera::Contagem { fim_s, .. } => *fim_s = self.relogio_s,
            FaseEspera::Andando => self.liberar_parada = self.parado_esperando(),
            FaseEspera::Chegando { .. } => {}
        }
    }

    // --- Espera tocar: a prática do core ---------------------------------------------------

    fn config_pratica(&self) -> ConfigPratica {
        ConfigPratica {
            modo: ModoPratica::EsperaTocar,
            tolerancia_ms: if self.config_et.tolerancia_ms.is_finite() {
                self.config_et.tolerancia_ms.max(1.0)
            } else {
                ConfigPratica::default().tolerancia_ms
            },
            retomada: self.config_et.retomada,
        }
    }

    /// Recria a prática com as notas do trecho a partir de `desde_audio`, somando o placar da
    /// anterior (se havia) ao acumulado.
    fn criar_pratica(&mut self, al: &Alinhamento, trecho: &Trecho, desde_audio: f64) {
        if let Some(v) = self.et.take() {
            let r = v.pratica.resumo();
            self.acumulado_et.esperadas += r.acertos as u32;
            self.acumulado_et.acertos += r.acertos as u32;
            self.acumulado_et.extras += r.extras as u32;
            self.acumulado_et.paradas += r.paradas as u32;
        }
        let eventos: Vec<EventoEsperado> = notas_do_trecho(al, trecho)
            .into_iter()
            .filter(|n| n.t_audio_s >= desde_audio - 1e-6)
            .map(|n| EventoEsperado {
                midi: n.midi,
                inicio_s: n.t_audio_s,
                duracao_s: 0.3,
                parte: 0,
                pauta: 1,
                compasso: n.compasso as usize,
            })
            .collect();
        let config = self.config_pratica();
        self.et = Some(PraticaDoTrecho {
            pratica: Pratica::nova(&eventos, config),
            config,
            exercicio_id: trecho.exercicio_id.clone(),
        });
        self.atualizar_placar();
    }

    /// De onde a prática nasce quando ninguém a refez antes: do instante atual, ou do começo do
    /// trecho se o vídeo acabou de entrar nele.
    fn desde_inicial(al: &Alinhamento, trecho: &Trecho, t_video: f64) -> f64 {
        let t_audio = t_video - al.offset_aula_s;
        if t_video - trecho.inicio_s < 0.25 {
            (trecho.inicio_s - al.offset_aula_s - 0.05).min(t_audio - 0.05)
        } else {
            t_audio - 0.05
        }
    }

    /// Recalcula `placar`, `acertos` e `erros` (acumulado + prática viva).
    fn atualizar_placar(&mut self) {
        let mut p = self.acumulado_et;
        if let Some(v) = &self.et {
            let r = v.pratica.resumo();
            p.esperadas += r.esperadas as u32;
            p.acertos += r.acertos as u32;
            p.extras += r.extras as u32;
            p.paradas += r.paradas as u32;
        }
        self.placar = p;
        self.acertos = p.acertos;
        self.erros = p.extras + p.paradas;
    }

    fn passo_espera_tocar(
        &mut self,
        al: &Alinhamento,
        trecho: &Trecho,
        t_video: f64,
        pausado: bool,
    ) -> Option<Comando> {
        let t_audio = t_video - al.offset_aula_s;
        if self
            .et
            .as_ref()
            .is_some_and(|v| v.exercicio_id != trecho.exercicio_id)
        {
            self.recomecar();
        }
        // Salto de fora: espera o vídeo chegar (a posição que o mpv informa pode demorar a
        // mudar) e refaz a prática dali, sem contar as notas que o salto deixou para trás.
        if let FaseEspera::Chegando {
            alvo_video,
            quadros,
        } = self.fase_et
        {
            let perto = (t_video - alvo_video).abs() < CHEGADA_S;
            if (quadros < 2 || !perto) && quadros < QUADROS_ATE_CHEGAR {
                self.fase_et = FaseEspera::Chegando {
                    alvo_video,
                    quadros: quadros + 1,
                };
                return None;
            }
            self.fase_et = FaseEspera::Andando;
            let desde = if perto {
                alvo_video - al.offset_aula_s
            } else {
                t_audio
            };
            self.criar_pratica(al, trecho, desde - 0.05);
            return None;
        }
        // Contagem da retomada: o vídeo fica parado até ela acabar.
        if let FaseEspera::Contagem { fim_s, .. } = self.fase_et {
            if self.relogio_s >= fim_s {
                self.fase_et = FaseEspera::Andando;
                return Some(Comando::Continuar);
            }
            return (!pausado).then_some(Comando::Pausar);
        }
        if self.et.is_none() {
            self.criar_pratica(al, trecho, Self::desde_inicial(al, trecho, t_video));
        }
        let cfg = self.config_pratica();
        let (parada, config_velha) = {
            let v = self.et.as_ref()?;
            (v.pratica.parada().is_some(), v.config)
        };
        if std::mem::take(&mut self.liberar_parada) && parada {
            // Retomou à mão: a nota parada fica como falta e a prática segue dali.
            self.criar_pratica(al, trecho, t_audio);
            return None;
        }
        if !parada && config_velha != cfg {
            // A tolerância ou a retomada mudaram: refaz, pulando as notas de janela ainda aberta.
            self.criar_pratica(al, trecho, t_audio + config_velha.tolerancia_ms / 1000.0);
            return None;
        }
        if parada {
            return (!pausado).then_some(Comando::Pausar);
        }
        if pausado {
            return None;
        }
        let v = self.et.as_mut()?;
        let parou = v.pratica.avancar_tempo(t_audio) && v.pratica.parada().is_some();
        if parou {
            self.atualizar_placar();
            return Some(Comando::Pausar);
        }
        None
    }

    fn nota_tocada_espera_tocar(
        &mut self,
        al: &Alinhamento,
        trecho: &Trecho,
        t_video: f64,
        midi: u8,
    ) -> Option<Comando> {
        // Na contagem (ou esperando o vídeo chegar) as notas não valem.
        if self.fase_et != FaseEspera::Andando {
            return None;
        }
        if self.et.is_none() {
            self.criar_pratica(al, trecho, Self::desde_inicial(al, trecho, t_video));
        }
        let t_audio = t_video - al.offset_aula_s;
        let v = self.et.as_mut()?;
        let estava_parada = v.pratica.parada().is_some();
        v.pratica.nota_tocada(midi, t_audio);
        let mut comando = None;
        if estava_parada {
            if let Some(t_alvo) = v.pratica.tomar_retomada() {
                let batida = batida_do_trecho(al, trecho, self.config_et.bpm);
                let batida_s = batida / self.config_et.velocidade.clamp(0.1, 4.0);
                self.fase_et = FaseEspera::Contagem {
                    fim_s: self.relogio_s + BATIDAS_RETOMADA * batida_s,
                    batida_s,
                };
                self.busca_propria = true;
                comando = Some(Comando::BuscarEPausar(t_alvo + al.offset_aula_s));
            } else if v.pratica.parada().is_none() {
                // Era o último passo: nada a voltar, só continuar de onde parou.
                comando = Some(Comando::Continuar);
            }
        }
        self.atualizar_placar();
        comando
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
            ModoAcompanhamento::NotaANota => {
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
            ModoAcompanhamento::EsperaTocar => {
                self.passo_espera_tocar(al, trecho, t_video, pausado)
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
            ModoAcompanhamento::NotaANota => {
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
            ModoAcompanhamento::EsperaTocar => {
                self.nota_tocada_espera_tocar(al, trecho, t_video, midi)
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

/// Duração de uma batida (s) no trecho, para a contagem da retomada do Espera tocar: a do
/// andamento do Exercício (`bpm`), se ele disser; senão a mediana dos intervalos entre os ataques
/// do trecho (entre 0,3 s e 1,5 s); sem ataques que bastem, [`BATIDA_PADRAO_S`].
pub fn batida_do_trecho(al: &Alinhamento, trecho: &Trecho, bpm: Option<f64>) -> f64 {
    if let Some(b) = bpm.filter(|b| b.is_finite() && *b >= 20.0) {
        return (60.0 / b).clamp(0.2, 3.0);
    }
    let mut ataques: Vec<f64> = notas_do_trecho(al, trecho)
        .iter()
        .map(|n| n.t_audio_s)
        .collect();
    ataques.dedup_by(|a, b| (*a - *b).abs() < 0.03);
    let mut intervalos: Vec<f64> = ataques
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| *d > 0.05)
        .collect();
    if intervalos.is_empty() {
        return BATIDA_PADRAO_S;
    }
    intervalos.sort_by(f64::total_cmp);
    intervalos[intervalos.len() / 2].clamp(0.3, 1.5)
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
        let mut m = ModoAcompanhamento::NotaANota;
        let mut vistos = vec![m];
        for _ in 0..ModoAcompanhamento::TODOS.len() {
            m = m.proximo();
            assert_eq!(ModoAcompanhamento::de_texto(m.como_texto()), Some(m));
            vistos.push(m);
        }
        // Os quatro modos, e a volta ao começo.
        assert_eq!(m, ModoAcompanhamento::NotaANota);
        for modo in ModoAcompanhamento::TODOS {
            assert!(vistos.contains(&modo), "{modo:?} ficou de fora da volta");
        }
        // O nome antigo "Espera" é o Nota a nota.
        assert_eq!(
            ModoAcompanhamento::de_texto("espera"),
            Some(ModoAcompanhamento::NotaANota)
        );
        assert_eq!(
            ModoAcompanhamento::de_texto("espera_tocar"),
            Some(ModoAcompanhamento::EsperaTocar)
        );
        assert_eq!(ModoAcompanhamento::de_texto("turbo"), None);
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

    // --- Espera tocar ------------------------------------------------------------------------

    /// Acompanhamento no Espera tocar com a tolerância (ms), a retomada e o andamento dados.
    /// O áudio de `al()` bate em 101 s, 102 s e 103 s do vídeo (Dó, Ré, Mi).
    fn espera_tocar(ms: f64, retomada: Retomada) -> Acompanhamento {
        let mut ac = Acompanhamento::default();
        ac.trocar_modo(ModoAcompanhamento::EsperaTocar);
        ac.definir_espera_tocar(ConfigEsperaTocar {
            tolerancia_ms: ms,
            retomada,
            bpm: Some(120.0), // batida de 0,5 s: a contagem dura 1 s
            velocidade: 1.0,
        });
        ac.definir_relogio(10.0);
        ac
    }

    #[test]
    fn espera_tocar_nao_pausa_enquanto_o_aluno_acerta() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        for (t_nota, m) in [(101.02, 60), (102.0, 62), (103.05, 64)] {
            // O vídeo anda em passos de 50 ms e o aluno toca no instante da nota.
            let mut t = t_nota - 0.5;
            while t < t_nota {
                assert_eq!(ac.passo(&a, &tr, t, false), None, "t = {t}");
                t += 0.05;
            }
            assert_eq!(ac.nota_tocada(&a, &tr, None, t_nota, m), None);
        }
        assert_eq!(ac.passo(&a, &tr, 103.4, false), None);
        assert_eq!((ac.acertos, ac.erros, ac.paradas()), (3, 0, 0));
        assert!(!ac.parado_esperando());
        assert!((ac.placar.precisao_pct() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn espera_tocar_pausa_quando_a_janela_passa_sem_a_nota() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        assert_eq!(ac.passo(&a, &tr, 100.8, false), None);
        assert_eq!(ac.passo(&a, &tr, 101.1, false), None, "ainda na janela");
        assert_eq!(ac.passo(&a, &tr, 101.2, false), Some(Comando::Pausar));
        assert!(ac.parado_esperando());
        assert_eq!(ac.notas_que_faltam(), vec![60]);
        assert_eq!(ac.paradas(), 1);
        assert_eq!(ac.erros, 1, "a parada conta como erro");
        // Se o vídeo ainda estiver andando, pede de novo; parado, não faz nada, e não conta outra.
        assert_eq!(ac.passo(&a, &tr, 101.25, false), Some(Comando::Pausar));
        assert_eq!(ac.passo(&a, &tr, 101.2, true), None);
        assert_eq!(ac.paradas(), 1);
        // O teclado mostra a nota que falta.
        let g = ac.guias_da_parada(&a, None).expect("guias");
        assert_eq!(g.notas, vec![(60, None, 0)]);
        assert!(!g.antecipada);
    }

    #[test]
    fn espera_tocar_nota_errada_nao_pausa_nem_libera() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.0, 61), None);
        assert_eq!(ac.passo(&a, &tr, 101.1, false), None, "errar não para");
        assert_eq!((ac.acertos, ac.erros), (0, 1));
        // Ainda dá tempo de acertar dentro da janela.
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.1, 60), None);
        assert_eq!(ac.passo(&a, &tr, 101.3, false), None);
        assert_eq!((ac.acertos, ac.erros), (1, 1));
        // Parado, a errada também não libera.
        assert_eq!(ac.passo(&a, &tr, 102.2, false), Some(Comando::Pausar));
        assert_eq!(ac.nota_tocada(&a, &tr, None, 102.2, 64), None);
        assert!(ac.parado_esperando());
        assert_eq!(
            ac.erros, 3,
            "uma errada antes, a parada e a errada na parada"
        );
    }

    #[test]
    fn espera_tocar_retoma_da_nota_em_que_parou_com_contagem() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.0, 60), None);
        // Perde o Ré: o vídeo para (em torno de 102,15 s).
        assert_eq!(ac.passo(&a, &tr, 102.2, false), Some(Comando::Pausar));
        // Acerta o Ré: volta ao instante do Ré (102 s) e fica parado contando "2, 1".
        assert_eq!(
            ac.nota_tocada(&a, &tr, None, 102.2, 62),
            Some(Comando::BuscarEPausar(102.0))
        );
        assert_eq!(ac.contagem_retomada(), Some(2));
        // Na contagem o vídeo não anda e as notas do aluno não valem (nem contam como erro).
        assert_eq!(ac.passo(&a, &tr, 102.0, false), Some(Comando::Pausar));
        assert_eq!(ac.passo(&a, &tr, 102.0, true), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 102.0, 70), None);
        assert_eq!(ac.erros, 1);
        ac.definir_relogio(10.6);
        assert_eq!(ac.contagem_retomada(), Some(1));
        assert_eq!(ac.passo(&a, &tr, 102.0, true), None);
        // A contagem acabou: continua do mesmo ponto, sem nova busca.
        ac.definir_relogio(11.0);
        assert_eq!(ac.contagem_retomada(), None);
        assert_eq!(ac.passo(&a, &tr, 102.0, true), Some(Comando::Continuar));
        assert!(!ac.parado_esperando());
        // Dali em diante o Mi ainda vale; perdê-lo para de novo (segunda parada).
        assert_eq!(ac.passo(&a, &tr, 102.5, false), None);
        assert_eq!(ac.passo(&a, &tr, 103.2, false), Some(Comando::Pausar));
        assert_eq!(ac.paradas(), 2);
        assert_eq!(ac.placar.acertos, 2);
    }

    #[test]
    fn espera_tocar_volta_uma_nota_e_confere_de_novo() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Uma);
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.0, 60), None);
        assert_eq!(ac.passo(&a, &tr, 102.2, false), Some(Comando::Pausar));
        // Volta uma nota: ao Dó (101 s), e o Dó e o Ré passam a valer de novo.
        assert_eq!(
            ac.nota_tocada(&a, &tr, None, 102.2, 62),
            Some(Comando::BuscarEPausar(101.0))
        );
        assert_eq!(ac.placar.acertos, 0, "as notas voltadas foram reabertas");
        ac.definir_relogio(11.0);
        assert_eq!(ac.passo(&a, &tr, 101.0, true), Some(Comando::Continuar));
        assert_eq!(ac.passo(&a, &tr, 101.05, false), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.05, 60), None);
        assert_eq!(ac.passo(&a, &tr, 102.05, false), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 102.05, 62), None);
        assert_eq!(ac.passo(&a, &tr, 102.5, false), None);
        assert_eq!(ac.paradas(), 1);
        assert_eq!(ac.placar.acertos, 2);
        // 1 parada em 3 esperadas (2 voltadas + o Mi que sobrou do trecho todo): a precisão
        // conta cada parada como falta a mais.
        assert!(ac.placar.precisao_pct() < 100.0);
    }

    #[test]
    fn espera_tocar_parada_no_ultimo_passo_so_continua() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.0, 60), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 102.0, 62), None);
        assert_eq!(ac.passo(&a, &tr, 103.2, false), Some(Comando::Pausar));
        assert_eq!(
            ac.nota_tocada(&a, &tr, None, 103.2, 64),
            Some(Comando::Continuar)
        );
        assert_eq!(ac.contagem_retomada(), None);
        assert!(!ac.parado_esperando());
    }

    #[test]
    fn espera_tocar_retomada_do_inicio_volta_ao_primeiro_passo() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Inicio);
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        assert_eq!(ac.nota_tocada(&a, &tr, None, 101.0, 60), None);
        assert_eq!(ac.passo(&a, &tr, 102.2, false), Some(Comando::Pausar));
        assert_eq!(
            ac.nota_tocada(&a, &tr, None, 102.2, 62),
            Some(Comando::BuscarEPausar(101.0))
        );
    }

    #[test]
    fn espera_tocar_contagem_dura_mais_com_o_video_mais_lento() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        ac.definir_espera_tocar(ConfigEsperaTocar {
            velocidade: 0.5,
            ..ac.config_et
        });
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        assert_eq!(ac.passo(&a, &tr, 101.2, false), Some(Comando::Pausar));
        assert!(ac.nota_tocada(&a, &tr, None, 101.2, 60).is_some());
        ac.definir_relogio(11.5);
        assert_eq!(ac.passo(&a, &tr, 101.0, true), None, "ainda contando (2 s)");
        ac.definir_relogio(12.0);
        assert_eq!(ac.passo(&a, &tr, 101.0, true), Some(Comando::Continuar));
    }

    #[test]
    fn espera_tocar_salto_de_fora_refaz_a_pratica_sem_parar_nas_puladas() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        // O aluno clica na linha do tempo e vai ao Ré; a posição velha ainda aparece uns quadros.
        ac.ao_executar(&Comando::Buscar(102.0));
        for t in [100.95, 101.0, 101.05] {
            assert_eq!(ac.passo(&a, &tr, t, false), None, "t = {t}");
        }
        for t in [102.0, 102.02, 102.04] {
            assert_eq!(ac.passo(&a, &tr, t, false), None, "t = {t}");
        }
        // O Dó que ficou para trás não conta; o Ré, na janela, ainda vale.
        assert_eq!(ac.nota_tocada(&a, &tr, None, 102.04, 62), None);
        assert_eq!(ac.passo(&a, &tr, 102.5, false), None);
        assert_eq!(ac.paradas(), 0);
        assert_eq!(ac.placar.acertos, 1);
        // O Mi não tocado ainda para.
        assert_eq!(ac.passo(&a, &tr, 103.2, false), Some(Comando::Pausar));
        assert_eq!(ac.placar.paradas, 1);
    }

    #[test]
    fn espera_tocar_liberar_a_mao_desiste_da_parada() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        assert_eq!(ac.passo(&a, &tr, 101.2, false), Some(Comando::Pausar));
        // O aluno aperta tocar: o vídeo anda e o Dó fica como falta (a parada já contou).
        ac.liberar_manual();
        assert_eq!(ac.passo(&a, &tr, 101.25, false), None);
        assert!(!ac.parado_esperando());
        assert_eq!(ac.passo(&a, &tr, 101.9, false), None);
        assert_eq!(ac.passo(&a, &tr, 102.2, false), Some(Comando::Pausar));
        assert_eq!(ac.paradas(), 2);
    }

    #[test]
    fn espera_tocar_tolerancia_maior_adia_a_parada() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        let mut ac = espera_tocar(150.0, Retomada::Zero);
        assert_eq!(ac.passo(&a, &tr, 100.9, false), None);
        ac.definir_espera_tocar(ConfigEsperaTocar {
            tolerancia_ms: 500.0,
            ..ac.config_et
        });
        // Com 500 ms o Dó ainda tem janela em 101,2 s; a prática refeita pula a nota de janela
        // aberta, e a próxima (Ré) vale com a janela nova.
        assert_eq!(ac.passo(&a, &tr, 101.2, false), None);
        assert_eq!(ac.passo(&a, &tr, 102.4, false), None);
        assert_eq!(ac.passo(&a, &tr, 102.6, false), Some(Comando::Pausar));
    }

    #[test]
    fn espera_tocar_modo_e_configuracao_sobrevivem_ao_recomecar() {
        let mut ac = espera_tocar(300.0, Retomada::Duas);
        ac.recomecar();
        assert_eq!(ac.modo, ModoAcompanhamento::EsperaTocar);
        assert_eq!(ac.config_et.tolerancia_ms, 300.0);
        assert_eq!(ac.config_et.retomada, Retomada::Duas);
        ac.trocar_modo(ModoAcompanhamento::NotaANota);
        assert_eq!(ac.config_et.retomada, Retomada::Duas);
    }

    #[test]
    fn batida_vem_do_andamento_ou_dos_ataques() {
        let a = al();
        let tr = trecho("x", 100.5, 103.5);
        assert!((batida_do_trecho(&a, &tr, Some(120.0)) - 0.5).abs() < 1e-9);
        // Ataques a cada 1 s: a mediana dos intervalos.
        assert!((batida_do_trecho(&a, &tr, None) - 1.0).abs() < 1e-9);
        // Um ataque só: o padrão.
        let sozinha = Alinhamento {
            notas: vec![nota(0, 60, 1.0)],
            ..al()
        };
        assert!((batida_do_trecho(&sozinha, &tr, None) - BATIDA_PADRAO_S).abs() < 1e-9);
    }
}
