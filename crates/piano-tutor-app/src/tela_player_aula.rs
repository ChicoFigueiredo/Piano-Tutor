//! Tela do Player de Aula (ADR-0011): vídeo da Aula embutido (ou destacado numa janela que
//! pode ser maximizada), partitura com o cursor no instante do vídeo, linha do tempo com os
//! trechos dos Exercícios e o teclado com as teclas e dedos que o professor toca.
//!
//! Atalhos: Espaço pausa e continua; Backspace ou o botão "voltar" do mouse voltam ao início do
//! trecho (dois toques: trecho anterior); Tab ou o botão "avançar" do mouse trocam o Modo de
//! acompanhamento.

use crate::cursos::{EstadoCursos, formatar_instante, indice_da_nota_no_instante};
use crate::espelho::EstadoEspelho;
use crate::player_aula::{
    Acompanhamento, Comando, Dedilhado, ModoAcompanhamento, Trecho, guias_no_instante,
    notas_do_trecho, trecho_no_instante,
};
use crate::roteiro_aula::{Pedido, RoteiroAula, TrechoInfo, ler_roteiro_da_pasta};
use crate::sincronia::{self, EdicaoSincronia, PASSO_ACELERAR, PASSO_TEMPO_S};
use crate::tela_espelho::desenhar_teclado_embutido;
use crate::tela_partitura::desenhar_vista_com_cursor;
use crate::video_embutido::VideoEmbutido;
use bancada_casca::egui::{
    self, Button, Color32, ComboBox, Context, CornerRadius, Key, Modifiers, PointerButton,
    RichText, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, ViewportBuilder,
    ViewportCommand, ViewportId, pos2, vec2,
};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::chamada_resposta::nome_da_nota_no_curso;
use std::path::PathBuf;

/// Tamanho máximo em que o mpv desenha o quadro (o egui escala para o painel).
const QUADRO_MAX: (u32, u32) = (1280, 720);
const VELOCIDADES: [f64; 7] = [0.5, 0.6, 0.75, 0.9, 1.0, 1.25, 1.5];

/// Estado do Player de Aula aberto.
pub struct EstadoPlayer {
    pub licao_id: String,
    pub arquivo: PathBuf,
    video: Option<VideoEmbutido>,
    /// Por que não há vídeo embutido (sem libmpv, arquivo não abre).
    pub erro_video: Option<String>,
    textura: Option<TextureHandle>,
    proporcao: f32,
    pub trechos: Vec<Trecho>,
    pub acompanhamento: Acompanhamento,
    pub dedilhado: Option<Dedilhado>,
    /// Exercício cujo dedilhado está carregado.
    dedilhado_de: Option<String>,
    /// Edição de sincronia em curso (modo editor).
    pub edicao: Option<EdicaoSincronia>,
    pub aviso_edicao: Option<String>,
    /// Pedido do botão "Editar sincronia" (ligar/desligar), atendido com o Exercício à mão.
    pub(crate) pedir_edicao: Option<bool>,
    /// Nota que está sendo arrastada na linha do tempo do editor.
    arrastando: Option<u32>,
    pub destacado: bool,
    /// Fração da largura do topo ocupada pelo vídeo.
    pub fracao_video: f32,
    /// Altura do topo (vídeo e partitura), em pontos; 0 = cabe na tela com o teclado.
    pub altura_topo: f32,
    /// Vídeo à direita da partitura em vez de à esquerda.
    pub video_a_direita: bool,
    pub velocidade: f64,
    /// Roteiro de Aula (Luau, ADR-0012), se a Aula tiver um.
    roteiro: Option<RoteiroAula>,
    /// De onde veio o Roteiro de Aula, ou por que não carregou.
    pub roteiro_info: Option<String>,
    /// Última mensagem que o Roteiro de Aula mostrou.
    pub mensagem_roteiro: Option<String>,
    /// Falas que o Roteiro de Aula pediu ao professor (o app as diz).
    pub falas: Vec<String>,
    trecho_do_roteiro: Option<usize>,
    t_anterior: f64,
}

impl std::fmt::Debug for EstadoPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EstadoPlayer")
            .field("licao_id", &self.licao_id)
            .field("arquivo", &self.arquivo)
            .finish_non_exhaustive()
    }
}

impl EstadoPlayer {
    /// Abre o player no vídeo `arquivo`, no instante `inicio_s`.
    pub fn abrir(licao_id: &str, arquivo: PathBuf, inicio_s: f64, trechos: Vec<Trecho>) -> Self {
        let (video, erro_video) = match VideoEmbutido::carregar() {
            Ok(mut v) => match v.abrir(&arquivo, inicio_s) {
                Ok(()) => (Some(v), None),
                Err(e) => (None, Some(e)),
            },
            Err(e) => (None, Some(e)),
        };
        Self {
            licao_id: licao_id.to_owned(),
            arquivo,
            video,
            erro_video,
            textura: None,
            proporcao: 16.0 / 9.0,
            trechos,
            acompanhamento: Acompanhamento::default(),
            dedilhado: None,
            dedilhado_de: None,
            edicao: None,
            aviso_edicao: None,
            pedir_edicao: None,
            arrastando: None,
            destacado: false,
            fracao_video: 0.45,
            altura_topo: 0.0,
            video_a_direita: false,
            velocidade: 1.0,
            roteiro: None,
            roteiro_info: None,
            mensagem_roteiro: None,
            falas: Vec::new(),
            trecho_do_roteiro: None,
            t_anterior: inicio_s,
        }
    }

