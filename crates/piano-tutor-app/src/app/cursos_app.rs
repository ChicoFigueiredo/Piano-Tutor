//! Os Cursos no app (M4): liga o estado dos Cursos, o professor falante e a Chamada e resposta
//! ao laço por quadro, às notas do aluno e ao motor de som.
//!
//! - [`AppPianoTutor::atualizar_cursos`] roda a cada `tick` (sem janela): lê o `mpv`, a
//!   instalação, a prática do Exercício de Curso e o exercício de fábrica, faz o professor
//!   dizer as falas (aprovadas, e as não revisadas se a preferência deixar) e anda a Chamada e resposta;
//! - [`AppPianoTutor::nota_para_cursos`] entrega a nota do aluno à Chamada (quando há) ou à
//!   prática do Exercício de Curso;
//! - [`AppPianoTutor::ao_mudar_de_rota`] encerra o `mpv`, a voz e a Chamada ao sair da tela.

use super::*;
use crate::chamada_app::{OrigemChamada, SaidaChamada, chamadas_da_partitura, chamadas_de_fabrica};
use crate::cursos::{EventoFabrica, EventoPratica, Mensagem, mao_para_partitura};
use crate::professor_falante::Momento;
use crate::tela_cursos::AcaoCursos;
use piano_tutor_core::NotaMidi;
use piano_tutor_core::comparador::EventoEsperado;

/// Folga entre pedir o trecho do professor e a primeira nota soar.
const FOLGA_DO_PROFESSOR_NS: u64 = 30_000_000;

impl AppPianoTutor {
    fn agora_s(&self) -> f64 {
        self.som.agora_ns() as f64 / 1e9
    }

    /// Roteiro de fábrica com a revisão do Chico aplicada; sem ela (ou com erro), o embutido.
    fn roteiro_da_fabrica(&self) -> Option<crate::professor_falante::Roteiro> {
        self.pasta_dados
            .as_deref()
            .and_then(|d| crate::revisao::roteiro_de_fabrica_revisado(d).ok())
    }

    /// Um passo por quadro dos Cursos, do professor e da Chamada e resposta.
    pub(super) fn atualizar_cursos(&mut self) {
        let agora_s = self.agora_s();
        for ev in self.cursos.atualizar() {
            match ev {
                EventoPratica::Passo(i) => {
                    self.professor.dizer(Momento::Passo, Some(i), false);
                }
                EventoPratica::FimDeTrecho { limpo } => {
                    // Acerto e erro são comentários de trecho, nunca de nota.
                    let m = if limpo {
                        Momento::Acerto
                    } else {
                        Momento::Erro
                    };
                    self.professor.dizer(m, None, false);
                }
                EventoPratica::Concluida { .. } => {
                    self.professor.dizer(Momento::Depois, None, false);
                }
            }
        }
        if self.rota_atual == ROTA_EXERCICIOS {
            for ev in self.observador_fabrica.observar(&self.exercicios) {
                match ev {
                    EventoFabrica::Abriu(n) => {
                        let roteiro = self.roteiro_da_fabrica();
                        self.professor.abrir_fabrica(&n, roteiro);
                    }
                    EventoFabrica::Concluiu { sem_erros } => {
                        let m = if sem_erros {
                            Momento::Acerto
                        } else {
                            Momento::Erro
                        };
                        self.professor.dizer(m, None, false);
                        self.professor.dizer(Momento::Depois, None, false);
                    }
                }
            }
        }
        if let Some(mut c) = self.chamada.take() {
            if !c.concluida() {
                let s = c.quadro(agora_s, &mut self.professor);
                self.aplicar_saida_chamada(&mut c, s);
            }
            self.chamada = Some(c);
        }
        self.professor.atualizar(agora_s);
    }

    /// A nota do aluno (note-on) vai para a Chamada em andamento; sem ela, para a prática do
    /// Exercício de Curso. Fora das telas de Cursos e Exercícios não faz nada.
    pub(super) fn nota_para_cursos(&mut self, ligada: bool, nota: NotaMidi, instante_ns: u64) {
        if self.rota_atual != ROTA_CURSOS && self.rota_atual != ROTA_EXERCICIOS {
            return;
        }
        if let Some(mut c) = self.chamada.take() {
            if ligada && !c.concluida() {
                let s = c.nota_tocada(nota.numero(), instante_ns as f64 / 1e9, &mut self.professor);
                self.aplicar_saida_chamada(&mut c, s);
            }
            self.chamada = Some(c);
            return;
        }
        if self.rota_atual == ROTA_CURSOS {
            self.cursos.partitura.nota_tocada(ligada, nota, instante_ns);
            if ligada && self.cursos.player.is_some() {
                self.cursos.notas_do_aluno.push(nota.numero());
            }
        }
    }

