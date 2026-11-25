//! Chamada e resposta no app (M4, etapa 7): o laço por quadro que liga a máquina do núcleo
//! ([`Sessao`]) à voz do professor, ao som e às notas do aluno.
//!
//! - o professor **anuncia** ("Agora o acorde de Lá."), **toca** o trecho e o aluno **repete**;
//!   o núcleo avalia com o comparador e devolve [`Acao`]s;
//! - `Acao::Falar` vai para o [`Professor`]; com voz de verdade o fim do anúncio é avisado por
//!   [`Sessao::fala_terminou`] quando a voz para de falar (`anuncio_por_estimativa = false`); sem
//!   voz (ou com o professor desligado) vale a estimativa de duração do texto;
//! - `Acao::TocarNotas` volta em [`SaidaChamada::tocar`] para o app agendar no motor de som;
//! - `Acao::Feedback` vira texto **com símbolo** (nunca só cor) e a fala `acerto` ou `erro`
//!   do Roteiro, se houver aprovada;
//! - estados visíveis: "Ouça…", "Sua vez", "✓ Acertou", "✗ Tente de novo (2/3)".
//!
//! Nada aqui mede latência de MIDI ou áudio: **NÃO VERIFICADO** com teclado e som reais.

use crate::exercicios::{Exercicio as ExercicioDeFabrica, Passo, Tipo};
use crate::professor::Professor;
use crate::professor_falante::Momento;
use piano_tutor_core::NotaMidi;
use piano_tutor_core::chamada_resposta::{
    Acao, Chamada, ConfigChamada, Detalhes, Estado, Motivo, Resumo, Sessao, chamadas_de_acordes,
    chamadas_de_frases, estimar_duracao_da_fala_s, nome_falado_do_acorde,
};
use piano_tutor_core::comparador::EventoEsperado;
use piano_tutor_core::teoria::{Acorde, identificar_acorde};

/// Símbolos do resultado. A tela só os usa se a fonte tiver o glifo (testado em `app.rs`).
pub const SIMBOLO_ACERTO: &str = "✓";
pub const SIMBOLO_ERRO: &str = "✗";

/// Compassos por frase nas chamadas de partitura.
pub const COMPASSOS_POR_FRASE: usize = 2;

/// Duração do acorde ou intervalo tocado pelo professor, em segundos.
const DURACAO_DO_ACORDE_S: f64 = 1.2;

/// De onde vem a Chamada em andamento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrigemChamada {
    /// Exercício de um Curso de tutor.
    Curso {
        curso_id: String,
        exercicio_id: String,
    },
    /// Exercício de fábrica (número de 1 a 10).
    Fabrica { exercicio: u8 },
}

/// O que o app precisa fazer depois de um passo da Chamada.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaidaChamada {
    /// Trechos que o professor toca agora (instantes relativos; primeira nota em 0).
    pub tocar: Vec<Vec<EventoEsperado>>,
    /// Preenchido quando a sessão acabou.
    pub concluida: Option<Resumo>,
}

/// Resultado da última resposta, para a tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedbackVisual {
    pub acertou: bool,
    /// "✓ Acertou" ou "✗ Tente de novo (2/3)".
    pub titulo: String,
    /// Por que (ex.: "Faltaram notas."), vazio quando acertou.
    pub detalhe: String,
}

#[derive(Clone, Copy, Debug)]
enum FimDaFala {
    /// A voz está dizendo; avisa quando `falando()` ficar falso.
    Voz,
    /// Sem voz: o núcleo já conta pela estimativa (ou o app avisa neste instante).
    Ate(f64),
}

/// Uma Chamada e resposta em andamento.
pub struct ChamadaAtiva {
    sessao: Sessao,
    pub origem: OrigemChamada,
    pub titulo: String,
    /// Último anúncio dito (e mostrado) pelo professor.
    pub anuncio: Option<String>,
    pub feedback: Option<FeedbackVisual>,
    espera_fala: Option<FimDaFala>,
    /// A sessão foi criada para esperar o fim da fala (`anuncio_por_estimativa = false`).
    espera_aviso_de_fim: bool,
    resumo_final: Option<Resumo>,
}

impl std::fmt::Debug for ChamadaAtiva {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChamadaAtiva")
            .field("origem", &self.origem)
            .field("estado", &self.sessao.estado())
            .finish_non_exhaustive()
    }
}