    pub fn tem_video(&self) -> bool {
        self.video.is_some()
    }

    fn posicao(&self) -> f64 {
        self.video
            .as_ref()
            .and_then(VideoEmbutido::posicao_s)
            .unwrap_or(0.0)
    }

    fn pausado(&self) -> bool {
        self.video.as_ref().is_none_or(VideoEmbutido::pausado)
    }

    /// Executa o que o Roteiro de Aula pediu.
    fn aplicar(&mut self, r: Result<Vec<Pedido>, String>) {
        let pedidos = match r {
            Ok(v) => v,
            Err(e) => {
                self.roteiro_info = Some(format!("Erro no Roteiro de Aula: {e}"));
                return;
            }
        };
        for pd in pedidos {
            match pd {
                Pedido::Modo(m) => self.acompanhamento.trocar_modo(m),
                Pedido::Ir(id) => {
                    if let Some(t) = self.trechos.iter().find(|t| t.exercicio_id == id) {
                        let s = t.inicio_s;
                        self.acompanhamento.recomecar();
                        self.executar(Comando::Buscar(s));
                    } else {
                        self.roteiro_info =
                            Some(format!("Roteiro pediu um trecho que não existe: {id}"));
                    }
                }
                Pedido::Repetir => {
                    let t = self.posicao() - 0.5;
                    if let Some(i) = trecho_no_instante(&self.trechos, t) {
                        let s = self.trechos[i].inicio_s;
                        self.acompanhamento.recomecar();
                        self.trecho_do_roteiro = None;
                        self.executar(Comando::Buscar(s));
                    }
                }
                Pedido::Pausar => self.executar(Comando::Pausar),
                Pedido::Continuar => self.executar(Comando::Continuar),
                Pedido::Velocidade(x) => self.definir_velocidade(x),
                Pedido::Mensagem(m) => self.mensagem_roteiro = Some(m),
                Pedido::Dizer(t) => self.falas.push(t),
            }
        }
        if let Some(r) = self.roteiro.as_mut() {
            for linha in r.tirar_log() {
                self.roteiro_info = Some(format!("Roteiro: {linha}"));
            }
        }
    }

    fn executar(&mut self, c: Comando) {
        let Some(v) = self.video.as_mut() else {
            return;
        };
        match c {
            Comando::Pausar => v.pausar(true),
            Comando::Continuar => v.pausar(false),
            Comando::Buscar(s) => {
                v.buscar(s);
                v.pausar(false);
            }
        }
    }

    fn alternar_pausa(&mut self) {
        let p = self.pausado();
        if let Some(v) = self.video.as_mut() {
            v.pausar(!p);
        }
        // Retomar à mão libera a espera.
        if p {
            self.acompanhamento.esperando = None;
            self.acompanhamento.aguardando_resposta = false;
        }
    }

    fn trocar_modo(&mut self, m: ModoAcompanhamento) {
        self.acompanhamento.trocar_modo(m);
        if let Some(v) = self.video.as_mut()
            && v.pausado()
        {
            v.pausar(false);
        }
    }

    fn definir_velocidade(&mut self, x: f64) {
        self.velocidade = x;
        if let Some(v) = self.video.as_mut() {
            v.definir_velocidade(x);
        }
    }

    /// Sobe o quadro novo do mpv para a textura.
    fn atualizar_textura(&mut self, ctx: &Context) {
        let Some(v) = self.video.as_mut() else {
            return;
        };
        v.atualizar();
        if let Some(img) = v.quadro_novo(QUADRO_MAX.0, QUADRO_MAX.1) {
            let [w, h] = img.size;
            if h > 0 {
                self.proporcao = w as f32 / h as f32;
            }
            match &mut self.textura {
                Some(t) => t.set(img, TextureOptions::LINEAR),
                None => {
                    self.textura =
                        Some(ctx.load_texture("player-aula-video", img, TextureOptions::LINEAR));
                }
            }
        }
    }
}

