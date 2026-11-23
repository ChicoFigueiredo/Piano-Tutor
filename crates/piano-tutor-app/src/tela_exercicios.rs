//! Tela Exercícios (F05): seletores de Lição e Exercício lado a lado, ficha com roteiro e
//! sequência, feedback com texto e símbolo (nunca só cor), atalhos de teclado, estatísticas e
//! comandos de progresso.

use crate::espelho::EstadoEspelho;
use crate::exercicios::{
    Classificacao, ConfigExercicios, EstadoExercicios, Exercicio, Fase, JANELA_MAX_MS,
    JANELA_MIN_MS, Tipo, Tom, nome_nota,
};
use crate::gravacao::agendar_gravacao;
use crate::licoes::{Origem, Situacao};
use crate::som::ServicoSom;
use crate::tela_espelho::desenhar_teclado_embutido;
use bancada_casca::egui::{
    Button, Color32, ComboBox, CornerRadius, Frame, Key, Margin, Modifiers, RichText, ScrollArea,
    Slider, Stroke, Ui, vec2,
};
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

/// Ações de teclado da tela (nenhuma colide com as letras que tocam no teclado do PC).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Atalho {
    Ouvir,
    OuvirTentativa,
    Repetir,
    Limpar,
    Pausar,
    Proximo,
    Anterior,
    Dica,
}

/// Tabela de atalhos: (tecla, ação, rótulo da tecla, descrição).
pub const ATALHOS: [(Key, Atalho, &str, &str); 10] = [
    (Key::M, Atalho::Ouvir, "M", "ouvir o modelo"),
    (Key::C, Atalho::OuvirTentativa, "C", "conferir o que toquei"),
    (Key::R, Atalho::Repetir, "R", "repetir o passo"),
    (
        Key::Backspace,
        Atalho::Limpar,
        "Backspace",
        "limpar e recomeçar",
    ),
    (Key::Space, Atalho::Pausar, "Espaço", "pausar ou retomar"),
    (Key::N, Atalho::Proximo, "N", "próximo exercício"),
    (Key::PageDown, Atalho::Proximo, "PgDn", "próximo exercício"),
    (Key::B, Atalho::Anterior, "B", "exercício anterior"),
    (Key::PageUp, Atalho::Anterior, "PgUp", "exercício anterior"),
    (Key::V, Atalho::Dica, "V", "ver ou esconder as teclas-guia"),
];

/// Rótulo da tecla de uma ação (a primeira da tabela).
fn tecla_de(a: Atalho) -> &'static str {
    ATALHOS
        .iter()
        .find(|(_, x, _, _)| *x == a)
        .map_or("", |(_, _, r, _)| r)
}

/// Botão com a tecla de atalho no rótulo ("Repetir  [R]").
fn botao(ui: &mut Ui, ativo: bool, texto: &str, a: Atalho, dica: &str) -> bool {
    ui.add_enabled(
        ativo,
        Button::new(format!("{texto}  [{}]", tecla_de(a))).min_size(vec2(0.0, 30.0)),
    )
    .on_hover_text(dica)
    .clicked()
}

/// Executa uma ação (botão ou atalho).
pub fn executar(a: Atalho, e: &mut EstadoExercicios, som: &mut ServicoSom) {
    match a {
        Atalho::Ouvir => ouvir(som, e),
        Atalho::OuvirTentativa => {
            ouvir_tentativa(som, e);
        }
        Atalho::Repetir => {
            e.repetir();
            if e.exercicio().is_some_and(|x| x.de_ouvido()) {
                ouvir(som, e);
            }
        }
        Atalho::Limpar => e.reiniciar(),
        Atalho::Pausar => {
            if e.tentativa.pausado_desde.is_some() {
                e.retomar(som.agora_ns());
            } else {
                e.pausar(som.agora_ns());
            }
        }
        Atalho::Proximo => {
            e.avancar();
        }
        Atalho::Anterior => {
            e.voltar();
        }
        Atalho::Dica => {
            e.config.mostrar_dica = !e.config.mostrar_dica;
            e.salvar();
        }
    }
}