    /// Ao trocar de rota: o que era dos Cursos ou do professor não continua em segundo plano.
    pub(super) fn ao_mudar_de_rota(&mut self, anterior: &str) {
        let atual = self.rota_atual.clone();
        if anterior == atual {
            return;
        }
        let eram_cursos = anterior == ROTA_CURSOS || anterior == ROTA_EXERCICIOS;
        if eram_cursos {
            self.parar_chamada();
            self.professor.fechar();
        }
        if anterior == ROTA_CURSOS {
            self.cursos.ao_sair_da_tela();
            self.cursos.partitura.parar(&mut self.som);
            self.cursos.partitura.limpar_pratica();
        }
        if atual == ROTA_EXERCICIOS {
            self.observador_fabrica.reiniciar();
        }
    }

    /// Corta a Chamada e resposta (e o som do professor) se houver.
    pub fn parar_chamada(&mut self) {
        if self.chamada.take().is_some() {
            self.professor.parar();
            self.som.panico();
        }
    }

    /// Toca os trechos do professor no canal de demonstração.
    fn tocar_trecho(&mut self, notas: &[EventoEsperado]) {
        let base = self.som.agora_ns().saturating_add(FOLGA_DO_PROFESSOR_NS);
        for e in notas {
            let Some(n) = NotaMidi::nova(e.midi) else {
                continue;
            };
            let ini = base.saturating_add((e.inicio_s.max(0.0) * 1e9) as u64);
            let dur = std::time::Duration::from_secs_f64(e.duracao_s.clamp(0.08, 10.0));
            self.som.demonstrar(&[n], ini, dur);
        }
    }

    fn aplicar_saida_chamada(&mut self, c: &mut ChamadaAtiva, s: SaidaChamada) {
        for t in &s.tocar {
            self.tocar_trecho(t);
        }
        if s.concluida.is_some() {
            if matches!(c.origem, OrigemChamada::Curso { .. }) {
                self.cursos.registrar_resultado(c.precisao_pct());
            }
            self.professor.dizer(Momento::Depois, None, false);
        }
    }

    fn comecar_chamada(
        &mut self,
        chamadas: Vec<piano_tutor_core::chamada_resposta::Chamada>,
        origem: OrigemChamada,
        titulo: String,
    ) -> Result<(), String> {
        self.parar_chamada();
        self.cursos.parar_professor();
        self.cursos.partitura.parar(&mut self.som);
        self.cursos.partitura.limpar_pratica();
        let mut c = ChamadaAtiva::nova(chamadas, origem, titulo, self.professor.com_voz())?;
        let s = c.iniciar(self.agora_s(), &mut self.professor);
        self.aplicar_saida_chamada(&mut c, s);
        self.chamada = Some(c);
        Ok(())
    }

    fn iniciar_chamada_do_curso(&mut self) {
        let Some(a) = &self.cursos.aberto else {
            return;
        };
        let origem = OrigemChamada::Curso {
            curso_id: a.curso_id.clone(),
            exercicio_id: a.exercicio.id.clone(),
        };
        let titulo = a.exercicio.titulo.clone();
        let esperados = self
            .cursos
            .partitura
            .esperados_para_chamada(mao_para_partitura(a.exercicio.mao));
        if let Err(e) = self.comecar_chamada(chamadas_da_partitura(&esperados), origem, titulo) {
            self.cursos.mensagem = Some(Mensagem::erro(e));
        }
    }

    fn iniciar_chamada_de_fabrica(&mut self) {
        let Some(ex) = self.exercicios.exercicio() else {
            return;
        };
        let (id, titulo) = (ex.id.clone(), ex.titulo.to_owned());
        let Some(chamadas) = chamadas_de_fabrica(ex) else {
            self.exercicios.mensagem =
                Some("Chamada e resposta só existe nos exercícios de acordes e intervalos.".into());
            return;
        };
        if let Err(e) =
            self.comecar_chamada(chamadas, OrigemChamada::Fabrica { exercicio: id }, titulo)
        {
            self.exercicios.mensagem = Some(e);
        }
    }

