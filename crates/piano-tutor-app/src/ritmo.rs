//! Serviço de ritmo (F08, F09): metrônomo, bateria de passos, pads e o exercício "toque no
//! pulso". Agenda notas no canal de percussão (10) pelo **mesmo relógio e mesma saída** do
//! piano: um só backend de áudio.
//!
//! A cada quadro a UI chama [`EstadoRitmo::atualizar`], que emite os tiques até
//! `agora + ANTECEDENCIA` e os envia ao motor com instante absoluto. Com a janela parada
//! (minimizada), a casca pode deixar de pedir quadros e o ritmo para de ser agendado: limitação
//! registrada; o uso pedagógico é com a janela visível.

use crate::som::{CANAL_PERCUSSAO, ServicoSom};
use piano_tutor_audio::Acao;
use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::CanalMidi;
use piano_tutor_core::ritmo::{
    Agendador, CLIQUE_FORTE, CLIQUE_FRACO, Clique, ConfigRitmo, EventoRitmo, INSTRUMENTOS,
    Julgamento, Padrao, Posicao, TapTempo, julgar, padroes_de_fabrica,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Quanto à frente os tiques são agendados.
pub const ANTECEDENCIA: Duration = Duration::from_millis(150);
/// Atraso do primeiro tique depois de "Tocar".
pub const ATRASO_INICIO: Duration = Duration::from_millis(120);
/// Duração das notas de percussão (o NoteOff vem depois disto).
pub const DURACAO_BATIDA: Duration = Duration::from_millis(90);

fn ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

/// Exercício "toque no pulso" (F08): cada nota tocada é comparada ao pulso mais próximo.
#[derive(Clone, Debug, PartialEq)]
pub struct ExercicioPulso {
    pub tolerancia_ms: u32,
    /// Compensação somada ao instante do toque (latência de saída percebida), em ms.
    pub compensacao_ms: i32,
    /// Desvios em ms e o julgamento, do mais antigo ao mais novo.
    pub toques: Vec<(f64, Julgamento)>,
}

impl Default for ExercicioPulso {
    fn default() -> Self {
        Self {
            tolerancia_ms: 50,
            compensacao_ms: 0,
            toques: Vec::new(),
        }
    }
}

impl ExercicioPulso {
    /// Resumo: total, no tempo, adiantados, atrasados e desvio médio absoluto (ms).
    pub fn resumo(&self) -> (usize, usize, usize, usize, f64) {
        let n = self.toques.len();
        let conta = |j| self.toques.iter().filter(|(_, x)| *x == j).count();
        let medio = if n == 0 {
            0.0
        } else {
            self.toques.iter().map(|(d, _)| d.abs()).sum::<f64>() / n as f64
        };
        (
            n,
            conta(Julgamento::NoTempo),
            conta(Julgamento::Adiantado),
            conta(Julgamento::Atrasado),
            medio,
        )
    }
}

/// Estado do ritmo.
#[derive(Debug)]
pub struct EstadoRitmo {
    pub padrao: Padrao,
    pub fabrica: Vec<Padrao>,
    pub config: ConfigRitmo,
    /// Volume da percussão 0–127 (CC7 do canal 10).
    pub volume: u8,
    pub exercicio: Option<ExercicioPulso>,
    pub mensagem: Option<String>,
    pub pasta: Option<PathBuf>,
    pub nome_arquivo: String,
    agendador: Option<Agendador>,
    tap: TapTempo,
    buffer: Vec<(u64, EventoRitmo)>,
    volume_enviado: Option<u8>,
}

impl EstadoRitmo {
    pub fn novo(pasta: Option<PathBuf>) -> Self {
        let fabrica = padroes_de_fabrica();
        let padrao = fabrica.first().cloned().unwrap_or_else(|| {
            Padrao::vazio(
                "Novo",
                piano_tutor_core::ritmo::Formula::QuatroQuatro,
                1,
                100,
            )
        });
        Self {
            nome_arquivo: padrao.nome.clone(),
            padrao,
            fabrica,
            config: ConfigRitmo::default(),
            volume: 100,
            exercicio: None,
            mensagem: None,
            pasta,
            agendador: None,
            tap: TapTempo::default(),
            buffer: Vec::new(),
            volume_enviado: None,
        }
    }

    pub fn tocando(&self) -> bool {
        self.agendador.is_some()
    }

    pub fn bpm(&self) -> u16 {
        self.padrao.bpm
    }

    /// Começa (metrônomo e/ou bateria conforme a configuração).
    pub fn tocar(&mut self, som: &mut ServicoSom) {
        let inicio = som.agora_ns().saturating_add(ns(ATRASO_INICIO));
        self.agendador = Some(Agendador::novo(
            inicio,
            self.padrao.bpm,
            self.padrao.formula,
        ));
        if let Some(e) = self.exercicio.as_mut() {
            e.toques.clear();
        }
        self.volume_enviado = None;
        self.atualizar(som);
    }

    /// Para de agendar. O que já está na fila do motor (até [`ANTECEDENCIA`]) ainda soa.
    pub fn parar(&mut self) {
        self.agendador = None;
    }

    /// Andamento novo (40–220), aplicado sem saltar batida se estiver tocando.
    pub fn definir_bpm(&mut self, bpm: u16) {
        self.padrao.bpm = bpm.clamp(
            piano_tutor_core::ritmo::BPM_MIN,
            piano_tutor_core::ritmo::BPM_MAX,
        );
        if let Some(a) = self.agendador.as_mut() {
            a.definir_bpm(self.padrao.bpm);
        }
    }

    /// Toque no botão de tap tempo.
    pub fn tap(&mut self, agora_ns: u64) {
        if let Some(b) = self.tap.tocar(agora_ns) {
            self.definir_bpm(b);
        }
    }

    /// Troca o padrão (para se a fórmula mudou, para recomeçar alinhado).
    pub fn escolher_padrao(&mut self, p: Padrao) {
        if p.formula != self.padrao.formula {
            self.agendador = None;
        }
        let bpm = p.bpm;
        self.nome_arquivo = p.nome.clone();
        self.padrao = p;
        self.definir_bpm(bpm);
    }

    /// Posição atual (para o marcador de pulso e a coluna do sequenciador).
    pub fn posicao(&self, agora_ns: u64) -> Option<Posicao> {
        self.agendador.as_ref()?.posicao(agora_ns)
    }

    /// Chamar a cada quadro: agenda o que falta até `agora + ANTECEDENCIA`.
    pub fn atualizar(&mut self, som: &mut ServicoSom) {
        let Some(a) = self.agendador.as_mut() else {
            return;
        };
        let perc = CanalMidi::novo(CANAL_PERCUSSAO).expect("canal 10");
        if self.volume_enviado != Some(self.volume) {
            som.agendar(
                som.agora_ns(),
                Acao::Controle {
                    canal: perc,
                    controle: 7,
                    valor: self.volume.min(127),
                },
            );
            self.volume_enviado = Some(self.volume);
        }
        let horizonte = som.agora_ns().saturating_add(ns(ANTECEDENCIA));
        self.buffer.clear();
        a.emitir_ate(horizonte, self.config, Some(&self.padrao), &mut self.buffer);
        for (t, e) in self.buffer.drain(..) {
            let (nota, vel) = match e {
                EventoRitmo::Clique(Clique::Forte) => (CLIQUE_FORTE, 127),
                EventoRitmo::Clique(Clique::Pulso) => (CLIQUE_FRACO, 100),
                EventoRitmo::Clique(Clique::Subdivisao) => (CLIQUE_FRACO, 50),
                EventoRitmo::Batida {
                    instrumento,
                    velocidade,
                } => (INSTRUMENTOS[instrumento].nota_gm, velocidade),
            };
            agendar_batida(som, t, nota, vel);
        }
    }

    /// Pad: toca já a peça `i` (sem depender do sequenciador).
    pub fn pad(&mut self, som: &mut ServicoSom, i: usize) {
        if let Some(inst) = INSTRUMENTOS.get(i) {
            let agora = som.agora_ns();
            agendar_batida(som, agora, inst.nota_gm, 110);
        }
    }

    /// Nota tocada pelo aluno (instante no relógio comum): alimenta o exercício de pulso.
    pub fn toque(&mut self, t_ns: u64) -> Option<(f64, Julgamento)> {
        let a = self.agendador.as_ref()?;
        let e = self.exercicio.as_mut()?;
        let t = if e.compensacao_ms >= 0 {
            t_ns.saturating_add(u64::from(e.compensacao_ms.unsigned_abs()) * 1_000_000)
        } else {
            t_ns.saturating_sub(u64::from(e.compensacao_ms.unsigned_abs()) * 1_000_000)
        };
        let d = a.desvio_do_pulso(t)?;
        let j = julgar(d, u64::from(e.tolerancia_ms) * 1_000_000);
        let r = (d as f64 / 1_000_000.0, j);
        e.toques.push(r);
        Some(r)
    }

    // --- Arquivos ---------------------------------------------------------------------------

    fn pasta_padroes(&self) -> Option<PathBuf> {
        self.pasta.as_ref().map(|p| p.join("padroes"))
    }

    /// Padrões salvos pelo usuário (nome do arquivo sem extensão).
    pub fn salvos(&self) -> Vec<String> {
        let Some(p) = self.pasta_padroes() else {
            return Vec::new();
        };
        let mut v: Vec<String> = std::fs::read_dir(p)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.strip_suffix(".ritmo").map(str::to_owned)
            })
            .collect();
        v.sort();
        v
    }

    /// Salva o padrão atual com `nome_arquivo`.
    pub fn salvar(&mut self) {
        let nome = nome_seguro(&self.nome_arquivo);
        let Some(pasta) = self.pasta_padroes() else {
            self.mensagem = Some("Sem pasta de dados (HOME não definido).".to_owned());
            return;
        };
        if nome.is_empty() {
            self.mensagem = Some("Dê um nome ao padrão antes de salvar.".to_owned());
            return;
        }
        self.padrao.nome = self.nome_arquivo.trim().to_owned();
        let caminho = pasta.join(format!("{nome}.ritmo"));
        let r = std::fs::create_dir_all(&pasta)
            .and_then(|()| std::fs::write(&caminho, self.padrao.para_texto()));
        self.mensagem = Some(match r {
            Ok(()) => format!("Salvo em {}", caminho.display()),
            Err(e) => format!("Não foi possível salvar: {e}"),
        });
    }

    /// Carrega um padrão salvo.
    pub fn carregar(&mut self, nome: &str) {
        let Some(pasta) = self.pasta_padroes() else {
            return;
        };
        let caminho = pasta.join(format!("{}.ritmo", nome_seguro(nome)));
        match ler_padrao(&caminho) {
            Ok(p) => {
                self.escolher_padrao(p);
                self.nome_arquivo = nome.to_owned();
                self.mensagem = Some(format!("Carregado: {nome}"));
            }
            Err(e) => self.mensagem = Some(e),
        }
    }
}