/// Lê os atalhos do quadro (sem modificador, fora de campo de texto e de menu aberto).
fn ler_atalhos(ui: &mut Ui) -> Vec<Atalho> {
    if ui.ctx().egui_wants_keyboard_input() || !ui.input(|i| i.focused) {
        return Vec::new();
    }
    ui.input_mut(|i| {
        ATALHOS
            .iter()
            .filter(|(k, _, _, _)| i.consume_key(Modifiers::NONE, *k))
            .map(|(_, a, _, _)| *a)
            .collect()
    })
}

pub fn desenhar_tela(
    ui: &mut Ui,
    e: &mut EstadoExercicios,
    espelho: &mut EstadoEspelho,
    som: &mut ServicoSom,
) {
    // Limite de tempo opcional e gravação do progresso quando as teclas estão soltas.
    e.verificar_tempo(som.agora_ns());
    if espelho.teclas().notas_ligadas().next().is_none() {
        e.salvar_se_sujo();
    }
    for a in ler_atalhos(ui) {
        executar(a, e, som);
    }
    ScrollArea::vertical()
        .id_salt("exercicios-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            desenhar_seletores(ui, e);
            ui.add_space(8.0);
            desenhar_ficha(ui, e, espelho.nomenclatura);
            ui.add_space(8.0);
            desenhar_execucao(ui, e, som);
            ui.add_space(6.0);
            desenhar_feedback(ui, e);
            desenhar_erros(ui, e);
            ui.add_space(6.0);
            espelho.definir_guias(e.guias(), e.dedos());
            desenhar_teclado_embutido(ui, espelho);
            ui.label(
                RichText::new(
                    "Bolinha verde = tecla a tocar, com o dedo sugerido dentro (escolha números ou \
                     letras em Início > Dedilhado). Azul = tecla que você está tocando. \
                     O resultado também aparece em texto: [OK] certo, [X] errado, [...] falta nota.",
                )
                .weak(),
            );
            ui.add_space(6.0);
            desenhar_estatisticas(ui, e);
            ui.add_space(6.0);
            desenhar_atalhos(ui);
            ui.add_space(6.0);
            desenhar_ajustes(ui, e);
            ui.add_space(6.0);
            desenhar_progresso(ui, e);
        });
}

fn trilhas(e: &EstadoExercicios) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for l in &e.licoes {
        if !v.contains(&l.trilha) {
            v.push(l.trilha.clone());
        }
    }
    v
}

fn texto_licao(e: &EstadoExercicios, i: usize) -> String {
    let l = &e.licoes[i];
    let (f, t) = e.andamento_licao(i);
    let marca = if f == t { "[OK] " } else { "" };
    format!("{marca}{}  ({f}/{t})", l.titulo)
}

fn texto_exercicio(e: &EstadoExercicios, x: usize) -> String {
    let ex = &e.exercicios[x];
    let estado = if e.concluido(x) {
        "  [OK]"
    } else if e.desbloqueado(x) {
        ""
    } else {
        "  [bloqueado]"
    };
    format!("{}. {}{estado}", ex.numero, ex.titulo)
}

