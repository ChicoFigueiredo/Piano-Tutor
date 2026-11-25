//! Vídeo embutido do Player de Aula (ADR-0011): a **libmpv** carregada em tempo de execução
//! (`dlopen` de `libmpv.so.2`, sem headers e sem ligar o app a ela) desenhando em software.
//!
//! - **Render API em modo software** (`MPV_RENDER_API_TYPE_SW`, "sw"): o mpv escreve quadros
//!   `rgb0` num buffer nosso; [`VideoEmbutido::quadro_novo`] os converte em
//!   [`bancada_casca::egui::ColorImage`], que o app sobe como textura (`TextureHandle::set`). Nenhum GL aqui.
//! - O mpv avisa por um *callback* de atualização (que só liga um `AtomicBool`; nada da API do mpv
//!   pode ser chamado dele) e a UI, a cada quadro, pergunta por [`VideoEmbutido::quadro_novo`].
//! - [`VideoEmbutido::atualizar`] esvazia a fila de eventos do mpv sem bloquear
//!   (`mpv_wait_event(ctx, 0)`).
//! - Sem a libmpv, [`VideoEmbutido::carregar`] devolve uma mensagem clara e o app cai no "Ver
//!   vídeo" externo (`mpv.rs`).
//!
//! `unsafe` fica confinado a este módulo (o crate usa `deny`; aqui há `allow`): é a fronteira com
//! a API C. Os ponteiros do mpv nunca saem daqui.
//!
//! **NÃO VERIFICADO** em máquinas sem libmpv 0.41 / sem uma saída de áudio: só os cálculos puros
//! têm teste automático; o teste de integração (`#[ignore]`) exige a libmpv e o vídeo da Aula 14.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_double, c_int, c_void};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bancada_casca::egui::{Color32, ColorImage};
use libloading::Library;

/// Nomes pelos quais a libmpv costuma existir, na ordem de tentativa.
#[cfg(target_os = "linux")]
const NOMES_DA_LIB: &[&str] = &["libmpv.so.2", "libmpv.so"];
#[cfg(target_os = "macos")]
const NOMES_DA_LIB: &[&str] = &["libmpv.2.dylib", "libmpv.dylib"];
#[cfg(target_os = "windows")]
const NOMES_DA_LIB: &[&str] = &["libmpv-2.dll", "mpv-2.dll", "mpv.dll"];
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
const NOMES_DA_LIB: &[&str] = &["libmpv.so.2"];

/// Como instalar a libmpv, em português, para mostrar ao aluno.
pub const COMO_INSTALAR_A_LIBMPV: &str = "A biblioteca libmpv não foi encontrada. Para ver o vídeo dentro do Piano Tutor, instale-a (no Kubuntu: sudo apt install libmpv2) e abra o programa de novo. Enquanto isso, o botão \"Ver vídeo\" abre o mpv numa janela à parte.";

// ---------------------------------------------------------------------------------------------
// Constantes da API C (client.h e render.h da libmpv 2.x), copiadas à mão: não há headers.
// ---------------------------------------------------------------------------------------------

const MPV_FORMAT_FLAG: c_int = 3;
const MPV_FORMAT_INT64: c_int = 4;
const MPV_FORMAT_DOUBLE: c_int = 5;

const MPV_EVENT_NONE: c_int = 0;
const MPV_EVENT_SHUTDOWN: c_int = 1;
const MPV_EVENT_END_FILE: c_int = 7;
const MPV_EVENT_FILE_LOADED: c_int = 8;

const MPV_END_FILE_REASON_ERROR: c_int = 4;

const MPV_RENDER_PARAM_INVALID: c_int = 0;
const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
const MPV_RENDER_PARAM_SW_SIZE: c_int = 17;
const MPV_RENDER_PARAM_SW_FORMAT: c_int = 18;
const MPV_RENDER_PARAM_SW_STRIDE: c_int = 19;
const MPV_RENDER_PARAM_SW_POINTER: c_int = 20;

const MPV_RENDER_UPDATE_FRAME: u64 = 1;

/// Formato de pixel pedido ao mpv: bytes R, G, B e um byte sem uso.
const FORMATO_SW: &[u8] = b"rgb0\0";
/// Alinhamento do início do buffer e do passo (stride) de cada linha, como a documentação pede.
const ALINHAMENTO: usize = 64;

#[repr(C)]
struct MpvHandle {
    _privado: [u8; 0],
}

#[repr(C)]
struct MpvRenderContext {
    _privado: [u8; 0],
}

#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

/// Começo de `mpv_event_end_file` (o resto não é lido).
#[repr(C)]
struct MpvEventEndFile {
    reason: c_int,
    error: c_int,
}

#[repr(C)]
struct MpvRenderParam {
    tipo: c_int,
    dados: *mut c_void,
}

type FnCreate = unsafe extern "C" fn() -> *mut MpvHandle;
type FnInitialize = unsafe extern "C" fn(*mut MpvHandle) -> c_int;
type FnTerminateDestroy = unsafe extern "C" fn(*mut MpvHandle);
type FnSetOptionString =
    unsafe extern "C" fn(*mut MpvHandle, *const c_char, *const c_char) -> c_int;
type FnSetPropertyString =
    unsafe extern "C" fn(*mut MpvHandle, *const c_char, *const c_char) -> c_int;