/// Texto curto do motivo, sem o símbolo (a tela põe o símbolo no título).
pub fn motivo_curto(m: Motivo) -> &'static str {
    match m {
        Motivo::Acertou => "",
        Motivo::NaoTocou => "Não ouvi você tocar.",
        Motivo::NotasFaltando => "Faltaram notas.",
        Motivo::NotaExtra => "Você tocou uma nota a mais.",
        Motivo::NotaErrada => "Alguma nota não era a pedida.",
        Motivo::ForaDoTempo => "As notas certas vieram fora do tempo (juntas, quando for acorde).",
    }
}

/// "✗ Tente de novo (2/3)": a tentativa que vem, de quantas.
pub fn titulo_do_erro(d: &Detalhes) -> String {
    if d.vai_repetir {
        format!(
            "{SIMBOLO_ERRO} Tente de novo ({}/{})",
            d.tentativa + 1,
            d.max_tentativas
        )
    } else {
        format!(
            "{SIMBOLO_ERRO} Vamos para a próxima ({} tentativas usadas)",
            d.tentativa
        )
    }
}

impl ChamadaAtiva {
    /// Cria a sessão. `com_voz`: o professor vai dizer os anúncios com voz (o fim da fala é
    /// avisado por ela); senão o núcleo usa a estimativa de duração do texto.
    pub fn nova(
        chamadas: Vec<Chamada>,
        origem: OrigemChamada,
        titulo: impl Into<String>,
        com_voz: bool,
    ) -> Result<Self, String> {
        if chamadas.is_empty() {
            return Err("Este exercício não tem trecho para a Chamada e resposta.".to_owned());
        }
        let config = ConfigChamada {
            anuncio_por_estimativa: !com_voz,
            pausa_feedback_s: 2.5,
            espera_inicio_s: 20.0,
            ..ConfigChamada::default()
        };
        let sessao = Sessao::nova(chamadas, config).map_err(|e| e.to_string())?;
        Ok(Self {
            sessao,
            origem,
            titulo: titulo.into(),
            anuncio: None,
            feedback: None,
            espera_fala: None,
            espera_aviso_de_fim: com_voz,
            resumo_final: None,
        })
    }

    pub fn estado(&self) -> Estado {
        self.sessao.estado()
    }

    pub fn concluida(&self) -> bool {
        self.sessao.concluida()
    }

    pub fn resumo(&self) -> Resumo {
        self.resumo_final
            .clone()
            .unwrap_or_else(|| self.sessao.resumo())
    }

    /// Chamada em curso (1-based) e total, para "Chamada 2 de 4".
    pub fn progresso(&self) -> (usize, usize) {
        let total = self.sessao.chamadas().len();
        match self.sessao.chamada_atual() {
            Some(i) => (i + 1, total),
            None => (total, total),
        }
    }

    /// Percentual de chamadas acertadas (para o progresso do Curso).
    pub fn precisao_pct(&self) -> f32 {
        let r = self.resumo();
        if r.total == 0 {
            100.0
        } else {
            100.0 * r.acertadas() as f32 / r.total as f32
        }
    }

    /// O estado que a tela mostra, em uma frase com símbolo.
    pub fn texto_do_estado(&self) -> String {
        match self.sessao.estado() {
            Estado::Pronto => "Pronto".to_owned(),
            Estado::Anunciando | Estado::ProfessorTocando | Estado::Avaliando => "Ouça…".to_owned(),
            Estado::AguardandoAluno => "Sua vez".to_owned(),
            Estado::Acertou => format!("{SIMBOLO_ACERTO} Acertou"),
            Estado::Errou => self
                .feedback
                .as_ref()
                .map_or_else(|| format!("{SIMBOLO_ERRO} Errou"), |f| f.titulo.clone()),
            Estado::Concluido => {
                let r = self.resumo();
                format!("Fim: {} de {} chamadas certas", r.acertadas(), r.total)
            }
        }
    }

    /// Começa a sessão em `agora_s`.
    pub fn iniciar(&mut self, agora_s: f64, professor: &mut Professor) -> SaidaChamada {
        let acoes = self.sessao.iniciar(agora_s);
        let mut saida = SaidaChamada::default();
        self.aplicar(acoes, agora_s, professor, &mut saida);
        saida
    }

