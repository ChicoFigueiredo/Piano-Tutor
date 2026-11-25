//! Tela Exercícios (F05): professor assistido com 10 exercícios progressivos, feedback com
//! texto e símbolo (nunca só cor), estatísticas e comandos de progresso.

use crate::chamada_app::{ChamadaAtiva, fabrica_tem_chamada};
use crate::espelho::EstadoEspelho;
use crate::exercicios::{
    Classificacao, ConfigExercicios, EstadoExercicios, Fase, JANELA_MAX_MS, JANELA_MIN_MS, Tipo,
    Tom, nome_nota,
};
use crate::gravacao::agendar_gravacao;
use crate::professor::Professor;
use crate::som::ServicoSom;
use crate::tela_cursos::{AcaoCursos, desenhar_chamada_de_fabrica, desenhar_professor};
use crate::tela_espelho::desenhar_teclado_embutido;
use bancada_casca::egui::{Button, Color32, RichText, ScrollArea, Slider, Ui, vec2};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::ritmo::{BPM_MAX, BPM_MIN};
use std::time::Duration;

const VERDE: Color32 = Color32::from_rgb(70, 200, 110);
const VERMELHO: Color32 = Color32::from_rgb(235, 100, 100);
/// Quantos erros a tela lista de uma vez.
const MAX_ERROS_NA_TELA: usize = 8;
const AMARELO: Color32 = Color32::from_rgb(230, 190, 70);

fn cor_do_tom(t: Tom) -> Option<Color32> {
    match t {
        Tom::Bom => Some(VERDE),
        Tom::Ruim => Some(VERMELHO),
        Tom::Aviso => Some(AMARELO),
        Tom::Neutro => None,
    }
}

/// Toca o gabarito do exercício atual (frase, sequência ou acordes).
fn ouvir(som: &mut ServicoSom, e: &EstadoExercicios) {
    let passos = e.passos_para_ouvir();
    let ms = e.ouvir_passo_ms();
    let t0 = som.agora_ns() + 30_000_000;
    let junto = matches!(
        e.exercicio().map(|x| x.tipo),
        Some(Tipo::Acorde | Tipo::Intervalos)
    );
    for (i, notas) in passos.iter().enumerate() {
        let dur = if junto {
            ms.saturating_sub(150)
        } else {
            ms.saturating_sub(40)
        };
        som.demonstrar(
            notas,
            t0 + i as u64 * ms * 1_000_000,
            Duration::from_millis(dur.max(60)),
        );
    }
}

/// Toca de novo o que o aluno tocou na tentativa (replay pela lógica da F06). Devolve `false`
/// quando não há nota registrada.
pub fn ouvir_tentativa(som: &mut ServicoSom, e: &EstadoExercicios) -> bool {
    let Some(g) = e.tentativa_como_gravacao() else {
        return false;
    };
    agendar_gravacao(som, &g, som.agora_ns() + 30_000_000);
    true
}

fn nomes(notas: &[NotaMidi]) -> String {
    notas
        .iter()
        .map(|n| nome_nota(*n))
        .collect::<Vec<_>>()
        .join(" + ")
}

pub fn desenhar_tela(
    ui: &mut Ui,
    e: &mut EstadoExercicios,
    espelho: &mut EstadoEspelho,
    som: &mut ServicoSom,
    professor: &Professor,
    chamada: Option<&ChamadaAtiva>,
) -> Vec<AcaoCursos> {
    let mut acoes = Vec::new();
    // Limite de tempo opcional e gravação do progresso quando as teclas estão soltas.
    e.verificar_tempo(som.agora_ns());
    if espelho.teclas().notas_ligadas().next().is_none() {
        e.salvar_se_sujo();
    }
    ScrollArea::vertical()
        .id_salt("exercicios-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Exercícios guiados");
            desenhar_lista(ui, e);
            ui.add_space(8.0);
            desenhar_exercicio(ui, e, som);
            ui.add_space(4.0);
            if let Some(a) = desenhar_professor(ui, professor) {
                acoes.push(a);
            }
            let tem_chamada = e.exercicio().is_some_and(fabrica_tem_chamada);
            acoes.extend(desenhar_chamada_de_fabrica(ui, tem_chamada, chamada));
            ui.add_space(6.0);
            desenhar_feedback(ui, e);
            desenhar_erros(ui, e);
            ui.add_space(6.0);
            desenhar_estatisticas(ui, e);
            ui.add_space(6.0);
            espelho.guias = e.guias();
            desenhar_teclado_embutido(ui, espelho);
            ui.label(
                RichText::new(
                    "Bolinha verde = tecla a tocar. Azul = tecla que você está tocando. \
                     O resultado também aparece em texto: [OK] certo, [X] errado, [...] falta nota.",
                )
                .weak(),
            );
            ui.add_space(6.0);
            desenhar_ajustes(ui, e);
            ui.add_space(6.0);
            desenhar_progresso(ui, e);
        });
    acoes
}