type FnSetProperty =
    unsafe extern "C" fn(*mut MpvHandle, *const c_char, c_int, *mut c_void) -> c_int;
type FnGetProperty =
    unsafe extern "C" fn(*mut MpvHandle, *const c_char, c_int, *mut c_void) -> c_int;
type FnCommand = unsafe extern "C" fn(*mut MpvHandle, *const *const c_char) -> c_int;
type FnWaitEvent = unsafe extern "C" fn(*mut MpvHandle, c_double) -> *mut MpvEvent;
type FnErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type FnRenderCreate =
    unsafe extern "C" fn(*mut *mut MpvRenderContext, *mut MpvHandle, *mut MpvRenderParam) -> c_int;
type FnRenderSetCallback = unsafe extern "C" fn(
    *mut MpvRenderContext,
    Option<unsafe extern "C" fn(*mut c_void)>,
    *mut c_void,
);
type FnRenderUpdate = unsafe extern "C" fn(*mut MpvRenderContext) -> u64;
type FnRenderRender = unsafe extern "C" fn(*mut MpvRenderContext, *mut MpvRenderParam) -> c_int;
type FnRenderReportSwap = unsafe extern "C" fn(*mut MpvRenderContext);
type FnRenderFree = unsafe extern "C" fn(*mut MpvRenderContext);

/// Funções da libmpv resolvidas por `dlsym`. A `Library` fica aqui dentro: os ponteiros valem
/// enquanto este objeto existir.
struct Api {
    create: FnCreate,
    initialize: FnInitialize,
    terminate_destroy: FnTerminateDestroy,
    set_option_string: FnSetOptionString,
    set_property_string: FnSetPropertyString,
    set_property: FnSetProperty,
    get_property: FnGetProperty,
    command: FnCommand,
    wait_event: FnWaitEvent,
    error_string: FnErrorString,
    render_create: FnRenderCreate,
    render_set_callback: FnRenderSetCallback,
    render_update: FnRenderUpdate,
    render_render: FnRenderRender,
    render_report_swap: FnRenderReportSwap,
    render_free: FnRenderFree,
    // Último campo: descarregada depois de tudo o que aponta para ela.
    _lib: Library,
}

impl Api {
    fn carregar() -> Result<Self, String> {
        let mut ultimo_erro = String::new();
        let mut lib = None;
        for nome in NOMES_DA_LIB {
            // SAFETY: carregar uma biblioteca de sistema roda seus inicializadores; a libmpv
            // não tem efeitos colaterais perigosos ao ser carregada.
            match unsafe { Library::new(nome) } {
                Ok(l) => {
                    lib = Some(l);
                    break;
                }
                Err(e) => ultimo_erro = e.to_string(),
            }
        }
        let lib = lib.ok_or_else(|| format!("{COMO_INSTALAR_A_LIBMPV} ({ultimo_erro})"))?;

        macro_rules! simbolo {
            ($ty:ty, $nome:literal) => {{
                // SAFETY: o tipo declarado confere com a assinatura de client.h/render.h.
                let s: libloading::Symbol<$ty> =
                    unsafe { lib.get(concat!($nome, "\0").as_bytes()) }.map_err(|e| {
                        format!(
                            "A libmpv instalada é antiga ou incompleta (falta {}): {e}",
                            $nome
                        )
                    })?;
                *s
            }};
        }

        Ok(Self {
            create: simbolo!(FnCreate, "mpv_create"),
            initialize: simbolo!(FnInitialize, "mpv_initialize"),
            terminate_destroy: simbolo!(FnTerminateDestroy, "mpv_terminate_destroy"),
            set_option_string: simbolo!(FnSetOptionString, "mpv_set_option_string"),
            set_property_string: simbolo!(FnSetPropertyString, "mpv_set_property_string"),
            set_property: simbolo!(FnSetProperty, "mpv_set_property"),
            get_property: simbolo!(FnGetProperty, "mpv_get_property"),
            command: simbolo!(FnCommand, "mpv_command"),
            wait_event: simbolo!(FnWaitEvent, "mpv_wait_event"),
            error_string: simbolo!(FnErrorString, "mpv_error_string"),
            render_create: simbolo!(FnRenderCreate, "mpv_render_context_create"),
            render_set_callback: simbolo!(
                FnRenderSetCallback,
                "mpv_render_context_set_update_callback"
            ),
            render_update: simbolo!(FnRenderUpdate, "mpv_render_context_update"),
            render_render: simbolo!(FnRenderRender, "mpv_render_context_render"),
            render_report_swap: simbolo!(FnRenderReportSwap, "mpv_render_context_report_swap"),
            render_free: simbolo!(FnRenderFree, "mpv_render_context_free"),
            _lib: lib,
        })
    }

    fn texto_do_erro(&self, codigo: c_int) -> String {
        // SAFETY: `mpv_error_string` devolve sempre um texto estático válido.
        let p = unsafe { (self.error_string)(codigo) };
        if p.is_null() {
            return format!("erro {codigo} do mpv");
        }
        // SAFETY: ponteiro não nulo para string C estática.
        let texto = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        format!("{texto} (código {codigo})")
    }
}