/// Dois seletores lado a lado: Lição (agrupada por Trilha) e Exercício da Lição.
fn desenhar_seletores(ui: &mut Ui, e: &mut EstadoExercicios) {
    let atual_licao = e.exercicio().map_or(0, |x| x.licao);
    let mut abrir_licao = None;
    let mut escolhido = None;
    let largura = ((ui.available_width() - 16.0) / 2.0).clamp(220.0, 520.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Grupo de exercícios (Trilha e Lição)")
                    .small()
                    .weak(),
            );
            let atual = e
                .licoes
                .get(atual_licao)
                .map(|l| format!("{}  >  {}", l.trilha, texto_licao(e, atual_licao)))
                .unwrap_or_default();
            ComboBox::from_id_salt("exercicios-licao")
                .selected_text(RichText::new(atual).size(15.0))
                .width(largura)
                .height(520.0)
                .show_ui(ui, |ui| {
                    for trilha in trilhas(e) {
                        ui.add_space(4.0);
                        ui.label(RichText::new(&trilha).strong().size(15.0));
                        for i in (0..e.licoes.len()).filter(|i| e.licoes[*i].trilha == trilha) {
                            let livre = e.faixas[i].clone().any(|x| e.desbloqueado(x));
                            let mut r = ui.add_enabled(
                                livre,
                                Button::selectable(
                                    i == atual_licao,
                                    format!("     {}", texto_licao(e, i)),
                                ),
                            );
                            if !e.licoes[i].descricao.is_empty() {
                                r = r.on_hover_text(&e.licoes[i].descricao);
                            }
                            if r.clicked() {
                                abrir_licao = Some(i);
                            }
                        }
                    }
                });
        });
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("Exercício da Lição").small().weak());
            let atual = if e.indice() < e.exercicios.len() {
                texto_exercicio(e, e.indice())
            } else {
                String::new()
            };
            ComboBox::from_id_salt("exercicios-exercicio")
                .selected_text(RichText::new(atual).size(15.0))
                .width(largura)
                .height(520.0)
                .show_ui(ui, |ui| {
                    for x in e.faixas.get(atual_licao).cloned().unwrap_or_default() {
                        let mut r = ui
                            .add_enabled(
                                e.desbloqueado(x),
                                Button::selectable(x == e.indice(), texto_exercicio(e, x)),
                            )
                            .on_hover_text(&e.exercicios[x].objetivo);
                        if let Some(m) = e.motivo_bloqueio(x) {
                            r = r.on_disabled_hover_text(m);
                        }
                        if r.clicked() {
                            escolhido = Some(x);
                        }
                    }
                });
        });
    });
    if let Some(i) = abrir_licao {
        e.abrir_licao(i);
    }
    if let Some(x) = escolhido {
        e.selecionar(x);
    }
    let total = e.exercicios.len();
    let feitos = (0..total).filter(|i| e.concluido(*i)).count();
    ui.label(
        RichText::new(format!(
            "{} Lições em {} Trilhas; {feitos} de {total} exercícios concluídos.",
            e.licoes.len(),
            trilhas(e).len()
        ))
        .small()
        .weak(),
    );
    for erro in &e.erros_carga {
        ui.label(RichText::new(format!("[!] {erro}")).color(AMARELO).small());
    }
}

/// Roteiro do exercício: o do arquivo, ou um padrão montado com os dados do exercício.
pub fn roteiro(ex: &Exercicio) -> Vec<String> {
    if !ex.roteiro.is_empty() {
        return ex.roteiro.clone();
    }
    let mut v = Vec::new();
    match (ex.mao.is_empty(), ex.dedilhado.is_empty()) {
        (true, true) => {}
        (false, true) => v.push(format!("Use a mão {}.", ex.mao)),
        (true, false) => v.push(format!("Dedilhado: {}.", ex.dedilhado)),
        (false, false) => v.push(format!(
            "Use a mão {}; dedilhado: {}.",
            ex.mao, ex.dedilhado
        )),
    }
    v.push(ex.instrucoes.clone());
    if ex.de_ouvido() {
        v.push(format!(
            "Ouça o modelo [{}] e repita de ouvido; as teclas não são marcadas.",
            tecla_de(Atalho::Ouvir)
        ));
    } else {
        v.push(format!(
            "Toque os {} passos na ordem; a bolinha verde mostra a próxima tecla.",
            ex.passos.len()
        ));
    }
    v.push(
        "Errou? A nota conta como erro, mas o passo continua valendo: corrija e siga.".to_owned(),
    );
    v
}