/// Abre o Player de Aula da Lição `licao_id`, no trecho do Exercício `exercicio_id` (ou no
/// começo da Aula). Se o vídeo não abrir embutido, cai no mpv externo.
pub fn abrir_player(e: &mut EstadoCursos, licao_id: &str, exercicio_id: Option<&str>) {
    let arquivo = match e.resolver_video(licao_id) {
        Ok(a) => a,
        Err(m) => {
            e.aviso_video = Some(crate::cursos::Mensagem::erro(m));
            return;
        }
    };
    let trechos = e.trechos_da_licao(licao_id);
    let inicio = exercicio_id
        .and_then(|id| trechos.iter().find(|t| t.exercicio_id == id))
        .map_or(0.0, |t| t.inicio_s);
    let mut p = EstadoPlayer::abrir(licao_id, arquivo, inicio, trechos);
    for pasta in e.pastas_de_roteiros() {
        match ler_roteiro_da_pasta(&pasta, licao_id) {
            Some(Ok(r)) => {
                p.roteiro = Some(r);
                let origem = if pasta.ends_with("oficina") {
                    "da oficina do tutor"
                } else {
                    "do pacote do Curso"
                };
                p.roteiro_info = Some(format!("Roteiro de Aula {licao_id}.luau, {origem}"));
                let r = p.roteiro.as_mut().map(RoteiroAula::ao_abrir);
                if let Some(r) = r {
                    p.aplicar(r);
                }
                break;
            }
            Some(Err(m)) => {
                p.roteiro_info = Some(format!("Roteiro de Aula não carregou: {m}"));
                break;
            }
            None => {}
        }
    }
    if let Some(erro) = &p.erro_video {
        // Sem libmpv: o vídeo vai para a janela do mpv, e o resto do player segue.
        e.ver_video(licao_id, exercicio_id);
        if let Some(m) = &mut e.aviso_video
            && !m.erro
        {
            m.texto = format!("{} (vídeo fora do app: {erro})", m.texto);
        }
    }
    e.midia.parar_audio();
    e.player = Some(p);
}

pub fn fechar_player(e: &mut EstadoCursos) {
    e.player = None;
    e.notas_do_aluno.clear();
}

/// Os atalhos do player (antes de a partitura ver o Espaço).
fn ler_atalhos(ui: &Ui) -> (bool, bool, bool) {
    if ui.ctx().egui_wants_keyboard_input() {
        return (false, false, false);
    }
    ui.input_mut(|i| {
        let pausa = i.consume_key(Modifiers::NONE, Key::Space);
        let voltar = i.consume_key(Modifiers::NONE, Key::Backspace)
            || i.pointer.button_pressed(PointerButton::Extra1);
        let modo = i.consume_key(Modifiers::NONE, Key::Tab)
            || i.pointer.button_pressed(PointerButton::Extra2);
        (pausa, voltar, modo)
    })
}

