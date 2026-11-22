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
    Julgamento, Padrao, Posicao, TapTempo, corta_nota, julgar, padroes_de_fabrica,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Quanto à frente os tiques são agendados.
pub const ANTECEDENCIA: Duration = Duration::from_millis(150);
/// Atraso do primeiro tique depois de "Tocar".
pub const ATRASO_INICIO: Duration = Duration::from_millis(120);
/// Duração das notas de percussão (o NoteOff vem depois disto).
pub const DURACAO_BATIDA: Duration = Duration::from_millis(90);
/// Folga depois do último instante agendado em que o NoteOff de segurança da parada é enviado.
pub const FOLGA_PARADA: Duration = Duration::from_millis(1);

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
    /// Peças mudas, na ordem de [`INSTRUMENTOS`]. Estado da sessão: não vai para o arquivo.
    pub mudo: [bool; INSTRUMENTOS.len()],
    /// Peças em solo: havendo alguma, só elas soam (o solo vence o mudo).
    pub solo: [bool; INSTRUMENTOS.len()],
    agendador: Option<Agendador>,
    tap: TapTempo,
    buffer: Vec<(u64, EventoRitmo)>,
    plano: Vec<(u64, Acao)>,
    volume_enviado: Option<u8>,
    /// Último instante (ns) agendado por este serviço, para calar tudo ao parar.
    ultimo_ns: Option<u64>,
    parada_pendente: bool,
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
            mudo: [false; INSTRUMENTOS.len()],
            solo: [false; INSTRUMENTOS.len()],
            agendador: None,
            tap: TapTempo::default(),
            buffer: Vec::new(),
            plano: Vec::new(),
            volume_enviado: None,
            ultimo_ns: None,
            parada_pendente: false,
        }
    }

    /// A peça `i` soa no sequenciador? Havendo solo em qualquer peça, só as em solo soam;
    /// sem solo, só as que não estão mudas.
    pub fn audivel(&self, i: usize) -> bool {
        if self.solo.iter().any(|s| *s) {
            self.solo.get(i).copied().unwrap_or(false)
        } else {
            !self.mudo.get(i).copied().unwrap_or(false)
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
        self.iniciar_em(inicio);
        self.volume_enviado = None;
        self.atualizar(som);
    }

    /// Começa com o primeiro tique em `inicio_ns` (relógio comum), sem tocar nada ainda.
    pub fn iniciar_em(&mut self, inicio_ns: u64) {
        self.agendador = Some(Agendador::novo(
            inicio_ns,
            self.padrao.bpm,
            self.padrao.formula,
        ));
        self.parada_pendente = false;
        if let Some(e) = self.exercicio.as_mut() {
            e.toques.clear();
        }
    }

    /// Para de agendar e marca o corte de segurança: o próximo [`EstadoRitmo::atualizar`]
    /// (um por quadro) agenda NoteOff de todas as peças logo depois do último instante já
    /// agendado, para que nenhuma nota fique soando.
    pub fn parar(&mut self) {
        self.agendador = None;
        self.parada_pendente = self.ultimo_ns.is_some();
    }

    /// [`EstadoRitmo::parar`] já enviando o corte de segurança ao motor.
    pub fn parar_e_silenciar(&mut self, som: &mut ServicoSom) {
        self.parar();
        self.atualizar(som);
    }

    /// Plano do corte da parada: NoteOff de toda peça e dos cliques em `último + 1 ms`.
    /// Consome a parada pendente.
    pub fn planejar_parada(&mut self, plano: &mut Vec<(u64, Acao)>) {
        if !std::mem::take(&mut self.parada_pendente) {
            return;
        }
        let Some(ultimo) = self.ultimo_ns else {
            return;
        };
        let t = ultimo.saturating_add(ns(FOLGA_PARADA));
        let Some(canal) = CanalMidi::novo(CANAL_PERCUSSAO) else {
            return;
        };
        let notas = INSTRUMENTOS
            .iter()
            .map(|i| i.nota_gm)
            .chain([CLIQUE_FORTE, CLIQUE_FRACO]);
        for n in notas {
            if let Some(nota) = NotaMidi::nova(n) {
                plano.push((t, Acao::NotaDesligada { canal, nota }));
            }
        }
        self.ultimo_ns = Some(t);
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

    /// Converte os eventos do agendador até `horizonte_ns` em ações de áudio com instante
    /// absoluto (sem tocar no motor): mudo/solo valem só para a bateria, não para o
    /// metrônomo; o chimbal fechado corta o aberto no mesmo instante.
    pub fn planejar(&mut self, horizonte_ns: u64, plano: &mut Vec<(u64, Acao)>) {
        let Some(a) = self.agendador.as_mut() else {
            return;
        };
        self.buffer.clear();
        a.emitir_ate(
            horizonte_ns,
            self.config,
            Some(&self.padrao),
            &mut self.buffer,
        );
        let eventos = std::mem::take(&mut self.buffer);
        let antes = plano.len();
        for &(t, e) in &eventos {
            match e {
                EventoRitmo::Clique(Clique::Forte) => planejar_batida(plano, t, CLIQUE_FORTE, 127),
                EventoRitmo::Clique(Clique::Pulso) => planejar_batida(plano, t, CLIQUE_FRACO, 100),
                EventoRitmo::Clique(Clique::Subdivisao) => {
                    planejar_batida(plano, t, CLIQUE_FRACO, 50);
                }
                EventoRitmo::Batida {
                    instrumento,
                    velocidade,
                } if self.audivel(instrumento) => {
                    planejar_batida(plano, t, INSTRUMENTOS[instrumento].nota_gm, velocidade);
                }
                EventoRitmo::Batida { .. } => {}
            }
        }
        self.buffer = eventos;
        self.marcar_ultimo(&plano[antes..]);
    }

    fn marcar_ultimo(&mut self, novos: &[(u64, Acao)]) {
        if let Some(m) = novos.iter().map(|(t, _)| *t).max() {
            self.ultimo_ns = Some(self.ultimo_ns.map_or(m, |u| u.max(m)));
        }
    }

    /// Chamar a cada quadro: agenda o que falta até `agora + ANTECEDENCIA`.
    pub fn atualizar(&mut self, som: &mut ServicoSom) {
        let mut plano = std::mem::take(&mut self.plano);
        plano.clear();
        self.planejar_parada(&mut plano);
        if self.agendador.is_some() {
            if let Some(perc) = CanalMidi::novo(CANAL_PERCUSSAO)
                && self.volume_enviado != Some(self.volume)
            {
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
            self.planejar(horizonte, &mut plano);
        }
        for (t, acao) in plano.drain(..) {
            som.agendar(t, acao);
        }
        self.plano = plano;
    }

    /// Pad: toca já a peça `i` (sem depender do sequenciador).
    pub fn pad(&mut self, som: &mut ServicoSom, i: usize) {
        if let Some(inst) = INSTRUMENTOS.get(i) {
            let agora = som.agora_ns();
            let mut plano = Vec::new();
            planejar_batida(&mut plano, agora, inst.nota_gm, 110);
            self.marcar_ultimo(&plano);
            for (t, acao) in plano {
                som.agendar(t, acao);
            }
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

/// Ações de uma batida no canal de percussão: o corte do grupo (se houver) e a nota, que
/// é solta após [`DURACAO_BATIDA`].
fn planejar_batida(plano: &mut Vec<(u64, Acao)>, t: u64, nota: u8, vel: u8) {
    let (Some(canal), Some(nota_midi)) = (CanalMidi::novo(CANAL_PERCUSSAO), NotaMidi::nova(nota))
    else {
        return;
    };
    if let Some(cortada) = corta_nota(nota).and_then(NotaMidi::nova) {
        plano.push((
            t,
            Acao::NotaDesligada {
                canal,
                nota: cortada,
            },
        ));
    }
    plano.push((
        t,
        Acao::NotaLigada {
            canal,
            nota: nota_midi,
            velocidade: vel.clamp(1, 127),
        },
    ));
    plano.push((
        t.saturating_add(ns(DURACAO_BATIDA)),
        Acao::NotaDesligada {
            canal,
            nota: nota_midi,
        },
    ));
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
    use piano_tutor_core::ritmo::Formula;
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
                    RegistroFalso::Ligada { canal, nota, .. } => Some((canal, nota)),
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
        assert_eq!(
            r.toque(t0 + 500 * ms + 10 * ms).map(|x| x.1),
            Some(Julgamento::NoTempo)
        );
        assert_eq!(
            r.toque(t0 + 1000 * ms - 120 * ms).map(|x| x.1),
            Some(Julgamento::Adiantado)
        );
        assert_eq!(
            r.toque(t0 + 1500 * ms + 150 * ms).map(|x| x.1),
            Some(Julgamento::Atrasado)
        );
        let (n, certo, adiantado, atrasado, _) = r
            .exercicio
            .as_ref()
            .map(ExercicioPulso::resumo)
            .unwrap_or_default();
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

    // --- Mudo, solo, corte do chimbal, parada e deriva (planejamento offline) --------------

    fn nota_ligada((t, a): &(u64, Acao)) -> Option<(u64, u8, u8)> {
        match a {
            Acao::NotaLigada {
                nota, velocidade, ..
            } => Some((*t, nota.numero(), *velocidade)),
            _ => None,
        }
    }

    fn ligadas(plano: &[(u64, Acao)]) -> Vec<(u64, u8, u8)> {
        plano.iter().filter_map(nota_ligada).collect()
    }

    fn so_bateria() -> ConfigRitmo {
        ConfigRitmo {
            metronomo: false,
            bateria: true,
            ..ConfigRitmo::default()
        }
    }

    /// 4/4 com bumbo (36) no passo 0, caixa (38) no 4 e chimbal fechado (42) nos passos 0 e 4.
    fn padrao_basico() -> Padrao {
        let mut p = Padrao::vazio("t", Formula::QuatroQuatro, 1, 120);
        for (i, passo) in [(0, 0), (1, 4), (2, 0), (2, 4)] {
            p.alternar(i, passo);
        }
        p
    }

    fn estado_basico() -> EstadoRitmo {
        let mut r = EstadoRitmo::novo(None);
        r.padrao = padrao_basico();
        r.config = so_bateria();
        r.iniciar_em(0);
        r
    }

    fn notas_de(plano: &[(u64, Acao)]) -> Vec<u8> {
        let mut n: Vec<u8> = ligadas(plano).iter().map(|x| x.1).collect();
        n.sort_unstable();
        n.dedup();
        n
    }

    #[test]
    fn sem_mudo_nem_solo_tudo_soa() {
        let mut r = estado_basico();
        let mut plano = Vec::new();
        r.planejar(1_000_000_000, &mut plano);
        assert_eq!(notas_de(&plano), vec![36, 38, 42]);
    }

    #[test]
    fn mudo_cala_so_a_peca_e_nao_o_metronomo() {
        let mut r = estado_basico();
        r.config.metronomo = true;
        r.mudo[0] = true;
        let mut plano = Vec::new();
        r.planejar(1_000_000_000, &mut plano);
        assert_eq!(notas_de(&plano), vec![38, 42, CLIQUE_FORTE, CLIQUE_FRACO]);
    }

    #[test]
    fn solo_deixa_so_a_peca_e_vence_o_mudo() {
        let mut r = estado_basico();
        r.solo[1] = true;
        // Solo na caixa e mudo nela mesma: o solo vence e a caixa toca.
        r.mudo[1] = true;
        // O bumbo está mudo e fora do solo: continua calado.
        r.mudo[0] = true;
        let mut plano = Vec::new();
        r.planejar(1_000_000_000, &mut plano);
        assert_eq!(notas_de(&plano), vec![38]);
        assert!(r.audivel(1) && !r.audivel(0) && !r.audivel(2));
        // Tirando o solo, volta o mudo (a caixa continua muda por si mesma).
        r.solo[1] = false;
        assert!(!r.audivel(0) && !r.audivel(1) && r.audivel(2));
    }

    #[test]
    fn mudo_e_solo_nao_entram_no_arquivo_do_padrao() {
        let mut r = estado_basico();
        let antes = r.padrao.para_texto();
        r.mudo[0] = true;
        r.solo[2] = true;
        assert_eq!(r.padrao.para_texto(), antes);
        assert_eq!(Padrao::de_texto(&antes).expect("lê"), r.padrao);
    }

    #[test]
    fn chimbal_fechado_corta_o_aberto_no_mesmo_instante_no_canal_10() {
        let mut r = estado_basico();
        let mut plano = Vec::new();
        r.planejar(1_000_000_000, &mut plano);
        let cortes: Vec<u64> = plano
            .iter()
            .filter_map(|(t, a)| match a {
                Acao::NotaDesligada { canal, nota }
                    if nota.numero() == 46 && canal.interno() == CANAL_PERCUSSAO =>
                {
                    Some(*t)
                }
                _ => None,
            })
            .collect();
        let fechados: Vec<u64> = ligadas(&plano)
            .iter()
            .filter(|x| x.1 == 42)
            .map(|x| x.0)
            .collect();
        assert_eq!(fechados, vec![0, 500_000_000]);
        assert_eq!(cortes, fechados);
        // Chimbal fechado mudo não corta o aberto.
        let mut r = estado_basico();
        r.mudo[2] = true;
        let mut plano = Vec::new();
        r.planejar(1_000_000_000, &mut plano);
        assert!(!plano.iter().any(|(_, a)| matches!(
            a,
            Acao::NotaDesligada { nota, .. } if nota.numero() == 46
        )));
    }

    #[test]
    fn parar_calar_todas_as_pecas_depois_do_ultimo_instante_agendado() {
        let mut r = estado_basico();
        r.config.metronomo = true;
        let mut plano = Vec::new();
        r.planejar(1_000_000_000, &mut plano);
        let ultimo = plano.iter().map(|(t, _)| *t).max().expect("agendou");
        r.parar();
        assert!(!r.tocando());
        let mut parada = Vec::new();
        r.planejar_parada(&mut parada);
        let esperado: Vec<u8> = {
            let mut v: Vec<u8> = INSTRUMENTOS.iter().map(|i| i.nota_gm).collect();
            v.extend([CLIQUE_FORTE, CLIQUE_FRACO]);
            v.sort_unstable();
            v
        };
        let mut notas: Vec<u8> = parada
            .iter()
            .map(|(t, a)| {
                assert_eq!(*t, ultimo + 1_000_000, "1 ms depois do último agendado");
                match a {
                    Acao::NotaDesligada { canal, nota } => {
                        assert_eq!(canal.interno(), CANAL_PERCUSSAO);
                        nota.numero()
                    }
                    outra => panic!("só NoteOff na parada: {outra:?}"),
                }
            })
            .collect();
        notas.sort_unstable();
        assert_eq!(notas, esperado);
        // Só uma vez, e nada sem ter agendado antes.
        let mut de_novo = Vec::new();
        r.planejar_parada(&mut de_novo);
        assert!(de_novo.is_empty());
        let mut virgem = EstadoRitmo::novo(None);
        virgem.parar();
        virgem.planejar_parada(&mut de_novo);
        assert!(de_novo.is_empty());
    }

    #[test]
    fn parar_e_silenciar_chega_ao_motor_e_nao_deixa_nota_soando() {
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        motor.renderizar(BLOCO, 0);
        let mut r = EstadoRitmo::novo(None);
        r.config.bateria = true;
        r.tocar(&mut som);
        for _ in 0..30 {
            relogio.avancar(Duration::from_millis(16));
            r.atualizar(&mut som);
        }
        r.parar_e_silenciar(&mut som);
        relogio.avancar(Duration::from_millis(500));
        motor.renderizar_no_relogio(48_000 * 2);
        assert_eq!(motor.notas_ativas(), 0, "nenhuma nota soando após a parada");
        let desligadas: Vec<u8> = motor
            .registro()
            .iter()
            .filter_map(|e| match *e {
                RegistroFalso::Desligada { canal, nota, .. } if canal == CANAL_PERCUSSAO => {
                    Some(nota)
                }
                _ => None,
            })
            .collect();
        for i in INSTRUMENTOS {
            assert!(desligadas.contains(&i.nota_gm), "{}", i.nome);
        }
    }

    /// Instante teórico do tique `k` a `bpm`, em ns inteiros (sem acúmulo): `k·5e9/bpm`.
    fn teorico(inicio: u64, k: u64, bpm: u64) -> u64 {
        inicio + (u128::from(k) * 5_000_000_000 / u128::from(bpm)) as u64
    }

    #[test]
    fn dez_minutos_de_bateria_e_metronomo_em_quadros_de_16_ms_sem_deriva() {
        const BPM: u64 = 117;
        const INICIO: u64 = 123_456_789;
        const DEZ_MIN: u64 = 600_000_000_000;
        let mut r = EstadoRitmo::novo(None);
        r.padrao = padroes_de_fabrica()
            .into_iter()
            .find(|p| p.nome == "Funk")
            .expect("funk");
        r.padrao.bpm = BPM as u16;
        r.config = ConfigRitmo {
            metronomo: true,
            subdivisao: piano_tutor_core::ritmo::Subdivisao::Semicolcheias,
            bateria: true,
            contagem: 0,
        };
        r.iniciar_em(INICIO);

        // Quadros de 16 ms com a antecedência real, como a UI.
        let mut fatiado = Vec::new();
        let mut quadro = 0u64;
        while quadro * 16_000_000 < DEZ_MIN {
            quadro += 1;
            let horizonte = INICIO + quadro * 16_000_000 + ns(ANTECEDENCIA);
            r.planejar(horizonte, &mut fatiado);
        }
        let horizonte_final = INICIO + quadro * 16_000_000 + ns(ANTECEDENCIA);

        // Referência: o mesmo trecho agendado de uma vez só.
        let mut r2 = EstadoRitmo::novo(None);
        r2.padrao = r.padrao.clone();
        r2.config = r.config;
        r2.iniciar_em(INICIO);
        let mut inteiro = Vec::new();
        r2.planejar(horizonte_final, &mut inteiro);
        assert_eq!(fatiado, inteiro, "fatiar em quadros não muda nenhum evento");

        // Todo NoteOn cai exatamente num instante teórico de tique (deriva zero).
        let ons = ligadas(&fatiado);
        assert!(ons.len() > 10_000, "{}", ons.len());
        let mut ultimo_k = 0;
        for (t, nota, _) in &ons {
            let k = ((t - INICIO) as u128 * BPM as u128).div_ceil(5_000_000_000) as u64;
            assert_eq!(
                *t,
                teorico(INICIO, k, BPM),
                "nota {nota} fora do instante teórico"
            );
            assert!(k >= ultimo_k, "ordem dos tiques");
            ultimo_k = k;
        }
        // Metrônomo: um clique forte por compasso, nos tiques múltiplos de 48, e nenhum perdido.
        let fortes: Vec<u64> = ons
            .iter()
            .filter(|x| x.1 == CLIQUE_FORTE)
            .map(|x| x.0)
            .collect();
        let compassos = (horizonte_final - INICIO).div_ceil(48 * 5_000_000_000 / BPM);
        assert!(fortes.len() as u64 >= compassos - 1, "{}", fortes.len());
        for (n, t) in fortes.iter().enumerate() {
            assert_eq!(*t, teorico(INICIO, n as u64 * 48, BPM));
        }
        // Último evento perto de 10 minutos.
        let (ultimo, _, _) = ons[ons.len() - 1];
        assert!(ultimo >= INICIO + DEZ_MIN);
    }

    #[test]
    fn nome_seguro_tira_barras() {
        assert_eq!(nome_seguro("../../etc/x"), "______etc_x");
        assert!(!nome_seguro("../../etc/x").contains('/'));
        assert_eq!(nome_seguro("  Rock 2 "), "Rock 2");
    }
}