/// O mpv exige `LC_NUMERIC=C` (ele recusa criar o player senão). Bibliotecas de GUI podem tê-lo
/// mudado no processo; no Linux repomos "C" por `setlocale` da libc já carregada.
#[cfg(target_os = "linux")]
fn garantir_locale_numerico_c() {
    const LC_NUMERIC_GLIBC: c_int = 1;
    type FnSetLocale = unsafe extern "C" fn(c_int, *const c_char) -> *mut c_char;
    let este_processo = libloading::os::unix::Library::this();
    // SAFETY: `setlocale` existe em toda libc do Linux com esta assinatura.
    if let Ok(setlocale) = unsafe { este_processo.get::<FnSetLocale>(b"setlocale\0") } {
        // SAFETY: "C" é um nome de locale válido e a string é terminada em nulo.
        unsafe { setlocale(LC_NUMERIC_GLIBC, c"C".as_ptr()) };
    }
}

#[cfg(not(target_os = "linux"))]
fn garantir_locale_numerico_c() {}

/// *Callback* de atualização da render context. Roda em thread do mpv: só liga o aviso.
unsafe extern "C" fn aviso_de_atualizacao(dados: *mut c_void) {
    if dados.is_null() {
        return;
    }
    // SAFETY: `dados` é o ponteiro do `Arc<AtomicBool>` guardado em `VideoEmbutido`, que só é
    // liberado depois de a render context ser destruída (nenhum aviso chega depois disso).
    let aviso = unsafe { &*(dados as *const AtomicBool) };
    aviso.store(true, Ordering::Release);
}

// ---------------------------------------------------------------------------------------------
// Parte pura (testada sem libmpv)
// ---------------------------------------------------------------------------------------------

/// Maior tamanho (em pixels, par) que cabe em `max_l` x `max_a` preservando a proporção
/// `video_l` : `video_a`. Devolve `(0, 0)` se algum lado for zero. Pode ampliar o vídeo além do
/// tamanho original: quem chama limita `max_l`/`max_a` ao que precisa.
pub fn tamanho_que_cabe(video_l: u32, video_a: u32, max_l: u32, max_a: u32) -> (u32, u32) {
    if video_l == 0 || video_a == 0 || max_l == 0 || max_a == 0 {
        return (0, 0);
    }
    // Compara as proporções por produto cruzado, em inteiros: l/a do vídeo contra max_l/max_a.
    let (vl, va, ml, ma) = (
        u64::from(video_l),
        u64::from(video_a),
        u64::from(max_l),
        u64::from(max_a),
    );
    let (l, a) = if vl * ma >= ml * va {
        // O vídeo é mais "largo" que a caixa: a largura manda.
        (ml, (ml * va / vl).max(1))
    } else {
        ((ma * vl / va).max(1), ma)
    };
    (par_para_baixo(l), par_para_baixo(a))
}

/// Arredonda para baixo ao par mais próximo, mas nunca abaixo de 2.
fn par_para_baixo(v: u64) -> u32 {
    let v = u32::try_from(v).unwrap_or(u32::MAX);
    (v & !1).max(2)
}

/// Passo (bytes por linha) de uma imagem `rgb0` de `largura` pixels, múltiplo de [`ALINHAMENTO`].
pub fn passo_rgb0(largura: u32) -> usize {
    (largura as usize * 4).div_ceil(ALINHAMENTO) * ALINHAMENTO
}

/// Converte um buffer `rgb0` (bytes R, G, B, sem uso; `passo` bytes por linha) em imagem do egui,
/// opaca. Linhas ou bytes que faltem ficam pretos em vez de dar pânico.
pub fn rgb0_para_imagem(dados: &[u8], passo: usize, largura: u32, altura: u32) -> ColorImage {
    let (l, a) = (largura as usize, altura as usize);
    let mut pixels = vec![Color32::BLACK; l * a];
    if passo >= l * 4 {
        for (linha, destino) in pixels.chunks_exact_mut(l.max(1)).enumerate().take(a) {
            let inicio = linha * passo;
            let Some(origem) = dados.get(inicio..inicio + l * 4) else {
                break;
            };
            for (px, rgb0) in destino.iter_mut().zip(origem.as_chunks::<4>().0) {
                *px = Color32::from_rgb(rgb0[0], rgb0[1], rgb0[2]);
            }
        }
    }
    ColorImage::new([l, a], pixels)
}

/// Buffer de quadro com o início alinhado a [`ALINHAMENTO`] (o `Vec` sozinho não garante).
struct BufferDoQuadro {
    bytes: Vec<u8>,
    deslocamento: usize,
    passo: usize,
    largura: u32,
    altura: u32,
}

impl BufferDoQuadro {
    fn vazio() -> Self {
        Self {
            bytes: Vec::new(),
            deslocamento: 0,
            passo: 0,
            largura: 0,
            altura: 0,
        }
    }

    fn redimensionar(&mut self, largura: u32, altura: u32) {
        if self.largura == largura && self.altura == altura && !self.bytes.is_empty() {
            return;
        }
        self.passo = passo_rgb0(largura);
        let util = self.passo * altura as usize;
        self.bytes = vec![0; util + ALINHAMENTO];
        let desalinhamento = self.bytes.as_ptr() as usize % ALINHAMENTO;
        self.deslocamento = (ALINHAMENTO - desalinhamento) % ALINHAMENTO;
        self.largura = largura;
        self.altura = altura;
    }