fn desenhar_lista(ui: &mut Ui, e: &mut EstadoExercicios) {
    let mut escolhido = None;
    for i in 0..e.exercicios.len() {
        let liberado = e.desbloqueado(i);
        let ex = &e.exercicios[i];
        let estado = if e.concluido(i) {
            "[OK] concluído".to_owned()
        } else if !liberado {
            format!("[bloqueado] {}", e.motivo_bloqueio(i).unwrap_or_default())
        } else if e.historico_atual().acertos > 0 && i == e.indice() {
            "em andamento".to_owned()
        } else {
            "liberado".to_owned()
        };
        let texto = format!(
            "{}. {}  ({})  -  {estado}",
            ex.id,
            ex.titulo,
            ex.tipo.texto()
        );
        let marcado = i == e.indice();
        let r = ui.add_enabled(
            liberado,
            Button::new(texto)
                .selected(marcado)
                .min_size(vec2(ui.available_width().min(560.0), 26.0)),
        );
        if r.clicked() {
            escolhido = Some(i);
        }
    }
    if let Some(i) = escolhido {
        e.selecionar(i);
    }
}

fn desenhar_exercicio(ui: &mut Ui, e: &mut EstadoExercicios, som: &mut ServicoSom) {
    let Some(ex) = e.exercicio().cloned() else {
        ui.label("Nenhum exercício disponível.");
        return;
    };
    ui.label(
        RichText::new(format!("Exercício {}: {}", ex.id, ex.titulo))
            .size(22.0)
            .strong(),
    );
    ui.label(format!("Objetivo: {}", ex.objetivo));
    ui.label(format!("Instruções: {}", ex.instrucoes));
    if !ex.dedilhado.is_empty() {
        ui.label(format!("Dedilhado sugerido: {}", ex.dedilhado));
    }
    match ex.prerequisito {
        Some(id) => {
            ui.label(RichText::new(format!("Pré-requisito: exercício {id} concluído.")).weak())
        }
        None => ui.label(RichText::new("Pré-requisito: nenhum.").weak()),
    };
    ui.add_space(4.0);

    let total = ex.passos.len();
    let t = &e.tentativa;
    if t.fase == Fase::Concluido {
        ui.label(
            RichText::new(format!("[OK] Concluído: {total} de {total} passos"))
                .size(22.0)
                .strong()
                .color(VERDE),
        );
    } else {
        let alvo = if ex.de_ouvido() && !e.config.mostrar_dica {
            "Ouça a frase e repita".to_owned()
        } else {
            e.passo_atual()
                .map(|p| format!("Toque: {}", nomes(&p.notas)))
                .unwrap_or_default()
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!("Passo {} de {total}", t.passo + 1))
                    .size(16.0)
                    .strong(),
            );
            ui.label(RichText::new(alvo).size(26.0).strong().color(VERDE));
            if let Some(p) = e.passo_atual()
                && !p.legenda.is_empty()
                && (!ex.de_ouvido() || e.config.mostrar_dica)
            {
                ui.label(RichText::new(&p.legenda).size(16.0));
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(Button::new("Ouvir").min_size(vec2(80.0, 30.0)))
            .on_hover_text("Toca o gabarito do exercício")
            .clicked()
        {
            ouvir(som, e);
        }
        let tem_toques = !e.tentativa.registro.is_empty();
        if ui
            .add_enabled(tem_toques, Button::new("Ouvir o que toquei"))
            .on_hover_text("Toca de novo as notas que você tocou nesta tentativa")
            .clicked()
        {
            ouvir_tentativa(som, e);
        }
        if ui
            .button("Reiniciar")
            .on_hover_text("Volta ao primeiro passo")
            .clicked()
        {
            e.reiniciar();
        }
        if ui
            .button("Repetir")
            .on_hover_text("Repete o passo atual, sem contar")
            .clicked()
        {
            e.repetir();
            if ex.de_ouvido() {
                ouvir(som, e);
            }
        }
        let pausado = e.tentativa.pausado_desde.is_some();
        if ui
            .button(if pausado { "Retomar" } else { "Pausar" })
            .clicked()
        {
            if pausado {
                e.retomar(som.agora_ns());
            } else {
                e.pausar(som.agora_ns());
            }
        }
        if ui
            .add_enabled(e.indice() > 0, Button::new("Anterior"))
            .clicked()
        {
            e.voltar();
        }
        let tem_proximo = e.desbloqueado(e.indice() + 1);
        let r = ui.add_enabled(tem_proximo, Button::new("Próximo"));
        if r.clicked() {
            e.avancar();
        }
        if !tem_proximo && e.indice() + 1 < e.exercicios.len() {
            ui.label(RichText::new("Conclua este exercício para liberar o próximo.").weak());
        }
    });
}

