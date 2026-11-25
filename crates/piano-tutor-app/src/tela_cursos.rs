//! Tela Cursos (M4, etapa 5, parte do aluno): escolher o Curso, instalar um pacote, percorrer
//! Seções e Lições, abrir um Exercício de Curso (partitura com Praticar, áudio do professor com
//! cursor, vídeo da Aula, Chamada e resposta) e ouvir o professor falar.
//!
//! A tela só mexe no [`EstadoCursos`]; o que atravessa várias partes do app (professor, Chamada
//! e resposta, navegação) volta como [`AcaoCursos`] para o app executar. Texto e símbolo
//! sempre juntos: nunca só cor.

use crate::chamada_app::{ChamadaAtiva, FeedbackVisual};
use crate::cursos::{EstadoCursos, LicaoNaArvore, Mensagem, Selo, formatar_instante};
use crate::espelho::EstadoEspelho;
use crate::exercicios::EstadoExercicios;
use crate::professor::Professor;
use crate::som::ServicoSom;
use crate::tela_espelho::desenhar_teclado_embutido;
use crate::tela_partitura::{OpcoesTela, desenhar_corpo};
use bancada_casca::egui::{
    Button, CollapsingHeader, Color32, RichText, ScrollArea, TextEdit, Ui, vec2,
};
use piano_tutor_curso::{Modo, TipoLicao};

const VERDE: Color32 = Color32::from_rgb(70, 200, 110);
const VERMELHO: Color32 = Color32::from_rgb(235, 100, 100);
const AMARELO: Color32 = Color32::from_rgb(230, 190, 70);

/// O que a tela pede ao app (além do que ela mesma faz no [`EstadoCursos`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcaoCursos {
    /// Um Exercício de Curso acabou de abrir: o professor diz o `antes`.
    AbriuExercicio,
    OuvirProfessor,
    IniciarChamadaCurso,
    IniciarChamadaFabrica,
    PararChamada,
    ProfessorFala(bool),
    /// Preferência "Usar conteúdo não revisado" mudou (o app grava e fecha o que ficou sem uso).
    UsarNaoRevisado(bool),
    /// O Roteiro de Aula pediu que o professor diga este texto.
    Dizer(String),
    /// Abrir o exercício de fábrica de índice `i` na tela Exercícios.
    IrParaFabrica(usize),
}

/// O que a tela precisa saber do resto do app.
pub struct ContextoCursos<'a> {
    pub professor: &'a Professor,
    pub chamada: Option<&'a ChamadaAtiva>,
    pub fabrica: &'a EstadoExercicios,
}

fn mensagem(ui: &mut Ui, m: &Mensagem) {
    if m.erro {
        ui.label(RichText::new(format!("[X] {}", m.texto)).color(VERMELHO));
    } else {
        ui.label(RichText::new(&m.texto).weak());
    }
}

fn cor_do_selo(s: Selo) -> Option<Color32> {
    match s {
        Selo::Fabrica | Selo::Confiavel => Some(VERDE),
        Selo::ChaveDesconhecida => Some(AMARELO),
        Selo::AssinaturaInvalida => Some(VERMELHO),
    }
}

/// Desenha a tela inteira e devolve o que o app deve executar.
pub fn desenhar_tela(
    ui: &mut Ui,
    e: &mut EstadoCursos,
    som: &mut ServicoSom,
    espelho: &mut EstadoEspelho,
    ctx: &ContextoCursos<'_>,
) -> Vec<AcaoCursos> {
    let mut acoes = Vec::new();
    abrir_player_do_ambiente(e);
    ScrollArea::vertical()
        .id_salt("cursos-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if e.player.is_some() {
                desenhar_cabecalho_do_player(ui, e, ctx, &mut acoes);
                crate::tela_player_aula::desenhar_player(ui, e, espelho);
                if let Some(p) = &mut e.player {
                    acoes.extend(p.falas.drain(..).map(AcaoCursos::Dizer));
                }
                return;
            }
            ui.heading("Cursos");
            desenhar_seletor(ui, e);
            ui.add_space(4.0);
            desenhar_instalacao(ui, e);
            ui.add_space(6.0);
            desenhar_detalhe(ui, e, ctx, &mut acoes);
            if e.aberto.is_some() {
                ui.add_space(8.0);
                ui.separator();
                desenhar_exercicio(ui, e, som, espelho, ctx, &mut acoes);
            }
        });
    acoes
}