    /// Executa o que as telas de Cursos e Exercícios pediram.
    pub(super) fn executar_acoes_cursos(&mut self, ctx: &egui::Context, acoes: Vec<AcaoCursos>) {
        for a in acoes {
            match a {
                AcaoCursos::AbriuExercicio => {
                    self.parar_chamada();
                    if let Some(x) = &self.cursos.aberto {
                        let (licao, ex, roteiro) = (
                            x.licao_id.clone(),
                            x.exercicio.id.clone(),
                            x.roteiro.clone(),
                        );
                        self.professor.abrir(&licao, &ex, roteiro);
                    }
                }
                AcaoCursos::OuvirProfessor => {
                    self.parar_chamada();
                    self.professor.parar();
                    let _ = self.cursos.ouvir_professor();
                }
                AcaoCursos::IniciarChamadaCurso => self.iniciar_chamada_do_curso(),
                AcaoCursos::IniciarChamadaFabrica => self.iniciar_chamada_de_fabrica(),
                AcaoCursos::PararChamada => self.parar_chamada(),
                AcaoCursos::ProfessorFala(ligado) => self.professor.definir_ligado(ligado),
                AcaoCursos::UsarNaoRevisado(usar) => {
                    self.professor.definir_usar_nao_revisado(usar);
                    self.cursos.usar_nao_revisado = usar;
                    // Desligou com um exercício não revisado aberto: ele deixa de valer.
                    if !usar
                        && self
                            .cursos
                            .aberto
                            .as_ref()
                            .is_some_and(|a| a.exercicio.nao_revisado())
                    {
                        self.parar_chamada();
                        self.professor.fechar();
                        self.cursos.fechar_exercicio();
                    }
                }
                AcaoCursos::IrParaFabrica(i) => {
                    if self.exercicios.selecionar(i) {
                        abrir(ctx, ROTA_EXERCICIOS);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod testes {
    //! Os Cursos no app, sem janela (relógio manual, som falso, voz falsa e `mpv` falso) e a
    //! tela desenhada num `egui::Context` sintético. Nada executa `mpv` nem `piper`.
    use super::*;
    use crate::cursos::apoio_testes::{ID_CURSO, instalar_sintetico};
    use crate::mpv::apoio_testes::{ExecutorFalso, midia_falsa};
    use crate::professor_falante::{Voz, VozFalsa, roteiro_de_fabrica};
    use crate::som::apoio_testes::{MotorDeTeste, servico_pronto};
    use bancada_casca::Casca;
    use bancada_casca::egui::{Context, Event, RawInput, Rect, pos2, vec2};
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_core::chamada_resposta::Estado;
    use piano_tutor_midi::RelogioManual;
    use std::time::Duration;

    struct Bancada {
        app: AppPianoTutor,
        relogio: RelogioManual,
        motor: MotorDeTeste,
        voz: VozFalsa,
        mpv: ExecutorFalso,
        _dados: tempfile::TempDir,
    }

    fn bancada(com_curso: bool) -> Bancada {
        let dados = tempfile::tempdir().expect("dados");
        if com_curso {
            instalar_sintetico(dados.path(), None);
        }
        let relogio = RelogioManual::novo();
        let comum: Arc<dyn Relogio> = Arc::new(relogio.clone());
        let (som, motor) = servico_pronto(Arc::clone(&comum));
        let mut app = AppPianoTutor::com_dispositivos(Dispositivos {
            fonte_midi: Box::new(FonteFalsa::nova(Vec::<String>::new())),
            relogio: comum,
            som,
            pasta_sessoes: Some(dados.path().join("sessoes")),
            baixador_sf2: None,
        });
        let voz = VozFalsa::nova();
        app.professor.trocar_voz(Box::new(voz.clone()));
        let (midia, mpv) = midia_falsa();
        app.cursos.midia = midia;
        Bancada {
            app,
            relogio,
            motor,
            voz,
            mpv,
            _dados: dados,
        }
    }

    impl Bancada {
        /// Avança `s` segundos em passos de 20 ms; a voz falsa "termina" a fala na hora.
        fn andar(&mut self, s: f64) {
            for _ in 0..(s / 0.02).round() as usize {
                if self.voz.falando() {
                    self.voz.terminar_a_fala();
                }
                self.relogio.avancar(Duration::from_millis(20));
                self.app.tick();
            }
        }

        fn abrir_curso(&mut self, exercicio: &str) {
            self.app.rota_atual = ROTA_CURSOS.to_owned();
            self.app.cursos.selecionar(1);
            self.app
                .cursos
                .abrir_exercicio("s03-a14", exercicio)
                .expect("abre");
            self.app
                .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::AbriuExercicio]);
        }

        fn tocadas(&self) -> Vec<(u8, f64)> {
            self.motor.renderizar(48_000 * 60, 0);
            self.motor
                .registro()
                .iter()
                .filter_map(|r| match *r {
                    RegistroFalso::Ligada { frame, nota, .. } => {
                        Some((nota, frame as f64 / 48_000.0))
                    }
                    _ => None,
                })
                .collect()
        }
    }

    const ROTEIRO_REVISADO: &str = r#"
[[falas]]
id = "fabrica-ex08-antes"
estado = "aprovada"
"#;

    #[test]
    fn exercicio_de_curso_o_professor_diz_antes_passo_e_depois_so_as_aprovadas() {
        let mut b = bancada(true);
        b.abrir_curso("cinco-notas");
        b.andar(0.1);
        assert_eq!(
            b.voz.pedidos(),
            ["Vamos tocar as cinco notas."],
            "só a aprovada; a hipótese fica calada"
        );
        assert_eq!(
            b.app.professor.legenda(),
            Some("Vamos tocar as cinco notas.")
        );
        // Praticar: ao chegar na nota 4 diz o passo; ao terminar, o acerto e o depois.
        let agora = b.app.som.agora_ns();
        b.app.cursos.partitura.iniciar_pratica(agora);
        let notas = [60u8, 62, 64, 65, 67, 69, 71, 72];
        for i in 0..16 {
            b.andar(0.1);
            let n = NotaMidi::nova(notas[i % 8]).expect("nota");
            let t = b.app.som.agora_ns();
            b.app.nota_para_cursos(true, n, t);
        }
        b.andar(0.5);
        let ditas = b.voz.pedidos();
        assert!(ditas.contains(&"Agora o Sol.".to_owned()), "{ditas:?}");
        assert!(
            ditas.contains(&"Muito bem.".to_owned()) || ditas.contains(&"Isso mesmo.".to_owned()),
            "{ditas:?}"
        );
        assert_eq!(
            ditas.last().map(String::as_str),
            Some("Terminamos este exercício.")
        );
        assert!(b.app.cursos.progresso(ID_CURSO).concluido("cinco-notas"));
        // Desligando o professor ele cala e nada novo é dito.
        b.app
            .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::ProfessorFala(false)]);
        let n = b.voz.pedidos().len();
        b.app.cursos.partitura.limpar_pratica();
        let agora = b.app.som.agora_ns();
        b.app.cursos.partitura.iniciar_pratica(agora);
        b.andar(0.5);
        assert_eq!(b.voz.pedidos().len(), n);
    }

    #[test]
    fn nao_revisado_abre_e_toca_com_a_preferencia_ligada_e_fecha_ao_desligar() {
        let mut b = bancada(true);
        assert!(b.app.cursos.usar_nao_revisado && b.app.professor.usar_nao_revisado());
        b.abrir_curso("ainda-hipotese");
        b.andar(0.1);
        let a = b.app.cursos.aberto.as_ref().expect("aberto");
        assert!(a.exercicio.nao_revisado(), "a tela mostra o selo");
        // O "antes" da lição tem uma aprovada e uma hipótese: a aprovada tem prioridade.
        assert_eq!(b.voz.pedidos(), ["Vamos tocar as cinco notas."]);
        assert!(!b.app.professor.legenda_nao_revisada());
        // Pratica de fato: a partitura carregou e aceita notas.
        let agora = b.app.som.agora_ns();
        b.app.cursos.partitura.iniciar_pratica(agora);
        b.andar(0.2);
        // Desligar fecha o exercício não revisado e o professor deixa de usá-lo.
        b.app.executar_acoes_cursos(
            &Context::default(),
            vec![AcaoCursos::UsarNaoRevisado(false)],
        );
        assert!(b.app.cursos.aberto.is_none());
        assert!(!b.app.cursos.usar_nao_revisado && !b.app.professor.usar_nao_revisado());
        assert!(
            b.app
                .cursos
                .abrir_exercicio("s03-a14", "em-revisao")
                .is_err()
        );
        assert!(
            b.app
                .cursos
                .abrir_exercicio("s03-a14", "cinco-notas")
                .is_ok()
        );
        // Rejeitado, nunca; ligando de novo, os não revisados voltam.
        b.app
            .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::UsarNaoRevisado(true)]);
        assert!(
            b.app
                .cursos
                .abrir_exercicio("s03-a14", "rejeitado")
                .is_err()
        );
        assert!(
            b.app
                .cursos
                .abrir_exercicio("s03-a14", "em-revisao")
                .is_ok()
        );
        // A preferência está gravada: um professor novo, na mesma pasta, a lê.
        let dados = b.app.pasta_dados.clone().expect("dados");
        b.app.executar_acoes_cursos(
            &Context::default(),
            vec![AcaoCursos::UsarNaoRevisado(false)],
        );
        let novo = AppPianoTutor::com_dispositivos(Dispositivos {
            fonte_midi: Box::new(FonteFalsa::nova(Vec::<String>::new())),
            relogio: Arc::new(RelogioManual::novo()),
            som: servico_pronto(Arc::new(RelogioManual::novo())).0,
            pasta_sessoes: Some(dados.join("sessoes")),
            baixador_sf2: None,
        });
        assert!(!novo.professor.usar_nao_revisado());
        assert!(
            !novo.cursos.usar_nao_revisado,
            "o estado dos Cursos acompanha"
        );
    }

