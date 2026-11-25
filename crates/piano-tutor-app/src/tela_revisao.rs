//! Tela de revisão do tutor (M4, etapa 5): onde o Chico confere e **aprova** as Hipóteses.
//!
//! - aba **Partituras**: a lista da oficina, a página do PDF (PNG, com zoom) ao lado da
//!   partitura renderizada (com **Ouvir** e andamento ajustável), as divergências e os botões
//!   Em revisão, Aprovar, Rejeitar, Abrir no MuseScore e Recarregar;
//! - aba **Roteiro**: as falas do `roteiro.toml` de cada oficina e as do Roteiro de fábrica,
//!   com texto editável, Ouvir (voz Piper) e os botões de estado.
//!
//! Só o humano aprova: esta tela é o único caminho para `aprovada`, sempre por **Em revisão**
//! (regras em [`crate::revisao`]). O estado puro e a gravação ficam em `revisao`; aqui há o
//! estado da tela ([`EstadoRevisao`], testável sem janela, com o MuseScore e a voz injetáveis) e
//! o desenho.

use crate::app::data_de_hoje;
use crate::professor_falante::{Fala, Momento, Roteiro, Voz, VozMuda, voz_do_sistema};
use crate::revisao::{
    ESTADOS, Estado, FonteDeRoteiro, ImagemRgba, ItemPartitura, Lancador, LancadorMuseScore,
    MudancaDeFala, Oficina, contar_por_estado, descobrir_oficinas, ler_png, rotulo_do_estado,
    simbolo_do_estado, transicao_valida,
};
use crate::som::ServicoSom;
use crate::tela_partitura::{BPM_MAX, BPM_MIN, EstadoPartitura, desenhar_vista};
use bancada_casca::egui::{
    self, Button, Color32, ColorImage, ComboBox, Frame, Image, Margin, Panel, RichText, ScrollArea,
    Slider, TextEdit, TextureHandle, TextureOptions, Ui, load::SizedTexture, vec2,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Rota da tela de revisão do tutor.
pub const ROTA_REVISAO: &str = "revisao-tutor";

/// Aviso fixo no topo: o aluno só vê o que o revisor aprovou.
pub const AVISO_DO_MODO_TUTOR: &str =
    "Modo tutor — conteúdo em Hipótese não aparece para o aluno até ser aprovado.";

/// De quanto em quanto tempo a tela confere a data do MusicXML no disco.
const INTERVALO_DA_CONFERENCIA: Duration = Duration::from_millis(1000);

const ZOOM_PAGINA_MIN: f32 = 0.1;
const ZOOM_PAGINA_MAX: f32 = 4.0;

/// Aba da tela.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbaRevisao {
    Partituras,
    Roteiro,
}

/// Aviso mostrado sob os botões.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Aviso {
    pub erro: bool,
    pub texto: String,
}

impl Aviso {
    fn ok(texto: impl Into<String>) -> Self {
        Self {
            erro: false,
            texto: texto.into(),
        }
    }

    fn erro(texto: impl Into<String>) -> Self {
        Self {
            erro: true,
            texto: texto.into(),
        }
    }
}

/// A página do PDF já como textura.
#[derive(Default)]
struct Pagina {
    /// Arquivo (e data) de que a textura veio; muda quando a seleção ou o arquivo muda.
    chave: Option<(PathBuf, Option<SystemTime>)>,
    textura: Option<TextureHandle>,
    tamanho: [usize; 2],
    erro: Option<String>,
}

/// Estado da tela de revisão.
pub struct EstadoRevisao {
    dados: Option<PathBuf>,
    extra: Option<PathBuf>,
    campo_extra: String,
    iniciado: bool,
    pub aba: AbaRevisao,
    pub oficinas: Vec<Oficina>,
    pub oficina_atual: usize,
    pub aviso: Option<Aviso>,
    lancador: Box<dyn Lancador>,
    voz: Option<Box<dyn Voz>>,
    hoje: fn() -> String,

    // Partituras
    pub itens: Vec<ItemPartitura>,
    pub selecionado: Option<usize>,
    pub filtro_de_partitura: Option<Estado>,
    partitura: EstadoPartitura,
    partitura_valida: bool,
    mtime_carregado: Option<SystemTime>,
    ultima_conferencia: Option<Instant>,
    /// O MusicXML mudou no disco depois de carregado (editado no MuseScore).
    pub arquivo_mudou: bool,
    divergencias: Option<String>,
    pagina: Pagina,
    pub zoom_da_pagina: f32,
    /// Zoom automático: a página ocupa a largura da coluna.
    pub ajustar_pagina: bool,
    pub rejeitando: bool,
    pub nota_da_rejeicao: String,

    // Roteiro
    pub fontes: Vec<FonteDeRoteiro>,
    pub fonte_atual: usize,
    pub roteiro: Option<Roteiro>,
    pub licao_filtrada: Option<String>,
    pub filtro_de_fala: Option<Estado>,
    edicoes: HashMap<String, String>,
    fala_em_voz: Option<String>,
}

impl EstadoRevisao {
    /// `dados` é a pasta de dados do app (`~/.local/share/piano-tutor`); sem ela, não há
    /// oficinas nem revisão do Roteiro de fábrica. As oficinas só são lidas no primeiro quadro.
    pub fn novo(dados: Option<PathBuf>) -> Self {
        let mut partitura = EstadoPartitura::novo(None);
        partitura.seguir = true;
        Self {
            dados,
            extra: None,
            campo_extra: String::new(),
            iniciado: false,
            aba: AbaRevisao::Partituras,
            oficinas: Vec::new(),
            oficina_atual: 0,
            aviso: None,
            lancador: Box::new(LancadorMuseScore),
            voz: None,
            hoje: data_de_hoje,
            itens: Vec::new(),
            selecionado: None,
            filtro_de_partitura: None,
            partitura,
            partitura_valida: false,
            mtime_carregado: None,
            ultima_conferencia: None,
            arquivo_mudou: false,
            divergencias: None,
            pagina: Pagina::default(),
            zoom_da_pagina: 1.0,
            ajustar_pagina: true,
            rejeitando: false,
            nota_da_rejeicao: String::new(),
            fontes: Vec::new(),
            fonte_atual: 0,
            roteiro: None,
            licao_filtrada: None,
            filtro_de_fala: None,
            edicoes: HashMap::new(),
            fala_em_voz: None,
        }
    }

    /// Troca o executor do MuseScore (testes).
    #[must_use]
    pub fn com_lancador(mut self, l: Box<dyn Lancador>) -> Self {
        self.lancador = l;
        self
    }

    /// Troca a voz (testes). Sem isso, a voz do sistema é detectada no primeiro Ouvir.
    #[must_use]
    pub fn com_voz(mut self, v: Box<dyn Voz>) -> Self {
        self.voz = Some(v);
        self
    }

    /// Troca a fonte da data de hoje (testes).
    #[must_use]
    pub fn com_data(mut self, hoje: fn() -> String) -> Self {
        self.hoje = hoje;
        self
    }

    /// Acrescenta uma pasta de oficina fora de `<dados>/fontes` (campo "Abrir outra oficina").
    pub fn definir_oficina_extra(&mut self, caminho: &Path) {
        self.extra = Some(caminho.to_path_buf());
        self.recarregar_oficinas();
        if let Some(i) = self
            .oficinas
            .iter()
            .position(|o| o.raiz.starts_with(caminho) || caminho.starts_with(&o.raiz))
        {
            self.trocar_oficina(i);
        } else {
            self.aviso = Some(Aviso::erro(format!(
                "Não achei uma oficina em {}",
                caminho.display()
            )));
        }
    }

    // --- Início e oficinas ---

    fn garantir_iniciado(&mut self) {
        if !self.iniciado {
            self.iniciado = true;
            self.recarregar_oficinas();
        }
    }