/// Para testar: `PIANO_TUTOR_PLAYER=<curso>/<lição>/<exercício>[/editar]` abre o Player de Aula no
/// primeiro quadro da tela (junto com `PIANO_TUTOR_ROTA=cursos`).
fn abrir_player_do_ambiente(e: &mut EstadoCursos) {
    static FEITO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if FEITO.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let Ok(pedido) = std::env::var("PIANO_TUTOR_PLAYER") else {
        return;
    };
    let partes: Vec<&str> = pedido.split('/').collect();
    let [curso, licao, resto @ ..] = partes.as_slice() else {
        return;
    };
    e.selecionar_por_id(curso);
    let exercicio = resto.first().copied();
    if let Some(x) = exercicio {
        let _ = e.abrir_exercicio(licao, x);
    }
    crate::tela_player_aula::abrir_player(e, licao, exercicio);
    if resto.get(1) == Some(&"editar")
        && let Some(p) = &mut e.player
    {
        p.pedir_edicao = Some(true);
    }
}

/// Topo da tela com o Player de Aula aberto: de onde veio, como voltar e os avisos.
fn desenhar_cabecalho_do_player(
    ui: &mut Ui,
    e: &mut EstadoCursos,
    ctx: &ContextoCursos<'_>,
    acoes: &mut Vec<AcaoCursos>,
) {
    let titulo = e
        .player
        .as_ref()
        .map(|p| {
            p.arquivo
                .file_stem()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Voltar às Lições")
            .on_hover_text("Fecha o Player de Aula")
            .clicked()
        {
            crate::tela_player_aula::fechar_player(e);
            e.fechar_exercicio();
            acoes.push(AcaoCursos::PararChamada);
        }
        ui.heading(format!("{} · {titulo}", e.curso_selecionado().titulo));
    });
    if let Some(m) = &e.aviso_video {
        mensagem(ui, m);
    }
    if let Some(m) = &e.mensagem {
        mensagem(ui, m);
    }
    if let Some(ac) = desenhar_professor(ui, ctx.professor) {
        acoes.push(ac);
    }
}

// --- Seletor e instalação ---------------------------------------------------------------------

fn desenhar_seletor(ui: &mut Ui, e: &mut EstadoCursos) {
    ui.label(RichText::new("Escolha um Curso").strong());
    let mut escolhido = None;
    ui.horizontal_wrapped(|ui| {
        for (i, c) in e.cursos.iter().enumerate() {
            let texto = format!("{} · {}", c.titulo, c.selo.rotulo());
            if ui
                .add(
                    Button::new(texto)
                        .selected(i == e.indice_selecionado())
                        .min_size(vec2(0.0, 28.0)),
                )
                .on_hover_text(format!("Autor: {}", c.autor))
                .clicked()
            {
                escolhido = Some(i);
            }
        }
    });
    if let Some(i) = escolhido {
        e.selecionar(i);
    }
}

fn desenhar_instalacao(ui: &mut Ui, e: &mut EstadoCursos) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Instalar pacote de Curso (.ptcurso):");
        ui.add(
            TextEdit::singleline(&mut e.campo_pacote)
                .hint_text("/caminho/curso.ptcurso")
                .desired_width(320.0),
        );
        let instalando = e.instalando();
        if ui
            .add_enabled(!instalando, Button::new("Instalar pacote…"))
            .on_hover_text(
                "Confere a assinatura e os arquivos do pacote e o instala na pasta de dados. \
                 Nada do pacote é executado.",
            )
            .clicked()
        {
            let c = e.campo_pacote.clone();
            e.iniciar_instalacao(&c);
        }
        if instalando {
            ui.spinner();
        }
    });
    if let Some(m) = &e.mensagem {
        mensagem(ui, m);
    }
}

// --- Detalhe do Curso e árvore ----------------------------------------------------------------