    fn area(&self) -> &[u8] {
        let fim = self.deslocamento + self.passo * self.altura as usize;
        &self.bytes[self.deslocamento..fim]
    }

    fn ponteiro(&mut self) -> *mut c_void {
        self.bytes[self.deslocamento..].as_mut_ptr().cast()
    }
}

// ---------------------------------------------------------------------------------------------
// Player
// ---------------------------------------------------------------------------------------------

/// Player de vídeo sobre a libmpv, desenhando em software. Use só na thread da UI.
pub struct VideoEmbutido {
    api: Api,
    ctx: *mut MpvHandle,
    render: *mut MpvRenderContext,
    /// Ligado pelo *callback* do mpv quando há quadro novo (ou a imagem precisa ser refeita).
    atualizacao: Arc<AtomicBool>,
    buffer: BufferDoQuadro,
    /// Tamanho do último quadro entregue, para refazer a imagem quando o painel muda de tamanho.
    ultimo_tamanho: Option<(u32, u32)>,
    carregado: bool,
    busca_pendente: Option<f64>,
    erro: Option<String>,
}

// SAFETY: o handle do mpv e a render context só são usados por `&mut self`/`&self` (uma thread por
// vez); a API do mpv é segura entre threads sob essa condição. O `Arc<AtomicBool>` é `Send`.
unsafe impl Send for VideoEmbutido {}

impl VideoEmbutido {
    /// Carrega a libmpv e cria o player (sem arquivo aberto). Falha com mensagem clara se a
    /// biblioteca não existe; o app então oferece o "Ver vídeo" externo.
    pub fn carregar() -> Result<Self, String> {
        let api = Api::carregar()?;
        garantir_locale_numerico_c();

        // SAFETY: `mpv_create` não tem pré-condições.
        let ctx = unsafe { (api.create)() };
        if ctx.is_null() {
            return Err("A libmpv não conseguiu criar o player (mpv_create devolveu nulo).".into());
        }

        // Se algo falhar daqui até o fim, destrói o handle para não vazar.
        let destruir = |api: &Api, ctx| {
            // SAFETY: `ctx` é um handle válido e nenhuma render context foi criada sobre ele.
            unsafe { (api.terminate_destroy)(ctx) };
        };

        let opcoes: &[(&str, &str)] = &[
            ("vo", "libmpv"),
            ("hwdec", "auto-safe"),
            ("keep-open", "yes"),
            ("idle", "yes"),
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("hr-seek", "yes"),
            ("ytdl", "no"),
        ];
        for (nome, valor) in opcoes {
            let (Ok(n), Ok(v)) = (CString::new(*nome), CString::new(*valor)) else {
                continue;
            };
            // SAFETY: strings C válidas; opção definida antes de `mpv_initialize`.
            let rc = unsafe { (api.set_option_string)(ctx, n.as_ptr(), v.as_ptr()) };
            if rc < 0 {
                let msg = format!("Opção {nome}={valor} recusada: {}", api.texto_do_erro(rc));
                destruir(&api, ctx);
                return Err(msg);
            }
        }

        // SAFETY: handle válido, ainda não inicializado.
        let rc = unsafe { (api.initialize)(ctx) };
        if rc < 0 {
            let msg = format!("mpv_initialize falhou: {}", api.texto_do_erro(rc));
            destruir(&api, ctx);
            return Err(msg);
        }

        let mut render: *mut MpvRenderContext = std::ptr::null_mut();
        let mut api_sw = *b"sw\0";
        let mut params = [
            MpvRenderParam {
                tipo: MPV_RENDER_PARAM_API_TYPE,
                dados: api_sw.as_mut_ptr().cast(),
            },
            MpvRenderParam {
                tipo: MPV_RENDER_PARAM_INVALID,
                dados: std::ptr::null_mut(),
            },
        ];
        // SAFETY: `params` é uma lista terminada por INVALID; "sw" é terminado em nulo.
        let rc = unsafe { (api.render_create)(&mut render, ctx, params.as_mut_ptr()) };
        if rc < 0 || render.is_null() {
            let msg = format!(
                "A libmpv não criou o renderizador por software: {}",
                api.texto_do_erro(rc)
            );
            destruir(&api, ctx);
            return Err(msg);
        }

        let atualizacao = Arc::new(AtomicBool::new(false));
        // SAFETY: o ponteiro do `Arc` permanece válido até `Drop`, que libera a render context
        // (e portanto encerra os avisos) antes de soltar o `Arc`.
        unsafe {
            (api.render_set_callback)(
                render,
                Some(aviso_de_atualizacao),
                Arc::as_ptr(&atualizacao).cast_mut().cast(),
            );
        }

        Ok(Self {
            api,
            ctx,
            render,
            atualizacao,
            buffer: BufferDoQuadro::vazio(),
            ultimo_tamanho: None,
            carregado: false,
            busca_pendente: None,
            erro: None,
        })
    }

    // -- comandos e propriedades ---------------------------------------------------------------

    fn registrar_erro(&mut self, o_que: &str, codigo: c_int) {
        self.erro = Some(format!("{o_que}: {}", self.api.texto_do_erro(codigo)));
    }