    /// Relê as oficinas e as fontes de Roteiro do disco.
    pub fn recarregar_oficinas(&mut self) {
        self.iniciado = true;
        self.oficinas = self
            .dados
            .as_deref()
            .map(|d| descobrir_oficinas(d, self.extra.as_deref()))
            .or_else(|| {
                self.extra
                    .as_deref()
                    .map(|x| descobrir_oficinas(Path::new(""), Some(x)))
            })
            .unwrap_or_default();
        if self.oficina_atual >= self.oficinas.len() {
            self.oficina_atual = 0;
        }
        self.itens = self
            .oficinas
            .get(self.oficina_atual)
            .map(Oficina::partituras)
            .unwrap_or_default();
        self.selecionado = None;
        self.limpar_arquivos_da_selecao();
        self.fontes = FonteDeRoteiro::listar(self.dados.as_deref(), &self.oficinas);
        if self.fonte_atual >= self.fontes.len() {
            self.fonte_atual = 0;
        }
        self.carregar_roteiro();
    }

    pub fn trocar_oficina(&mut self, i: usize) {
        if i >= self.oficinas.len() {
            return;
        }
        self.oficina_atual = i;
        self.itens = self.oficinas[i].partituras();
        self.selecionado = None;
        self.limpar_arquivos_da_selecao();
    }

    // --- Partituras ---

    pub fn item_selecionado(&self) -> Option<&ItemPartitura> {
        self.selecionado.and_then(|i| self.itens.get(i))
    }

    fn limpar_arquivos_da_selecao(&mut self) {
        self.partitura_valida = false;
        self.mtime_carregado = None;
        self.arquivo_mudou = false;
        self.divergencias = None;
        self.pagina = Pagina::default();
        self.rejeitando = false;
        self.nota_da_rejeicao.clear();
    }

    /// Há partitura carregada e válida para mostrar.
    pub fn partitura_pronta(&self) -> bool {
        self.partitura_valida
    }

    pub fn tocando(&self) -> bool {
        self.partitura.tocando()
    }

    pub fn bpm(&self) -> f32 {
        self.partitura.bpm
    }

    pub fn divergencias(&self) -> Option<&str> {
        self.divergencias.as_deref()
    }

    /// Escolhe a partitura `i` da lista, carrega o MusicXML e as divergências.
    pub fn selecionar(&mut self, i: usize, som: &mut ServicoSom) {
        if i >= self.itens.len() {
            return;
        }
        self.partitura.parar(som);
        self.selecionado = Some(i);
        self.limpar_arquivos_da_selecao();
        self.carregar_arquivos_do_item();
    }

    fn carregar_arquivos_do_item(&mut self) {
        let Some(item) = self.item_selecionado().cloned() else {
            return;
        };
        self.divergencias = item.divergencias_em_texto();
        self.mtime_carregado = item.mtime_do_musicxml();
        self.arquivo_mudou = false;
        self.pagina = Pagina::default();
        if let Some(p) = &item.problema {
            self.partitura_valida = false;
            self.aviso = Some(Aviso::erro(format!("{}: {p}", item.id)));
            return;
        }
        if !item.musicxml_existe() {
            self.partitura_valida = false;
            self.aviso = Some(Aviso::erro(format!(
                "{}: o MusicXML não existe em {}",
                item.id,
                item.musicxml.display()
            )));
            return;
        }
        self.partitura
            .abrir_caminho(&item.musicxml.to_string_lossy());
        match self.partitura.erro.clone() {
            None => {
                self.partitura_valida = true;
                self.aviso = None;
            }
            Some(e) => {
                self.partitura_valida = false;
                self.aviso = Some(Aviso::erro(format!("MusicXML ilegível: {e}")));
            }
        }
    }

    /// Relê a lista do disco e a partitura selecionada (depois de editar no MuseScore).
    pub fn recarregar_selecionada(&mut self, som: &mut ServicoSom) {
        self.partitura.parar(som);
        let id = self.item_selecionado().map(|i| i.id.clone());
        if let Some(o) = self.oficinas.get(self.oficina_atual) {
            self.itens = o.partituras();
        }
        self.selecionado = id.and_then(|id| self.itens.iter().position(|i| i.id == id));
        self.limpar_arquivos_da_selecao();
        self.carregar_arquivos_do_item();
        if self.partitura_valida {
            self.aviso = Some(Aviso::ok("Partitura recarregada do disco."));
        }
    }

    /// Compara a data do MusicXML no disco com a de quando foi carregado.
    pub fn verificar_mudanca(&mut self) {
        let Some(item) = self.item_selecionado() else {
            self.arquivo_mudou = false;
            return;
        };
        self.arquivo_mudou = item.mtime_do_musicxml() != self.mtime_carregado;
    }

    /// Chamado a cada quadro com a tela aberta: confere o disco a cada segundo.
    fn conferir_disco_de_tempos_em_tempos(&mut self) {
        let agora = Instant::now();
        if self
            .ultima_conferencia
            .is_none_or(|t| agora.duration_since(t) >= INTERVALO_DA_CONFERENCIA)
        {
            self.ultima_conferencia = Some(agora);
            self.verificar_mudanca();
        }
    }

    /// Relê só a lista (estados), mantendo a seleção, sem mexer na partitura carregada.
    fn reler_estados(&mut self) {
        let id = self.item_selecionado().map(|i| i.id.clone());
        if let Some(o) = self.oficinas.get(self.oficina_atual) {
            self.itens = o.partituras();
        }
        self.selecionado = id.and_then(|id| self.itens.iter().position(|i| i.id == id));
    }

    /// Muda o estado da partitura selecionada (a regra de transição é a de [`crate::revisao`]).
    pub fn mudar_estado_da_selecionada(&mut self, para: Estado, nota: Option<&str>) {
        let Some(item) = self.item_selecionado().cloned() else {
            return;
        };
        if let Some(p) = &item.problema {
            self.aviso = Some(Aviso::erro(format!("{}: {p}", item.id)));
            return;
        }
        match item.mudar_estado(para, nota, &(self.hoje)()) {
            Ok(()) => {
                self.aviso = Some(Aviso::ok(format!(
                    "{}: agora {}.",
                    item.titulo,
                    rotulo_do_estado(para)
                )));
                self.rejeitando = false;
                self.nota_da_rejeicao.clear();
                self.reler_estados();
            }
            Err(e) => self.aviso = Some(Aviso::erro(e.to_string())),
        }
    }

    /// Rejeita a selecionada com a nota digitada (opcional).
    pub fn confirmar_rejeicao(&mut self) {
        let nota = std::mem::take(&mut self.nota_da_rejeicao);
        self.mudar_estado_da_selecionada(Estado::Rejeitada, Some(&nota));
    }

    /// Abre o MusicXML da selecionada no MuseScore, sem travar a tela.
    pub fn abrir_no_musescore(&mut self) {
        let Some(item) = self.item_selecionado() else {
            return;
        };
        if !item.musicxml_existe() {
            self.aviso = Some(Aviso::erro("Esta partitura ainda não tem MusicXML."));
            return;
        }
        let arquivo = item.musicxml.clone();
        self.aviso = Some(match self.lancador.abrir_no_editor(&arquivo) {
            Ok(()) => Aviso::ok(
                "Aberto no MuseScore. Depois de corrigir, exporte o MusicXML sobre o mesmo \
                 arquivo e clique em Recarregar.",
            ),
            Err(e) => Aviso::erro(e),
        });
    }

    pub fn ouvir_partitura(&mut self, som: &mut ServicoSom) {
        if self.partitura_valida {
            self.partitura.tocar(som);
        }
    }

    pub fn parar_partitura(&mut self, som: &mut ServicoSom) {
        self.partitura.parar(som);
    }

    pub fn definir_bpm(&mut self, bpm: f32) {
        self.partitura.definir_bpm(bpm);
    }

    /// Para a partitura e a voz (ao sair da tela).
    pub fn ao_sair(&mut self, som: &mut ServicoSom) {
        self.partitura.parar(som);
        if self.fala_em_voz.take().is_some()
            && let Some(v) = self.voz.as_mut()
        {
            v.parar();
        }
    }

    // --- Roteiro ---

    pub fn fonte(&self) -> Option<&FonteDeRoteiro> {
        self.fontes.get(self.fonte_atual)
    }

    pub fn trocar_fonte(&mut self, i: usize) {
        if i < self.fontes.len() {
            self.fonte_atual = i;
            self.licao_filtrada = None;
            self.carregar_roteiro();
        }
    }