fn desenhar_detalhe(
    ui: &mut Ui,
    e: &mut EstadoCursos,
    ctx: &ContextoCursos<'_>,
    acoes: &mut Vec<AcaoCursos>,
) {
    // Sem clonar o Curso: com ~150 Lições a cópia por quadro pesaria.
    let idx = e.indice_selecionado();
    let curso = &e.cursos[idx];
    ui.group(|ui| {
        ui.label(RichText::new(&curso.titulo).size(20.0).strong());
        ui.label(format!("Autor: {}", curso.autor));
        if let Some(l) = &curso.licenciado {
            ui.label(format!("Licenciado para: {l}"));
        }
        if let Some(l) = &curso.licenca {
            ui.label(RichText::new(l).weak());
        }
        if let Some(k) = &curso.chave_curta {
            ui.label(format!("Chave de quem assinou: {k}"));
        }
        let mut selo = RichText::new(format!(
            "Selo: {} {}",
            curso.selo.simbolo(),
            curso.selo.rotulo()
        ))
        .strong();
        if let Some(c) = cor_do_selo(curso.selo) {
            selo = selo.color(c);
        }
        ui.label(selo);
        ui.label(RichText::new(curso.selo.explicacao()).weak());
    });
    ui.add_space(4.0);
    if curso.e_fabrica() {
        desenhar_fabrica(ui, ctx.fabrica, acoes);
    } else {
        desenhar_tutor(ui, e, idx, acoes);
    }
}

fn desenhar_fabrica(ui: &mut Ui, fabrica: &EstadoExercicios, acoes: &mut Vec<AcaoCursos>) {
    let feitos = (0..fabrica.exercicios.len())
        .filter(|i| fabrica.concluido(*i))
        .count();
    ui.label(
        RichText::new(format!(
            "Exercícios guiados: {feitos} de {} concluídos",
            fabrica.exercicios.len()
        ))
        .strong(),
    );
    for (i, ex) in fabrica.exercicios.iter().enumerate() {
        let estado = if fabrica.concluido(i) {
            "[OK] concluído".to_owned()
        } else if !fabrica.desbloqueado(i) {
            format!(
                "[bloqueado] {}",
                fabrica.motivo_bloqueio(i).unwrap_or_default()
            )
        } else {
            "liberado".to_owned()
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{}. {} ({}) · {estado}",
                ex.id,
                ex.titulo,
                ex.tipo.texto()
            ));
            if ui
                .add_enabled(fabrica.desbloqueado(i), Button::new("Abrir"))
                .on_hover_text("Abre o exercício na tela Exercícios, com o professor")
                .clicked()
            {
                acoes.push(AcaoCursos::IrParaFabrica(i));
            }
        });
    }
}

/// Cursos com mais Seções que isto abrem só a primeira (a árvore de ~18 Seções e ~150 Lições
/// fica recolhida e leve; o aluno abre a Seção que quer).
const SECOES_ABERTAS_ATE: usize = 3;

fn desenhar_tutor(ui: &mut Ui, e: &mut EstadoCursos, idx: usize, acoes: &mut Vec<AcaoCursos>) {
    let permitir = e.usar_nao_revisado;
    let (feitos, total) = e.resumo_de_progresso(&e.cursos[idx]);
    let (curso_id, aguardando) = (e.cursos[idx].id.clone(), e.cursos[idx].aguardando);
    ui.label(
        RichText::new(format!(
            "Seu progresso: {feitos} de {total} exercício(s) concluído(s)"
        ))
        .strong(),
    );
    let mut usar = permitir;
    if ui
        .checkbox(&mut usar, "Usar conteúdo não revisado")
        .on_hover_text(
            "Ligado: exercícios e falas do professor que o tutor ainda não revisou (hipótese ou \
             em revisão) abrem e tocam normalmente, marcados «não revisado». Desligado: só o \
             conteúdo aprovado. Conteúdo rejeitado nunca é usado. Fica salvo.",
        )
        .changed()
    {
        acoes.push(AcaoCursos::UsarNaoRevisado(usar));
    }
    if !permitir && aguardando > 0 {
        ui.label(
            RichText::new(format!(
                "[!] {} aguardando revisão do tutor (ainda não aparecem para praticar).",
                exercicios_aguardando(aguardando)
            ))
            .color(AMARELO),
        );
    }
    desenhar_pasta_de_videos(ui, e);
    // O aviso do vídeo fica junto do campo da pasta: no fim da árvore ele some da vista e o
    // botão "Ver vídeo" parecia não fazer nada.
    if let Some(m) = &e.aviso_video {
        mensagem(ui, m);
    }
    ui.add_space(4.0);
    if e.cursos[idx].arvore.is_empty() {
        ui.label("Este Curso não tem Seções para mostrar.");
        return;
    }
    let progresso = e.progresso(&curso_id);
    let mut abrir: Option<(String, String)> = None;
    let mut video: Option<String> = None;
    let secoes = e.cursos[idx].arvore.len();
    for (n, s) in e.cursos[idx].arvore.iter().enumerate() {
        CollapsingHeader::new(RichText::new(format!("Seção {}: {}", s.numero, s.titulo)).strong())
            .id_salt(format!("secao-{curso_id}-{}", s.id))
            .default_open(secoes <= SECOES_ABERTAS_ATE || n == 0)
            .show(ui, |ui| {
                for l in &s.licoes {
                    desenhar_licao(ui, l, permitir, &progresso, &mut abrir, &mut video);
                }
            });
    }
    if let Some(l) = video {
        crate::tela_player_aula::abrir_player(e, &l, None);
    }
    if let Some((l, x)) = abrir {
        // O professor é ligado ao Exercício pelo app (AbriuExercicio).
        if e.abrir_exercicio(&l, &x).is_ok() {
            acoes.push(AcaoCursos::AbriuExercicio);
            // Exercício com trecho na Aula e vídeo achado: abre direto no Player de Aula.
            let tem_trecho = e.trechos_da_licao(&l).iter().any(|t| t.exercicio_id == x);
            if tem_trecho && e.resolver_video(&l).is_ok() {
                crate::tela_player_aula::abrir_player(e, &l, Some(&x));
            }
        }
    }
}