    fn comando(&mut self, args: &[&str]) -> Result<(), String> {
        let cstrings: Vec<CString> = args
            .iter()
            .map(|a| CString::new(*a).map_err(|_| format!("Argumento inválido para o mpv: {a:?}")))
            .collect::<Result<_, _>>()?;
        let mut ponteiros: Vec<*const c_char> = cstrings.iter().map(|c| c.as_ptr()).collect();
        ponteiros.push(std::ptr::null());
        // SAFETY: lista terminada em nulo de strings C que vivem até o fim da chamada.
        let rc = unsafe { (self.api.command)(self.ctx, ponteiros.as_ptr()) };
        if rc < 0 {
            let msg = format!(
                "{}: {}",
                args.first().unwrap_or(&"?"),
                self.api.texto_do_erro(rc)
            );
            self.erro = Some(msg.clone());
            return Err(msg);
        }
        Ok(())
    }

    fn propriedade_double(&self, nome: &str) -> Option<f64> {
        let n = CString::new(nome).ok()?;
        let mut v: f64 = 0.0;
        // SAFETY: `v` é um double e o formato pedido é DOUBLE.
        let rc = unsafe {
            (self.api.get_property)(
                self.ctx,
                n.as_ptr(),
                MPV_FORMAT_DOUBLE,
                (&mut v as *mut f64).cast(),
            )
        };
        (rc >= 0 && v.is_finite()).then_some(v)
    }

    fn propriedade_inteira(&self, nome: &str) -> Option<i64> {
        let n = CString::new(nome).ok()?;
        let mut v: i64 = 0;
        // SAFETY: `v` é um int64 e o formato pedido é INT64.
        let rc = unsafe {
            (self.api.get_property)(
                self.ctx,
                n.as_ptr(),
                MPV_FORMAT_INT64,
                (&mut v as *mut i64).cast(),
            )
        };
        (rc >= 0).then_some(v)
    }

    fn propriedade_flag(&self, nome: &str) -> Option<bool> {
        let n = CString::new(nome).ok()?;
        let mut v: c_int = 0;
        // SAFETY: `v` é um int C e o formato pedido é FLAG.
        let rc = unsafe {
            (self.api.get_property)(
                self.ctx,
                n.as_ptr(),
                MPV_FORMAT_FLAG,
                (&mut v as *mut c_int).cast(),
            )
        };
        (rc >= 0).then_some(v != 0)
    }

    fn definir_double(&mut self, nome: &str, valor: f64) {
        let Ok(n) = CString::new(nome) else { return };
        let mut v = valor;
        // SAFETY: `v` é um double e o formato dado é DOUBLE.
        let rc = unsafe {
            (self.api.set_property)(
                self.ctx,
                n.as_ptr(),
                MPV_FORMAT_DOUBLE,
                (&mut v as *mut f64).cast(),
            )
        };
        if rc < 0 {
            self.registrar_erro(nome, rc);
        }
    }

    fn definir_texto(&mut self, nome: &str, valor: &str) {
        let (Ok(n), Ok(v)) = (CString::new(nome), CString::new(valor)) else {
            return;
        };
        // SAFETY: strings C válidas.
        let rc = unsafe { (self.api.set_property_string)(self.ctx, n.as_ptr(), v.as_ptr()) };
        if rc < 0 {
            self.registrar_erro(nome, rc);
        }
    }

    // -- API pública ---------------------------------------------------------------------------

    /// Abre `arquivo` e começa a tocar em `inicio_s` segundos (busca exata). Troca o que estava
    /// aberto. Volta na hora: o arquivo só fica pronto no `FILE_LOADED` (veja [`Self::atualizar`]).
    pub fn abrir(&mut self, arquivo: &Path, inicio_s: f64) -> Result<(), String> {
        let caminho = arquivo
            .to_str()
            .ok_or_else(|| format!("Caminho sem UTF-8 válido: {}", arquivo.display()))?;
        let inicio = if inicio_s.is_finite() {
            inicio_s.max(0.0)
        } else {
            0.0
        };
        self.carregado = false;
        self.busca_pendente = None;
        self.ultimo_tamanho = None;
        self.erro = None;
        self.comando(&[
            "loadfile",
            caminho,
            "replace",
            "-1",
            &format!("start={inicio:.3}"),
        ])
    }

    /// Pausa (`true`) ou continua (`false`).
    pub fn pausar(&mut self, pausar: bool) {
        self.definir_texto("pause", if pausar { "yes" } else { "no" });
    }

    /// Está pausado? (`false` se o mpv não souber dizer.)
    pub fn pausado(&self) -> bool {
        self.propriedade_flag("pause").unwrap_or(false)
    }

    /// Busca absoluta e exata para `s` segundos. Antes de o arquivo carregar, fica pendente e é
    /// aplicada quando ele carregar.
    pub fn buscar(&mut self, s: f64) {
        if !s.is_finite() {
            return;
        }
        let s = s.max(0.0);
        if !self.carregado {
            self.busca_pendente = Some(s);
            return;
        }
        let _ = self.comando(&["seek", &format!("{s:.3}"), "absolute+exact"]);
    }