    /// Relê o Roteiro da fonte atual (descarta as edições não salvas).
    pub fn carregar_roteiro(&mut self) {
        self.edicoes.clear();
        self.roteiro = None;
        let Some(f) = self.fontes.get(self.fonte_atual) else {
            return;
        };
        match f.carregar() {
            Ok(r) => self.roteiro = Some(r),
            Err(e) => {
                self.aviso = Some(Aviso::erro(format!("{}: {e}", f.rotulo())));
            }
        }
    }

    /// Lições que aparecem no Roteiro, na ordem em que surgem.
    pub fn licoes_do_roteiro(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for f in self.roteiro.iter().flat_map(|r| &r.falas) {
            if !v.contains(&f.licao) {
                v.push(f.licao.clone());
            }
        }
        v
    }

    /// Falas que passam pelos filtros de Lição e de estado, na ordem do arquivo.
    pub fn falas_filtradas(&self) -> Vec<Fala> {
        self.roteiro
            .iter()
            .flat_map(|r| &r.falas)
            .filter(|f| {
                self.licao_filtrada.as_ref().is_none_or(|l| *l == f.licao)
                    && self.filtro_de_fala.is_none_or(|e| e == f.estado)
            })
            .cloned()
            .collect()
    }

    /// Texto em edição de uma fala (o do arquivo, se ninguém mexeu).
    pub fn texto_em_edicao(&self, id: &str) -> Option<String> {
        self.edicoes.get(id).cloned().or_else(|| {
            self.roteiro
                .as_ref()?
                .falas
                .iter()
                .find(|f| f.id == id)
                .map(|f| f.texto.clone())
        })
    }

    pub fn editar_texto(&mut self, id: &str, texto: &str) {
        self.edicoes.insert(id.to_owned(), texto.to_owned());
    }

    /// A fala tem texto digitado ainda não salvo.
    pub fn tem_edicao_pendente(&self, id: &str) -> bool {
        match (
            self.edicoes.get(id),
            self.roteiro
                .as_ref()
                .and_then(|r| r.falas.iter().find(|f| f.id == id)),
        ) {
            (Some(e), Some(f)) => e.trim() != f.texto.trim(),
            _ => false,
        }
    }

    pub fn desfazer_edicao(&mut self, id: &str) {
        self.edicoes.remove(id);
    }

    fn aplicar_na_fala(&mut self, id: &str, mudanca: &MudancaDeFala, mensagem: &str) {
        let Some(f) = self.fontes.get(self.fonte_atual) else {
            return;
        };
        match f.aplicar(id, mudanca) {
            Ok(novo) => {
                self.edicoes.remove(id);
                let edicoes = std::mem::take(&mut self.edicoes);
                self.carregar_roteiro();
                self.edicoes = edicoes;
                self.aviso = Some(Aviso::ok(format!(
                    "{id}: {mensagem} (agora {}).",
                    rotulo_do_estado(novo)
                )));
            }
            Err(e) => self.aviso = Some(Aviso::erro(e.to_string())),
        }
    }

    /// Muda o estado de uma fala (Em revisão, Aprovar, Rejeitar) e grava no arquivo.
    pub fn mudar_estado_da_fala(&mut self, id: &str, para: Estado) {
        if self.tem_edicao_pendente(id) {
            self.aviso = Some(Aviso::erro(
                "Salve o texto editado antes de mudar o estado da fala.",
            ));
            return;
        }
        self.aplicar_na_fala(
            id,
            &MudancaDeFala {
                estado: Some(para),
                texto: None,
            },
            "estado gravado",
        );
    }

    /// Grava o texto editado (uma fala Aprovada volta a Em revisão).
    pub fn salvar_texto_da_fala(&mut self, id: &str) {
        let Some(texto) = self.edicoes.get(id).cloned() else {
            return;
        };
        self.aplicar_na_fala(
            id,
            &MudancaDeFala {
                estado: None,
                texto: Some(texto),
            },
            "texto salvo",
        );
    }

    fn voz_mut(&mut self) -> &mut Box<dyn Voz> {
        let dados = self.dados.clone();
        self.voz.get_or_insert_with(|| match dados {
            Some(d) => voz_do_sistema(&d),
            None => Box::new(VozMuda::nova()),
        })
    }

    /// Diz o texto de uma fala (o que está na caixa, mesmo sem salvar).
    pub fn ouvir_fala(&mut self, id: &str) {
        let Some(texto) = self.texto_em_edicao(id) else {
            return;
        };
        let v = self.voz_mut();
        let r = v.falar(&texto);
        let estado = v.estado();
        self.fala_em_voz = Some(id.to_owned());
        self.aviso = match (r, estado) {
            (Err(e), _) => Some(Aviso::erro(e.to_string())),
            (Ok(()), crate::professor_falante::EstadoVoz::Indisponivel(m)) => Some(Aviso::erro(m)),
            _ => None,
        };
    }

    pub fn parar_voz(&mut self) {
        if let Some(v) = self.voz.as_mut() {
            v.parar();
        }
        self.fala_em_voz = None;
    }

    /// A voz está dizendo a fala `id`.
    pub fn falando(&self, id: &str) -> bool {
        self.fala_em_voz.as_deref() == Some(id) && self.voz.as_ref().is_some_and(|v| v.falando())
    }

    fn colher_erro_da_voz(&mut self) {
        if let Some(e) = self.voz.as_mut().and_then(|v| v.tomar_erro()) {
            self.aviso = Some(Aviso::erro(e.to_string()));
        }
    }

    // --- Página do PDF ---

    /// Decodifica a página (PNG) da selecionada e cria a textura, se ainda não há.
    fn garantir_pagina(&mut self, ctx: &egui::Context) {
        let Some(item) = self.item_selecionado() else {
            return;
        };
        let Some(caminho) = item.pagina_png.clone() else {
            return;
        };
        let mtime = std::fs::metadata(&caminho).and_then(|m| m.modified()).ok();
        let chave = (caminho.clone(), mtime);
        if self.pagina.chave.as_ref() == Some(&chave) {
            return;
        }
        self.pagina = Pagina {
            chave: Some(chave),
            ..Pagina::default()
        };
        match ler_png(&caminho) {
            Ok(ImagemRgba {
                largura,
                altura,
                rgba,
            }) => {
                let img = ColorImage::from_rgba_unmultiplied([largura, altura], &rgba);
                self.pagina.textura = Some(ctx.load_texture(
                    format!("pagina-{}", caminho.display()),
                    img,
                    TextureOptions::LINEAR,
                ));
                self.pagina.tamanho = [largura, altura];
            }
            Err(e) => self.pagina.erro = Some(e),
        }
    }

    /// Tamanho da página decodificada (para testes e para o zoom automático).
    pub fn tamanho_da_pagina(&self) -> Option<[usize; 2]> {
        self.pagina.textura.as_ref().map(|_| self.pagina.tamanho)
    }

    pub fn erro_da_pagina(&self) -> Option<&str> {
        self.pagina.erro.as_deref()
    }
}

// --- Desenho --------------------------------------------------------------------------------

const AMARELO: Color32 = Color32::from_rgb(230, 190, 70);
const AZUL: Color32 = Color32::from_rgb(110, 170, 235);
const VERDE: Color32 = Color32::from_rgb(70, 200, 110);
const VERMELHO: Color32 = Color32::from_rgb(235, 100, 100);

fn cor_do_estado(e: Estado) -> Color32 {
    match e {
        Estado::Hipotese => AMARELO,
        Estado::EmRevisao => AZUL,
        Estado::Aprovada => VERDE,
        Estado::Rejeitada => VERMELHO,
    }
}

/// Símbolo e nome do estado, com a cor: nunca só a cor.
fn texto_do_estado(e: Estado) -> RichText {
    RichText::new(format!("{} {}", simbolo_do_estado(e), rotulo_do_estado(e)))
        .color(cor_do_estado(e))
}

fn desenhar_aviso(ui: &mut Ui, a: &Aviso) {
    let cor = if a.erro { VERMELHO } else { VERDE };
    ui.label(RichText::new(&a.texto).color(cor));
}