/// Desenha o player inteiro. Chamado no lugar do corpo do Exercício quando o player está aberto.
pub fn desenhar_player(ui: &mut Ui, e: &mut EstadoCursos, espelho: &mut EstadoEspelho) {
    let Some(mut p) = e.player.take() else {
        return;
    };
    let ctx = ui.ctx().clone();
    p.atualizar_textura(&ctx);

    // --- entradas: atalhos e notas do aluno ---
    let (pausa, voltar, modo) = ler_atalhos(ui);
    if pausa {
        p.alternar_pausa();
    }
    if modo {
        let m = p.acompanhamento.modo.proximo();
        p.trocar_modo(m);
    }
    let agora = ui.input(|i| i.time);
    let t = p.posicao();
    if voltar && let Some(c) = p.acompanhamento.voltar(&p.trechos, t, agora) {
        p.executar(c);
    }

    // O trecho em que o vídeo está define o Exercício aberto (partitura e Alinhamento).
    let idx = trecho_no_instante(&p.trechos, t);
    if let Some(i) = idx {
        let id = p.trechos[i].exercicio_id.clone();
        let aberto = e.aberto.as_ref().map(|a| a.exercicio.id.clone());
        if aberto.as_deref() != Some(id.as_str()) && e.abrir_exercicio(&p.licao_id, &id).is_ok() {
            p.acompanhamento.recomecar();
        }
    }
    let exercicio = e.aberto.as_ref().map(|a| a.exercicio.id.clone());
    if p.dedilhado_de != exercicio {
        p.dedilhado = e.dedilhado_do_aberto();
        p.dedilhado_de = exercicio;
        if e.usar_alinhamento_editado() {
            p.aviso_edicao = Some(
                "Usando o Alinhamento ajustado no editor (Hipótese, ainda fora do pacote).".into(),
            );
        }
        if p.edicao.take().is_some_and(|ed| ed.alterada()) {
            p.aviso_edicao = Some("A edição não salva do trecho anterior foi descartada.".into());
        }
    }
    let alinhamento = match &p.edicao {
        Some(ed) => Some(ed.atual.clone()),
        None => e.aberto.as_ref().and_then(|a| a.alinhamento.clone()),
    };
    let notas: Vec<u8> = std::mem::take(&mut e.notas_do_aluno);
    // Roteiro de Aula: entrou num trecho, ou o vídeo passou do fim dele.
    if let Some(i) = idx
        && p.roteiro.is_some()
    {
        let info = TrechoInfo::de_trecho(&p.trechos[i], i);
        if p.trecho_do_roteiro != Some(i) {
            p.trecho_do_roteiro = Some(i);
            let r = p.roteiro.as_mut().map(|r| r.ao_entrar_trecho(&info));
            if let Some(r) = r {
                p.aplicar(r);
            }
        }
        let agora_t = p.posicao();
        let fim = p.trechos[i].fim_s;
        if !p.pausado()
            && p.t_anterior < fim
            && agora_t >= fim
            && agora_t - p.t_anterior < 1.0
            && p.acompanhamento.modo != ModoAcompanhamento::ChamadaEResposta
        {
            let (a, er) = (p.acompanhamento.acertos, p.acompanhamento.erros);
            let r = p.roteiro.as_mut().map(|r| r.ao_fim_trecho(&info, a, er));
            if let Some(r) = r {
                p.aplicar(r);
            }
        }
    }
    p.t_anterior = p.posicao();
    if let (Some(al), Some(i)) = (&alinhamento, idx) {
        let trecho = p.trechos[i].clone();
        let proximo = p.trechos.get(i + 1).cloned();
        for m in notas {
            let acertos_antes = p.acompanhamento.acertos;
            if let Some(c) =
                p.acompanhamento
                    .nota_tocada(al, &trecho, proximo.as_ref(), p.posicao(), m)
            {
                p.executar(c);
            }
            if p.roteiro.is_some() {
                let certa = p.acompanhamento.acertos > acertos_antes;
                let r = p.roteiro.as_mut().map(|r| r.ao_nota(m, certa));
                if let Some(r) = r {
                    p.aplicar(r);
                }
            }
        }
        let pausado = p.pausado();
        if let Some(c) = p.acompanhamento.passo(al, &trecho, p.posicao(), pausado) {
            p.executar(c);
        }
    }

    // --- barra de cima ---
    desenhar_barra(ui, &mut p);
    if let Some(m) = &p.mensagem_roteiro {
        ui.label(
            RichText::new(m)
                .strong()
                .color(Color32::from_rgb(120, 190, 240)),
        );
    }
    if let Some(m) = &p.roteiro_info {
        ui.label(RichText::new(m).small().weak());
    }
    if let Some(i) = idx {
        desenhar_editor(ui, e, &mut p, i);
    }
    if let Some(m) = &p.aviso_edicao {
        ui.label(RichText::new(m).weak());
    }
    if let Some(erro) = &p.erro_video {
        ui.label(
            RichText::new(format!("[!] Vídeo fora do app: {erro}"))
                .color(Color32::from_rgb(230, 190, 70)),
        );
    }

    // --- topo: vídeo e partitura, com alça entre eles e embaixo ---
    let t = p.posicao();
    let cursor = alinhamento.as_ref().and_then(|al| {
        indice_da_nota_no_instante(al, t - al.offset_aula_s)
            .and_then(|i| e.partitura.destaque_da_nota(i as usize))
    });
    let largura = ui.available_width();
    if p.altura_topo <= 0.0 {
        // Primeira vez: deixa lugar para a linha do tempo e o teclado sem rolar.
        p.altura_topo = (ui.available_height() - 230.0).clamp(160.0, 560.0);
    }
    let altura = p.altura_topo.clamp(160.0, 900.0);
    let alca = 6.0;
    let w_video = if p.destacado {
        0.0
    } else {
        (largura * p.fracao_video).clamp(160.0, largura - 200.0)
    };
    let w_partitura = largura - w_video - if p.destacado { 0.0 } else { alca };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let desenhar_video = |ui: &mut Ui, p: &mut EstadoPlayer| {
            if !p.destacado {
                ui.allocate_ui(vec2(w_video, altura), |ui| {
                    desenhar_quadro(ui, p, w_video, altura)
                });
            }
        };
        let desenhar_partitura = |ui: &mut Ui, e: &mut EstadoCursos| {
            ui.allocate_ui(vec2(w_partitura, altura), |ui| {
                ui.set_min_size(vec2(w_partitura, altura));
                desenhar_vista_com_cursor(ui, &mut e.partitura, cursor);
            });
        };
        if p.video_a_direita {
            desenhar_partitura(ui, e);
            if !p.destacado {
                alca_vertical(ui, &mut p, altura, largura, true);
            }
            desenhar_video(ui, &mut p);
        } else {
            desenhar_video(ui, &mut p);
            if !p.destacado {
                alca_vertical(ui, &mut p, altura, largura, false);
            }
            desenhar_partitura(ui, e);
        }
    });
    alca_horizontal(ui, &mut p);

    // --- linha do tempo ---
    match (&p.edicao, idx) {
        (Some(_), Some(i)) => desenhar_linha_do_editor(ui, &mut p, i),
        _ => desenhar_linha_do_tempo(ui, &mut p, alinhamento.as_ref()),
    }

    // --- teclado com as notas e dedos do professor ---
    if let Some(al) = &alinhamento {
        let g = guias_no_instante(al, p.dedilhado.as_ref(), t - al.offset_aula_s);
        let guias: Vec<NotaMidi> = g
            .notas
            .iter()
            .filter_map(|(m, _, _)| NotaMidi::nova(*m))
            .collect();
        let dedos: Vec<(NotaMidi, u8)> = g
            .notas
            .iter()
            .filter_map(|(m, d, _)| Some((NotaMidi::nova(*m)?, (*d)?)))
            .collect();
        espelho.definir_guias(guias, dedos);
        let texto = match (g.antecipada, g.notas.is_empty()) {
            (_, true) => String::new(),
            (true, false) => "Próxima: ".to_owned(),
            (false, false) => "Agora: ".to_owned(),
        };
        if !texto.is_empty() {
            let nomes: Vec<String> = g
                .notas
                .iter()
                .map(|(m, d, _)| {
                    let nome = nome_da_nota_no_curso(*m);
                    match d {
                        Some(d) => format!("{nome} (dedo {d})"),
                        None => nome,
                    }
                })
                .collect();
            ui.label(RichText::new(format!("{texto}{}", nomes.join(", "))).strong());
        }
    }
    desenhar_teclado_embutido(ui, espelho);

    // --- janela destacada ---
    if p.destacado {
        let mut fechar = false;
        let textura = p.textura.clone();
        let proporcao = p.proporcao;
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("player-aula-video"),
            ViewportBuilder::default()
                .with_title("Piano Tutor · Aula")
                .with_inner_size([960.0, 540.0]),
            |ui, _classe| {
                let r = ui.max_rect();
                ui.painter().rect_filled(r, 0.0, Color32::BLACK);
                quadro_em(ui, textura.as_ref(), proporcao, r);
                let vctx = ui.ctx().clone();
                if ui.input(|i| i.key_pressed(Key::F)) {
                    let cheia = vctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    vctx.send_viewport_cmd(ViewportCommand::Fullscreen(!cheia));
                }
                if vctx.input(|i| i.viewport().close_requested()) {
                    fechar = true;
                }
            },
        );
        if fechar {
            p.destacado = false;
        }
    }

    if !p.pausado() || p.destacado {
        ctx.request_repaint();
    }
    e.player = Some(p);
}