    /// Um quadro: avisa o fim da fala (se a voz parou) e faz o tempo andar.
    pub fn quadro(&mut self, agora_s: f64, professor: &mut Professor) -> SaidaChamada {
        let mut saida = SaidaChamada::default();
        let terminou = match self.espera_fala {
            Some(FimDaFala::Voz) => !professor.falando(),
            Some(FimDaFala::Ate(t)) => agora_s >= t,
            None => false,
        };
        if terminou {
            self.espera_fala = None;
            let acoes = self.sessao.fala_terminou(agora_s);
            self.aplicar(acoes, agora_s, professor, &mut saida);
        }
        let acoes = self.sessao.avancar(agora_s);
        self.aplicar(acoes, agora_s, professor, &mut saida);
        saida
    }

    /// O aluno tocou `midi` em `agora_s`.
    pub fn nota_tocada(
        &mut self,
        midi: u8,
        agora_s: f64,
        professor: &mut Professor,
    ) -> SaidaChamada {
        let acoes = self.sessao.nota_tocada(midi, agora_s);
        let mut saida = SaidaChamada::default();
        self.aplicar(acoes, agora_s, professor, &mut saida);
        saida
    }

    fn aplicar(
        &mut self,
        acoes: Vec<Acao>,
        agora_s: f64,
        professor: &mut Professor,
        saida: &mut SaidaChamada,
    ) {
        for a in acoes {
            match a {
                Acao::Falar(texto) => {
                    self.anuncio = Some(texto.clone());
                    self.feedback = None;
                    let com_voz = professor.anunciar(&texto);
                    // Só precisa do aviso quando a sessão espera por ele (`anuncio_por_estimativa`
                    // desligado); com voz o fim é a voz parar; sem voz, a estimativa do texto.
                    self.espera_fala = if !self.espera_aviso_de_fim {
                        None
                    } else if com_voz {
                        Some(FimDaFala::Voz)
                    } else {
                        Some(FimDaFala::Ate(agora_s + estimar_duracao_da_fala_s(&texto)))
                    };
                }
                Acao::TocarNotas(notas) => saida.tocar.push(notas),
                Acao::Feedback { acertou, detalhes } => {
                    self.feedback = Some(if acertou {
                        FeedbackVisual {
                            acertou: true,
                            titulo: format!("{SIMBOLO_ACERTO} Acertou"),
                            detalhe: String::new(),
                        }
                    } else {
                        FeedbackVisual {
                            acertou: false,
                            titulo: titulo_do_erro(&detalhes),
                            detalhe: motivo_curto(detalhes.motivo).to_owned(),
                        }
                    });
                    let momento = if acertou {
                        Momento::Acerto
                    } else {
                        Momento::Erro
                    };
                    professor.dizer(momento, None, false);
                }
                Acao::Concluido(r) => {
                    self.resumo_final = Some(r.clone());
                    saida.concluida = Some(r);
                }
            }
        }
    }
}

// --- Geradores de chamadas --------------------------------------------------------------------

/// Frases de 2 compassos de uma partitura (Exercício de Curso).
pub fn chamadas_da_partitura(esperados: &[EventoEsperado]) -> Vec<Chamada> {
    chamadas_de_frases(esperados, COMPASSOS_POR_FRASE, true)
}

/// O Exercício de fábrica tem Chamada e resposta? Acordes e intervalos têm.
pub fn fabrica_tem_chamada(ex: &ExercicioDeFabrica) -> bool {
    matches!(ex.tipo, Tipo::Acorde | Tipo::Intervalos)
}

fn notas_do_passo(p: &Passo) -> Vec<u8> {
    p.notas.iter().map(|n| n.numero()).collect()
}