/// Desenha a tela inteira.
pub fn desenhar_tela(ui: &mut Ui, e: &mut EstadoRevisao, som: &mut ServicoSom) {
    e.garantir_iniciado();
    e.colher_erro_da_voz();
    if e.partitura_valida {
        e.partitura.atualizar(som);
    }
    ui.horizontal_wrapped(|ui| {
        ui.heading("Revisão do tutor");
        Frame::group(ui.style())
            .inner_margin(Margin::symmetric(8, 2))
            .show(ui, |ui| {
                ui.label(RichText::new(AVISO_DO_MODO_TUTOR).color(AMARELO).strong());
            });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut e.aba, AbaRevisao::Partituras, "Partituras");
        ui.selectable_value(&mut e.aba, AbaRevisao::Roteiro, "Roteiro");
        ui.separator();
        desenhar_oficinas(ui, e);
    });
    ui.add_space(2.0);
    match e.aba {
        AbaRevisao::Partituras => desenhar_aba_partituras(ui, e, som),
        AbaRevisao::Roteiro => desenhar_aba_roteiro(ui, e),
    }
}

fn desenhar_oficinas(ui: &mut Ui, e: &mut EstadoRevisao) {
    if e.oficinas.is_empty() {
        ui.label(RichText::new("Nenhuma oficina de tutor encontrada.").weak());
    } else {
        let atual = e.oficinas[e.oficina_atual].tutor.clone();
        let mut escolha = None;
        ComboBox::from_id_salt("rev-oficina")
            .selected_text(format!("Oficina: {atual}"))
            .show_ui(ui, |ui| {
                for (i, o) in e.oficinas.iter().enumerate() {
                    if ui
                        .selectable_label(i == e.oficina_atual, &o.tutor)
                        .clicked()
                    {
                        escolha = Some(i);
                    }
                }
            });
        if let Some(i) = escolha {
            e.trocar_oficina(i);
        }
    }
    if ui
        .button("Reler pastas")
        .on_hover_text("Procura de novo as oficinas, as partituras e os roteiros no disco")
        .clicked()
    {
        e.recarregar_oficinas();
    }
    ui.separator();
    let r = ui.add(
        TextEdit::singleline(&mut e.campo_extra)
            .hint_text("/outra/oficina")
            .desired_width(180.0),
    );
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if (ui.button("Abrir outra oficina").clicked() || enter) && !e.campo_extra.trim().is_empty() {
        let c = PathBuf::from(e.campo_extra.trim());
        e.definir_oficina_extra(&c);
    }
}

// --- Aba Partituras ---

fn desenhar_aba_partituras(ui: &mut Ui, e: &mut EstadoRevisao, som: &mut ServicoSom) {
    if e.partitura_valida {
        e.conferir_disco_de_tempos_em_tempos();
        ui.ctx().request_repaint_after(INTERVALO_DA_CONFERENCIA);
    }
    Panel::left("rev-lista")
        .resizable(true)
        .default_size(250.0)
        .size_range(170.0..=480.0)
        .show(ui, |ui| desenhar_lista(ui, e, som));
    // Um painel inferior não serve aqui (num `Ui` de cima para baixo o conteúdo seguinte o
    // cobriria): a altura é repartida à mão entre o corpo e as divergências.
    let total = ui.available_height();
    let largura = ui.available_width();
    let altura_das_divergencias = (total * 0.24).clamp(90.0, 220.0).min(total * 0.5);
    let altura_do_corpo = (total - altura_das_divergencias - 8.0).max(60.0);
    ui.allocate_ui(vec2(largura, altura_do_corpo), |ui| {
        ui.set_min_size(vec2(largura, altura_do_corpo));
        desenhar_corpo(ui, e, som);
    });
    ui.separator();
    ui.allocate_ui(vec2(largura, altura_das_divergencias - 8.0), |ui| {
        desenhar_divergencias(ui, e);
    });
}

fn desenhar_lista(ui: &mut Ui, e: &mut EstadoRevisao, som: &mut ServicoSom) {
    ui.label(RichText::new("Partituras").strong());
    ui.horizontal_wrapped(|ui| {
        ui.label("Estado:");
        let rotulo = e
            .filtro_de_partitura
            .map_or("Todos", |s| rotulo_do_estado(s));
        ComboBox::from_id_salt("rev-filtro-partitura")
            .selected_text(rotulo)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut e.filtro_de_partitura, None, "Todos");
                for s in ESTADOS {
                    ui.selectable_value(&mut e.filtro_de_partitura, Some(s), rotulo_do_estado(s));
                }
            });
    });
    ui.separator();
    if e.itens.is_empty() {
        ui.label(RichText::new("Nenhuma partitura na oficina.").weak());
        return;
    }
    let mut clicada = None;
    ScrollArea::vertical()
        .id_salt("rev-lista-itens")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, item) in e.itens.iter().enumerate() {
                if e.filtro_de_partitura.is_some_and(|f| f != item.estado) {
                    continue;
                }
                let selecionado = e.selecionado == Some(i);
                let mut linha = item.titulo.clone();
                if let Some(a) = item.aula {
                    linha.push_str(&format!("  ·  aula {a}"));
                }
                let resposta = ui.selectable_label(selecionado, RichText::new(linha).strong());
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(texto_do_estado(item.estado));
                    if let Some(c) = item.confianca {
                        ui.label(RichText::new(format!("confiança {:.0}%", c * 100.0)).weak());
                    }
                    if item.problema.is_some() {
                        ui.label(RichText::new("[!] com problema").color(VERMELHO));
                    }
                });
                ui.add_space(2.0);
                if resposta.clicked() {
                    clicada = Some(i);
                }
            }
        });
    if let Some(i) = clicada {
        e.selecionar(i, som);
    }
}

fn desenhar_divergencias(ui: &mut Ui, e: &EstadoRevisao) {
    ui.label(RichText::new("Divergências e dúvidas").strong());
    ScrollArea::vertical()
        .id_salt("rev-divergencias")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(item) = e.item_selecionado() {
                if !item.proveniencia.is_empty() {
                    ui.label(RichText::new(format!("Proveniência: {}", item.proveniencia)).weak());
                }
                if let Some(pdf) = &item.pdf {
                    ui.label(RichText::new(format!("PDF de origem: {pdf}")).weak());
                }
                for h in &item.historico {
                    let nota = h
                        .nota
                        .as_deref()
                        .map(|n| format!(" — {n}"))
                        .unwrap_or_default();
                    ui.label(
                        RichText::new(format!(
                            "{}: {} → {} ({}){nota}",
                            h.em, h.de, h.para, h.revisor
                        ))
                        .weak(),
                    );
                }
            }
            match e.divergencias() {
                Some(t) => {
                    ui.add(egui::Label::new(t).wrap());
                }
                None => {
                    ui.label(RichText::new("Sem arquivo de divergências.").weak());
                }
            }
        });
}

fn desenhar_corpo(ui: &mut Ui, e: &mut EstadoRevisao, som: &mut ServicoSom) {
    let Some(item) = e.item_selecionado().cloned() else {
        ui.add_space(12.0);
        ui.label("Escolha uma partitura na lista para conferir.");
        if let Some(a) = &e.aviso {
            desenhar_aviso(ui, a);
        }
        return;
    };
    // Título e estado.
    ui.horizontal_wrapped(|ui| {
        ui.heading(&item.titulo);
        ui.label(texto_do_estado(item.estado));
        if let Some(a) = item.aula {
            ui.label(RichText::new(format!("aula {a}")).weak());
        }
        if let Some(c) = item.confianca {
            ui.label(RichText::new(format!("confiança {:.0}%", c * 100.0)).weak());
        }
    });
    desenhar_botoes_de_estado(ui, e, &item, som);
    if e.rejeitando {
        ui.horizontal_wrapped(|ui| {
            ui.label("Motivo (opcional):");
            ui.add(
                TextEdit::singleline(&mut e.nota_da_rejeicao)
                    .hint_text("ex.: notas erradas no compasso 3")
                    .desired_width(320.0),
            );
            if ui.button("Confirmar rejeição").clicked() {
                e.confirmar_rejeicao();
            }
            if ui.button("Cancelar").clicked() {
                e.rejeitando = false;
                e.nota_da_rejeicao.clear();
            }
        });
    }
    if e.arquivo_mudou {
        ui.label(
            RichText::new(
                "O MusicXML mudou no disco (editado no MuseScore?). Clique em Recarregar para ver a versão nova.",
            )
            .color(AMARELO),
        );
    }
    if let Some(a) = &e.aviso {
        desenhar_aviso(ui, a);
    }
    ui.add_space(4.0);
    e.garantir_pagina(ui.ctx());
    ui.columns(2, |colunas| {
        desenhar_coluna_da_pagina(&mut colunas[0], e);
        desenhar_coluna_da_partitura(&mut colunas[1], e, som);
    });
}