fn desenhar_barra(ui: &mut Ui, p: &mut EstadoPlayer) {
    ui.horizontal_wrapped(|ui| {
        let t = p.posicao();
        if ui
            .button("Voltar trecho")
            .on_hover_text("Volta ao início do trecho (Backspace ou botão 'voltar' do mouse; dois toques: trecho anterior)")
            .clicked()
        {
            let agora = ui.input(|i| i.time);
            if let Some(c) = p.acompanhamento.voltar(&p.trechos, t, agora) {
                p.executar(c);
            }
        }
        let rotulo = if p.pausado() { "Tocar" } else { "Pausar" };
        if ui.button(rotulo).on_hover_text("Espaço").clicked() {
            p.alternar_pausa();
        }
        ui.label(formatar_instante(t));
        ui.separator();
        ui.label("Velocidade:");
        let mut v = p.velocidade;
        ComboBox::from_id_salt("player-velocidade")
            .selected_text(format!("{:.0}%", v * 100.0))
            .show_ui(ui, |ui| {
                for x in VELOCIDADES {
                    ui.selectable_value(&mut v, x, format!("{:.0}%", x * 100.0));
                }
            });
        if (v - p.velocidade).abs() > 1e-9 {
            p.definir_velocidade(v);
        }
        ui.separator();
        ui.label("Acompanhamento:");
        let mut m = p.acompanhamento.modo;
        ComboBox::from_id_salt("player-modo")
            .selected_text(m.rotulo())
            .show_ui(ui, |ui| {
                for x in ModoAcompanhamento::TODOS {
                    ui.selectable_value(&mut m, x, x.rotulo())
                        .on_hover_text(x.explicacao());
                }
            })
            .response
            .on_hover_text("Tab ou botão 'avançar' do mouse trocam o modo");
        if m != p.acompanhamento.modo {
            p.trocar_modo(m);
        }
        let a = &p.acompanhamento;
        ui.label(format!("acertos {} · erros {}", a.acertos, a.erros));
        if a.esperando.is_some() {
            ui.label(RichText::new("Esperando você tocar a nota…").color(Color32::from_rgb(230, 190, 70)));
        }
        if a.aguardando_resposta {
            ui.label(RichText::new(format!("Sua vez: repita o trecho ({} certas)", a.repetidas)).color(Color32::from_rgb(230, 190, 70)));
        }
        ui.separator();
        let destacar = if p.destacado { "Recolher vídeo" } else { "Destacar vídeo" };
        if ui
            .add_enabled(p.tem_video(), Button::new(destacar))
            .on_hover_text("Leva o vídeo para uma janela própria (F nela: tela cheia)")
            .clicked()
        {
            p.destacado = !p.destacado;
        }
        if ui
            .button("Trocar lados")
            .on_hover_text("Troca o vídeo e a partitura de lado")
            .clicked()
        {
            p.video_a_direita = !p.video_a_direita;
        }
        let editando = p.edicao.is_some();
        if ui
            .add(Button::new("Editar sincronia").selected(editando))
            .on_hover_text(
                "Ajusta quando cada nota bate no vídeo: tempo±, acelerar± e Âncoras (M marca a \
                 nota mais perto do instante atual)",
            )
            .clicked()
        {
            p.pedir_edicao = Some(!editando);
        }
    });
}