    /// Posição de reprodução (`time-pos`), em segundos; `None` sem arquivo ou antes de começar.
    pub fn posicao_s(&self) -> Option<f64> {
        self.propriedade_double("time-pos")
    }

    /// Duração do arquivo, em segundos.
    pub fn duracao_s(&self) -> Option<f64> {
        self.propriedade_double("duration")
    }

    /// Velocidade de reprodução (1.0 = normal), com correção de tom do áudio (padrão do mpv).
    pub fn definir_velocidade(&mut self, velocidade: f64) {
        if velocidade.is_finite() {
            self.definir_double("speed", velocidade.clamp(0.01, 100.0));
        }
    }

    /// Velocidade atual (1.0 se o mpv não souber dizer).
    pub fn velocidade(&self) -> f64 {
        self.propriedade_double("speed").unwrap_or(1.0)
    }

    /// Volume de 0.0 a 100.0 (o mpv aceita mais, mas aqui não passa de 100).
    pub fn definir_volume(&mut self, volume: f64) {
        if volume.is_finite() {
            self.definir_double("volume", volume.clamp(0.0, 100.0));
        }
    }

    /// O arquivo terminou e está parado no último quadro (`keep-open`).
    pub fn chegou_ao_fim(&self) -> bool {
        self.propriedade_flag("eof-reached").unwrap_or(false)
    }

    /// Já há um arquivo carregado (evento `FILE_LOADED` recebido)?
    pub fn arquivo_carregado(&self) -> bool {
        self.carregado
    }

    /// Último erro do mpv (comando recusado, arquivo que não abre), para a UI mostrar.
    pub fn ultimo_erro(&self) -> Option<&str> {
        self.erro.as_deref()
    }

    /// Tamanho do vídeo já com a proporção de exibição aplicada (`video-params/dw` e `/dh`).
    pub fn dimensoes_do_video(&self) -> Option<(u32, u32)> {
        let l = self
            .propriedade_inteira("video-params/dw")
            .or_else(|| self.propriedade_inteira("dwidth"))?;
        let a = self
            .propriedade_inteira("video-params/dh")
            .or_else(|| self.propriedade_inteira("dheight"))?;
        let (l, a) = (u32::try_from(l).ok()?, u32::try_from(a).ok()?);
        (l > 0 && a > 0).then_some((l, a))
    }