/// "1 exercício" / "3 exercícios".
pub fn exercicios_aguardando(n: usize) -> String {
    if n == 1 {
        "1 exercício".to_owned()
    } else {
        format!("{n} exercícios")
    }
}

/// O selo discreto dos Exercícios não revisados: símbolo e texto, nunca só cor.
pub const SELO_NAO_REVISADO: &str = "[!] não revisado";

/// "1 não revisado" / "3 não revisados".
pub fn nao_revisados(n: usize) -> String {
    if n == 1 {
        "1 não revisado".to_owned()
    } else {
        format!("{n} não revisados")
    }
}

fn selo_nao_revisado(ui: &mut Ui) {
    ui.label(RichText::new(SELO_NAO_REVISADO).small().color(AMARELO))
        .on_hover_text(
            "O tutor ainda não revisou este conteúdo (hipótese ou em revisão). Pode conter \
             erros; use por sua conta.",
        );
}

fn desenhar_licao(
    ui: &mut Ui,
    l: &LicaoNaArvore,
    permitir: bool,
    progresso: &crate::cursos::ProgressoCurso,
    abrir: &mut Option<(String, String)>,
    video: &mut Option<String>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Aula {}: {}", l.numero, l.titulo)).strong());
        if l.tipo == TipoLicao::Assistir
            && ui
                .button("Ver vídeo")
                .on_hover_text("Abre esta Aula no Player (você indica a pasta dos vídeos uma vez)")
                .clicked()
        {
            *video = Some(l.id.clone());
        }
    });
    if l.tipo == TipoLicao::Assistir {
        ui.label(RichText::new("Lição para assistir.").weak());
        return;
    }
    ui.indent(format!("licao-{}", l.id), |ui| {
        let mut mostrados = 0;
        for x in l.visiveis(permitir) {
            mostrados += 1;
            ui.horizontal_wrapped(|ui| {
                let situacao = match progresso.item(&x.id) {
                    Some(i) if i.concluido => {
                        format!("[OK] concluído, melhor {:.0}%", i.melhor_pct)
                    }
                    Some(i) => format!("melhor até agora: {:.0}%", i.melhor_pct),
                    None => "ainda não praticado".to_owned(),
                };
                let modo = match x.modo {
                    Modo::Praticar => "praticar",
                    Modo::ChamadaEResposta => "chamada e resposta",
                };
                ui.label(format!("{} ({modo}) · {situacao}", x.titulo));
                if x.nao_revisado() {
                    selo_nao_revisado(ui);
                }
                if ui.button("Abrir").clicked() {
                    *abrir = Some((l.id.clone(), x.id.clone()));
                }
            });
        }
        if mostrados == 0 && l.aguardando == 0 {
            ui.label(RichText::new("Sem exercícios nesta Lição.").weak());
        }
        if permitir && l.aguardando > 0 {
            ui.label(RichText::new(nao_revisados(l.aguardando)).small().weak());
        }
        if !permitir && l.aguardando > 0 {
            ui.label(
                RichText::new(format!(
                    "[!] {} aguardando revisão do tutor",
                    exercicios_aguardando(l.aguardando)
                ))
                .color(AMARELO),
            );
        }
    });
}