fn desenhar_botoes_de_estado(
    ui: &mut Ui,
    e: &mut EstadoRevisao,
    item: &ItemPartitura,
    som: &mut ServicoSom,
) {
    let livre = item.problema.is_none();
    ui.horizontal_wrapped(|ui| {
        let botao = |ui: &mut Ui, rotulo: &str, para: Estado, dica: &str| -> bool {
            let ok = livre && transicao_valida(item.estado, para);
            ui.add_enabled(ok, Button::new(rotulo))
                .on_hover_text(dica)
                .on_disabled_hover_text(format!(
                    "Não dá para ir de {} para {} (o fluxo é Hipótese → Em revisão → Aprovada \
                     ou Rejeitada).",
                    rotulo_do_estado(item.estado),
                    rotulo_do_estado(para)
                ))
                .clicked()
        };
        if botao(
            ui,
            "Em revisão",
            Estado::EmRevisao,
            "Começo a conferir esta partitura (ou volto atrás numa decisão)",
        ) {
            e.mudar_estado_da_selecionada(Estado::EmRevisao, None);
        }
        if botao(
            ui,
            "Aprovar",
            Estado::Aprovada,
            "Conferi: pode virar Lição e Gabarito. Fica registrado no histórico.",
        ) {
            e.mudar_estado_da_selecionada(Estado::Aprovada, None);
        }
        if botao(
            ui,
            "Rejeitar",
            Estado::Rejeitada,
            "Descarta esta transcrição (fica registrada, nunca é usada)",
        ) {
            e.rejeitando = true;
        }
        ui.separator();
        if ui
            .add_enabled(item.musicxml_existe(), Button::new("Abrir no MuseScore"))
            .on_hover_text("Abre o MusicXML no MuseScore para corrigir")
            .clicked()
        {
            e.abrir_no_musescore();
        }
        let recarregar = if e.arquivo_mudou {
            Button::new(RichText::new("Recarregar").strong()).fill(AMARELO.gamma_multiply(0.35))
        } else {
            Button::new("Recarregar")
        };
        if ui
            .add(recarregar)
            .on_hover_text("Relê o MusicXML, a lista e as divergências do disco")
            .clicked()
        {
            e.recarregar_selecionada(som);
        }
    });
}

fn desenhar_coluna_da_pagina(ui: &mut Ui, e: &mut EstadoRevisao) {
    ui.label(RichText::new("Página do PDF").strong());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().slider_width = 90.0;
        let largura = (ui.available_width() - 16.0).max(100.0);
        let [w, _] = e.pagina.tamanho;
        let zoom_auto = if w > 0 {
            (largura / w as f32).clamp(ZOOM_PAGINA_MIN, ZOOM_PAGINA_MAX)
        } else {
            1.0
        };
        let mut z = if e.ajustar_pagina {
            zoom_auto
        } else {
            e.zoom_da_pagina
        };
        let r = ui.add(
            Slider::new(&mut z, ZOOM_PAGINA_MIN..=ZOOM_PAGINA_MAX)
                .logarithmic(true)
                .text("zoom")
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
        );
        if r.changed() {
            e.zoom_da_pagina = z;
            e.ajustar_pagina = false;
        }
        if ui.small_button("Ajustar").clicked() {
            e.ajustar_pagina = true;
        }
        if ui.small_button("100%").clicked() {
            e.zoom_da_pagina = 1.0;
            e.ajustar_pagina = false;
        }
    });
    let Some(textura) = e.pagina.textura.clone() else {
        let msg = match (
            &e.pagina.erro,
            e.item_selecionado().and_then(|i| i.pagina_png.as_ref()),
        ) {
            (Some(erro), _) => format!("Não consegui ler a página: {erro}"),
            (None, Some(c)) => format!("Página não encontrada: {}", c.display()),
            (None, None) => "Esta partitura não tem página de PDF.".to_owned(),
        };
        ui.label(RichText::new(msg).weak());
        return;
    };
    let largura = (ui.available_width() - 16.0).max(100.0);
    let [w, h] = e.pagina.tamanho;
    let zoom = if e.ajustar_pagina {
        (largura / w.max(1) as f32).clamp(ZOOM_PAGINA_MIN, ZOOM_PAGINA_MAX)
    } else {
        e.zoom_da_pagina.clamp(ZOOM_PAGINA_MIN, ZOOM_PAGINA_MAX)
    };
    let area = ScrollArea::both()
        .id_salt("rev-pagina")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let tam = vec2(w as f32 * zoom, h as f32 * zoom);
            ui.add(Image::new(SizedTexture::new(textura.id(), tam)));
        });
    // Ctrl + roda do mouse também dá zoom.
    if ui.rect_contains_pointer(area.inner_rect) {
        let dz = ui.input(|i| i.zoom_delta());
        if (dz - 1.0).abs() > f32::EPSILON {
            e.zoom_da_pagina = (zoom * dz).clamp(ZOOM_PAGINA_MIN, ZOOM_PAGINA_MAX);
            e.ajustar_pagina = false;
        }
    }
}

fn desenhar_coluna_da_partitura(ui: &mut Ui, e: &mut EstadoRevisao, som: &mut ServicoSom) {
    ui.label(RichText::new("Partitura renderizada (MusicXML)").strong());
    if !e.partitura_valida {
        ui.label(RichText::new("Sem MusicXML legível para mostrar.").weak());
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().slider_width = 90.0;
        if e.tocando() {
            if ui.button("Parar").clicked() {
                e.parar_partitura(som);
            }
        } else if ui
            .button("Ouvir")
            .on_hover_text("Toca a partitura no som do app, no andamento ao lado")
            .clicked()
        {
            e.ouvir_partitura(som);
        }
        let mut bpm = e.bpm();
        if ui
            .add(
                Slider::new(&mut bpm, BPM_MIN..=BPM_MAX)
                    .text("semínima/min")
                    .integer(),
            )
            .changed()
        {
            e.definir_bpm(bpm);
        }
        ui.label(format!("zoom {:.0}%", e.partitura.zoom * 100.0));
        let (min, max) = (
            crate::partitura_visual::ZOOM_MIN,
            crate::partitura_visual::ZOOM_MAX,
        );
        if ui
            .small_button("−")
            .on_hover_text("Diminui a partitura")
            .clicked()
        {
            e.partitura.zoom = (e.partitura.zoom / 1.25).clamp(min, max);
        }
        if ui
            .small_button("+")
            .on_hover_text("Aumenta a partitura")
            .clicked()
        {
            e.partitura.zoom = (e.partitura.zoom * 1.25).clamp(min, max);
        }
    });
    if let Some(a) = &e.partitura.aviso {
        ui.label(RichText::new(a).weak());
    }
    desenhar_vista(ui, &mut e.partitura, som);
}

// --- Aba Roteiro ---

/// O que o usuário pediu numa fala (aplicado depois do desenho, sem emprestar `e` duas vezes).
enum AcaoDaFala {
    Ouvir(String),
    Parar,
    Estado(String, Estado),
    Salvar(String),
    Desfazer(String),
}

fn rotulo_do_momento(f: &Fala) -> String {
    match f.momento {
        Momento::Antes => "antes".to_owned(),
        Momento::Passo => format!(
            "passo {}",
            f.passo.map_or_else(|| "?".to_owned(), |p| p.to_string())
        ),
        Momento::Acerto => "acerto".to_owned(),
        Momento::Erro => "erro".to_owned(),
        Momento::Depois => "depois".to_owned(),
    }
}