    /// Esvazia, sem bloquear, a fila de eventos do mpv (`mpv_wait_event(ctx, 0)`). Chame a cada
    /// quadro da UI (já é chamado por [`Self::quadro_novo`]).
    pub fn atualizar(&mut self) {
        loop {
            // SAFETY: `ctx` é válido; o evento devolvido vale até a próxima chamada.
            let ev = unsafe { (self.api.wait_event)(self.ctx, 0.0) };
            if ev.is_null() {
                return;
            }
            // SAFETY: ponteiro não nulo devolvido pelo mpv, lido antes da próxima chamada.
            let (id, erro, dados) = unsafe { ((*ev).event_id, (*ev).error, (*ev).data) };
            match id {
                MPV_EVENT_NONE => return,
                MPV_EVENT_SHUTDOWN => {
                    self.carregado = false;
                    return;
                }
                MPV_EVENT_FILE_LOADED => {
                    self.carregado = true;
                    if let Some(s) = self.busca_pendente.take() {
                        let _ = self.comando(&["seek", &format!("{s:.3}"), "absolute+exact"]);
                    }
                }
                MPV_EVENT_END_FILE => {
                    self.carregado = false;
                    if !dados.is_null() {
                        // SAFETY: em END_FILE, `data` aponta para `mpv_event_end_file`.
                        let fim = unsafe { &*(dados as *const MpvEventEndFile) };
                        if fim.reason == MPV_END_FILE_REASON_ERROR {
                            let codigo = if fim.error < 0 { fim.error } else { erro };
                            self.registrar_erro("O vídeo não abriu", codigo);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Se o mpv sinalizou atualização (ou o painel mudou de tamanho), desenha o quadro atual no
    /// tamanho que cabe em `largura_max` x `altura_max` preservando a proporção do vídeo
    /// (`dwidth`/`dheight`) e o devolve; senão `None`. Chamar a cada quadro da UI.
    pub fn quadro_novo(&mut self, largura_max: u32, altura_max: u32) -> Option<ColorImage> {
        self.atualizar();
        let (vl, va) = self.dimensoes_do_video()?;
        let (l, a) = tamanho_que_cabe(vl, va, largura_max, altura_max);
        if l == 0 || a == 0 {
            return None;
        }
        let avisado = self.atualizacao.swap(false, Ordering::AcqRel);
        let mudou_tamanho = self.ultimo_tamanho.is_some_and(|t| t != (l, a));
        if !avisado && !mudou_tamanho {
            return None;
        }

        // SAFETY: render context válida; deve ser chamada depois de cada aviso.
        let flags = unsafe { (self.api.render_update)(self.render) };
        if flags & MPV_RENDER_UPDATE_FRAME == 0 && !mudou_tamanho {
            return None;
        }

        self.buffer.redimensionar(l, a);
        let mut tamanho = [l as c_int, a as c_int];
        let mut passo: usize = self.buffer.passo;
        let mut formato = *b"rgb0\0";
        debug_assert_eq!(formato.as_slice(), FORMATO_SW);
        let ponteiro = self.buffer.ponteiro();
        let mut params = [
            MpvRenderParam {
                tipo: MPV_RENDER_PARAM_SW_SIZE,
                dados: tamanho.as_mut_ptr().cast(),
            },
            MpvRenderParam {
                tipo: MPV_RENDER_PARAM_SW_FORMAT,
                dados: formato.as_mut_ptr().cast(),
            },
            MpvRenderParam {
                tipo: MPV_RENDER_PARAM_SW_STRIDE,
                dados: (&mut passo as *mut usize).cast(),
            },
            MpvRenderParam {
                tipo: MPV_RENDER_PARAM_SW_POINTER,
                dados: ponteiro,
            },
            MpvRenderParam {
                tipo: MPV_RENDER_PARAM_INVALID,
                dados: std::ptr::null_mut(),
            },
        ];
        // SAFETY: o buffer tem `passo * altura` bytes a partir de `ponteiro`, alinhado a 64; os
        // parâmetros terminam em INVALID e vivem até o fim da chamada.
        let rc = unsafe { (self.api.render_render)(self.render, params.as_mut_ptr()) };
        if rc < 0 {
            self.registrar_erro("Falha ao desenhar o quadro", rc);
            return None;
        }
        // SAFETY: render context válida.
        unsafe { (self.api.render_report_swap)(self.render) };

        self.ultimo_tamanho = Some((l, a));
        Some(rgb0_para_imagem(
            self.buffer.area(),
            self.buffer.passo,
            l,
            a,
        ))
    }
}

impl Drop for VideoEmbutido {
    fn drop(&mut self) {
        // Ordem exigida pela libmpv: tirar o callback, liberar a render context e só então
        // destruir o handle. O `Arc` do aviso e a biblioteca saem depois (campos).
        // SAFETY: ponteiros criados em `carregar` e liberados uma única vez aqui.
        unsafe {
            (self.api.render_set_callback)(self.render, None, std::ptr::null_mut());
            (self.api.render_free)(self.render);
            (self.api.terminate_destroy)(self.ctx);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cabe_preservando_a_proporcao_pela_largura() {
        // 16:9 numa caixa 800x800: a largura manda.
        assert_eq!(tamanho_que_cabe(1920, 1080, 800, 800), (800, 450));
    }

    #[test]
    fn cabe_preservando_a_proporcao_pela_altura() {
        // 16:9 numa caixa 1000x300: a altura manda (300 -> 533, par = 532).
        assert_eq!(tamanho_que_cabe(1920, 1080, 1000, 300), (532, 300));
    }

    #[test]
    fn cabe_com_proporcao_igual_a_da_caixa() {
        assert_eq!(tamanho_que_cabe(640, 360, 1280, 720), (1280, 720));
    }

    #[test]
    fn cabe_video_vertical() {
        assert_eq!(tamanho_que_cabe(1080, 1920, 600, 600), (336, 600));
    }

    #[test]
    fn cabe_com_zeros_devolve_zero() {
        assert_eq!(tamanho_que_cabe(0, 1080, 800, 600), (0, 0));
        assert_eq!(tamanho_que_cabe(1920, 0, 800, 600), (0, 0));
        assert_eq!(tamanho_que_cabe(1920, 1080, 0, 600), (0, 0));
        assert_eq!(tamanho_que_cabe(1920, 1080, 800, 0), (0, 0));
    }

    #[test]
    fn cabe_nunca_passa_da_caixa_nem_fica_impar() {
        for (vl, va) in [(1920, 1080), (1000, 3), (3, 1000), (1281, 721)] {
            for (ml, ma) in [(2, 2), (7, 5), (333, 777), (4000, 10)] {
                let (l, a) = tamanho_que_cabe(vl, va, ml, ma);
                assert!(l >= 2 && a >= 2, "{vl}x{va} em {ml}x{ma}: {l}x{a}");
                assert!(l % 2 == 0 && a % 2 == 0, "{l}x{a} deveria ser par");
                // Só pode passar da caixa pelo piso de 2 pixels.
                assert!(
                    l <= ml.max(2) && a <= ma.max(2),
                    "{vl}x{va} em {ml}x{ma}: {l}x{a}"
                );
            }
        }
    }

    #[test]
    fn passo_e_multiplo_de_64() {
        assert_eq!(passo_rgb0(16), 64);
        assert_eq!(passo_rgb0(17), 128);
        assert_eq!(passo_rgb0(800), 3200);
        assert_eq!(passo_rgb0(800) % ALINHAMENTO, 0);
    }

    #[test]
    fn rgb0_vira_imagem_opaca_ignorando_o_byte_sem_uso_e_o_enchimento() {
        // 2x2 com passo 16: cada linha = 2 pixels (8 bytes) + 8 de enchimento.
        let mut dados = vec![0u8; 32];
        dados[0..4].copy_from_slice(&[255, 0, 0, 7]); // vermelho (o 4o byte é lixo)
        dados[4..8].copy_from_slice(&[0, 255, 0, 0]); // verde
        dados[8..16].fill(99); // enchimento
        dados[16..20].copy_from_slice(&[0, 0, 255, 0]); // azul
        dados[20..24].copy_from_slice(&[10, 20, 30, 0]);
        let img = rgb0_para_imagem(&dados, 16, 2, 2);
        assert_eq!(img.size, [2, 2]);
        assert_eq!(img.pixels[0], Color32::from_rgb(255, 0, 0));
        assert_eq!(img.pixels[1], Color32::from_rgb(0, 255, 0));
        assert_eq!(img.pixels[2], Color32::from_rgb(0, 0, 255));
        assert_eq!(img.pixels[3], Color32::from_rgb(10, 20, 30));
        assert!(img.pixels.iter().all(|p| p.a() == 255));
    }

    #[test]
    fn rgb0_com_buffer_curto_nao_da_panico() {
        let img = rgb0_para_imagem(&[1, 2, 3, 0], 64, 2, 3);
        assert_eq!(img.size, [2, 3]);
        assert_eq!(img.pixels.len(), 6);
        // Passo menor que a linha também não derruba.
        let img = rgb0_para_imagem(&[0; 64], 4, 4, 2);
        assert_eq!(img.pixels.len(), 8);
    }

    #[test]
    fn buffer_do_quadro_fica_alinhado() {
        let mut b = BufferDoQuadro::vazio();
        b.redimensionar(100, 50);
        assert_eq!(b.passo, 448);
        assert_eq!(b.ponteiro() as usize % ALINHAMENTO, 0);
        assert_eq!(b.area().len(), 448 * 50);
        b.redimensionar(333, 7);
        assert_eq!(b.ponteiro() as usize % ALINHAMENTO, 0);
        assert_eq!(b.area().len(), passo_rgb0(333) * 7);
    }

    /// Integração com a libmpv e o vídeo real da Aula 14. Precisa da `libmpv.so.2` e do arquivo:
    /// `cargo test -p piano-tutor-app video_embutido -- --ignored --nocapture`.
    #[test]
    #[ignore = "exige libmpv.so.2 e o vídeo da Aula 14 em /mnt/e2"]
    fn integracao_toca_um_segundo_e_entrega_quadros() {
        use std::time::{Duration, Instant};

        let arquivo = Path::new(
            "/mnt/e2/Musica/Curso-Piano-1/03 - Mão direita/14 - Música (Cinco notas no piano).mp4",
        );
        assert!(
            arquivo.exists(),
            "vídeo de teste ausente: {}",
            arquivo.display()
        );

        let mut v = VideoEmbutido::carregar().expect("libmpv deveria carregar");
        v.definir_volume(0.0);
        v.abrir(arquivo, 10.0).expect("loadfile");

        // Espera o primeiro quadro (até 15 s: abrir arquivo em /mnt/e2 pode demorar).
        let t0 = Instant::now();
        let mut primeiro = None;
        while t0.elapsed() < Duration::from_secs(15) {
            if let Some(img) = v.quadro_novo(640, 480) {
                primeiro = Some(img);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let primeiro = primeiro.expect("nenhum quadro em 15 s");
        let pos0 = v.posicao_s().expect("time-pos depois do primeiro quadro");
        let dur = v.duracao_s().expect("duração");
        let dim = v.dimensoes_do_video().expect("dimensões");
        println!(
            "primeiro quadro {:?} após {:?}; vídeo {dim:?}, duração {dur:.1} s, time-pos {pos0:.3}",
            primeiro.size,
            t0.elapsed()
        );
        assert!(pos0 >= 9.0, "deveria começar perto de 10 s, veio {pos0}");
        assert!(!v.pausado());
        assert_eq!(primeiro.size[0] % 2, 0);

        // Deixa rodar ~1 s coletando quadros.
        let t1 = Instant::now();
        let mut quadros = 1usize;
        let mut nao_preto = nao_e_preto(&primeiro);
        while t1.elapsed() < Duration::from_millis(1000) {
            if let Some(img) = v.quadro_novo(640, 480) {
                quadros += 1;
                nao_preto |= nao_e_preto(&img);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let pos1 = v.posicao_s().expect("time-pos");
        println!(
            "quadros em ~1 s: {quadros}; time-pos {pos0:.3} -> {pos1:.3}; não preto: {nao_preto}"
        );
        assert!(nao_preto, "todos os quadros vieram pretos");
        assert!(quadros >= 5, "poucos quadros em 1 s: {quadros}");
        assert!(pos1 - pos0 >= 0.5, "posição não avançou: {pos0} -> {pos1}");

        // Pausa, busca exata e velocidade.
        v.pausar(true);
        std::thread::sleep(Duration::from_millis(100));
        assert!(v.pausado());
        v.buscar(20.0);
        let t2 = Instant::now();
        let mut pos_busca = None;
        while t2.elapsed() < Duration::from_secs(5) {
            let _ = v.quadro_novo(640, 480);
            if let Some(p) = v.posicao_s()
                && (p - 20.0).abs() < 0.05
            {
                pos_busca = Some(p);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        println!("após buscar(20.0): time-pos = {pos_busca:?}");
        assert!(pos_busca.is_some(), "busca exata não chegou a 20 s");
        v.definir_velocidade(0.5);
        assert!((v.velocidade() - 0.5).abs() < 1e-9);
    }

    fn nao_e_preto(img: &ColorImage) -> bool {
        img.pixels.iter().any(|p| p.r().max(p.g()).max(p.b()) > 24)
    }
}