fn desenhar_pasta_de_videos(ui: &mut Ui, e: &mut EstadoCursos) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Pasta dos vídeos do Curso:");
        ui.add(
            TextEdit::singleline(&mut e.campo_videos)
                .hint_text("/pasta/com/os/vídeos")
                .desired_width(320.0),
        );
        if ui
            .button("Usar esta pasta")
            .on_hover_text(
                "Os vídeos ficam no seu computador; o Curso só diz o nome do arquivo e o instante",
            )
            .clicked()
        {
            let c = e.campo_videos.clone();
            e.definir_pasta_de_videos(&c);
        }
    });
}

// --- Exercício aberto -------------------------------------------------------------------------

/// "Professor fala": o interruptor e a legenda do que ele disse. Devolve a ação se mudou.
pub fn desenhar_professor(ui: &mut Ui, professor: &Professor) -> Option<AcaoCursos> {
    let mut acao = None;
    let mut ligado = professor.ligado();
    ui.horizontal_wrapped(|ui| {
        if ui
            .checkbox(&mut ligado, "Professor fala")
            .on_hover_text("Liga ou desliga a voz do professor e o texto das falas (fica salvo)")
            .changed()
        {
            acao = Some(AcaoCursos::ProfessorFala(ligado));
        }
        if professor.falando() {
            ui.label(RichText::new("falando…").weak());
        }
    });
    if professor.ligado() {
        if let Some(t) = professor.legenda() {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("Professor: «{t}»")).size(16.0));
                    if professor.legenda_nao_revisada() {
                        selo_nao_revisado(ui);
                    }
                });
            });
        }
        if let Some(a) = professor.aviso_de_voz() {
            ui.label(RichText::new(format!("[!] {a}")).weak());
        }
        if !professor.tem_falas_para_dizer() {
            let aviso = if professor.usar_nao_revisado() {
                "Este exercício não tem falas no Roteiro: o professor fica em silêncio."
            } else {
                "Este exercício ainda não tem falas aprovadas: o professor fica em silêncio."
            };
            ui.label(RichText::new(aviso).weak());
        }
    }
    acao
}

/// Painel da Chamada e resposta em andamento: estado, anúncio, resultado e como parar.
pub fn desenhar_painel_chamada(ui: &mut Ui, c: &ChamadaAtiva) -> Vec<AcaoCursos> {
    let mut acoes = Vec::new();
    ui.group(|ui| {
        let (n, total) = c.progresso();
        ui.label(RichText::new(format!("Chamada e resposta · chamada {n} de {total}")).strong());
        let cor = match &c.feedback {
            Some(FeedbackVisual { acertou: true, .. }) => Some(VERDE),
            Some(_) => Some(VERMELHO),
            None => None,
        };
        let mut estado = RichText::new(c.texto_do_estado()).size(26.0).strong();
        if let Some(cor) = cor {
            estado = estado.color(cor);
        }
        ui.label(estado);
        if let Some(f) = &c.feedback
            && !f.detalhe.is_empty()
        {
            ui.label(&f.detalhe);
        }
        if let Some(a) = &c.anuncio {
            ui.label(RichText::new(format!("Professor: «{a}»")).weak());
        }
        let rotulo = if c.concluida() {
            "Fechar a chamada"
        } else {
            "Parar a chamada"
        };
        if ui.button(rotulo).clicked() {
            acoes.push(AcaoCursos::PararChamada);
        }
    });
    acoes
}

