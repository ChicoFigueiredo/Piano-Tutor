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
    Julgamento, Padrao, Posicao, TOLERANCIA_MAX_MS, TOLERANCIA_MIN_MS, TapTempo, corta_nota,
    julgar, padroes_de_fabrica,
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
    /// Define a tolerância (± ms) dentro da faixa aceita
    /// [`TOLERANCIA_MIN_MS`]..=[`TOLERANCIA_MAX_MS`]. Vale para os toques seguintes; os já
    /// julgados não são reavaliados.
    pub fn definir_tolerancia_ms(&mut self, ms: u32) {
        self.tolerancia_ms = ms.clamp(TOLERANCIA_MIN_MS, TOLERANCIA_MAX_MS);
    }

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
    /// Aviso da última troca de andamento feita **durante a execução** (F08): diz ao aluno
    /// que o pulso em curso foi acomodado, sem batida pulada nem clique duplo. Some ao
    /// iniciar ou parar.
    pub aviso_andamento: Option<String>,
    pub pasta: Option<PathBuf>,
    pub nome_arquivo: String,
    /// Peças mudas, na ordem de [`INSTRUMENTOS`]. Estado da sessão: não vai para o arquivo.
    pub mudo: [bool; INSTRUMENTOS.len()],
    /// Peças em solo: havendo alguma, só elas soam (o solo vence o mudo).
    pub solo: [bool; INSTRUMENTOS.len()],
    /// Tolerância e compensação do exercício de pulso, lembradas para a próxima vez que ele é
    /// ligado (vão para o perfil).
    pub pulso_salvo: (u32, i32),
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
            aviso_andamento: None,
            pasta,
            pulso_salvo: (50, 0),
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
        self.aviso_andamento = None;
        if let Some(e) = self.exercicio.as_mut() {
            e.toques.clear();
        }
    }

    /// Para de agendar e marca o corte de segurança: o próximo [`EstadoRitmo::atualizar`]
    /// (um por quadro) agenda NoteOff de todas as peças logo depois do último instante já
    /// agendado, para que nenhuma nota fique soando.
    pub fn parar(&mut self) {
        self.agendador = None;
        self.aviso_andamento = None;
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
        let antes = self.padrao.bpm;
        self.padrao.bpm = bpm.clamp(
            piano_tutor_core::ritmo::BPM_MIN,
            piano_tutor_core::ritmo::BPM_MAX,
        );
        if let Some(a) = self.agendador.as_mut() {
            a.definir_bpm(self.padrao.bpm);
            if self.padrao.bpm != antes {
                self.aviso_andamento = Some(texto_mudanca_de_andamento(antes, self.padrao.bpm));
            }
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

    /// Instante (relógio comum) do primeiro tique do compasso `c`, contado desde o início.
    pub fn instante_do_compasso(&self, c: u64) -> Option<u64> {
        let a = self.agendador.as_ref()?;
        Some(a.instante(c * a.formula().pulsos() * piano_tutor_core::ritmo::TIQUES_POR_PULSO))
    }

    /// Duração de um pulso no andamento atual (só tocando).
    pub fn pulso_ns(&self) -> Option<u64> {
        self.agendador.as_ref().map(Agendador::pulso_ns)
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

/// Texto do aviso quando o andamento muda com o ritmo tocando (F08).
pub fn texto_mudanca_de_andamento(de: u16, para: u16) -> String {
    format!(
        "Andamento mudou de {de} para {para} bpm: o pulso em curso termina num tempo \
         intermediário e os seguintes seguem {para} bpm; nenhuma batida é pulada nem repetida."
    )
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

    // --- Render offline do ritmo no motor de teste (F08-1, F08-4, F09-1, F09-2, F09-3) ------
    //
    // Tudo aqui é **offline**: relógio manual, despertares ideais e o sintetizador falso do
    // motor de teste (um impulso por NoteOn). Mede o posicionamento pelo relógio mestre de
    // áudio, não a latência de um dispositivo real.

    use piano_tutor_audio::jitter::{Estatistica, Pseudoaleatorio};
    use piano_tutor_audio::relogio::{frames_para_ns, ns_para_frames};
    use piano_tutor_audio::{ComandoAudio, ConfigMotor, MotorAudio, Quando, SintetizadorFalso};
    use piano_tutor_core::midi::EventoMidi;

    /// Instante do relógio comum em que o frame 0 do relógio mestre é renderizado.
    const T0_NS: u64 = 1_000_000_000;
    const QUADRO_MS: u64 = 16;
    /// 16 ms a 48 kHz = 768 frames exatos.
    const FRAMES_QUADRO: usize = 768;

    struct Render {
        /// Canal esquerdo renderizado (impulso por NoteOn).
        audio: Vec<f32>,
        registro: Vec<RegistroFalso>,
        /// Latência alvo do motor, em frames (somada a todo `Quando::Instante`).
        latencia: u64,
    }

    impl Render {
        /// Frames com algum impulso no áudio renderizado.
        fn onsets_do_audio(&self) -> Vec<u64> {
            self.audio
                .iter()
                .enumerate()
                .filter(|(_, v)| **v != 0.0)
                .map(|(i, _)| i as u64)
                .collect()
        }

        fn ligadas(&self) -> Vec<(u64, u8, u8)> {
            self.registro
                .iter()
                .filter_map(|e| match *e {
                    RegistroFalso::Ligada {
                        frame, canal, nota, ..
                    } => Some((frame, canal, nota)),
                    _ => None,
                })
                .collect()
        }

        fn frames(&self) -> u64 {
            self.audio.len() as u64
        }

        /// Frame em que o instante `t_ns` do relógio comum deve soar.
        fn frame_ideal(&self, t_ns: u64) -> u64 {
            ns_para_frames(t_ns - T0_NS, taxa()) + self.latencia
        }
    }

    /// Roda `quadros` quadros de 16 ms como a UI: `por_quadro` (para mexer no estado e no
    /// serviço), `atualizar` e o render do quadro. O primeiro render fixa a âncora do relógio
    /// mestre em `T0_NS` **antes** de qualquer agendamento.
    fn renderizar_ritmo(
        r: &mut EstadoRitmo,
        inicio_ns: u64,
        quadros: usize,
        mut por_quadro: impl FnMut(&mut EstadoRitmo, &mut ServicoSom, usize),
    ) -> Render {
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        relogio.avancar(Duration::from_nanos(T0_NS));
        let mut audio: Vec<f32> = motor
            .renderizar_no_relogio(FRAMES_QUADRO)
            .iter()
            .step_by(2)
            .copied()
            .collect();
        relogio.avancar(Duration::from_millis(QUADRO_MS));
        r.iniciar_em(inicio_ns);
        for q in 0..quadros {
            por_quadro(r, &mut som, q);
            r.atualizar(&mut som);
            audio.extend(motor.renderizar_no_relogio(FRAMES_QUADRO).iter().step_by(2));
            relogio.avancar(Duration::from_millis(QUADRO_MS));
        }
        assert_eq!(
            som.comandos_perdidos(),
            0,
            "nenhum comando perdido na fila do motor"
        );
        Render {
            audio,
            registro: motor.registro(),
            latencia: motor.latencia_alvo_frames(),
        }
    }

    fn so_metronomo(bpm: u16) -> EstadoRitmo {
        let mut r = EstadoRitmo::novo(None);
        r.definir_bpm(bpm);
        r.config = ConfigRitmo::default();
        r
    }

    /// Frames ideais dos cliques de um metrônomo de 4/4 a `bpm` que caem no render.
    fn cliques_ideais(render: &Render, inicio: u64, bpm: u64) -> Vec<u64> {
        let mut v = Vec::new();
        for k in 0.. {
            let f = render.frame_ideal(teorico(inicio, k * 12, bpm));
            if f >= render.frames() {
                break;
            }
            v.push(f);
        }
        v
    }

    /// Erro de cada clique em ms (renderizado − ideal), e as estatísticas do aceite:
    /// erro médio absoluto e jitter (|erro − erro médio|) com p95.
    fn estatisticas_de_erro(observados: &[u64], ideais: &[u64]) -> (f64, Estatistica) {
        assert_eq!(observados.len(), ideais.len(), "mesmo número de onsets");
        let ms = |frames: i64| frames as f64 * 1000.0 / f64::from(TAXA_HZ);
        let erros: Vec<f64> = observados
            .iter()
            .zip(ideais)
            .map(|(o, i)| ms(*o as i64 - *i as i64))
            .collect();
        let medio_abs = erros.iter().map(|e| e.abs()).sum::<f64>() / erros.len() as f64;
        let media = erros.iter().sum::<f64>() / erros.len() as f64;
        let jitter: Vec<f64> = erros.iter().map(|e| (e - media).abs()).collect();
        (medio_abs, Estatistica::de(&jitter))
    }

    #[test]
    fn f08_1_a_120_bpm_o_metronomo_renderizado_offline_cumpre_erro_medio_e_jitter() {
        let inicio = T0_NS + 200_000_000;
        let mut r = so_metronomo(120);
        // 60 s de metrônomo em quadros de 16 ms.
        let render = renderizar_ritmo(&mut r, inicio, 3_750, |_, _, _| {});
        let obs = render.onsets_do_audio();
        let ideais = cliques_ideais(&render, inicio, 120);
        assert!(ideais.len() >= 119, "{}", ideais.len());
        // Os impulsos do áudio são os NoteOn do registro (o áudio é a fonte da medição).
        let do_registro: Vec<u64> = render.ligadas().iter().map(|x| x.0).collect();
        assert_eq!(obs, do_registro);
        // Nenhum clique extra e nenhum faltando.
        let (medio, jitter) = estatisticas_de_erro(&obs, &ideais);
        // Semínima a 120 bpm = 500 ms = 24 000 frames exatos a 48 kHz.
        for w in obs.windows(2) {
            assert_eq!(w[1] - w[0], 24_000, "intervalo entre onsets");
        }
        println!(
            "F08-1 offline 120 bpm, 48 kHz, quadros de 16 ms: {} onsets; erro médio {medio:.4} ms; \
             jitter p95 {:.4} ms; jitter máx. {:.4} ms (latência alvo constante {} frames \
             descontada)",
            obs.len(),
            jitter.p95,
            jitter.max,
            render.latencia,
        );
        assert!(medio <= 2.0, "erro médio {medio} ms");
        assert!(jitter.p95 <= 10.0, "jitter p95 {} ms", jitter.p95);
    }

    #[test]
    fn f08_1_o_aceite_vale_em_outros_andamentos_com_periodo_fora_da_grade_de_frames() {
        // 97 bpm e 117 bpm dão períodos que não são inteiros em frames: o erro é só de
        // arredondamento (≤ 1 frame = 0,021 ms). 40 e 220 são os extremos da faixa.
        for bpm in [40u64, 97, 117, 220] {
            let inicio = T0_NS + 200_000_000;
            let mut r = so_metronomo(bpm as u16);
            let render = renderizar_ritmo(&mut r, inicio, 1_875, |_, _, _| {});
            let obs = render.onsets_do_audio();
            let ideais = cliques_ideais(&render, inicio, bpm);
            let (medio, jitter) = estatisticas_de_erro(&obs, &ideais);
            let maior = obs
                .iter()
                .zip(&ideais)
                .map(|(o, i)| o.abs_diff(*i))
                .max()
                .unwrap_or(0);
            println!(
                "F08-1 offline {bpm} bpm: {} onsets; erro médio {medio:.4} ms; jitter p95 {:.4} ms; \
                 maior desvio {maior} frame(s)",
                obs.len(),
                jitter.p95
            );
            assert!(maior <= 1, "{bpm} bpm: {maior} frames");
            assert!(medio <= 2.0 && jitter.p95 <= 10.0, "{bpm} bpm");
        }
    }

    /// Metrônomo pelo caminho real do motor, mas com **despertares do callback irregulares**
    /// (modelo do escalonador do SO, mesmo gerador de `jitter`): cada bloco acorda atrasado
    /// uniformemente em [0, 1 ms], e 5% deles têm 2 ms a mais. É um MODELO, não medição.
    fn medir_com_despertar_irregular(bpm: u16, semente: u64) -> (usize, f64, Estatistica, u64) {
        const BLOCO_FRAMES: usize = 256;
        let config = ConfigMotor::padrao(taxa());
        let latencia = config.latencia_alvo_frames;
        let (mut motor, mut controle) =
            MotorAudio::novo(config, SintetizadorFalso::novo()).expect("motor de teste");
        let mut r = so_metronomo(bpm);
        let inicio = T0_NS + 200_000_000;
        r.iniciar_em(inicio);
        let mut rng = Pseudoaleatorio::novo(semente);
        let mut plano = Vec::new();
        let mut saida = vec![0.0_f32; BLOCO_FRAMES * 2];
        let mut obs: Vec<u64> = Vec::new();
        let blocos = 60 * 48_000 / BLOCO_FRAMES;
        for k in 0..blocos {
            let ideal = T0_NS + frames_para_ns((k * BLOCO_FRAMES) as u64, taxa());
            let mut atraso = rng.ate(1_000_000);
            if rng.ate(99) < 5 {
                atraso += 2_000_000;
            }
            let acordou = ideal + atraso;
            // A UI agenda a cada 16 ms (3 blocos).
            if k % 3 == 0 {
                plano.clear();
                r.planejar(acordou + ns(ANTECEDENCIA), &mut plano);
                for (t, acao) in plano.drain(..) {
                    controle
                        .enviar(ComandoAudio::novo(Quando::Instante(t), acao))
                        .expect("fila");
                }
            }
            let base = motor.frame_atual();
            motor.processar_intercalado(&mut saida, 2, acordou);
            for (i, q) in saida.as_chunks::<2>().0.iter().enumerate() {
                if q[0] != 0.0 {
                    obs.push(base + i as u64);
                }
            }
        }
        let ideais: Vec<u64> = (0..)
            .map(|k| {
                ns_para_frames(teorico(inicio, k * 12, u64::from(bpm)) - T0_NS, taxa()) + latencia
            })
            .take_while(|f| *f < (blocos * BLOCO_FRAMES) as u64)
            .collect();
        let (medio, jitter) = estatisticas_de_erro(&obs, &ideais);
        let atrasados = motor.estado().telemetria().comandos_atrasados;
        (obs.len(), medio, jitter, atrasados)
    }

    #[test]
    fn f08_1_com_despertar_irregular_modelado_continua_dentro_do_aceite() {
        for semente in [1u64, 42, 2026] {
            let (n, medio, jitter, atrasados) = medir_com_despertar_irregular(120, semente);
            println!(
                "F08-1 offline MODELO (despertar 0–1 ms + picos de 2 ms em 5%), 120 bpm, semente \
                 {semente}: {n} onsets; erro médio {medio:.4} ms; jitter p95 {:.4} ms; máx. {:.4} ms; \
                 comandos atrasados {atrasados}",
                jitter.p95, jitter.max
            );
            assert!(n >= 119);
            assert_eq!(atrasados, 0, "latência alvo cobre bloco + despertar");
            assert!(medio <= 2.0, "erro médio {medio} ms");
            assert!(jitter.p95 <= 10.0, "jitter p95 {} ms", jitter.p95);
        }
    }

    // --- F08-4: troca de andamento durante a execução ---------------------------------------

    #[test]
    fn f08_4_mudar_o_andamento_tocando_avisa_o_usuario_com_o_de_e_o_para() {
        let relogio = RelogioManual::novo();
        let (mut som, _m) = servico_pronto(Arc::new(relogio) as Arc<dyn Relogio>);
        let mut r = EstadoRitmo::novo(None);
        r.definir_bpm(120);
        assert_eq!(r.aviso_andamento, None, "parado: nada a avisar");
        r.tocar(&mut som);
        assert_eq!(r.aviso_andamento, None, "iniciar não é troca");
        r.definir_bpm(120);
        assert_eq!(r.aviso_andamento, None, "mesmo andamento: sem aviso");
        r.definir_bpm(90);
        let aviso = r.aviso_andamento.clone().expect("aviso");
        assert_eq!(aviso, texto_mudanca_de_andamento(120, 90));
        assert!(aviso.contains("120") && aviso.contains("90"), "{aviso}");
        assert!(
            aviso.contains("nenhuma batida é pulada nem repetida"),
            "{aviso}"
        );
        // Fora da faixa: o aviso diz o andamento realmente aplicado.
        r.definir_bpm(900);
        assert_eq!(r.aviso_andamento, Some(texto_mudanca_de_andamento(90, 220)));
        // Parar apaga o aviso; trocar parado não cria aviso.
        r.parar();
        assert_eq!(r.aviso_andamento, None);
        r.definir_bpm(60);
        assert_eq!(r.aviso_andamento, None);
        assert_eq!(r.bpm(), 60);
    }

    #[test]
    fn f08_4_o_tap_tempo_tocando_tambem_avisa() {
        let relogio = RelogioManual::novo();
        let (mut som, _m) = servico_pronto(Arc::new(relogio) as Arc<dyn Relogio>);
        let mut r = EstadoRitmo::novo(None);
        r.definir_bpm(120);
        r.tocar(&mut som);
        r.tap(0);
        assert_eq!(r.aviso_andamento, None, "um toque só não estima andamento");
        r.tap(1_000_000_000);
        assert_eq!(r.bpm(), 60);
        assert_eq!(r.aviso_andamento, Some(texto_mudanca_de_andamento(120, 60)));
    }

    #[test]
    fn f08_4_no_audio_renderizado_a_troca_de_andamento_nao_duplica_nem_pula_clique() {
        let inicio = T0_NS + 200_000_000;
        let mut r = so_metronomo(120);
        // 120 → 60 bpm no quadro 100 (1,6 s depois do início do render).
        let render = renderizar_ritmo(&mut r, inicio, 600, |r, _, q| {
            if q == 100 {
                r.definir_bpm(60);
                assert!(r.aviso_andamento.is_some());
            }
        });
        let obs = render.onsets_do_audio();
        let dif: Vec<u64> = obs.windows(2).map(|w| w[1] - w[0]).collect();
        // Antes: 500 ms (24 000 frames). Um intervalo de transição entre 500 ms e 1 s. Depois:
        // 1 s (48 000 frames). Nenhum menor que 500 ms (clique duplo) nem maior que 1 s (pulo).
        let transicoes: Vec<u64> = dif
            .iter()
            .copied()
            .filter(|d| *d != 24_000 && *d != 48_000)
            .collect();
        assert_eq!(transicoes.len(), 1, "{dif:?}");
        assert!((24_000..48_000).contains(&transicoes[0]), "{transicoes:?}");
        assert!(dif.iter().all(|d| (24_000..=48_000).contains(d)), "{dif:?}");
        assert!(dif.iter().filter(|d| **d == 24_000).count() >= 3);
        assert!(dif.iter().filter(|d| **d == 48_000).count() >= 3);
        // A ordem dos acentos segue o compasso: forte a cada 4 pulsos, sem pular o 1.
        let fortes: Vec<usize> = render
            .ligadas()
            .iter()
            .enumerate()
            .filter(|(_, x)| x.2 == CLIQUE_FORTE)
            .map(|(i, _)| i)
            .collect();
        assert!(fortes.windows(2).all(|w| w[1] - w[0] == 4), "{fortes:?}");
    }

    // --- F08-3: tolerância configurável do exercício de pulso --------------------------------

    #[test]
    fn f08_3_tolerancia_configurada_pelo_usuario_muda_o_julgamento_do_toque() {
        let ms = 1_000_000u64;
        let julga = |tolerancia_ms: u32, desvio_ms: i64| -> Julgamento {
            let mut r = EstadoRitmo::novo(None);
            r.definir_bpm(120);
            r.exercicio = Some(ExercicioPulso::default());
            r.exercicio
                .as_mut()
                .expect("exercício")
                .definir_tolerancia_ms(tolerancia_ms);
            let t0 = 10_000 * ms;
            r.iniciar_em(t0);
            // Toque perto do 2º pulso (t0 + 500 ms).
            let t = (t0 as i64 + 500 * ms as i64 + desvio_ms * ms as i64) as u64;
            r.toque(t).expect("julgado").1
        };
        // Toque 30 ms adiantado e 30 ms atrasado: depende da tolerância escolhida.
        assert_eq!(julga(20, -30), Julgamento::Adiantado);
        assert_eq!(julga(20, 30), Julgamento::Atrasado);
        assert_eq!(julga(30, -30), Julgamento::NoTempo);
        assert_eq!(julga(30, 30), Julgamento::NoTempo);
        assert_eq!(julga(100, -30), Julgamento::NoTempo);
        // Faixa: o valor é limitado a 10..=150 ms, em qualquer entrada.
        let mut e = ExercicioPulso::default();
        e.definir_tolerancia_ms(0);
        assert_eq!(e.tolerancia_ms, TOLERANCIA_MIN_MS);
        e.definir_tolerancia_ms(10_000);
        assert_eq!(e.tolerancia_ms, TOLERANCIA_MAX_MS);
        e.definir_tolerancia_ms(75);
        assert_eq!(e.tolerancia_ms, 75);
    }

    #[test]
    fn f08_3_tres_exercicios_adiantado_certo_e_atrasado_sao_resumidos() {
        // Cada exercício toca 8 vezes, deliberadamente adiantado, certo ou atrasado em relação
        // ao pulso, a 120 bpm, com tolerância de 40 ms.
        let ms = 1_000_000i64;
        let rodar = |deslocamento_ms: i64| -> (usize, usize, usize, usize) {
            let mut r = EstadoRitmo::novo(None);
            r.definir_bpm(120);
            let mut e = ExercicioPulso::default();
            e.definir_tolerancia_ms(40);
            r.exercicio = Some(e);
            let t0 = 10_000 * ms as u64;
            r.iniciar_em(t0);
            for k in 1..=8i64 {
                let t = t0 as i64 + k * 500 * ms + deslocamento_ms * ms;
                r.toque(t as u64).expect("julgado");
            }
            let (n, certo, adiantado, atrasado, medio) =
                r.exercicio.as_ref().expect("exercício").resumo();
            assert!(
                (medio - deslocamento_ms.abs() as f64).abs() < 1e-9,
                "{medio}"
            );
            (n, certo, adiantado, atrasado)
        };
        assert_eq!(rodar(-90), (8, 0, 8, 0), "adiantado");
        assert_eq!(rodar(5), (8, 8, 0, 0), "certo");
        assert_eq!(rodar(100), (8, 0, 0, 8), "atrasado");
    }

    // --- F09-1: oito pads, oito sons ---------------------------------------------------------

    #[test]
    fn f09_1_oito_pads_no_mesmo_instante_soam_oito_notas_gm_distintas_no_canal_10() {
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        relogio.avancar(Duration::from_nanos(T0_NS));
        motor.renderizar_no_relogio(FRAMES_QUADRO);
        relogio.avancar(Duration::from_millis(QUADRO_MS));
        let mut r = EstadoRitmo::novo(None);
        for i in 0..8 {
            r.pad(&mut som, i);
        }
        // Pad inexistente não faz nada (nem nota presa).
        r.pad(&mut som, INSTRUMENTOS.len());
        let audio = motor.renderizar_no_relogio(FRAMES_QUADRO);
        // Logo depois do disparo: as 8 notas soam ao mesmo tempo (NoteOff só 90 ms depois).
        assert_eq!(motor.notas_ativas(), 8);
        let ligadas: Vec<(u64, u8, u8)> = motor
            .registro()
            .iter()
            .filter_map(|e| match *e {
                RegistroFalso::Ligada {
                    frame, canal, nota, ..
                } => Some((frame, canal, nota)),
                _ => None,
            })
            .collect();
        assert_eq!(ligadas.len(), 8);
        assert!(ligadas.iter().all(|x| x.1 == CANAL_PERCUSSAO));
        assert!(
            ligadas.iter().all(|x| x.0 == ligadas[0].0),
            "mesmo frame: reprodução simultânea"
        );
        let mut notas: Vec<u8> = ligadas.iter().map(|x| x.2).collect();
        notas.sort_unstable();
        notas.dedup();
        assert_eq!(notas.len(), 8, "8 notas distintas");
        let esperado: Vec<u8> = INSTRUMENTOS[..8].iter().map(|i| i.nota_gm).collect();
        let por_pad: Vec<u8> = ligadas.iter().map(|x| x.2).collect();
        assert_eq!(por_pad, esperado, "pad n toca a peça n");
        // Os 8 NoteOn viram um único impulso somado no frame do disparo (as 8 vozes juntas).
        let impulsos = audio.iter().step_by(2).filter(|v| **v != 0.0).count();
        assert_eq!(impulsos, 1);
        // Depois de 90 ms os 8 NoteOff chegaram: nenhuma nota infinita.
        motor.renderizar_no_relogio(48_000 / 5);
        assert_eq!(motor.notas_ativas(), 0);
    }

    // --- F09-2: grade em 3/4 e 6/8 e render de padrão ----------------------------------------

    /// NoteOn de bateria (canal 10, sem os cliques) do render, `(frame, nota)`, em ordem.
    fn batidas(render: &Render) -> Vec<(u64, u8)> {
        render
            .ligadas()
            .iter()
            .filter(|x| x.1 == CANAL_PERCUSSAO && x.2 != CLIQUE_FORTE && x.2 != CLIQUE_FRACO)
            .map(|x| (x.0, x.2))
            .collect()
    }

    /// Batidas esperadas de `p` (laço de `passos`) no render, calculadas só da notação do
    /// padrão e da fórmula, sem passar pelo agendador.
    fn batidas_esperadas(render: &Render, inicio: u64, p: &Padrao) -> Vec<(u64, u8)> {
        let ticks_passo = p.formula.tiques_por_passo();
        let mut v = Vec::new();
        for passo_abs in 0.. {
            let passo = passo_abs as usize % p.passos();
            let t = teorico(inicio, passo_abs * ticks_passo, u64::from(p.bpm));
            let f = render.frame_ideal(t);
            if f >= render.frames() {
                break;
            }
            for (i, linha) in p.linhas.iter().enumerate() {
                if linha[passo] > 0 {
                    v.push((f, INSTRUMENTOS[i].nota_gm));
                }
            }
        }
        v.sort_unstable();
        v
    }

    fn renderizar_padrao(p: &Padrao, quadros: usize) -> (Render, u64) {
        let inicio = T0_NS + 200_000_000;
        let mut r = EstadoRitmo::novo(None);
        r.padrao = p.clone();
        r.config = so_bateria();
        let render = renderizar_ritmo(&mut r, inicio, quadros, |_, _, _| {});
        (render, inicio)
    }

    fn fabrica(nome: &str) -> Padrao {
        padroes_de_fabrica()
            .into_iter()
            .find(|p| p.nome == nome)
            .unwrap_or_else(|| panic!("padrão {nome}"))
    }

    #[test]
    fn f09_2_padroes_em_4_4_3_4_e_6_8_renderizados_offline_caem_nos_passos_da_notacao() {
        for (nome, formula, passos) in [
            ("Funk", Formula::QuatroQuatro, 16),
            ("Valsa", Formula::TresQuatro, 12),
            ("Balada 6/8", Formula::SeisOito, 12),
        ] {
            let p = fabrica(nome);
            assert_eq!((p.formula, p.passos()), (formula, passos), "{nome}");
            // 12 s: de 2 a 6 compassos conforme o andamento.
            let (render, inicio) = renderizar_padrao(&p, 750);
            let obs = batidas(&render);
            let mut esperado = batidas_esperadas(&render, inicio, &p);
            let mut observado = obs.clone();
            observado.sort_unstable();
            esperado.sort_unstable();
            assert!(
                observado.len() >= 2 * p.linhas.iter().flatten().filter(|v| **v > 0).count() / 2
            );
            assert_eq!(observado.len(), esperado.len(), "{nome}: nº de batidas");
            let maior = observado
                .iter()
                .zip(&esperado)
                .map(|(o, e)| {
                    assert_eq!(o.1, e.1, "{nome}: mesma peça na mesma posição");
                    o.0.abs_diff(e.0)
                })
                .max()
                .unwrap_or(0);
            println!(
                "F09-2 offline {nome} ({}): {} batidas renderizadas, maior desvio {maior} frame(s) \
                 em relação à notação do padrão",
                formula.texto(),
                observado.len()
            );
            assert!(maior <= 1, "{nome}: {maior} frames");
            // Dentro de cada pulso a grade é regular: o 1º compasso do 6/8 e do 3/4 repete.
            let um_compasso = ns_para_frames(
                p.formula.pulsos() * 60_000_000_000 / u64::from(p.bpm),
                taxa(),
            );
            let primeiro: Vec<u8> = observado
                .iter()
                .filter(|x| x.0 < observado[0].0 + um_compasso - 2)
                .map(|x| x.1)
                .collect();
            let segundo: Vec<u8> = observado
                .iter()
                .filter(|x| {
                    x.0 >= observado[0].0 + um_compasso - 2
                        && x.0 < observado[0].0 + 2 * um_compasso - 2
                })
                .map(|x| x.1)
                .collect();
            assert_eq!(primeiro, segundo, "{nome}: compasso 2 repete o 1");
        }
    }

    #[test]
    fn f09_2_padrao_salvo_e_recarregado_renderiza_identico() {
        let pasta = std::env::temp_dir().join(format!("pt-ritmo-render-{}", std::process::id()));
        let mut r = EstadoRitmo::novo(Some(pasta.clone()));
        r.escolher_padrao(fabrica("Bossa nova"));
        r.padrao.alternar(4, 5);
        r.nome_arquivo = "Bossa editada".to_owned();
        r.salvar();
        let original = r.padrao.clone();
        r.escolher_padrao(fabrica("Valsa"));
        r.carregar("Bossa editada");
        assert_eq!(r.padrao, original);
        let recarregado = r.padrao.clone();
        let (a, _) = renderizar_padrao(&original, 500);
        let (b, _) = renderizar_padrao(&recarregado, 500);
        assert_eq!(batidas(&a), batidas(&b), "mesmos frames, mesmas peças");
        assert_eq!(a.onsets_do_audio(), b.onsets_do_audio(), "mesmo áudio");
        assert!(batidas(&a).len() > 50);
        let _ = std::fs::remove_dir_all(pasta);
    }

    // --- F09-3: metrônomo, sequenciador e piano no mesmo relógio -----------------------------

    #[test]
    fn f09_3_metronomo_sequenciador_e_piano_caem_no_mesmo_frame_do_mesmo_relogio_de_audio() {
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        relogio.avancar(Duration::from_nanos(T0_NS));
        let taxa_ = taxa();
        let mut audio: Vec<f32> = motor
            .renderizar_no_relogio(FRAMES_QUADRO)
            .iter()
            .step_by(2)
            .copied()
            .collect();
        relogio.avancar(Duration::from_millis(QUADRO_MS));

        // Rock básico a 120 bpm com metrônomo: kick (36) e chimbal (42) nos passos 0 e 4,
        // clique forte no tique 0 e clique de pulso no tique 12.
        let inicio = T0_NS + 400_000_000;
        let mut r = EstadoRitmo::novo(None);
        r.padrao = fabrica("Rock básico");
        r.padrao.bpm = 120;
        r.config = ConfigRitmo {
            metronomo: true,
            bateria: true,
            ..ConfigRitmo::default()
        };
        r.iniciar_em(inicio);

        // Piano: o teclado físico entrega NoteOn com timestamp do mesmo relógio monotônico.
        // Uma nota exatamente no 1º pulso, outra no 2º pulso (+500 ms) e uma 1 ms fora da grade.
        let canal0 = CanalMidi::novo(0).expect("canal");
        let piano = |nota: u8, t: u64| EventoMidi::NotaLigada {
            canal: canal0,
            nota: NotaMidi::nova(nota).expect("nota"),
            velocidade: 90,
            timestamp_monotonico_ns: t,
        };
        som.evento_midi(&piano(60, inicio));
        som.evento_midi(&piano(62, inicio + 500_000_000));
        som.evento_midi(&piano(64, inicio + 1_000_000_000 + 1_000_000));

        for _ in 0..200 {
            r.atualizar(&mut som);
            audio.extend(motor.renderizar_no_relogio(FRAMES_QUADRO).iter().step_by(2));
            relogio.avancar(Duration::from_millis(QUADRO_MS));
        }
        let registro = motor.registro();
        let frame_de = |canal: u8, nota: u8, n: usize| -> u64 {
            registro
                .iter()
                .filter_map(|e| match *e {
                    RegistroFalso::Ligada {
                        frame,
                        canal: c,
                        nota: x,
                        ..
                    } if c == canal && x == nota => Some(frame),
                    _ => None,
                })
                .nth(n)
                .unwrap_or_else(|| panic!("canal {canal} nota {nota} #{n} não soou"))
        };
        let lat = motor.latencia_alvo_frames();
        let ideal = |t: u64| ns_para_frames(t - T0_NS, taxa_) + lat;
        let p = CANAL_PERCUSSAO;

        // 1º pulso: clique forte, bumbo, chimbal fechado e piano caem no MESMO frame.
        let f0 = frame_de(p, CLIQUE_FORTE, 0);
        assert_eq!(frame_de(p, 36, 0), f0, "bumbo = clique");
        assert_eq!(frame_de(p, 42, 0), f0, "chimbal = clique");
        assert_eq!(frame_de(0, 60, 0), f0, "piano = clique");
        assert!(f0.abs_diff(ideal(inicio)) <= 1);
        // 2º pulso: clique de pulso, e o passo 4 do padrão (caixa 38 e chimbal), e piano.
        let f1 = frame_de(p, CLIQUE_FRACO, 0);
        assert_eq!(
            frame_de(p, 38, 0),
            f1,
            "caixa do passo 4 = clique do 2º pulso"
        );
        assert_eq!(frame_de(0, 62, 0), f1, "piano = clique do 2º pulso");
        assert_eq!(f1 - f0, 24_000, "500 ms entre os pulsos, em frames");
        // Controle negativo: 1 ms fora da grade é 48 frames fora do 3º pulso (a comparação
        // distingue de fato instantes diferentes).
        let f2 = frame_de(p, CLIQUE_FRACO, 1);
        assert_eq!(frame_de(0, 64, 0) - f2, 48);
        // O áudio renderizado tem impulsos exatamente nesses frames.
        let onsets: std::collections::BTreeSet<u64> = audio
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != 0.0)
            .map(|(i, _)| i as u64)
            .collect();
        assert!(onsets.contains(&f0) && onsets.contains(&f1) && onsets.contains(&(f2 + 48)));
        // Um único backend: a saída foi aberta uma vez e metrônomo, bateria e piano usaram-na.
        assert_eq!(
            *motor.aberturas.lock().expect("lock"),
            1,
            "uma só saída de áudio aberta"
        );
    }

    #[test]
    fn f09_4_dez_minutos_de_bateria_metronomo_e_piano_juntos_no_mesmo_relogio_de_audio() {
        // Offline: 10 min a 120 bpm, Funk com metrônomo, e um "aluno" que toca uma nota de
        // piano em cada pulso (NoteOn com timestamp do relógio comum, entregue ~150 ms antes).
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        relogio.avancar(Duration::from_nanos(T0_NS));
        motor.renderizar_no_relogio(FRAMES_QUADRO);
        relogio.avancar(Duration::from_millis(QUADRO_MS));
        let inicio = T0_NS + 200_000_000;
        let mut r = EstadoRitmo::novo(None);
        r.padrao = fabrica("Funk");
        r.padrao.bpm = 120;
        r.config = ConfigRitmo {
            metronomo: true,
            bateria: true,
            ..ConfigRitmo::default()
        };
        r.iniciar_em(inicio);
        let canal0 = CanalMidi::novo(0).expect("canal");
        let nota = NotaMidi::nova(60).expect("nota");
        let evento = |ligada: bool, t: u64| {
            if ligada {
                EventoMidi::NotaLigada {
                    canal: canal0,
                    nota,
                    velocidade: 90,
                    timestamp_monotonico_ns: t,
                }
            } else {
                EventoMidi::NotaDesligada {
                    canal: canal0,
                    nota,
                    velocidade: 0,
                    timestamp_monotonico_ns: t,
                }
            }
        };
        let quadros = 37_500; // 600 s
        let mut proximo_pulso = 0u64;
        for _ in 0..quadros {
            let horizonte = som.agora_ns() + 150_000_000;
            while inicio + proximo_pulso * 500_000_000 < horizonte {
                let t = inicio + proximo_pulso * 500_000_000;
                som.evento_midi(&evento(true, t));
                som.evento_midi(&evento(false, t + 100_000_000));
                proximo_pulso += 1;
            }
            r.atualizar(&mut som);
            motor.renderizar_no_relogio(FRAMES_QUADRO);
            relogio.avancar(Duration::from_millis(QUADRO_MS));
        }
        assert_eq!(som.comandos_perdidos(), 0);
        let registro = motor.registro();
        let frames_de = |canal: u8, nota: u8| -> Vec<u64> {
            registro
                .iter()
                .filter_map(|e| match *e {
                    RegistroFalso::Ligada {
                        frame,
                        canal: c,
                        nota: n,
                        ..
                    } if c == canal && n == nota => Some(frame),
                    _ => None,
                })
                .collect()
        };
        // Cliques de pulso: forte (76) no 1 de cada 4 e fraco (77) nos outros 3.
        let mut cliques: Vec<u64> = frames_de(CANAL_PERCUSSAO, CLIQUE_FORTE);
        cliques.extend(frames_de(CANAL_PERCUSSAO, CLIQUE_FRACO));
        cliques.sort_unstable();
        let piano = frames_de(0, 60);
        let n = piano.len().min(cliques.len());
        assert!(n >= 1_190, "{n} pulsos em 10 min a 120 bpm");
        // Os últimos pulsos do render podem ter só um dos lados já entregue ao motor.
        let n = n - 1;
        let erros: Vec<f64> = (0..n)
            .map(|k| piano[k] as f64 - cliques[k] as f64)
            .collect();
        assert!(
            erros.iter().all(|e| *e == 0.0),
            "piano e metrônomo no mesmo frame em todos os {n} pulsos"
        );
        // O tique de cada pulso está no frame teórico (deriva zero em 10 min).
        let lat = motor.latencia_alvo_frames();
        let desvios: Vec<f64> = (0..n)
            .map(|k| {
                let ideal = ns_para_frames(inicio + k as u64 * 500_000_000 - T0_NS, taxa()) + lat;
                piano[k].abs_diff(ideal) as f64
            })
            .collect();
        let est = Estatistica::de(&desvios);
        println!(
            "F09-4 offline 10 min, 120 bpm: {n} pulsos com piano e metrônomo; diferença piano-clique \
             0 frame em todos; desvio ao tique teórico: média {:.4} frame, p95 {:.4} frame \
             (= {:.4} ms), máx. {:.4} frame; bateria (Funk): {} NoteOn no canal 10",
            est.media,
            est.p95,
            est.p95 * 1000.0 / f64::from(TAXA_HZ),
            est.max,
            registro
                .iter()
                .filter(|e| matches!(e, RegistroFalso::Ligada { canal, .. } if *canal == CANAL_PERCUSSAO))
                .count()
        );
        assert!(est.p95 * 1000.0 / f64::from(TAXA_HZ) <= 10.0);
        assert!(est.max <= 1.0);
        assert!(motor.notas_ativas() <= 20, "notas presas no fim do render");
    }
}