fn desenhar_feedback(ui: &mut Ui, e: &EstadoExercicios) {
    let texto = e.feedback_texto();
    let mut t = RichText::new(texto).size(20.0).strong();
    if let Some(c) = cor_do_tom(e.tentativa.feedback.tom()) {
        t = t.color(c);
    }
    ui.label(t);
    if let Some(ultimo) = e.tentativa.registro.last() {
        let rotulo = match ultimo.resultado {
            Classificacao::PassoCompleto | Classificacao::ExercicioCompleto => "[OK] certa",
            Classificacao::ParteCerta => "[...] parte do conjunto",
            Classificacao::Errada => "[X] errada",
            Classificacao::ForaDaJanela => "[!] fora da janela",
            Classificacao::Ignorada => "ignorada",
        };
        ui.label(
            RichText::new(format!(
                "Última nota: {} ({rotulo})",
                nome_nota(ultimo.nota)
            ))
            .weak(),
        );
    }
}

fn desenhar_erros(ui: &mut Ui, e: &EstadoExercicios) {
    let erros = e.erros_localizados();
    if erros.is_empty() {
        return;
    }
    ui.label(RichText::new("Onde errei").strong());
    for erro in erros.iter().take(MAX_ERROS_NA_TELA) {
        ui.label(erro.texto());
    }
    if erros.len() > MAX_ERROS_NA_TELA {
        ui.label(
            RichText::new(format!(
                "... e mais {} erro(s) nesta tentativa.",
                erros.len() - MAX_ERROS_NA_TELA
            ))
            .weak(),
        );
    }
}

fn desenhar_estatisticas(ui: &mut Ui, e: &EstadoExercicios) {
    let t = &e.tentativa;
    let aprov = |c: crate::exercicios::Contagem| {
        c.aproveitamento()
            .map_or("sem notas ainda".to_owned(), |a| format!("{a}%"))
    };
    let tentativa = crate::exercicios::Contagem {
        acertos: t.acertos,
        erros: t.erros,
    };
    let h = e.historico_atual();
    let historico = crate::exercicios::Contagem {
        acertos: h.acertos,
        erros: h.erros,
    };
    ui.label(format!(
        "Tentativa: {} acertos, {} erros ({})",
        t.acertos,
        t.erros,
        aprov(tentativa)
    ));
    ui.label(format!(
        "Esta sessão: {} acertos, {} erros ({}), {} exercício(s) concluído(s)",
        e.sessao.acertos,
        e.sessao.erros,
        aprov(e.sessao),
        e.sessao_concluidos
    ));
    ui.label(format!(
        "Neste exercício (histórico): {} acertos, {} erros ({}), concluído {} vez(es)",
        h.acertos,
        h.erros,
        aprov(historico),
        h.vezes
    ));
}

fn desenhar_ajustes(ui: &mut Ui, e: &mut EstadoExercicios) {
    ui.label(RichText::new("Ajustes do professor").strong());
    let ConfigExercicios {
        mut janela_ms,
        mut limite_s,
        mut mostrar_dica,
        mut bpm_grade,
    } = e.config;
    ui.horizontal_wrapped(|ui| {
        ui.label("Janela para notas juntas (acordes e intervalos):");
        ui.add(Slider::new(&mut janela_ms, JANELA_MIN_MS..=JANELA_MAX_MS).suffix(" ms"));
    });
    ui.horizontal_wrapped(|ui| {
        let mut com_limite = limite_s > 0;
        if ui
            .checkbox(&mut com_limite, "Limite de tempo por tentativa")
            .changed()
        {
            limite_s = if com_limite { 60 } else { 0 };
        }
        if com_limite {
            ui.add(Slider::new(&mut limite_s, 5..=600).suffix(" s"));
        } else {
            ui.label(RichText::new("Sem limite de tempo (padrão).").weak());
        }
    });
    ui.checkbox(
        &mut mostrar_dica,
        "Mostrar teclas-guia nos exercícios de ouvido",
    );
    ui.horizontal_wrapped(|ui| {
        ui.label("Andamento da grade do compasso do erro (4/4):");
        ui.add(Slider::new(&mut bpm_grade, BPM_MIN..=BPM_MAX).suffix(" bpm"));
    });
    let novo = ConfigExercicios {
        janela_ms,
        limite_s,
        mostrar_dica,
        bpm_grade,
    };
    if novo != e.config {
        e.config = novo;
        e.salvar();
    }
}