fn desenhar_aba_roteiro(ui: &mut Ui, e: &mut EstadoRevisao) {
    if e.fontes.is_empty() {
        ui.label("Nenhum roteiro para revisar (sem pasta de dados nem oficina com roteiro.toml).");
        return;
    }
    ui.horizontal_wrapped(|ui| {
        let mut trocar = None;
        ComboBox::from_id_salt("rev-fonte-roteiro")
            .selected_text(e.fonte().map(FonteDeRoteiro::rotulo).unwrap_or_default())
            .width(260.0)
            .show_ui(ui, |ui| {
                for (i, f) in e.fontes.iter().enumerate() {
                    if ui
                        .selectable_label(i == e.fonte_atual, f.rotulo())
                        .clicked()
                    {
                        trocar = Some(i);
                    }
                }
            });
        if let Some(i) = trocar {
            e.trocar_fonte(i);
        }
        ui.label("Lição:");
        let licoes = e.licoes_do_roteiro();
        ComboBox::from_id_salt("rev-licao")
            .selected_text(
                e.licao_filtrada
                    .clone()
                    .unwrap_or_else(|| "Todas".to_owned()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut e.licao_filtrada, None, "Todas");
                for l in licoes {
                    ui.selectable_value(&mut e.licao_filtrada, Some(l.clone()), l);
                }
            });
        ui.label("Estado:");
        ComboBox::from_id_salt("rev-estado-fala")
            .selected_text(e.filtro_de_fala.map_or("Todos", |s| rotulo_do_estado(s)))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut e.filtro_de_fala, None, "Todos");
                for s in ESTADOS {
                    ui.selectable_value(&mut e.filtro_de_fala, Some(s), rotulo_do_estado(s));
                }
            });
        if ui
            .button("Recarregar")
            .on_hover_text("Relê o roteiro do disco (descarta o texto não salvo)")
            .clicked()
        {
            e.carregar_roteiro();
        }
    });
    if let Some(r) = &e.roteiro {
        let n = contar_por_estado(&r.falas);
        let partes: Vec<String> = ESTADOS
            .iter()
            .zip(n)
            .map(|(s, n)| format!("{} {}: {n}", simbolo_do_estado(*s), rotulo_do_estado(*s)))
            .collect();
        ui.label(RichText::new(partes.join("   ·   ")).weak());
    }
    if let Some(a) = &e.aviso {
        desenhar_aviso(ui, a);
    }
    ui.separator();
    let falas = e.falas_filtradas();
    if falas.is_empty() {
        ui.label(RichText::new("Nenhuma fala com este filtro.").weak());
        return;
    }
    let mut acao = None;
    ScrollArea::vertical()
        .id_salt("rev-falas")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for f in &falas {
                if let Some(a) = desenhar_fala(ui, e, f) {
                    acao = Some(a);
                }
                ui.add_space(4.0);
            }
        });
    match acao {
        Some(AcaoDaFala::Ouvir(id)) => e.ouvir_fala(&id),
        Some(AcaoDaFala::Parar) => e.parar_voz(),
        Some(AcaoDaFala::Estado(id, s)) => e.mudar_estado_da_fala(&id, s),
        Some(AcaoDaFala::Salvar(id)) => e.salvar_texto_da_fala(&id),
        Some(AcaoDaFala::Desfazer(id)) => e.desfazer_edicao(&id),
        None => {}
    }
    if e.fala_em_voz.is_some() {
        ui.ctx().request_repaint_after(Duration::from_millis(200));
    }
}

fn desenhar_fala(ui: &mut Ui, e: &mut EstadoRevisao, f: &Fala) -> Option<AcaoDaFala> {
    let mut acao = None;
    let pendente = e.tem_edicao_pendente(&f.id);
    let falando = e.falando(&f.id);
    Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(texto_do_estado(f.estado));
            ui.label(RichText::new(&f.id).monospace().weak());
            ui.label(format!("lição {}", f.licao));
            if let Some(x) = &f.exercicio {
                ui.label(format!("exercício {x}"));
            }
            ui.label(rotulo_do_momento(f));
            match f.origem {
                Some(o) => {
                    ui.label(
                        RichText::new(format!(
                            "origem: aula {}, {:.0}–{:.0} s",
                            o.aula, o.inicio_s, o.fim_s
                        ))
                        .weak(),
                    );
                }
                None => {
                    ui.label(RichText::new("origem: texto de fábrica").weak());
                }
            }
        });
        let buf = e
            .edicoes
            .entry(f.id.clone())
            .or_insert_with(|| f.texto.clone());
        ui.add(
            TextEdit::multiline(buf)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .id_salt(("rev-texto", &f.id)),
        );
        ui.horizontal_wrapped(|ui| {
            if falando {
                if ui.button("Parar").clicked() {
                    acao = Some(AcaoDaFala::Parar);
                }
            } else if ui
                .button("Ouvir")
                .on_hover_text("Diz o texto desta caixa com a voz do professor (Piper)")
                .clicked()
            {
                acao = Some(AcaoDaFala::Ouvir(f.id.clone()));
            }
            if pendente {
                if ui.button("Salvar texto").clicked() {
                    acao = Some(AcaoDaFala::Salvar(f.id.clone()));
                }
                if ui.button("Desfazer").clicked() {
                    acao = Some(AcaoDaFala::Desfazer(f.id.clone()));
                }
                ui.label(RichText::new("texto alterado, ainda não salvo").color(AMARELO));
            }
            ui.separator();
            for (rotulo, para) in [
                ("Em revisão", Estado::EmRevisao),
                ("Aprovar", Estado::Aprovada),
                ("Rejeitar", Estado::Rejeitada),
            ] {
                let ok = !pendente && transicao_valida(f.estado, para);
                if ui
                    .add_enabled(ok, Button::new(rotulo))
                    .on_disabled_hover_text(if pendente {
                        "Salve ou desfaça o texto antes.".to_owned()
                    } else {
                        format!(
                            "Não dá para ir de {} para {}.",
                            rotulo_do_estado(f.estado),
                            rotulo_do_estado(para)
                        )
                    })
                    .clicked()
                {
                    acao = Some(AcaoDaFala::Estado(f.id.clone(), para));
                }
            }
        });
    });
    acao
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::professor_falante::VozFalsa;
    use crate::revisao::LancadorFalso;
    use crate::som::apoio_testes::servico_pronto;
    use bancada_casca::egui::{Context, Event, Modifiers, RawInput, Rect, pos2};
    use piano_tutor_midi::{Relogio, RelogioManual};
    use std::fs;
    use std::sync::Arc;

    fn hoje_fixo() -> String {
        "2026-11-24".to_owned()
    }

    const ESTADO_TOML: &str = "id = \"cinco-notas\"\naula = 14\ntitulo = \"Cinco Notas no Piano\"\nestado = \"hipotese\"\nconfianca = 0.95\npagina_png = \"paginas/cinco-notas.png\"\n";

    const ROTEIRO: &str = r#"formato_versao = 1

[[falas]]
id = "a-1"
licao = "s03-a14"
momento = "antes"
texto = "Esta música usa só cinco notas."
estado = "hipotese"