/// Chamadas do Exercício de fábrica: uma por acorde (qualquer inversão e oitava) ou por
/// intervalo (notas exatas). `None` nos outros tipos.
pub fn chamadas_de_fabrica(ex: &ExercicioDeFabrica) -> Option<Vec<Chamada>> {
    if !fabrica_tem_chamada(ex) {
        return None;
    }
    let abertura = "Vamos tocar em chamada e resposta. ";
    if ex.tipo == Tipo::Acorde {
        let acordes: Vec<Acorde> = ex
            .passos
            .iter()
            .filter_map(|p| identificar_acorde(&p.notas))
            .collect();
        if acordes.len() == ex.passos.len() {
            let base = NotaMidi::nova(60)?;
            return Some(chamadas_de_acordes(
                &acordes,
                base,
                DURACAO_DO_ACORDE_S,
                true,
            ));
        }
    }
    let intervalo = ex.tipo == Tipo::Intervalos;
    Some(
        ex.passos
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let c = Chamada::de_acorde(&notas_do_passo(p), DURACAO_DO_ACORDE_S);
                let c = if p.por_classe { c.por_classe() } else { c };
                let nome = identificar_acorde(&p.notas)
                    .map(|a| nome_falado_do_acorde(&a))
                    .unwrap_or_else(|| p.legenda.clone());
                let fala = if intervalo {
                    format!("Agora o intervalo: {}.", p.legenda)
                } else {
                    format!("Agora o acorde de {nome}.")
                };
                c.com_anuncio(if i == 0 {
                    format!("{abertura}{fala}")
                } else {
                    fala
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::exercicios::exercicios_de_fabrica;
    use crate::professor_falante::{VozFalsa, ler_roteiro};

    const ROTEIRO: &str = r#"formato_versao = 1

[[falas]]
id = "a"
licao = "l"
momento = "acerto"
texto = "Muito bem."
estado = "aprovada"

[[falas]]
id = "e"
licao = "l"
momento = "erro"
texto = "Calma."
estado = "aprovada"
"#;

    fn professor() -> (Professor, VozFalsa) {
        let voz = VozFalsa::nova();
        let mut p = Professor::novo(Box::new(voz.clone()), None);
        p.abrir("l", "x", Some(ler_roteiro(ROTEIRO).expect("roteiro")));
        p.atualizar(0.0);
        (p, voz)
    }

    fn frase() -> Chamada {
        Chamada::de_notas(&[(60, 0.5), (64, 0.5), (67, 0.5)]).com_anuncio("Ouça e repita.")
    }

    fn pedidos_sem_antes(voz: &VozFalsa) -> Vec<String> {
        voz.pedidos()
    }

    /// Anda no tempo em passos de 10 ms, como os quadros do app, e junta as saídas.
    fn andar(
        c: &mut ChamadaAtiva,
        p: &mut Professor,
        de: f64,
        ate: f64,
        saida: &mut SaidaChamada,
    ) -> f64 {
        let mut t = de;
        while t < ate {
            t += 0.01;
            p.atualizar(t);
            let s = c.quadro(t, p);
            saida.tocar.extend(s.tocar);
            if s.concluida.is_some() {
                saida.concluida = s.concluida;
            }
        }
        t
    }

    #[test]
    fn com_voz_o_anuncio_espera_a_voz_parar_e_so_depois_o_professor_toca() {
        let (mut p, voz) = professor();
        let mut c = ChamadaAtiva::nova(
            vec![frase()],
            OrigemChamada::Fabrica { exercicio: 7 },
            "t",
            true,
        )
        .expect("sessão");
        let mut saida = c.iniciar(10.0, &mut p);
        assert_eq!(voz.pedidos(), ["Ouça e repita."], "anunciou com a voz");
        assert_eq!(c.texto_do_estado(), "Ouça…");
        assert!(saida.tocar.is_empty());
        // Enquanto a voz fala, mesmo depois de muito tempo, o professor não toca.
        let t = andar(&mut c, &mut p, 10.0, 15.0, &mut saida);
        assert!(saida.tocar.is_empty(), "a estimativa está desligada");
        assert_eq!(c.estado(), Estado::Anunciando);
        // A voz termina: no quadro seguinte o professor toca as três notas.
        voz.terminar_a_fala();
        let t = andar(&mut c, &mut p, t, t + 0.05, &mut saida);
        assert_eq!(saida.tocar.len(), 1);
        let midis: Vec<u8> = saida.tocar[0].iter().map(|e| e.midi).collect();
        assert_eq!(midis, [60, 64, 67]);
        assert_eq!(c.estado(), Estado::ProfessorTocando);
        // Depois do trecho (1,5 s + folga) é a vez do aluno.
        let t = andar(&mut c, &mut p, t, t + 2.1, &mut saida);
        assert_eq!(c.texto_do_estado(), "Sua vez");
        // O aluno acerta: ✓ Acertou, e o professor diz a fala de acerto aprovada.
        let mut s = SaidaChamada::default();
        for (i, m) in [60u8, 64, 67].into_iter().enumerate() {
            let r = c.nota_tocada(m, t + 0.5 * i as f64, &mut p);
            s.tocar.extend(r.tocar);
        }
        let t = andar(&mut c, &mut p, t + 1.0, t + 6.0, &mut s);
        assert!(c.feedback.as_ref().is_some_and(|f| f.acertou));
        assert!(
            pedidos_sem_antes(&voz).contains(&"Muito bem.".to_owned()),
            "{:?}",
            voz.pedidos()
        );
        assert!(c.concluida(), "uma só chamada");
        assert_eq!(s.concluida.as_ref().map(Resumo::acertadas), Some(1));
        assert_eq!(c.texto_do_estado(), "Fim: 1 de 1 chamadas certas");
        assert!((c.precisao_pct() - 100.0).abs() < 1e-3);
        let _ = t;
    }

    #[test]
    fn sem_voz_vale_a_estimativa_e_o_texto_aparece() {
        let mut p = Professor::mudo();
        p.abrir("l", "x", Some(ler_roteiro(ROTEIRO).expect("roteiro")));
        let mut c = ChamadaAtiva::nova(
            vec![frase()],
            OrigemChamada::Fabrica { exercicio: 7 },
            "t",
            false,
        )
        .expect("sessão");
        let mut saida = c.iniciar(0.0, &mut p);
        assert_eq!(c.anuncio.as_deref(), Some("Ouça e repita."));
        assert_eq!(p.legenda(), Some("Ouça e repita."));
        // "Ouça e repita." ~ 14 caracteres: 0,8 + 0,06·14 = 1,64 s.
        andar(&mut c, &mut p, 0.0, 1.5, &mut saida);
        assert!(saida.tocar.is_empty());
        andar(&mut c, &mut p, 1.5, 1.8, &mut saida);
        assert_eq!(saida.tocar.len(), 1, "tocou pela estimativa");
    }

    #[test]
    fn professor_desligado_no_meio_nao_trava_a_sessao_que_esperava_a_voz() {
        let (mut p, voz) = professor();
        let mut c = ChamadaAtiva::nova(
            vec![frase(), frase()],
            OrigemChamada::Fabrica { exercicio: 7 },
            "t",
            true,
        )
        .expect("sessão");
        let mut saida = c.iniciar(0.0, &mut p);
        voz.terminar_a_fala();
        let t = andar(&mut c, &mut p, 0.0, 0.1, &mut saida);
        // Desliga o professor antes do próximo anúncio: ele só aparece na tela e a estimativa
        // do app avisa o fim da fala.
        p.definir_ligado(false);
        // Responde certo a primeira chamada.
        let t = andar(&mut c, &mut p, t, t + 2.1, &mut saida);
        for (i, m) in [60u8, 64, 67].into_iter().enumerate() {
            c.nota_tocada(m, t + 0.2 * i as f64, &mut p);
        }
        let mut s = SaidaChamada::default();
        andar(&mut c, &mut p, t + 0.6, t + 8.0, &mut s);
        assert!(
            !s.tocar.is_empty(),
            "a segunda chamada tocou: não ficou presa no anúncio"
        );
        assert_eq!(c.anuncio.as_deref(), Some("Ouça e repita."));
    }

    #[test]
    fn errar_mostra_tente_de_novo_com_tentativa_e_fala_o_erro() {
        let (mut p, voz) = professor();
        let mut c = ChamadaAtiva::nova(
            vec![frase()],
            OrigemChamada::Fabrica { exercicio: 7 },
            "t",
            false,
        )
        .expect("sessão");
        let mut saida = c.iniciar(0.0, &mut p);
        voz.terminar_a_fala();
        let t = andar(&mut c, &mut p, 0.0, 4.0, &mut saida);
        assert_eq!(c.texto_do_estado(), "Sua vez");
        // Toca só uma nota e se cala: faltaram notas.
        c.nota_tocada(60, t, &mut p);
        let t = andar(&mut c, &mut p, t, t + 2.5, &mut saida);
        let f = c.feedback.clone().expect("feedback");
        assert!(!f.acertou);
        assert_eq!(f.titulo, "✗ Tente de novo (2/3)");
        assert_eq!(f.detalhe, "Faltaram notas.");
        assert_eq!(c.texto_do_estado(), "✗ Tente de novo (2/3)");
        assert!(
            voz.pedidos().contains(&"Calma.".to_owned()),
            "{:?}",
            voz.pedidos()
        );
        // O professor toca de novo (repetição) e o aluno tem outra chance.
        let antes = saida.tocar.len();
        andar(&mut c, &mut p, t, t + 3.0, &mut saida);
        assert_eq!(saida.tocar.len(), antes + 1);
    }

    #[test]
    fn esgotar_as_tentativas_segue_para_a_proxima_e_o_resumo_conta() {
        let mut p = Professor::mudo();
        let mut c = ChamadaAtiva::nova(
            vec![frase(), frase()],
            OrigemChamada::Fabrica { exercicio: 7 },
            "t",
            false,
        )
        .expect("sessão");
        let mut saida = c.iniciar(0.0, &mut p);
        // Nunca toca nada: 3 tentativas sem resposta (espera de 20 s cada), depois a próxima.
        let mut t = 0.0;
        let mut vistas = Vec::new();
        for _ in 0..4000 {
            t += 0.05;
            p.atualizar(t);
            let s = c.quadro(t, &mut p);
            saida.tocar.extend(s.tocar);
            if let Some(f) = &c.feedback
                && vistas.last() != Some(&f.titulo)
            {
                vistas.push(f.titulo.clone());
            }
            if s.concluida.is_some() {
                saida.concluida = s.concluida;
                break;
            }
        }
        assert_eq!(
            vistas[..3],
            [
                "✗ Tente de novo (2/3)".to_owned(),
                "✗ Tente de novo (3/3)".to_owned(),
                "✗ Vamos para a próxima (3 tentativas usadas)".to_owned()
            ]
        );
        let r = saida.concluida.expect("terminou");
        assert_eq!(r.nao_resolvidas(), 2);
        assert!((c.precisao_pct() - 0.0).abs() < 1e-3);
    }

    #[test]
    fn chamadas_de_partitura_sao_frases_de_dois_compassos_com_anuncio() {
        let ev = |midi, inicio_s, compasso| EventoEsperado {
            midi,
            inicio_s,
            duracao_s: 0.5,
            parte: 0,
            pauta: 1,
            compasso,
        };
        let esperados: Vec<EventoEsperado> = (0..16)
            .map(|i| ev(60 + (i % 8) as u8, f64::from(i) * 0.5, (i / 4) as usize))
            .collect();
        let c = chamadas_da_partitura(&esperados);
        assert_eq!(c.len(), 2, "4 compassos = 2 frases de 2 compassos");
        assert_eq!(c[0].trecho.len(), 8);
        assert!(
            c[0].anuncio
                .as_deref()
                .is_some_and(|a| a.contains("Frase 1"))
        );
        assert!(
            c[1].anuncio
                .as_deref()
                .is_some_and(|a| a.contains("Frase 2"))
        );
    }

    #[test]
    fn fabrica_acordes_e_intervalos_viram_chamadas_e_os_outros_tipos_nao() {
        let todos = exercicios_de_fabrica();
        let tem: Vec<u8> = todos
            .iter()
            .filter(|e| fabrica_tem_chamada(e))
            .map(|e| e.id)
            .collect();
        assert_eq!(
            tem,
            [5, 7, 8],
            "intervalos, tríade de Dó e tríades maiores e menores"
        );
        for e in &todos {
            assert_eq!(
                chamadas_de_fabrica(e).is_some(),
                fabrica_tem_chamada(e),
                "ex {}",
                e.id
            );
        }
        let acordes = chamadas_de_fabrica(&todos[7]).expect("ex 8");
        assert_eq!(acordes.len(), 6);
        let anuncios: Vec<&str> = acordes
            .iter()
            .map(|c| c.anuncio.as_deref().unwrap_or_default())
            .collect();
        assert_eq!(
            anuncios[0],
            "Vamos tocar em chamada e resposta. Agora o acorde de Dó."
        );
        assert_eq!(anuncios[1], "Agora o acorde de Lá menor.");
        // Resposta por classe de altura: vale qualquer oitava e inversão.
        assert_eq!(
            acordes[0].modo,
            piano_tutor_core::chamada_resposta::ModoResposta::PorClasse
        );
        let intervalos = chamadas_de_fabrica(&todos[4]).expect("ex 5");
        assert_eq!(intervalos.len(), 4);
        assert_eq!(
            intervalos[0].anuncio.as_deref(),
            Some("Vamos tocar em chamada e resposta. Agora o intervalo: terça maior.")
        );
        // Intervalos: alturas exatas.
        assert_eq!(
            intervalos[0].modo,
            piano_tutor_core::chamada_resposta::ModoResposta::Exato
        );
        assert_eq!(intervalos[0].trecho.len(), 2);
    }

    #[test]
    fn sem_chamadas_e_um_erro_claro() {
        let e = ChamadaAtiva::nova(
            Vec::new(),
            OrigemChamada::Fabrica { exercicio: 1 },
            "t",
            false,
        )
        .expect_err("vazio");
        assert!(e.contains("não tem trecho"), "{e}");
    }
}