/// Ficha do exercício: Lição, objetivo, roteiro numerado e sequência do gabarito.
fn desenhar_ficha(ui: &mut Ui, e: &EstadoExercicios, dedos: crate::dedos::NomenclaturaDedos) {
    let Some(ex) = e.exercicio() else {
        ui.label("Nenhum exercício disponível.");
        return;
    };
    let Some(l) = e.licao_atual() else {
        return;
    };
    Frame::group(ui.style())
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(format!("{}  >  {}", l.trilha, l.titulo)).weak());
            let total = e.faixas.get(ex.licao).map_or(0, |f| f.len());
            ui.label(
                RichText::new(format!("Exercício {} de {total}: {}", ex.numero, ex.titulo))
                    .size(22.0)
                    .strong(),
            );
            ui.horizontal_wrapped(|ui| {
                let mut chips = vec![ex.tipo.texto().to_owned()];
                if !ex.mao.is_empty() {
                    chips.push(format!("mão {}", ex.mao));
                }
                if !l.nivel.is_empty() {
                    chips.push(format!("nível {}", l.nivel));
                }
                chips.push(match ex.passos.len() {
                    1 => "1 passo".to_owned(),
                    n => format!("{n} passos"),
                });
                ui.label(RichText::new(chips.join("  ·  ")).small());
                match l.situacao {
                    Situacao::Aprovada => {
                        ui.label(RichText::new("Conteúdo aprovado").small().color(VERDE));
                    }
                    Situacao::Proposta => {
                        ui.label(
                            RichText::new("Proposta: aguardando sua revisão")
                                .small()
                                .color(AMARELO),
                        )
                        .on_hover_text(
                            "Lição escrita a partir de fontes públicas e ainda não revisada. \
                             Para aprovar, mude situacao para \"aprovada\" no arquivo da Lição.",
                        );
                    }
                }
                if !l.fontes.is_empty() {
                    ui.label(RichText::new("Fontes").small().underline())
                        .on_hover_text(l.fontes.join("\n"));
                }
                if let Origem::Usuario(c) = &l.origem {
                    ui.label(
                        RichText::new(format!("arquivo {}", c.display()))
                            .small()
                            .weak(),
                    );
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new(format!("Objetivo: {}", ex.objetivo)).size(16.0));
            ui.add_space(6.0);
            ui.label(RichText::new("Roteiro").strong());
            for (i, passo) in roteiro(ex).iter().enumerate() {
                ui.label(format!("{}.  {passo}", i + 1));
            }
            ui.add_space(6.0);
            ui.label(RichText::new("Sequência").strong());
            desenhar_sequencia(ui, e, ex);
            if !ex.dedilhado.is_empty() {
                let texto = crate::dedos::dedilhado_na_nomenclatura(&ex.dedilhado, dedos);
                ui.label(RichText::new(format!("Dedilhado: {texto}")).weak());
            }
            match ex.prerequisito.as_deref().and_then(|id| e.posicao(id)) {
                Some(i) => ui.label(
                    RichText::new(format!(
                        "Pré-requisito: {} (concluído).",
                        e.exercicios[i].titulo
                    ))
                    .weak()
                    .small(),
                ),
                None => ui.label(RichText::new("Pré-requisito: nenhum.").weak().small()),
            };
        });
}

/// Passos do gabarito como etiquetas: feitos em verde com [OK], o atual destacado.
fn desenhar_sequencia(ui: &mut Ui, e: &EstadoExercicios, ex: &Exercicio) {
    let escondido = ex.de_ouvido() && !e.config.mostrar_dica;
    let feito_tudo = e.tentativa.fase == Fase::Concluido;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (i, p) in ex.passos.iter().enumerate() {
            let feito = feito_tudo || i < e.tentativa.passo;
            let atual = !feito_tudo && i == e.tentativa.passo;
            let nome = if escondido && !feito {
                "?".to_owned()
            } else if p.legenda.is_empty() {
                nomes(&p.notas)
            } else {
                format!("{} ({})", p.legenda, nomes(&p.notas))
            };
            let texto = if feito {
                RichText::new(format!("[OK] {nome}")).color(VERDE)
            } else if atual {
                RichText::new(format!("> {nome}")).strong()
            } else {
                RichText::new(nome).weak()
            };
            let cor = if atual {
                ui.visuals().selection.stroke.color
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke.color
            };
            Frame::new()
                .stroke(Stroke::new(if atual { 2.0 } else { 1.0 }, cor))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(Margin::symmetric(6, 3))
                .show(ui, |ui| {
                    ui.label(texto);
                });
        }
    });
}