    #[test]
    fn professor_comenta_erro_so_ao_fim_do_trecho_nunca_a_cada_nota() {
        let mut b = bancada(true);
        b.abrir_curso("cinco-notas");
        b.andar(0.1);
        let antes = b.voz.pedidos().len();
        let agora = b.app.som.agora_ns();
        b.app.cursos.partitura.iniciar_pratica(agora);
        // Nota errada isolada: só o retorno visual, nenhuma fala.
        b.andar(0.1);
        let t = b.app.som.agora_ns();
        b.app
            .nota_para_cursos(true, NotaMidi::nova(61).expect("nota"), t);
        b.andar(0.5);
        assert_eq!(b.voz.pedidos().len(), antes, "{:?}", b.voz.pedidos());
        // Toca as 4 primeiras notas certas e chega ao começo da próxima frase (passo 4): o
        // trecho acabou com erro, então vem o comentário de erro, depois a fala do passo.
        let notas = [60u8, 62, 64, 65, 67];
        for n in notas {
            b.andar(0.1);
            let t = b.app.som.agora_ns();
            b.app
                .nota_para_cursos(true, NotaMidi::nova(n).expect("nota"), t);
        }
        b.andar(1.0);
        let ditas = b.voz.pedidos();
        assert_eq!(
            &ditas[antes..],
            ["Calma, confira a nota.", "Agora o Sol."],
            "{ditas:?}"
        );
    }