fn desenhar_progresso(ui: &mut Ui, e: &mut EstadoExercicios) {
    ui.label(RichText::new("Progresso salvo").strong());
    ui.horizontal_wrapped(|ui| {
        if ui.button("Exportar progresso (.txt)").clicked() {
            let _ = e.exportar();
        }
        if e.confirmar_apagar {
            ui.label(
                RichText::new("Apagar todo o progresso? Não dá para desfazer.")
                    .color(VERMELHO)
                    .strong(),
            );
            if ui.button("Sim, apagar").clicked() {
                e.apagar_progresso();
            }
            if ui.button("Cancelar").clicked() {
                e.confirmar_apagar = false;
            }
        } else if ui.button("Apagar progresso").clicked() {
            e.confirmar_apagar = true;
        }
    });
    if let Some(m) = &e.mensagem {
        ui.label(RichText::new(m).weak());
    }
    ui.label(
        RichText::new(
            "O compasso do erro conta a partir da sua primeira nota (compasso 1, tempo 1), \
             em 4/4 no andamento escolhido acima.",
        )
        .weak(),
    );
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::som::CANAL_DEMONSTRACAO;
    use crate::som::apoio_testes::{BLOCO, servico_pronto};
    use piano_tutor_audio::RelogioManual;
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_core::tempo::Relogio;
    use std::sync::Arc;

    const MS: u64 = 1_000_000;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota")
    }

    fn estado_no_exercicio_2() -> EstadoExercicios {
        let mut e = EstadoExercicios::novo(None);
        e.progresso[0].concluido = true;
        assert!(e.selecionar(1));
        e
    }

    #[test]
    fn ouvir_tentativa_toca_de_novo_o_que_o_aluno_tocou_inclusive_o_erro() {
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        motor.renderizar(BLOCO, 0);
        let mut e = estado_no_exercicio_2();
        // Sem toques: nada a ouvir e nada agendado.
        assert!(!ouvir_tentativa(&mut som, &e));
        // Dó (certo), Ré (certo), Fá# (erro), Mi (certo), em instantes do relógio do aluno.
        for (x, t_ms) in [(60u8, 2000u64), (62, 2500), (66, 3100), (64, 3600)] {
            e.classificar(true, n(x), t_ms * MS);
        }
        assert!(ouvir_tentativa(&mut som, &e));
        motor.renderizar(48_000 * 4, 0);
        let v: Vec<(bool, u8, u8, u64)> = motor
            .registro()
            .iter()
            .filter_map(|r| match *r {
                RegistroFalso::Ligada {
                    frame, canal, nota, ..
                } => Some((true, canal, nota, frame)),
                RegistroFalso::Desligada {
                    frame, canal, nota, ..
                } => Some((false, canal, nota, frame)),
                _ => None,
            })
            .collect();
        assert!(v.iter().all(|x| x.1 == CANAL_DEMONSTRACAO));
        let ligadas: Vec<(u8, u64)> = v.iter().filter(|x| x.0).map(|x| (x.2, x.3)).collect();
        assert_eq!(
            ligadas.iter().map(|x| x.0).collect::<Vec<_>>(),
            vec![60, 62, 66, 64],
            "mesma ordem, com o erro"
        );
        // Mesmos intervalos do que foi tocado: 500, 600 e 500 ms (48 frames por ms).
        for (i, ms) in [(1usize, 500.0), (2, 1100.0), (3, 1600.0)] {
            let d = (ligadas[i].1 - ligadas[0].1) as f64;
            assert!((d - ms * 48.0).abs() <= 2.0 * BLOCO as f64, "nota {i}: {d}");
        }
        assert_eq!(motor.notas_ativas(), 0, "tudo solto no fim");
        assert_eq!(v.iter().filter(|x| !x.0).count(), 4);
    }
}