fn indices_do_trecho(al: &piano_tutor_curso::Alinhamento, trecho: &Trecho) -> Vec<u32> {
    notas_do_trecho(al, trecho)
        .iter()
        .map(|n| n.indice)
        .collect()
}

/// Barra do editor de sincronia (só no modo editor).
fn desenhar_editor(ui: &mut Ui, e: &mut EstadoCursos, p: &mut EstadoPlayer, i: usize) {
    match p.pedir_edicao.take() {
        Some(true) => {
            if let Some(al) = e.aberto.as_ref().and_then(|a| a.alinhamento.as_ref()) {
                p.edicao = Some(EdicaoSincronia::nova(al));
                p.aviso_edicao = None;
                p.acompanhamento
                    .trocar_modo(ModoAcompanhamento::SegueNoTempo);
            } else {
                p.aviso_edicao = Some("Este exercício não tem Alinhamento para ajustar.".into());
            }
        }
        Some(false) if p.edicao.take().is_some_and(|ed| ed.alterada()) => {
            p.aviso_edicao = Some("Edição descartada (não foi salva).".into());
        }
        Some(false) | None => {}
    }
    let trecho = p.trechos[i].clone();
    let t = p.posicao();
    let Some(ed) = p.edicao.as_mut() else {
        return;
    };
    let indices = indices_do_trecho(&ed.atual, &trecho);
    let offset = ed.atual.offset_aula_s;
    let mut salvar = false;
    let mut descartar = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Sincronia do trecho:").strong());
        if ui
            .button("Tempo −")
            .on_hover_text("Notas 50 ms mais cedo")
            .clicked()
        {
            ed.deslocar(&indices, -PASSO_TEMPO_S);
        }
        if ui
            .button("Tempo +")
            .on_hover_text("Notas 50 ms mais tarde")
            .clicked()
        {
            ed.deslocar(&indices, PASSO_TEMPO_S);
        }
        if ui
            .button("Acelerar −")
            .on_hover_text("Estica o trecho 1% (notas mais espaçadas)")
            .clicked()
        {
            ed.escalar(&indices, 1.0 + PASSO_ACELERAR);
        }
        if ui
            .button("Acelerar +")
            .on_hover_text("Comprime o trecho 1% (notas mais juntas)")
            .clicked()
        {
            ed.escalar(&indices, 1.0 - PASSO_ACELERAR);
        }
        let marcar = ui
            .button("Marcar Âncora aqui (M)")
            .on_hover_text("A nota mais perto do instante atual passa a bater exatamente aqui")
            .clicked()
            || (!ui.ctx().egui_wants_keyboard_input()
                && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::M)));
        if marcar {
            match ed.marcar_aqui(&indices, t - offset) {
                Some(n) => p.aviso_edicao = Some(format!("Âncora na nota {}.", n + 1)),
                None => p.aviso_edicao = Some("Nenhuma nota perto deste instante.".into()),
            }
        }
        let desfazer = ui.button("Desfazer").on_hover_text("Ctrl+Z").clicked()
            || (!ui.ctx().egui_wants_keyboard_input()
                && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)));
        if desfazer {
            ed.desfazer();
        }
        ui.label(format!("{} Âncoras", ed.ancoras.len()));
        if ui
            .add_enabled(ed.alterada(), Button::new("Salvar como Hipótese"))
            .on_hover_text(
                "Grava o Alinhamento na oficina do tutor; vai para a revisão e entra no pacote \
                 com scripts/atualizar-pacote-curso.sh",
            )
            .clicked()
        {
            salvar = true;
        }
        if ui.button("Descartar").clicked() {
            descartar = true;
        }
    });
    if descartar {
        p.edicao = None;
        p.aviso_edicao = Some("Edição descartada.".into());
        return;
    }
    if salvar && let Some(ed) = p.edicao.as_ref() {
        let al = ed.para_salvar();
        let r = e
            .destino_do_alinhamento(&al.exercicio)
            .ok_or_else(|| "sem pasta de dados".to_owned())
            .and_then(|d| sincronia::salvar(&d, &al).map(|c| (d, c)));
        match r {
            Ok((d, copia)) => {
                if let Some(a) = &mut e.aberto {
                    a.alinhamento = Some(al);
                }
                p.edicao = None;
                p.aviso_edicao = Some(format!(
                    "Salvo como Hipótese em {}{}.",
                    d.display(),
                    copia.map_or_else(String::new, |c| format!(
                        " (cópia do anterior: {})",
                        c.display()
                    ))
                ));
            }
            Err(m) => p.aviso_edicao = Some(format!("Não consegui salvar: {m}")),
        }
    }
}