[[falas]]
id = "a-2"
licao = "s03-a15"
momento = "passo"
passo = 4
texto = "Abra a mão."
estado = "em_revisao"
"#;

    fn png_sintetico() -> Vec<u8> {
        let mut saida = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut saida, 4, 6);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().expect("cabeçalho");
            w.write_image_data(&[200u8; 4 * 6 * 3]).expect("dados");
        }
        saida
    }

    /// Dados sintéticos com uma oficina (com PNG, MusicXML e roteiro).
    fn dados_sinteticos() -> tempfile::TempDir {
        let d = tempfile::tempdir().expect("tempdir");
        let raiz = d.path().join("fontes/tutor-teste/oficina");
        fs::create_dir_all(raiz.join("partituras")).expect("p");
        fs::create_dir_all(raiz.join("paginas")).expect("g");
        fs::write(raiz.join("partituras/cinco-notas.estado.toml"), ESTADO_TOML).expect("e");
        fs::write(
            raiz.join("partituras/cinco-notas.musicxml"),
            include_str!(
                "../../../assets/partituras/tradicional-brilha-brilha-estrelinha.musicxml"
            ),
        )
        .expect("x");
        fs::write(
            raiz.join("partituras/cinco-notas.divergencias.md"),
            "# Dúvidas\n- uma dúvida\n",
        )
        .expect("d");
        fs::write(raiz.join("paginas/cinco-notas.png"), png_sintetico()).expect("png");
        fs::write(raiz.join("roteiro.toml"), ROTEIRO).expect("r");
        d
    }

    fn estado(d: &tempfile::TempDir) -> EstadoRevisao {
        EstadoRevisao::novo(Some(d.path().to_path_buf()))
            .com_lancador(Box::new(LancadorFalso::novo()))
            .com_voz(Box::new(VozFalsa::nova()))
            .com_data(hoje_fixo)
    }

    fn som() -> ServicoSom {
        let relogio: Arc<dyn Relogio> = Arc::new(RelogioManual::novo());
        servico_pronto(relogio).0
    }

    #[test]
    fn descobre_lista_seleciona_e_carrega_partitura_divergencias_e_mtime() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        let mut s = som();
        e.recarregar_oficinas();
        assert_eq!(e.oficinas.len(), 1);
        assert_eq!(e.itens.len(), 1);
        assert_eq!(e.itens[0].estado, Estado::Hipotese);
        assert!(!e.partitura_pronta());

        e.selecionar(0, &mut s);
        assert!(e.partitura_pronta(), "{:?}", e.aviso);
        assert!(e.divergencias().expect("div").contains("uma dúvida"));
        assert!(!e.arquivo_mudou);
        assert_eq!(e.item_selecionado().expect("sel").id, "cinco-notas");
    }

    #[test]
    fn aprovar_pela_tela_so_passa_por_em_revisao_e_grava_historico() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        let mut s = som();
        e.recarregar_oficinas();
        e.selecionar(0, &mut s);

        // Direto de Hipótese a Aprovada: recusado, nada muda.
        e.mudar_estado_da_selecionada(Estado::Aprovada, None);
        assert!(e.aviso.as_ref().expect("aviso").erro);
        assert_eq!(e.itens[0].estado, Estado::Hipotese);

        e.mudar_estado_da_selecionada(Estado::EmRevisao, None);
        assert_eq!(e.itens[0].estado, Estado::EmRevisao);
        e.mudar_estado_da_selecionada(Estado::Aprovada, Some("conferido"));
        assert_eq!(e.itens[0].estado, Estado::Aprovada);
        assert_eq!(e.item_selecionado().expect("sel").id, "cinco-notas");
        assert_eq!(e.itens[0].historico.len(), 2);
        assert_eq!(e.itens[0].historico[1].em, "2026-11-24");
        assert_eq!(e.itens[0].historico[1].revisor, "Chico");
        assert!(!e.aviso.as_ref().expect("aviso").erro);
        // No disco também.
        let t = fs::read_to_string(
            d.path()
                .join("fontes/tutor-teste/oficina/partituras/cinco-notas.estado.toml"),
        )
        .expect("ler");
        assert!(t.contains("estado = \"aprovada\""), "{t}");
    }

    #[test]
    fn rejeitar_pede_nota_opcional_e_registra() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        let mut s = som();
        e.recarregar_oficinas();
        e.selecionar(0, &mut s);
        e.rejeitando = true;
        e.nota_da_rejeicao = "notas erradas".into();
        e.confirmar_rejeicao();
        assert_eq!(e.itens[0].estado, Estado::Rejeitada);
        assert_eq!(
            e.itens[0].historico[0].nota.as_deref(),
            Some("notas erradas")
        );
        assert!(!e.rejeitando);
        assert!(e.nota_da_rejeicao.is_empty());
    }

    #[test]
    fn abrir_no_musescore_usa_o_lancador_e_mostra_o_erro_de_quem_nao_tem() {
        let d = dados_sinteticos();
        let mut falso = LancadorFalso::novo();
        falso.resposta = Err("sem mscore aqui".into());
        let mut e = estado(&d).com_lancador(Box::new(falso));
        let mut s = som();
        e.recarregar_oficinas();
        e.selecionar(0, &mut s);
        e.abrir_no_musescore();
        let a = e.aviso.clone().expect("aviso");
        assert!(a.erro);
        assert_eq!(a.texto, "sem mscore aqui");

        let mut e = estado(&d);
        e.recarregar_oficinas();
        e.selecionar(0, &mut s);
        e.abrir_no_musescore();
        let a = e.aviso.clone().expect("aviso");
        assert!(!a.erro, "{a:?}");
        assert!(a.texto.contains("MuseScore"));
    }

    #[test]
    fn mudanca_do_musicxml_no_disco_e_detectada_e_recarregar_zera() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        let mut s = som();
        e.recarregar_oficinas();
        e.selecionar(0, &mut s);
        e.verificar_mudanca();
        assert!(!e.arquivo_mudou);

        let xml = d
            .path()
            .join("fontes/tutor-teste/oficina/partituras/cinco-notas.musicxml");
        let texto = fs::read_to_string(&xml).expect("ler");
        fs::write(&xml, texto).expect("regravar");
        // Garante data diferente mesmo em sistemas de arquivos de resolução grossa.
        let nova = std::time::SystemTime::now() + Duration::from_secs(5);
        fs::File::options()
            .write(true)
            .open(&xml)
            .expect("abrir")
            .set_modified(nova)
            .expect("mtime");
        e.verificar_mudanca();
        assert!(e.arquivo_mudou, "deveria perceber a edição");

        e.recarregar_selecionada(&mut s);
        assert!(!e.arquivo_mudou);
        assert!(e.partitura_pronta());
        e.verificar_mudanca();
        assert!(!e.arquivo_mudou);
    }

    #[test]
    fn ouvir_toca_no_motor_e_parar_corta_e_o_andamento_e_ajustavel() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        let mut s = som();
        e.recarregar_oficinas();
        e.selecionar(0, &mut s);
        e.definir_bpm(100.0);
        assert!((e.bpm() - 100.0).abs() < 0.01);
        e.ouvir_partitura(&mut s);
        assert!(e.tocando());
        e.parar_partitura(&mut s);
        assert!(!e.tocando());
        e.ouvir_partitura(&mut s);
        e.ao_sair(&mut s);
        assert!(!e.tocando(), "sair da tela corta o som");
    }

    #[test]
    fn partitura_sem_musicxml_ou_com_problema_nao_carrega_e_avisa() {
        let d = dados_sinteticos();
        let raiz = d.path().join("fontes/tutor-teste/oficina/partituras");
        fs::write(
            raiz.join("sem-xml.estado.toml"),
            "aula = 99\nestado = \"hipotese\"\n",
        )
        .expect("w");
        fs::write(
            raiz.join("xml-ruim.estado.toml"),
            "aula = 98\nestado = \"hipotese\"\n",
        )
        .expect("w");
        fs::write(raiz.join("xml-ruim.musicxml"), "isto não é xml de música").expect("w");
        let mut e = estado(&d);
        let mut s = som();
        e.recarregar_oficinas();
        let i = e
            .itens
            .iter()
            .position(|i| i.id == "sem-xml")
            .expect("sem-xml");
        e.selecionar(i, &mut s);
        assert!(!e.partitura_pronta());
        assert!(e.aviso.as_ref().expect("aviso").erro);
        let i = e
            .itens
            .iter()
            .position(|i| i.id == "xml-ruim")
            .expect("xml-ruim");
        e.selecionar(i, &mut s);
        assert!(!e.partitura_pronta(), "não mostra a partitura anterior");
        assert!(e.aviso.as_ref().expect("aviso").erro);
    }

    // --- Roteiro ---

    #[test]
    fn roteiro_filtra_por_licao_e_estado_e_aprovar_grava_e_rele() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        e.recarregar_oficinas();
        // Fontes: fábrica e oficina.
        assert_eq!(e.fontes.len(), 2);
        let i = e
            .fontes
            .iter()
            .position(|f| matches!(f, FonteDeRoteiro::Oficina { .. }))
            .expect("oficina");
        e.trocar_fonte(i);
        assert_eq!(e.licoes_do_roteiro(), vec!["s03-a14", "s03-a15"]);
        assert_eq!(e.falas_filtradas().len(), 2);
        e.licao_filtrada = Some("s03-a15".into());
        assert_eq!(e.falas_filtradas().len(), 1);
        e.licao_filtrada = None;
        e.filtro_de_fala = Some(Estado::EmRevisao);
        assert_eq!(e.falas_filtradas()[0].id, "a-2");
        e.filtro_de_fala = None;

        e.mudar_estado_da_fala("a-2", Estado::Aprovada);
        assert!(!e.aviso.as_ref().expect("aviso").erro, "{:?}", e.aviso);
        let r = e.roteiro.as_ref().expect("roteiro");
        assert_eq!(r.falas[1].estado, Estado::Aprovada);
        // Relê do disco para provar a gravação.
        e.carregar_roteiro();
        assert_eq!(
            e.roteiro.as_ref().expect("r").falas[1].estado,
            Estado::Aprovada
        );
        // Hipótese não pula para Aprovada.
        e.mudar_estado_da_fala("a-1", Estado::Aprovada);
        assert!(e.aviso.as_ref().expect("aviso").erro);
        assert_eq!(
            e.roteiro.as_ref().expect("r").falas[0].estado,
            Estado::Hipotese
        );
    }

    #[test]
    fn editar_texto_exige_salvar_antes_de_mudar_o_estado_e_aprovada_volta_a_em_revisao() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        e.recarregar_oficinas();
        let i = e
            .fontes
            .iter()
            .position(|f| matches!(f, FonteDeRoteiro::Oficina { .. }))
            .expect("oficina");
        e.trocar_fonte(i);
        e.mudar_estado_da_fala("a-2", Estado::Aprovada);
        e.editar_texto("a-2", "Abra bem a mão.");
        assert!(e.tem_edicao_pendente("a-2"));
        // Estado bloqueado com texto pendente.
        e.mudar_estado_da_fala("a-2", Estado::EmRevisao);
        assert!(e.aviso.as_ref().expect("aviso").erro);
        e.salvar_texto_da_fala("a-2");
        assert!(!e.tem_edicao_pendente("a-2"));
        let f = &e.roteiro.as_ref().expect("r").falas[1];
        assert_eq!(f.texto, "Abra bem a mão.");
        assert_eq!(
            f.estado,
            Estado::EmRevisao,
            "aprovada editada volta a Em revisão"
        );
        // Desfazer descarta o rascunho.
        e.editar_texto("a-2", "outra coisa");
        e.desfazer_edicao("a-2");
        assert_eq!(e.texto_em_edicao("a-2").as_deref(), Some("Abra bem a mão."));
    }

    #[test]
    fn ouvir_fala_pede_a_voz_injetada_com_o_texto_da_caixa() {
        let d = dados_sinteticos();
        let voz = VozFalsa::nova();
        let mut e = estado(&d).com_voz(Box::new(voz.clone()));
        e.recarregar_oficinas();
        let i = e
            .fontes
            .iter()
            .position(|f| matches!(f, FonteDeRoteiro::Oficina { .. }))
            .expect("oficina");
        e.trocar_fonte(i);
        e.editar_texto("a-1", "Texto ainda não salvo.");
        e.ouvir_fala("a-1");
        assert_eq!(voz.pedidos(), vec!["Texto ainda não salvo."]);
        assert!(e.falando("a-1"));
        e.parar_voz();
        assert!(!e.falando("a-1"));
        assert_eq!(voz.vezes_que_parou(), 1);
    }

    #[test]
    fn voz_muda_avisa_como_instalar_mas_nao_quebra() {
        let d = dados_sinteticos();
        let mut e = estado(&d).com_voz(Box::new(VozMuda::nova()));
        e.recarregar_oficinas();
        e.ouvir_fala("fabrica-ex01-antes");
        let a = e.aviso.clone().expect("aviso");
        assert!(a.erro);
        assert!(a.texto.contains("Piper"), "{}", a.texto);
    }

    #[test]
    fn roteiro_de_fabrica_aprovado_pela_tela_vai_para_o_arquivo_sobreposto() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        e.recarregar_oficinas();
        let i = e
            .fontes
            .iter()
            .position(|f| matches!(f, FonteDeRoteiro::Fabrica { .. }))
            .expect("fábrica");
        e.trocar_fonte(i);
        assert!(e.roteiro.as_ref().expect("r").falas.len() >= 40);
        e.mudar_estado_da_fala("fabrica-ex01-antes", Estado::EmRevisao);
        e.mudar_estado_da_fala("fabrica-ex01-antes", Estado::Aprovada);
        assert!(!e.aviso.as_ref().expect("aviso").erro, "{:?}", e.aviso);
        let r = crate::revisao::roteiro_de_fabrica_revisado(d.path()).expect("revisado");
        let f = r
            .falas
            .iter()
            .find(|f| f.id == "fabrica-ex01-antes")
            .expect("fala");
        assert_eq!(f.estado, Estado::Aprovada);
        assert!(d.path().join("roteiro-fabrica-revisao.toml").is_file());
    }

    // --- Página e janela ---

    fn quadro(ctx: &Context, e: &mut EstadoRevisao, s: &mut ServicoSom, largura: f32, altura: f32) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(largura, altura))),
            events: vec![Event::ModifiersChanged(Modifiers::NONE)],
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| desenhar_tela(ui, e, s))
            .drop_without_applying_deltas();
    }

    #[test]
    fn a_tela_desenha_vazia_com_partitura_e_nas_duas_abas_e_decodifica_a_pagina() {
        let d = dados_sinteticos();
        let mut e = estado(&d);
        let mut s = som();
        let ctx = Context::default();
        quadro(&ctx, &mut e, &mut s, 1280.0, 800.0);
        assert_eq!(e.itens.len(), 1, "lê as oficinas no primeiro quadro");
        e.selecionar(0, &mut s);
        for _ in 0..3 {
            quadro(&ctx, &mut e, &mut s, 1280.0, 800.0);
        }
        assert_eq!(
            e.tamanho_da_pagina(),
            Some([4, 6]),
            "{:?}",
            e.erro_da_pagina()
        );
        assert!(e.erro_da_pagina().is_none());

        // Tocando, rejeitando e com o aviso de arquivo mudado.
        e.ouvir_partitura(&mut s);
        e.rejeitando = true;
        e.arquivo_mudou = true;
        quadro(&ctx, &mut e, &mut s, 1280.0, 800.0);
        e.ao_sair(&mut s);

        e.aba = AbaRevisao::Roteiro;
        for f in 0..e.fontes.len() {
            e.trocar_fonte(f);
            quadro(&ctx, &mut e, &mut s, 1280.0, 800.0);
        }
        // Tamanhos pequenos não dão pânico.
        e.aba = AbaRevisao::Partituras;
        quadro(&ctx, &mut e, &mut s, 420.0, 300.0);
    }

    #[test]
    fn sem_pasta_de_dados_a_tela_desenha_e_diz_que_nao_ha_oficina() {
        let mut e = EstadoRevisao::novo(None);
        let mut s = som();
        let ctx = Context::default();
        quadro(&ctx, &mut e, &mut s, 1000.0, 700.0);
        assert!(e.oficinas.is_empty());
        e.aba = AbaRevisao::Roteiro;
        quadro(&ctx, &mut e, &mut s, 1000.0, 700.0);
    }

    #[test]
    fn png_corrompido_vira_mensagem_na_coluna_da_pagina() {
        let d = dados_sinteticos();
        fs::write(
            d.path()
                .join("fontes/tutor-teste/oficina/paginas/cinco-notas.png"),
            b"nao e png",
        )
        .expect("w");
        let mut e = estado(&d);
        let mut s = som();
        let ctx = Context::default();
        quadro(&ctx, &mut e, &mut s, 1280.0, 800.0);
        e.selecionar(0, &mut s);
        quadro(&ctx, &mut e, &mut s, 1280.0, 800.0);
        assert!(e.erro_da_pagina().is_some());
        assert!(e.tamanho_da_pagina().is_none());
    }

    #[test]
    fn oficina_extra_aceita_um_caminho_fora_de_fontes() {
        let d = tempfile::tempdir().expect("tmp");
        let fora = dados_sinteticos();
        let mut e = estado(&d);
        e.definir_oficina_extra(&fora.path().join("fontes/tutor-teste/oficina"));
        assert_eq!(e.oficinas.len(), 1);
        assert_eq!(e.itens.len(), 1);
        e.definir_oficina_extra(Path::new("/nao/existe/mesmo"));
        assert!(e.aviso.as_ref().expect("aviso").erro);
    }
}