fn agendar_batida(som: &mut ServicoSom, t: u64, nota: u8, vel: u8) {
    let (Some(canal), Some(nota)) = (CanalMidi::novo(CANAL_PERCUSSAO), NotaMidi::nova(nota)) else {
        return;
    };
    som.agendar(
        t,
        Acao::NotaLigada {
            canal,
            nota,
            velocidade: vel.clamp(1, 127),
        },
    );
    som.agendar(
        t.saturating_add(ns(DURACAO_BATIDA)),
        Acao::NotaDesligada { canal, nota },
    );
}

fn ler_padrao(caminho: &Path) -> Result<Padrao, String> {
    let meta = std::fs::metadata(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    if meta.len() > 64 * 1024 {
        return Err("Arquivo de padrão grande demais (máx. 64 KiB).".to_owned());
    }
    let t = std::fs::read_to_string(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    Padrao::de_texto(&t).map_err(|e| format!("Padrão inválido: {e}"))
}

/// Nome de arquivo sem barras nem caracteres de controle.
pub fn nome_seguro(nome: &str) -> String {
    nome.trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::som::apoio_testes::*;
    use piano_tutor_audio::RelogioManual;
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_core::tempo::Relogio;
    use std::sync::Arc;

    #[test]
    fn metronomo_agenda_no_canal_10_com_acento() {
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        motor.renderizar(BLOCO, 0);
        let mut r = EstadoRitmo::novo(None);
        r.definir_bpm(120);
        r.tocar(&mut som);
        // 2 s de quadros de 16 ms.
        for _ in 0..125 {
            relogio.avancar(Duration::from_millis(16));
            r.atualizar(&mut som);
        }
        let ligadas: Vec<(u8, u8)> = {
            motor.renderizar_no_relogio(48_000 * 3);
            motor
                .registro()
                .iter()
                .filter_map(|e| match *e {
                    RegistroFalso::Ligada {
                        canal, nota, ..
                    } => Some((canal, nota)),
                    _ => None,
                })
                .collect()
        };
        assert!(ligadas.iter().all(|(c, _)| *c == CANAL_PERCUSSAO));
        assert!(ligadas.len() >= 4, "{ligadas:?}");
        assert_eq!(ligadas[0].1, CLIQUE_FORTE);
        assert_eq!(ligadas[1].1, CLIQUE_FRACO);
        r.parar();
        assert!(!r.tocando());
    }

    #[test]
    fn exercicio_de_pulso_julga_e_resume() {
        let relogio = RelogioManual::novo();
        let (mut som, _motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        let mut r = EstadoRitmo::novo(None);
        r.definir_bpm(120);
        r.exercicio = Some(ExercicioPulso::default());
        let t0 = som.agora_ns() + ns(ATRASO_INICIO);
        r.tocar(&mut som);
        let ms = 1_000_000u64;
        assert_eq!(r.toque(t0 + 500 * ms + 10 * ms).map(|x| x.1), Some(Julgamento::NoTempo));
        assert_eq!(r.toque(t0 + 1000 * ms - 120 * ms).map(|x| x.1), Some(Julgamento::Adiantado));
        assert_eq!(r.toque(t0 + 1500 * ms + 150 * ms).map(|x| x.1), Some(Julgamento::Atrasado));
        let (n, certo, adiantado, atrasado, _) =
            r.exercicio.as_ref().map(ExercicioPulso::resumo).unwrap_or_default();
        assert_eq!((n, certo, adiantado, atrasado), (3, 1, 1, 1));
    }

    #[test]
    fn salvar_e_carregar_padrao_identico() {
        let pasta = std::env::temp_dir().join(format!("pt-ritmo-{}", std::process::id()));
        let mut r = EstadoRitmo::novo(Some(pasta.clone()));
        r.padrao.alternar(4, 3);
        r.nome_arquivo = "Meu ritmo".to_owned();
        r.salvar();
        let salvo = r.padrao.clone();
        assert_eq!(r.salvos(), vec!["Meu ritmo".to_owned()]);
        r.escolher_padrao(r.fabrica[2].clone());
        r.carregar("Meu ritmo");
        assert_eq!(r.padrao, salvo);
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn nome_seguro_tira_barras() {
        assert_eq!(nome_seguro("../../etc/x"), "______etc_x");
        assert!(!nome_seguro("../../etc/x").contains('/'));
        assert_eq!(nome_seguro("  Rock 2 "), "Rock 2");
    }
}
