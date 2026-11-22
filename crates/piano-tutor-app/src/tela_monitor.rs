//! Painel Monitor MIDI (F01, #32): portas, conexão, estado, capacidades, log NDJSON e a tabela
//! dos últimos eventos. Só desenha e coleta pedidos; o estado está em [`crate::monitor`].

use crate::monitor::{AcaoMonitor, EstadoMonitor};
use bancada_casca::egui::{self, Color32, RichText, ScrollArea, Ui};
use piano_tutor_midi::EstadoConexao;

const VERDE: Color32 = Color32::from_rgb(98, 200, 120);
const AMBAR: Color32 = Color32::from_rgb(255, 214, 102);
const VERMELHO: Color32 = Color32::from_rgb(255, 120, 110);

/// Desenha o painel e executa os pedidos feitos neste quadro. Devolve os pedidos (testes).
pub fn desenhar_tela(ui: &mut Ui, m: &mut EstadoMonitor) -> Vec<AcaoMonitor> {
    let mut pedidos = Vec::new();
    ScrollArea::vertical()
        .id_salt("monitor-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Monitor MIDI");
            ui.label(
                "Escolha a porta do teclado e conecte. Se o cabo sair, o estado muda sozinho, as \
                 notas são soltas e a conexão volta quando a porta reaparecer.",
            );
            ui.add_space(8.0);
            desenhar_conexao(ui, m, &mut pedidos);
            ui.add_space(10.0);
            ui.separator();
            desenhar_capacidades(ui, m);
            ui.add_space(10.0);
            ui.separator();
            desenhar_log(ui, m, &mut pedidos);
            ui.add_space(10.0);
            ui.separator();
            desenhar_eventos(ui, m, &mut pedidos);
        });
    if !pedidos.is_empty() {
        for p in pedidos.iter().cloned() {
            m.executar(p);
        }
        // As saídas (limpar notas, novo estado) saem no próximo `atualizar`: pedir o quadro já.
        ui.ctx().request_repaint();
    }
    pedidos
}

fn cor_do_estado(e: &EstadoConexao) -> Color32 {
    match e {
        EstadoConexao::Conectado { .. } => VERDE,
        EstadoConexao::Conectando { .. } | EstadoConexao::Desconectado => AMBAR,
        EstadoConexao::Erro { .. } => VERMELHO,
    }
}

fn desenhar_conexao(ui: &mut Ui, m: &EstadoMonitor, pedidos: &mut Vec<AcaoMonitor>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Estado:").strong());
        ui.label(
            RichText::new(m.texto_estado())
                .strong()
                .color(cor_do_estado(m.estado())),
        );
    });
    if let Some(d) = m.detalhe_estado() {
        ui.label(RichText::new(d).weak());
    }
    ui.add_space(6.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Porta:");
        let texto = m.selecao().unwrap_or("(nenhuma)").to_owned();
        egui::ComboBox::from_id_salt("porta-midi")
            .width(380.0)
            .selected_text(texto)
            .show_ui(ui, |ui| {
                for p in m.portas() {
                    let marcada = m.selecao() == Some(p.as_str());
                    if ui.selectable_label(marcada, p).clicked() {
                        pedidos.push(AcaoMonitor::Escolher(p.clone()));
                    }
                }
            });
        if ui
            .button("Atualizar lista")
            .on_hover_text("Lista de novo as portas MIDI de entrada")
            .clicked()
        {
            pedidos.push(AcaoMonitor::AtualizarLista);
        }
    });
    if m.portas().is_empty() {
        ui.label(RichText::new("Nenhuma porta MIDI de entrada encontrada.").color(AMBAR));
    }
    if let Some(e) = m.gerenciador().ultimo_erro_de_listagem() {
        ui.label(RichText::new(format!("Falha ao listar portas: {e}")).color(AMBAR));
    }

    let tem_porta = m.gerenciador().porta_selecionada().is_some();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(m.selecao().is_some(), egui::Button::new("Conectar"))
            .clicked()
        {
            pedidos.push(AcaoMonitor::Conectar);
        }
        if ui
            .add_enabled(tem_porta, egui::Button::new("Desconectar"))
            .on_hover_text("Fecha a porta, esquece a seleção e solta as notas")
            .clicked()
        {
            pedidos.push(AcaoMonitor::Desconectar);
        }
        if ui
            .add_enabled(tem_porta, egui::Button::new("Reconectar"))
            .on_hover_text("Fecha e reabre a porta escolhida")
            .clicked()
        {
            pedidos.push(AcaoMonitor::Reconectar);
        }
    });
}