fn desenhar_exercicio(
    ui: &mut Ui,
    e: &mut EstadoCursos,
    som: &mut ServicoSom,
    espelho: &mut EstadoEspelho,
    ctx: &ContextoCursos<'_>,
    acoes: &mut Vec<AcaoCursos>,
) {
    let Some(a) = e.aberto.clone() else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("Exercício: {}", a.exercicio.titulo))
                .size(22.0)
                .strong(),
        );
        if a.exercicio.nao_revisado() {
            selo_nao_revisado(ui);
        }
    });
    ui.label(
        RichText::new(format!(
            "{} · Aula {}: {}",
            a.curso_titulo, a.licao_numero, a.licao_titulo
        ))
        .weak(),
    );
    let chamada_ativa = ctx.chamada.is_some_and(|c| !c.concluida());
    let modo_chamada = a.modo() == Modo::ChamadaEResposta;
    ui.horizontal_wrapped(|ui| {
        let botao_chamada = |ui: &mut Ui, acoes: &mut Vec<AcaoCursos>| {
            let r = ui
                .add_enabled(!chamada_ativa, Button::new("Chamada e resposta"))
                .on_hover_text(
                    "O professor anuncia e toca uma frase de 2 compassos; você a repete no teclado",
                );
            if r.clicked() {
                acoes.push(AcaoCursos::IniciarChamadaCurso);
            }
        };
        if modo_chamada {
            botao_chamada(ui, acoes);
        }
        let tem_audio = a.audio.is_some();
        let tocando = e.midia.audio_tocando();
        if tocando {
            if ui.button("Parar o professor").clicked() {
                e.parar_professor();
            }
        } else if ui
            .add_enabled(tem_audio, Button::new("Ouvir o professor"))
            .on_hover_text(if tem_audio {
                "Toca o áudio do tutor (mpv); o cursor da partitura acompanha"
            } else {
                "Este exercício não traz áudio do professor"
            })
            .clicked()
        {
            acoes.push(AcaoCursos::OuvirProfessor);
        }
        if ui
            .add_enabled(a.aula.is_some(), Button::new("Ver vídeo"))
            .on_hover_text(
                "Abre o Player de Aula: vídeo, partitura e teclado juntos, no instante em que o \
                 tutor toca este exercício",
            )
            .clicked()
        {
            let (l, x) = (a.licao_id.clone(), a.exercicio.id.clone());
            crate::tela_player_aula::abrir_player(e, &l, Some(&x));
        }
        if !modo_chamada {
            botao_chamada(ui, acoes);
        }
        if ui.button("Fechar exercício").clicked() {
            e.fechar_exercicio();
            acoes.push(AcaoCursos::PararChamada);
        }
    });
    if modo_chamada {
        ui.label(
            RichText::new("Este exercício é de chamada e resposta: ouça o professor e repita.")
                .weak(),
        );
    }
    if e.midia.audio_tocando() {
        ui.label(RichText::new("Ouvindo o professor… o cursor segue o áudio.").weak());
    }
    if let Some(t) = a.exercicio.trecho_aula {
        ui.label(
            RichText::new(format!(
                "No vídeo da Aula: de {} a {}.",
                formatar_instante(t.inicio_s),
                formatar_instante(t.fim_s)
            ))
            .weak(),
        );
    }
    if let Some(m) = &e.aviso_video {
        mensagem(ui, m);
    }
    if let Some(m) = &e.mensagem {
        mensagem(ui, m);
    }
    if let Some(ac) = desenhar_professor(ui, ctx.professor) {
        acoes.push(ac);
    }
    if let Some(c) = ctx.chamada {
        acoes.extend(desenhar_painel_chamada(ui, c));
    }
    ui.add_space(4.0);
    let cursor = e.cursor_do_audio();
    desenhar_corpo(
        ui,
        &mut e.partitura,
        som,
        OpcoesTela {
            com_titulo: false,
            com_origem: false,
            com_exportacao: false,
            cursor_externo: cursor,
        },
    );
    ui.add_space(4.0);
    desenhar_teclado_embutido(ui, espelho);
}

// Para a tela Exercícios (fábrica): o botão da Chamada e resposta e o painel.

/// Botão "Chamada e resposta" (só em acordes e intervalos) e o painel da chamada em curso.
pub fn desenhar_chamada_de_fabrica(
    ui: &mut Ui,
    habilitado: bool,
    chamada: Option<&ChamadaAtiva>,
) -> Vec<AcaoCursos> {
    let mut acoes = Vec::new();
    let ativa = chamada.is_some_and(|c| !c.concluida());
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(habilitado && !ativa, Button::new("Chamada e resposta"))
            .on_hover_text(if habilitado {
                "O professor anuncia e toca cada acorde ou intervalo; você o repete no teclado"
            } else {
                "Só nos exercícios de acordes e intervalos"
            })
            .clicked()
        {
            acoes.push(AcaoCursos::IniciarChamadaFabrica);
        }
    });
    if let Some(c) = chamada {
        acoes.extend(desenhar_painel_chamada(ui, c));
    }
    acoes
}