    #[test]
    fn chamada_e_resposta_de_ponta_a_ponta_voz_som_notas_e_progresso() {
        let mut b = bancada(true);
        b.abrir_curso("ida-e-volta");
        b.app
            .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::IniciarChamadaCurso]);
        let c = b.app.chamada.as_ref().expect("chamada");
        assert_eq!(c.texto_do_estado(), "Ouça…");
        let anuncio = c.anuncio.clone().expect("anúncio");
        assert!(anuncio.contains("Frase 1"), "{anuncio}");
        assert!(
            b.voz.pedidos().contains(&anuncio),
            "o professor disse o anúncio"
        );

        let esperados = b.app.cursos.partitura.esperados_para_chamada(None);
        assert_eq!(esperados.len(), 16);
        // 8 notas por frase (2 compassos), no andamento da partitura.
        let frase1: Vec<(u8, f64)> = esperados[..8]
            .iter()
            .map(|e| (e.midi, e.inicio_s - esperados[0].inicio_s))
            .collect();
        let frase2: Vec<(u8, f64)> = esperados[8..]
            .iter()
            .map(|e| (e.midi, e.inicio_s - esperados[8].inicio_s))
            .collect();

        for (n_frase, frase) in [frase1, frase2].into_iter().enumerate() {
            // Espera a vez do aluno (anúncio, professor tocando e folga).
            let mut quadros = 0;
            while b.app.chamada.as_ref().map(ChamadaAtiva::estado) != Some(Estado::AguardandoAluno)
            {
                b.andar(0.02);
                quadros += 1;
                assert!(
                    quadros < 2000,
                    "nunca chegou a vez do aluno (frase {n_frase})"
                );
            }
            assert_eq!(
                b.app.chamada.as_ref().map(ChamadaAtiva::texto_do_estado),
                Some("Sua vez".to_owned())
            );
            // O aluno repete a frase nos mesmos tempos.
            let inicio = b.app.som.agora_ns();
            for (midi, dt) in &frase {
                let alvo = inicio + (dt * 1e9) as u64;
                while b.app.som.agora_ns() < alvo {
                    b.andar(0.02);
                }
                let n = NotaMidi::nova(*midi).expect("nota");
                let t = b.app.som.agora_ns();
                b.app.nota_para_cursos(true, n, t);
            }
            b.andar(1.0);
            let f = b.app.chamada.as_ref().and_then(|c| c.feedback.clone());
            assert!(
                f.as_ref().is_some_and(|f| f.acertou),
                "frase {n_frase}: {f:?}"
            );
            b.andar(3.0);
        }
        let c = b.app.chamada.as_ref().expect("chamada");
        assert!(c.concluida());
        assert_eq!(c.resumo().acertadas(), 2);
        assert_eq!(c.texto_do_estado(), "Fim: 2 de 2 chamadas certas");
        // Progresso do Curso gravado e o `depois` dito.
        assert!(b.app.cursos.progresso(ID_CURSO).concluido("ida-e-volta"));
        assert_eq!(
            b.voz.pedidos().last().map(String::as_str),
            Some("Terminamos este exercício.")
        );
        // O professor tocou as duas frases no motor: 16 notas, na ordem da partitura.
        let ligadas = b.tocadas();
        let do_professor: Vec<u8> = ligadas.iter().map(|(n, _)| *n).collect();
        // As notas do aluno entram pelo MIDI (não passam por aqui): só o professor, 2 frases.
        assert_eq!(
            do_professor,
            [
                60, 62, 64, 65, 67, 69, 71, 72, 60, 62, 64, 65, 67, 69, 71, 72
            ],
            "as duas frases tocadas pelo professor, na ordem da partitura"
        );
    }

    #[test]
    fn errar_na_chamada_mostra_tente_de_novo_e_o_professor_repete_o_trecho() {
        let mut b = bancada(true);
        b.abrir_curso("ida-e-volta");
        b.app
            .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::IniciarChamadaCurso]);
        let mut quadros = 0;
        while b.app.chamada.as_ref().map(ChamadaAtiva::estado) != Some(Estado::AguardandoAluno) {
            b.andar(0.02);
            quadros += 1;
            assert!(quadros < 2000);
        }
        // Uma nota errada e silêncio.
        let t = b.app.som.agora_ns();
        b.app
            .nota_para_cursos(true, NotaMidi::nova(61).expect("nota"), t);
        b.andar(2.2);
        let c = b.app.chamada.as_ref().expect("chamada");
        assert_eq!(c.texto_do_estado(), "[X] Tente de novo (2/3)");
        assert!(!c.concluida());
        // Passada a pausa, o professor toca a frase de novo (16 notas: 2 vezes as 8 da frase 1).
        b.andar(4.0);
        let n_professor = b.tocadas().len();
        assert!(n_professor >= 16, "{n_professor}");
        // Sair da tela corta a Chamada.
        b.app.rota_atual = ROTA_INICIO.to_owned();
        b.app.ao_mudar_de_rota(ROTA_CURSOS);
        assert!(b.app.chamada.is_none());
    }

    #[test]
    fn chamada_sem_voz_instalada_usa_a_estimativa_e_ainda_chega_a_vez_do_aluno() {
        let mut b = bancada(true);
        b.app
            .professor
            .trocar_voz(Box::new(crate::professor_falante::VozMuda::nova()));
        b.abrir_curso("ida-e-volta");
        b.app
            .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::IniciarChamadaCurso]);
        assert!(
            b.app
                .professor
                .aviso_de_voz()
                .expect("aviso")
                .contains("Piper")
        );
        let mut quadros = 0;
        while b.app.chamada.as_ref().map(ChamadaAtiva::estado) != Some(Estado::AguardandoAluno) {
            b.andar(0.02);
            quadros += 1;
            assert!(quadros < 3000, "travou no anúncio sem voz");
        }
        assert!(
            b.app
                .professor
                .legenda()
                .is_some_and(|l| l.contains("Frase 1"))
        );
    }

    #[test]
    fn ouvir_o_professor_para_ao_sair_da_tela_e_nao_trava_sem_mpv() {
        let mut b = bancada(true);
        b.abrir_curso("cinco-notas");
        b.app
            .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::OuvirProfessor]);
        assert_eq!(b.mpv.comandos().len(), 1);
        assert_eq!(b.mpv.vivos(), [true]);
        b.app.rota_atual = ROTA_INICIO.to_owned();
        b.app.ao_mudar_de_rota(ROTA_CURSOS);
        assert_eq!(b.mpv.vivos(), [false], "encerrado ao sair da tela");
        assert!(!b.app.cursos.midia.audio_tocando());
        assert!(b.voz.vezes_que_parou() > 0);
    }

    #[test]
    fn fabrica_o_professor_usa_a_revisao_aprovada_e_a_chamada_cobre_so_acordes_e_intervalos() {
        let mut b = bancada(false);
        let dados = b.app.pasta_dados.clone().expect("dados");
        std::fs::write(dados.join("roteiro-fabrica-revisao.toml"), ROTEIRO_REVISADO)
            .expect("revisão");
        // Libera o exercício 8 (tríades) e abre a tela de Exercícios.
        for p in b.app.exercicios.progresso.iter_mut().take(7) {
            p.concluido = true;
        }
        assert!(b.app.exercicios.selecionar(7));
        b.app.rota_atual = ROTA_EXERCICIOS.to_owned();
        b.andar(0.1);
        let esperada = roteiro_de_fabrica()
            .expect("embutido")
            .falas
            .into_iter()
            .find(|f| f.id == "fabrica-ex08-antes")
            .expect("fala")
            .texto;
        assert_eq!(b.voz.pedidos(), [esperada], "só a aprovada na revisão");
        assert!(b.app.professor.legenda().is_some());
        // Chamada e resposta em acordes: o anúncio vem do núcleo.
        b.app
            .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::IniciarChamadaFabrica]);
        let c = b.app.chamada.as_ref().expect("chamada");
        assert_eq!(
            c.origem,
            OrigemChamada::Fabrica {
                exercicio: "primeiros-passos/8".into()
            }
        );
        assert_eq!(
            c.anuncio.as_deref(),
            Some("Vamos tocar em chamada e resposta. Agora o acorde de Dó.")
        );
        // Com a chamada ativa, as notas do aluno não passam pelo exercício comum.
        let antes = b.app.exercicios.tentativa.registro.len();
        let t = b.app.som.agora_ns();
        b.app
            .nota_para_cursos(true, NotaMidi::nova(60).expect("nota"), t);
        assert_eq!(b.app.exercicios.tentativa.registro.len(), antes);
        // Exercício sem acorde ou intervalo: sem chamada.
        b.app.parar_chamada();
        assert!(b.app.exercicios.selecionar(1));
        b.app
            .executar_acoes_cursos(&Context::default(), vec![AcaoCursos::IniciarChamadaFabrica]);
        assert!(b.app.chamada.is_none());
        assert!(
            b.app
                .exercicios
                .mensagem
                .as_deref()
                .is_some_and(|m| m.contains("acordes e intervalos"))
        );
    }

    // --- a janela desenha ---

    fn casca_com(b: Bancada) -> (Casca<AppPianoTutor>, tempfile::TempDir, MotorDeTeste) {
        let Bancada {
            app, motor, _dados, ..
        } = b;
        (
            Casca::nova(app, config()).expect("tokens e temas"),
            _dados,
            motor,
        )
    }

    fn quadro(c: &mut Casca<AppPianoTutor>, ctx: &Context) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 900.0))),
            events: Vec::<Event>::new(),
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| c.desenhar(ui))
            .drop_without_applying_deltas();
    }

    #[test]
    fn a_janela_desenha_a_tela_de_cursos_em_todos_os_estados() {
        let b = bancada(true);
        let (mut c, _dados, _motor) = casca_com(b);
        let ctx = Context::default();
        // Fábrica (a primeira do seletor), sem exercício aberto.
        c.abrir(&ctx, ROTA_CURSOS.into(), Local::Centro);
        for _ in 0..3 {
            quadro(&mut c, &ctx);
        }
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
        // Curso de tutor: árvore com Lição de assistir, exercícios e o aviso de revisão.
        c.app.cursos.selecionar(1);
        for _ in 0..3 {
            quadro(&mut c, &ctx);
        }
        // Exercício aberto, com áudio do professor e Chamada e resposta.
        c.app
            .cursos
            .abrir_exercicio("s03-a14", "cinco-notas")
            .expect("abre");
        c.app
            .executar_acoes_cursos(&ctx, vec![AcaoCursos::AbriuExercicio]);
        for _ in 0..3 {
            quadro(&mut c, &ctx);
        }
        c.app
            .executar_acoes_cursos(&ctx, vec![AcaoCursos::IniciarChamadaCurso]);
        for _ in 0..3 {
            quadro(&mut c, &ctx);
        }
        assert!(c.app.chamada.is_some());
        // Sem professor e com mensagem de erro de instalação.
        c.app
            .executar_acoes_cursos(&ctx, vec![AcaoCursos::ProfessorFala(false)]);
        c.app.cursos.iniciar_instalacao("/nao/existe.ptcurso");
        for _ in 0..3 {
            quadro(&mut c, &ctx);
        }
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
        // A tela de Exercícios (fábrica) também desenha, com a chamada cortada ao trocar de rota.
        c.abrir(&ctx, ROTA_EXERCICIOS.into(), Local::Centro);
        for _ in 0..3 {
            quadro(&mut c, &ctx);
        }
        assert!(c.app.chamada.is_none(), "trocar de tela corta a chamada");
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }

    #[test]
    fn a_janela_desenha_com_a_preferencia_de_nao_revisado_nos_dois_estados() {
        let b = bancada(true);
        let (mut c, _dados, _motor) = casca_com(b);
        let ctx = Context::default();
        c.abrir(&ctx, ROTA_CURSOS.into(), Local::Centro);
        c.app.cursos.selecionar(1);
        for usar in [true, false, true] {
            c.app
                .executar_acoes_cursos(&ctx, vec![AcaoCursos::UsarNaoRevisado(usar)]);
            for _ in 0..3 {
                quadro(&mut c, &ctx);
            }
            c.app
                .cursos
                .abrir_exercicio("s03-a14", "cinco-notas")
                .expect("aprovado abre sempre");
            c.app
                .executar_acoes_cursos(&ctx, vec![AcaoCursos::AbriuExercicio]);
            if usar {
                c.app
                    .cursos
                    .abrir_exercicio("s03-a14", "ainda-hipotese")
                    .expect("hipótese abre");
                c.app
                    .executar_acoes_cursos(&ctx, vec![AcaoCursos::AbriuExercicio]);
            }
            for _ in 0..3 {
                quadro(&mut c, &ctx);
            }
            assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
        }
    }

    #[test]
    fn curso_grande_com_18_secoes_e_150_licoes_desenha_sem_travar() {
        use crate::cursos::{CursoListado, LicaoNaArvore, SecaoNaArvore, Selo};
        use piano_tutor_curso::{EstadoConteudo, Exercicio, Mao, Modo, TipoLicao};
        let b = bancada(false);
        let (mut c, _dados, _motor) = casca_com(b);
        let ctx = Context::default();
        let exercicio = |id: String, estado| Exercicio {
            titulo: id.clone(),
            id,
            modo: Modo::Praticar,
            mao: Mao::Direita,
            estado,
            partitura: "p.musicxml".to_owned(),
            audio: None,
            alinhamento: None,
            bpm: None,
            trecho_aula: None,
        };
        let mut n = 0;
        let arvore: Vec<SecaoNaArvore> = (1..=18)
            .map(|s| SecaoNaArvore {
                id: format!("s{s:02}"),
                numero: s,
                titulo: format!("Seção {s}"),
                licoes: (0..8)
                    .map(|_| {
                        n += 1;
                        let estados = [EstadoConteudo::Aprovada, EstadoConteudo::Hipotese];
                        let exercicios: Vec<_> = estados
                            .iter()
                            .enumerate()
                            .map(|(i, e)| exercicio(format!("ex-{n}-{i}"), *e))
                            .collect();
                        LicaoNaArvore {
                            id: format!("l{n}"),
                            numero: n,
                            titulo: format!("Lição {n}"),
                            tipo: TipoLicao::Exercicio,
                            aula: None,
                            aguardando: 1,
                            exercicios,
                        }
                    })
                    .collect(),
            })
            .collect();
        c.app.cursos.cursos.push(CursoListado {
            id: "grande".to_owned(),
            titulo: "Curso grande".to_owned(),
            autor: "Autor".to_owned(),
            licenciado: None,
            licenca: None,
            versao: "1".to_owned(),
            chave_curta: None,
            selo: Selo::ChaveDesconhecida,
            pasta: None,
            manifesto: None,
            arvore,
            aprovados: 144,
            aguardando: 144,
        });
        c.abrir(&ctx, ROTA_CURSOS.into(), Local::Centro);
        let ultimo = c.app.cursos.cursos.len() - 1;
        c.app.cursos.selecionar(ultimo);
        let inicio = std::time::Instant::now();
        for usar in [true, false] {
            c.app
                .executar_acoes_cursos(&ctx, vec![AcaoCursos::UsarNaoRevisado(usar)]);
            for _ in 0..20 {
                quadro(&mut c, &ctx);
            }
        }
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(20),
            "40 quadros levaram {:?}",
            inicio.elapsed()
        );
    }

    #[test]
    fn a_fonte_da_janela_tem_os_simbolos_dos_cursos() {
        let b = bancada(false);
        let (mut c, _d, _m) = casca_com(b);
        let ctx = Context::default();
        quadro(&mut c, &ctx);
        quadro(&mut c, &ctx);
        let fonte = egui::FontId::proportional(14.0);
        let usados = format!(
            "{}{}«»·…[!]",
            crate::chamada_app::SIMBOLO_ACERTO,
            crate::chamada_app::SIMBOLO_ERRO
        );
        let faltam: String = usados
            .chars()
            .filter(|ch| !ctx.fonts_mut(|f| f.has_glyph(&fonte, *ch)))
            .collect();
        assert!(faltam.is_empty(), "símbolos ausentes: {faltam:?}");
    }

    #[test]
    fn navegacao_tem_o_grupo_cursos_e_a_acao_do_ribbon_abre_a_rota() {
        let app = AppPianoTutor::novo();
        let grupos = app.navegacao();
        let g = grupos.iter().find(|g| g.rotulo == "Cursos").expect("grupo");
        assert!(g.itens.iter().any(|i| i.rota.como_texto() == ROTA_CURSOS));
        let mut c = Casca::nova(AppPianoTutor::novo(), config()).expect("casca");
        let ctx = Context::default();
        quadro(&mut c, &ctx);
        bancada_casca::executar(&ctx, crate::estado::ids::CURSOS);
        for _ in 0..3 {
            quadro(&mut c, &ctx);
        }
        assert_eq!(c.estado.historico.atual().como_texto(), ROTA_CURSOS);
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }
}