fn desenhar_capacidades(ui: &mut Ui, m: &EstadoMonitor) {
    ui.label(RichText::new("Capacidades observadas").strong());
    ui.label(
        RichText::new(
            "Só o que chegou do dispositivo nesta conexão. Nada é presumido: o que não foi \
             observado aparece como não suportado.",
        )
        .weak(),
    );
    egui::Grid::new("capacidades")
        .num_columns(2)
        .spacing([24.0, 4.0])
        .show(ui, |ui| {
            for l in m.capacidades() {
                ui.label(l.nome);
                let t = RichText::new(l.texto);
                ui.label(if l.observada { t } else { t.color(AMBAR) });
                ui.end_row();
            }
        });
    let c = m.contadores();
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "Mensagens {} · eventos {} · erros de interpretação {} · conexões {} · \
             reconexões {} · desconexões detectadas {}",
            c.mensagens_recebidas,
            c.eventos,
            c.erros_de_interpretacao,
            c.conexoes,
            c.reconexoes,
            c.desconexoes_detectadas
        ))
        .weak(),
    );
}

fn desenhar_log(ui: &mut Ui, m: &EstadoMonitor, pedidos: &mut Vec<AcaoMonitor>) {
    ui.label(RichText::new("Log da sessão (NDJSON anonimizado)").strong());
    ui.horizontal(|ui| {
        if m.registrando() {
            if ui.button("Parar log").clicked() {
                pedidos.push(AcaoMonitor::PararLog);
            }
            ui.label(RichText::new("gravando").color(VERDE));
        } else if ui
            .button("Iniciar log")
            .on_hover_text("Grava um arquivo .ndjson novo por sessão, sem seriais nem VID:PID")
            .clicked()
        {
            pedidos.push(AcaoMonitor::IniciarLog);
        }
    });
    match m.pasta_sessoes() {
        Some(p) => ui.label(RichText::new(format!("Pasta: {}", p.display())).weak()),
        None => ui.label(RichText::new("Pasta de dados do usuário desconhecida.").color(AMBAR)),
    };
    if m.registrando() {
        if let Some(a) = m.arquivo_log() {
            ui.label(format!("Arquivo: {}", a.display()));
        }
        if let Some((linhas, falhas)) = m.gerenciador().registro_linhas_e_falhas() {
            ui.label(format!("{linhas} linhas, {falhas} falhas de escrita"));
        }
        if let Some(e) = m.gerenciador().ultimo_erro_do_registro() {
            ui.label(RichText::new(format!("Último erro de escrita: {e}")).color(AMBAR));
        }
    }
    if let Some(msg) = m.mensagem_log() {
        ui.label(msg);
    }
}

fn desenhar_eventos(ui: &mut Ui, m: &EstadoMonitor, pedidos: &mut Vec<AcaoMonitor>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Últimos eventos").strong());
        ui.label(RichText::new(format!("({})", m.eventos().len())).weak());
        if ui.small_button("Limpar").clicked() {
            pedidos.push(AcaoMonitor::LimparEventos);
        }
    });
    if m.eventos().len() == 0 {
        ui.label(RichText::new("Nenhum evento recebido ainda.").weak());
        return;
    }
    ScrollArea::vertical()
        .id_salt("monitor-eventos")
        .max_height(280.0)
        .stick_to_bottom(true)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("eventos-midi")
                .num_columns(5)
                .striped(true)
                .spacing([18.0, 2.0])
                .show(ui, |ui| {
                    for t in ["Tempo", "Tipo", "Canal", "Dado", "Valor"] {
                        ui.label(RichText::new(t).strong());
                    }
                    ui.end_row();
                    for l in m.eventos() {
                        ui.label(RichText::new(l.tempo()).monospace());
                        ui.label(l.tipo);
                        ui.label(l.canal.map_or_else(|| "—".to_owned(), |c| c.to_string()));
                        ui.label(&l.dado);
                        ui.label(&l.valor);
                        ui.end_row();
                    }
                });
        });
}

#[cfg(test)]
mod testes {
    use super::*;
    use bancada_casca::egui::{Context, RawInput, Rect, pos2, vec2};
    use piano_tutor_midi::{FonteFalsa, RelogioManual};
    use std::sync::Arc;

    fn quadro(ctx: &Context, m: &mut EstadoMonitor) -> Vec<AcaoMonitor> {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            ..Default::default()
        };
        let mut pedidos = Vec::new();
        ctx.run_ui(entrada, |ui| pedidos = desenhar_tela(ui, m))
            .drop_without_applying_deltas();
        pedidos
    }

    #[test]
    fn desenha_vazio_conectado_e_com_eventos_sem_panico() {
        let fonte = FonteFalsa::nova(["Teclado:Teclado MIDI 1 20:0"]);
        let mut m = EstadoMonitor::novo(
            Box::new(fonte.clone()),
            Arc::new(RelogioManual::novo()),
            None,
        );
        let ctx = Context::default();
        assert!(quadro(&ctx, &mut m).is_empty());
        let _ = m.atualizar();
        m.executar(AcaoMonitor::Conectar);
        for n in 60..70 {
            assert!(fonte.injetar(&[0x90, n, 100]));
            assert!(fonte.injetar(&[0x80, n, 0]));
        }
        let _ = m.atualizar();
        for _ in 0..2 {
            quadro(&ctx, &mut m);
        }
        assert_eq!(m.eventos().len(), 20);
        fonte.definir_falha_de_listagem(true);
        m.executar(AcaoMonitor::AtualizarLista);
        quadro(&ctx, &mut m);
    }
}