/// Passo atual, alvo e botões (cada um com sua tecla de atalho).
fn desenhar_execucao(ui: &mut Ui, e: &mut EstadoExercicios, som: &mut ServicoSom) {
    let Some(ex) = e.exercicio().cloned() else {
        return;
    };
    let total = ex.passos.len();
    if e.tentativa.fase == Fase::Concluido {
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
                RichText::new(format!("Passo {} de {total}", e.tentativa.passo + 1))
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
    let mut acao = None;
    ui.horizontal_wrapped(|ui| {
        let tem_toques = !e.tentativa.registro.is_empty();
        let pausado = e.tentativa.pausado_desde.is_some();
        let tem_proximo = e.desbloqueado(e.indice() + 1);
        let botoes = [
            (true, "Ouvir", Atalho::Ouvir, "Toca o modelo do exercício"),
            (
                tem_toques,
                "Ouvir o que toquei",
                Atalho::OuvirTentativa,
                "Toca de novo as notas que você tocou nesta tentativa",
            ),
            (
                true,
                "Repetir",
                Atalho::Repetir,
                "Repete o passo atual, sem contar",
            ),
            (
                true,
                "Limpar",
                Atalho::Limpar,
                "Limpa a tentativa e volta ao primeiro passo",
            ),
            (
                true,
                if pausado { "Retomar" } else { "Pausar" },
                Atalho::Pausar,
                "Notas não contam enquanto pausado",
            ),
            (
                e.indice() > 0,
                "Anterior",
                Atalho::Anterior,
                "Exercício anterior",
            ),
            (tem_proximo, "Próximo", Atalho::Proximo, "Próximo exercício"),
        ];
        for (ativo, texto, a, dica) in botoes {
            if botao(ui, ativo, texto, a, dica) {
                acao = Some(a);
            }
        }
        if !tem_proximo && e.indice() + 1 < e.exercicios.len() {
            ui.label(RichText::new("Conclua este exercício para liberar o próximo.").weak());
        }
    });
    if let Some(a) = acao {
        executar(a, e, som);
    }
}

/// Legenda dos atalhos.
fn desenhar_atalhos(ui: &mut Ui) {
    ui.label(RichText::new("Atalhos do teclado do PC").strong());
    let texto = ATALHOS
        .iter()
        .map(|(_, _, tecla, desc)| format!("[{tecla}] {desc}"))
        .collect::<Vec<_>>()
        .join("   ");
    ui.label(RichText::new(texto).weak());
    ui.label(
        RichText::new(
            "As letras A S D F G H J K L ; W E T Y U O P continuam tocando notas no modo simulação.",
        )
        .weak()
        .small(),
    );
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
    #[test]
    fn atalhos_nao_colidem_com_as_teclas_que_tocam_notas() {
        for (k, _, rotulo, _) in ATALHOS {
            assert!(
                !crate::espelho::MAPA_PC.iter().any(|(t, _, _)| *t == k),
                "atalho [{rotulo}] tocaria uma nota na simulação"
            );
        }
        let mut teclas: Vec<_> = ATALHOS.iter().map(|x| x.0).collect();
        teclas.dedup();
        assert_eq!(teclas.len(), ATALHOS.len(), "tecla repetida");
    }

    #[test]
    fn roteiro_vem_do_arquivo_ou_e_montado_com_os_dados_do_exercicio() {
        let mut ex = crate::exercicios::exercicios_de_fabrica()[1].clone();
        assert!(!ex.roteiro.is_empty(), "primeiros-passos traz roteiro");
        assert_eq!(roteiro(&ex), ex.roteiro);
        ex.roteiro.clear();
        let r = roteiro(&ex);
        assert!(r.len() >= 3);
        assert!(r.iter().any(|l| l.contains("10 passos")), "{r:?}");
        assert!(r.iter().any(|l| l.contains(&ex.dedilhado)));
    }

    #[test]
    fn atalhos_executam_as_acoes_da_tela() {
        let relogio = RelogioManual::novo();
        let (mut som, _motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        let mut e = estado_no_exercicio_2();
        e.classificar(true, n(60), 0);
        assert_eq!(e.tentativa.passo, 1);
        executar(Atalho::Limpar, &mut e, &mut som);
        assert_eq!(e.tentativa.passo, 0, "Limpar volta ao primeiro passo");
        executar(Atalho::Pausar, &mut e, &mut som);
        assert!(e.tentativa.pausado_desde.is_some());
        executar(Atalho::Pausar, &mut e, &mut som);
        assert!(e.tentativa.pausado_desde.is_none());
        let dica = e.config.mostrar_dica;
        executar(Atalho::Dica, &mut e, &mut som);
        assert_ne!(e.config.mostrar_dica, dica);
        executar(Atalho::Anterior, &mut e, &mut som);
        assert_eq!(e.indice(), 0);
        executar(Atalho::Proximo, &mut e, &mut som);
        assert_eq!(e.indice(), 1);
    }

    #[test]
    fn abrir_licao_vai_ao_primeiro_exercicio_liberado_e_nao_concluido() {
        let mut e = EstadoExercicios::novo(None);
        e.progresso[0].concluido = true;
        assert!(e.abrir_licao(0));
        assert_eq!(e.indice(), 1);
    }
}