/// Linha do tempo do editor: só o trecho, ampliado; cada nota é um traço que se arrasta (vira
/// Âncora ao soltar); Âncoras em amarelo.
fn desenhar_linha_do_editor(ui: &mut Ui, p: &mut EstadoPlayer, i: usize) {
    let trecho = p.trechos[i].clone();
    let (ini, fim) = (trecho.inicio_s - 1.0, trecho.fim_s + 1.0);
    let dur = (fim - ini).max(1.0);
    let (r, resp) =
        ui.allocate_exact_size(vec2(ui.available_width(), 54.0), Sense::click_and_drag());
    let x_de = |s: f64| r.left() + ((s - ini) / dur) as f32 * r.width();
    let s_de = |x: f32| ini + f64::from((x - r.left()) / r.width()) * dur;
    let pintor = ui.painter_at(r);
    pintor.rect_filled(r, CornerRadius::same(3), ui.visuals().extreme_bg_color);
    let faixa = egui::Rect::from_min_max(
        pos2(x_de(trecho.inicio_s), r.top()),
        pos2(x_de(trecho.fim_s), r.bottom()),
    );
    pintor.rect_filled(
        faixa,
        0.0,
        Color32::from_rgba_unmultiplied(70, 130, 220, 40),
    );
    // Marcas de segundo.
    let mut s = ini.ceil();
    while s <= fim {
        let x = x_de(s);
        pintor.line_segment(
            [pos2(x, r.bottom() - 6.0), pos2(x, r.bottom())],
            Stroke::new(1.0, Color32::GRAY),
        );
        s += 1.0;
    }
    let Some(ed) = p.edicao.as_mut() else {
        return;
    };
    let offset = ed.atual.offset_aula_s;
    let ponteiro = ui.input(|i| i.pointer.interact_pos());
    let solto = ui.input(|i| i.pointer.any_released());
    let pressionado = ui.input(|i| i.pointer.primary_pressed());
    let mut perto: Option<(u32, f32)> = None;
    for n in notas_do_trecho(&ed.atual, &trecho) {
        let x = x_de(n.t_audio_s + offset);
        let ancora = ed.ancoras.contains(&n.indice);
        let cor = if ancora {
            Color32::from_rgb(230, 190, 70)
        } else {
            Color32::WHITE
        };
        pintor.line_segment(
            [pos2(x, r.top() + 4.0), pos2(x, r.bottom() - 10.0)],
            Stroke::new(2.0, cor),
        );
        let nome = nome_da_nota_no_curso(n.midi);
        pintor.text(
            pos2(x + 2.0, r.top() + 2.0),
            egui::Align2::LEFT_TOP,
            nome,
            egui::FontId::proportional(10.0),
            cor,
        );
        if let Some(pp) = ponteiro
            && r.contains(pp)
        {
            let d = (pp.x - x).abs();
            if d < 8.0 && perto.is_none_or(|(_, dd)| d < dd) {
                perto = Some((n.indice, d));
            }
        }
    }
    if pressionado && let Some((n, _)) = perto {
        p.arrastando = Some(n);
    }
    let t = p
        .video
        .as_ref()
        .and_then(VideoEmbutido::posicao_s)
        .unwrap_or(0.0);
    let x = x_de(t);
    pintor.line_segment(
        [pos2(x, r.top()), pos2(x, r.bottom())],
        Stroke::new(2.0, Color32::from_rgb(235, 100, 100)),
    );
    if let Some(n) = p.arrastando
        && let Some(pp) = ponteiro
    {
        pintor.line_segment(
            [pos2(pp.x, r.top()), pos2(pp.x, r.bottom())],
            Stroke::new(1.0, Color32::from_rgb(230, 190, 70)),
        );
        if solto {
            ed.fixar_ancora(n, s_de(pp.x) - offset);
            p.arrastando = None;
        }
    } else if p.arrastando.is_none()
        && resp.clicked()
        && let Some(pp) = resp.interact_pointer_pos()
        && let Some(v) = p.video.as_mut()
    {
        v.buscar(s_de(pp.x));
    }
    if solto {
        p.arrastando = None;
    }
    resp.on_hover_text(
        "Arraste uma nota para onde ela bate no vídeo (vira Âncora); clique para ir ao instante",
    );
}

fn desenhar_quadro(ui: &mut Ui, p: &mut EstadoPlayer, w: f32, h: f32) {
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    ui.painter()
        .rect_filled(r, CornerRadius::same(4), Color32::BLACK);
    quadro_em(ui, p.textura.as_ref(), p.proporcao, r);
    if resp.clicked() {
        p.alternar_pausa();
    }
    if resp.double_clicked() && p.tem_video() {
        p.destacado = true;
    }
}

fn quadro_em(ui: &Ui, textura: Option<&TextureHandle>, proporcao: f32, r: egui::Rect) {
    let Some(t) = textura else {
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            "sem vídeo",
            egui::FontId::proportional(16.0),
            Color32::GRAY,
        );
        return;
    };
    let (mut w, mut h) = (r.width(), r.width() / proporcao.max(0.1));
    if h > r.height() {
        h = r.height();
        w = h * proporcao;
    }
    let alvo = egui::Rect::from_center_size(r.center(), vec2(w, h));
    ui.painter().image(
        t.id(),
        alvo,
        egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn alca_vertical(ui: &mut Ui, p: &mut EstadoPlayer, altura: f32, largura: f32, invertida: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(6.0, altura), Sense::drag());
    let cor = if resp.hovered() || resp.dragged() {
        ui.visuals().widgets.active.bg_fill
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };
    ui.painter()
        .rect_filled(r.shrink2(vec2(2.0, 0.0)), 1.0, cor);
    if resp.dragged() {
        let dx = resp.drag_delta().x / largura.max(1.0);
        p.fracao_video = (p.fracao_video + if invertida { -dx } else { dx }).clamp(0.15, 0.85);
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
}

fn alca_horizontal(ui: &mut Ui, p: &mut EstadoPlayer) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::drag());
    let cor = if resp.hovered() || resp.dragged() {
        ui.visuals().widgets.active.bg_fill
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };
    ui.painter()
        .rect_filled(r.shrink2(vec2(0.0, 2.0)), 1.0, cor);
    if resp.dragged() {
        p.altura_topo = (p.altura_topo + resp.drag_delta().y).clamp(160.0, 900.0);
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
}

/// Linha do tempo da Aula: trechos dos Exercícios, notas do Alinhamento e o instante atual.
/// Clicar ou arrastar leva o vídeo àquele instante.
fn desenhar_linha_do_tempo(
    ui: &mut Ui,
    p: &mut EstadoPlayer,
    al: Option<&piano_tutor_curso::Alinhamento>,
) {
    let duracao = p
        .video
        .as_ref()
        .and_then(VideoEmbutido::duracao_s)
        .or_else(|| {
            p.trechos
                .iter()
                .map(|t| t.fim_s)
                .reduce(f64::max)
                .map(|f| f + 10.0)
        })
        .unwrap_or(60.0)
        .max(1.0);
    let (r, resp) =
        ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click_and_drag());
    let pintor = ui.painter_at(r);
    let x_de = |s: f64| r.left() + (s / duracao) as f32 * r.width();
    pintor.rect_filled(r, CornerRadius::same(3), ui.visuals().extreme_bg_color);
    let t = p.posicao();
    let atual = trecho_no_instante(&p.trechos, t);
    for (i, tr) in p.trechos.iter().enumerate() {
        let faixa = egui::Rect::from_min_max(
            pos2(x_de(tr.inicio_s), r.top() + 4.0),
            pos2(
                x_de(tr.fim_s).max(x_de(tr.inicio_s) + 2.0),
                r.bottom() - 12.0,
            ),
        );
        let cor = if Some(i) == atual {
            Color32::from_rgb(70, 130, 220)
        } else {
            Color32::from_rgb(70, 90, 120)
        };
        pintor.rect_filled(faixa, CornerRadius::same(2), cor);
        pintor.text(
            pos2(faixa.left() + 2.0, r.bottom() - 1.0),
            egui::Align2::LEFT_BOTTOM,
            &tr.titulo,
            egui::FontId::proportional(10.0),
            ui.visuals().text_color(),
        );
    }
    if let (Some(al), Some(i)) = (al, atual) {
        for n in notas_do_trecho(al, &p.trechos[i]) {
            let x = x_de(n.t_audio_s + al.offset_aula_s);
            pintor.line_segment(
                [pos2(x, r.top() + 6.0), pos2(x, r.top() + 14.0)],
                Stroke::new(1.0, Color32::WHITE),
            );
        }
    }
    let x = x_de(t);
    pintor.line_segment(
        [pos2(x, r.top()), pos2(x, r.bottom())],
        Stroke::new(2.0, Color32::from_rgb(235, 100, 100)),
    );
    pintor.rect_stroke(
        r,
        CornerRadius::same(3),
        ui.visuals().widgets.noninteractive.bg_stroke,
        StrokeKind::Inside,
    );
    if (resp.clicked() || resp.dragged())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let s = f64::from(((pos.x - r.left()) / r.width()).clamp(0.0, 1.0)) * duracao;
        if let Some(v) = p.video.as_mut() {
            v.buscar(s);
        }
        p.acompanhamento.recomecar();
    }
    resp.on_hover_text(format!(
        "{} · clique para ir ao instante",
        formatar_instante(t)
    ));
}
