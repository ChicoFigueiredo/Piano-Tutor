//! Roteiro de Aula em Luau (ADR-0012): o script que conduz o Player de Aula.
//!
//! Módulo **puro** (sem UI, sem áudio, sem arquivos além de `ler_roteiro_da_pasta`): carrega um
//! script Luau num sandbox, chama os callbacks que o script definiu e devolve **pedidos** que o
//! player decide executar. O script nunca age direto: toda função da API global `aula` só
//! ENFILEIRA um [`Pedido`], entregue ao Rust quando o callback termina.
//!
//! ## API que o script enxerga (versão 1)
//!
//! Callbacks globais, todos opcionais:
//!
//! - `ao_abrir()`: a Aula foi aberta.
//! - `ao_entrar_trecho(t)`: o player entrou num trecho. `t = {id, titulo, inicio, fim, indice}`
//!   (`inicio`/`fim` em segundos do vídeo; `indice` conta de **1**, como é costume em Lua).
//! - `ao_fim_trecho(t, r)`: o trecho acabou. `r = {acertos, erros}`.
//! - `ao_nota(midi, certa)`: o aluno tocou uma nota (`certa` é booleano).
//!
//! Tabela global `aula` (somente leitura; funções só enfileiram pedidos):
//!
//! - `aula.versao_api` (constante, 1);
//! - `aula.modo("nota_a_nota" | "espera_tocar" | "segue" | "chamada")` ("espera" = nota a nota), `aula.ir(id_do_trecho)`, `aula.repetir()`,
//!   `aula.pausar()`, `aula.continuar()`, `aula.velocidade(x)` (de 0.25 a 2), `aula.mensagem(texto)`,
//!   `aula.dizer(texto)` (fala do professor por voz sintética);
//! - `aula.contar(chave)` soma 1 ao contador `chave` e devolve o novo valor; `aula.contador(chave)`
//!   lê (0 se nunca contado); `aula.zerar(chave)` volta a 0. Os contadores vivem no Rust e
//!   sobrevivem entre callbacks (variáveis globais e locais do próprio script também sobrevivem);
//! - `print(...)` não escreve em stdout: vai para o log do Roteiro ([`RoteiroAula::tirar_log`]),
//!   para o modo de depuração da tela.
//!
//! `require("nome")` carrega um módulo da biblioteca comum do Curso (mapa nome → código passado a
//! [`RoteiroAula::carregar`]); qualquer outro nome dá erro. Não existe acesso a arquivos.
//!
//! ## Sandbox
//!
//! Luau com `sandbox(true)`; sem `io`, `os`, `debug`, `package`, `loadstring`, `load` nem
//! `dofile`; memória limitada a [`LIMITE_MEMORIA`]; cada chamada tem orçamento de
//! [`LIMITE_TEMPO`] de relógio e de [`LIMITE_INTERRUPCOES`] interrupções do Luau (laços e
//! chamadas). Estourar o orçamento ou a memória aborta **aquela** chamada com erro em português;
//! o Roteiro continua utilizável. Limite conhecido: o orçamento só é conferido em chamadas de
//! função e voltas de laço do Luau; uma única chamada nativa pesada (um `string.find` com padrão
//! patológico, por exemplo) não é interrompida no meio.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use mlua::{Error as ErroLua, Function, Lua, LuaOptions, StdLib, Table, Value, VmState};

use crate::player_aula::{ModoAcompanhamento, Trecho};

/// Versão da API do player exposta como `aula.versao_api`.
pub const VERSAO_API: i64 = 1;
/// Teto de memória do Luau por Roteiro (16 MB).
pub const LIMITE_MEMORIA: usize = 16 * 1024 * 1024;
/// Teto de relógio por chamada (carregamento do script ou um callback).
pub const LIMITE_TEMPO: Duration = Duration::from_millis(50);
/// Teto de interrupções do Luau (chamadas e voltas de laço) por chamada.
pub const LIMITE_INTERRUPCOES: u64 = 5_000_000;
/// Tamanho máximo de um script (principal ou módulo), em bytes.
pub const LIMITE_CODIGO: usize = 256 * 1024;
/// Máximo de pedidos que uma só chamada pode enfileirar.
pub const LIMITE_PEDIDOS: usize = 256;
/// Máximo de caracteres de um texto de `aula.mensagem` / `aula.dizer`.
pub const LIMITE_TEXTO: usize = 2000;
/// Máximo de linhas guardadas no log do Roteiro (as mais antigas caem).
const LIMITE_LOG: usize = 500;

const NOME_PRINCIPAL: &str = "roteiro";
const MARCA_ORCAMENTO: &str = "orcamento-estourado";

/// Pedido do Roteiro ao player; o player decide quando e se executa.
#[derive(Clone, Debug, PartialEq)]
pub enum Pedido {
    /// Trocar o Modo de acompanhamento.
    Modo(ModoAcompanhamento),
    /// Ir ao trecho de id dado.
    Ir(String),
    /// Voltar ao início do trecho atual.
    Repetir,
    Pausar,
    Continuar,
    /// Velocidade de reprodução, de 0.25 a 2.
    Velocidade(f64),
    /// Mensagem escrita na tela.
    Mensagem(String),
    /// Fala do professor por voz sintética.
    Dizer(String),
}

/// O trecho como o Roteiro o vê.
#[derive(Clone, Debug, PartialEq)]
pub struct TrechoInfo {
    pub id: String,
    pub titulo: String,
    pub inicio_s: f64,
    pub fim_s: f64,
    /// Posição do trecho na lista da Aula, contada de 0 no Rust (o Luau recebe `indice + 1`).
    pub indice: usize,
}

impl TrechoInfo {
    /// Converte um [`Trecho`] do player; `indice` é a posição dele na lista (de 0).
    pub fn de_trecho(t: &Trecho, indice: usize) -> Self {
        Self {
            id: t.exercicio_id.clone(),
            titulo: t.titulo.clone(),
            inicio_s: t.inicio_s,
            fim_s: t.fim_s,
            indice,
        }
    }
}

/// Orçamento de uma chamada: relógio e contagem de interrupções.
struct Orcamento {
    base: Instant,
    prazo_ns: AtomicU64,
    contagem: AtomicU64,
    estourou: AtomicBool,
}

impl Orcamento {
    fn novo() -> Self {
        Self {
            base: Instant::now(),
            prazo_ns: AtomicU64::new(u64::MAX),
            contagem: AtomicU64::new(0),
            estourou: AtomicBool::new(false),
        }
    }

    fn iniciar(&self) {
        let agora = u64::try_from(self.base.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let limite = u64::try_from(LIMITE_TEMPO.as_nanos()).unwrap_or(u64::MAX);
        self.prazo_ns
            .store(agora.saturating_add(limite), Ordering::Relaxed);
        self.contagem.store(0, Ordering::Relaxed);
        self.estourou.store(false, Ordering::Relaxed);
    }

    /// `true` se a chamada já passou do orçamento (e continua assim até o próximo `iniciar`).
    fn passou(&self) -> bool {
        if self.estourou.load(Ordering::Relaxed) {
            return true;
        }
        let n = self.contagem.fetch_add(1, Ordering::Relaxed) + 1;
        let agora = u64::try_from(self.base.elapsed().as_nanos()).unwrap_or(u64::MAX);
        if n > LIMITE_INTERRUPCOES || agora > self.prazo_ns.load(Ordering::Relaxed) {
            self.estourou.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }
}

fn travar<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Um Roteiro de Aula carregado e pronto para receber eventos.
pub struct RoteiroAula {
    lua: Lua,
    pedidos: Arc<Mutex<Vec<Pedido>>>,
    orcamento: Arc<Orcamento>,
    log: Arc<Mutex<Vec<String>>>,
}

impl std::fmt::Debug for RoteiroAula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoteiroAula").finish_non_exhaustive()
    }
}

impl RoteiroAula {
    /// Carrega o script da Aula (`principal`) com a biblioteca comum do Curso (`modulos`, nome →
    /// código, para `require("nome")`). Roda o corpo do script uma vez (é onde ele define os
    /// callbacks e variáveis). `Err` traz o motivo em português, com a linha quando há.
    pub fn carregar(principal: &str, modulos: &BTreeMap<String, String>) -> Result<Self, String> {
        if principal.len() > LIMITE_CODIGO {
            return Err(format!(
                "O Roteiro de Aula tem {} bytes; o limite é {LIMITE_CODIGO}.",
                principal.len()
            ));
        }
        for (nome, codigo) in modulos {
            if codigo.len() > LIMITE_CODIGO {
                return Err(format!(
                    "O módulo «{nome}» tem {} bytes; o limite é {LIMITE_CODIGO}.",
                    codigo.len()
                ));
            }
        }

        let pedidos = Arc::new(Mutex::new(Vec::new()));
        let orcamento = Arc::new(Orcamento::novo());
        let log = Arc::new(Mutex::new(Vec::new()));

        // Sem OS, PACKAGE nem DEBUG: só o que um roteiro de aula precisa.
        let libs = StdLib::COROUTINE
            | StdLib::TABLE
            | StdLib::STRING
            | StdLib::UTF8
            | StdLib::BIT
            | StdLib::MATH
            | StdLib::BUFFER
            | StdLib::VECTOR;
        let lua = Lua::new_with(libs, LuaOptions::default())
            .map_err(|e| format!("Não consegui iniciar o Luau: {e}"))?;
        lua.set_memory_limit(LIMITE_MEMORIA)
            .map_err(|e| format!("Não consegui limitar a memória do Luau: {e}"))?;
        {
            let orc = Arc::clone(&orcamento);
            lua.set_interrupt(move |_| {
                if orc.passou() {
                    Err(ErroLua::runtime(MARCA_ORCAMENTO))
                } else {
                    Ok(VmState::Continue)
                }
            });
        }

        let roteiro = Self {
            lua,
            pedidos,
            orcamento,
            log,
        };
        roteiro.preparar_globais(modulos).map_err(|e| {
            format!(
                "Não consegui preparar o ambiente do Roteiro: {}",
                roteiro.explicar(&e)
            )
        })?;

        roteiro.orcamento.iniciar();
        let r = roteiro
            .lua
            .load(principal)
            .set_name(format!("={NOME_PRINCIPAL}"))
            .exec();
        // Pedidos feitos no corpo do script não têm quem os execute: descartados.
        travar(&roteiro.pedidos).clear();
        r.map_err(|e| {
            format!(
                "Erro ao carregar o Roteiro de Aula: {}",
                roteiro.explicar(&e)
            )
        })?;
        Ok(roteiro)
    }

    /// Monta `aula`, `require`, `print` e remove o que o roteiro não deve ter; liga o sandbox.
    fn preparar_globais(&self, modulos: &BTreeMap<String, String>) -> Result<(), ErroLua> {
        let lua = &self.lua;
        let globais = lua.globals();

        for nome in [
            "loadstring",
            "load",
            "loadfile",
            "dofile",
            "require",
            "package",
            "io",
            "os",
            "debug",
            "getfenv",
            "setfenv",
            "newproxy",
        ] {
            globais.raw_set(nome, Value::Nil)?;
        }

        // print -> log do Roteiro
        {
            let log = Arc::clone(&self.log);
            let print = lua.create_function(move |lua, args: mlua::Variadic<Value>| {
                let mut partes = Vec::with_capacity(args.len());
                for v in args.iter() {
                    let texto = match v {
                        Value::Nil => "nil".to_string(),
                        Value::Boolean(b) => b.to_string(),
                        Value::String(s) => s.to_string_lossy().to_string(),
                        outro => lua
                            .coerce_string(outro.clone())?
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| format!("<{}>", outro.type_name())),
                    };
                    partes.push(texto);
                }
                let mut l = travar(&log);
                if l.len() >= LIMITE_LOG {
                    l.remove(0);
                }
                l.push(partes.join("\t"));
                Ok(())
            })?;
            globais.raw_set("print", print)?;
        }

        // require -> só a biblioteca comum do Curso
        {
            let fontes: Arc<BTreeMap<String, String>> = Arc::new(modulos.clone());
            let carregados: Table = lua.create_table()?;
            let em_carga: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let require = lua.create_function(move |lua, nome: String| {
                let Some(codigo) = fontes.get(&nome) else {
                    let disponiveis: Vec<&str> = fontes.keys().map(String::as_str).collect();
                    let lista = if disponiveis.is_empty() {
                        "o Curso não tem módulos comuns".to_string()
                    } else {
                        format!("disponíveis: {}", disponiveis.join(", "))
                    };
                    return Err(ErroLua::runtime(format!(
                        "require(\"{nome}\"): módulo inexistente ({lista})"
                    )));
                };
                let pronto: Value = carregados.raw_get(nome.as_str())?;
                if !matches!(pronto, Value::Nil) {
                    return Ok(pronto);
                }
                {
                    let mut pilha = travar(&em_carga);
                    if pilha.contains(&nome) {
                        return Err(ErroLua::runtime(format!(
                            "require(\"{nome}\"): dependência circular ({})",
                            pilha.join(" -> ")
                        )));
                    }
                    pilha.push(nome.clone());
                }
                let r: Result<Value, ErroLua> = lua
                    .load(codigo.as_str())
                    .set_name(format!("=comum/{nome}"))
                    .eval();
                travar(&em_carga).retain(|n| n != &nome);
                let valor = match r? {
                    Value::Nil => Value::Boolean(true),
                    v => v,
                };
                carregados.raw_set(nome.as_str(), valor.clone())?;
                Ok(valor)
            })?;
            globais.raw_set("require", require)?;
        }

        globais.raw_set("aula", self.criar_api_aula()?)?;
        lua.sandbox(true)?;
        Ok(())
    }

    fn criar_api_aula(&self) -> Result<Table, ErroLua> {
        let lua = &self.lua;
        let aula = lua.create_table()?;
        aula.raw_set("versao_api", VERSAO_API)?;

        let fila = |pedidos: &Arc<Mutex<Vec<Pedido>>>, p: Pedido| -> Result<(), ErroLua> {
            let mut f = travar(pedidos);
            if f.len() >= LIMITE_PEDIDOS {
                return Err(ErroLua::runtime(format!(
                    "pedidos demais numa só chamada (o limite é {LIMITE_PEDIDOS})"
                )));
            }
            f.push(p);
            Ok(())
        };

        let p = Arc::clone(&self.pedidos);
        aula.raw_set(
            "modo",
            lua.create_function(move |_, texto: String| {
                let Some(modo) = ModoAcompanhamento::de_texto(&texto) else {
                    return Err(ErroLua::runtime(format!(
                        "aula.modo: modo «{texto}» desconhecido (use \"nota_a_nota\", \"espera_tocar\", \"segue\" ou \"chamada\")"
                    )));
                };
                fila(&p, Pedido::Modo(modo))
            })?,
        )?;

        let p = Arc::clone(&self.pedidos);
        aula.raw_set(
            "ir",
            lua.create_function(move |_, id: String| {
                if id.trim().is_empty() {
                    return Err(ErroLua::runtime("aula.ir: o id do trecho está vazio"));
                }
                fila(&p, Pedido::Ir(id))
            })?,
        )?;

        let p = Arc::clone(&self.pedidos);
        aula.raw_set(
            "repetir",
            lua.create_function(move |_, ()| fila(&p, Pedido::Repetir))?,
        )?;
        let p = Arc::clone(&self.pedidos);
        aula.raw_set(
            "pausar",
            lua.create_function(move |_, ()| fila(&p, Pedido::Pausar))?,
        )?;
        let p = Arc::clone(&self.pedidos);
        aula.raw_set(
            "continuar",
            lua.create_function(move |_, ()| fila(&p, Pedido::Continuar))?,
        )?;

        let p = Arc::clone(&self.pedidos);
        aula.raw_set(
            "velocidade",
            lua.create_function(move |_, x: f64| {
                if !(0.25..=2.0).contains(&x) {
                    return Err(ErroLua::runtime(format!(
                        "aula.velocidade: {x} está fora da faixa de 0.25 a 2"
                    )));
                }
                fila(&p, Pedido::Velocidade(x))
            })?,
        )?;

        let p = Arc::clone(&self.pedidos);
        aula.raw_set(
            "mensagem",
            lua.create_function(move |_, texto: String| {
                fila(&p, Pedido::Mensagem(limitar_texto("aula.mensagem", texto)?))
            })?,
        )?;
        let p = Arc::clone(&self.pedidos);
        aula.raw_set(
            "dizer",
            lua.create_function(move |_, texto: String| {
                fila(&p, Pedido::Dizer(limitar_texto("aula.dizer", texto)?))
            })?,
        )?;

        let contadores: Arc<Mutex<HashMap<String, i64>>> = Arc::new(Mutex::new(HashMap::new()));
        let c = Arc::clone(&contadores);
        aula.raw_set(
            "contar",
            lua.create_function(move |_, chave: String| {
                let mut m = travar(&c);
                if !m.contains_key(&chave) && m.len() >= 1000 {
                    return Err(ErroLua::runtime("aula.contar: chaves demais (limite 1000)"));
                }
                let v = m.entry(chave).or_insert(0);
                *v = v.saturating_add(1);
                Ok(*v)
            })?,
        )?;
        let c = Arc::clone(&contadores);
        aula.raw_set(
            "contador",
            lua.create_function(move |_, chave: String| {
                Ok(travar(&c).get(&chave).copied().unwrap_or(0))
            })?,
        )?;
        let c = contadores;
        aula.raw_set(
            "zerar",
            lua.create_function(move |_, chave: String| {
                travar(&c).remove(&chave);
                Ok(())
            })?,
        )?;

        Ok(aula)
    }

    /// Linhas que o script escreveu com `print` desde a última chamada (e as esvazia).
    pub fn tirar_log(&mut self) -> Vec<String> {
        std::mem::take(&mut *travar(&self.log))
    }

    /// O script avisa que a Aula foi aberta (`ao_abrir()`).
    pub fn ao_abrir(&mut self) -> Result<Vec<Pedido>, String> {
        self.chamar_com("ao_abrir", |_| Ok(()))
    }

    /// O player entrou num trecho (`ao_entrar_trecho(t)`).
    pub fn ao_entrar_trecho(&mut self, t: &TrechoInfo) -> Result<Vec<Pedido>, String> {
        self.chamar_com("ao_entrar_trecho", |lua| tabela_trecho(lua, t))
    }

    /// O trecho acabou (`ao_fim_trecho(t, {acertos, erros})`).
    pub fn ao_fim_trecho(
        &mut self,
        t: &TrechoInfo,
        acertos: u32,
        erros: u32,
    ) -> Result<Vec<Pedido>, String> {
        self.chamar_com("ao_fim_trecho", |lua| {
            let r = lua.create_table()?;
            r.raw_set("acertos", acertos)?;
            r.raw_set("erros", erros)?;
            Ok((tabela_trecho(lua, t)?, r))
        })
    }

    /// O aluno tocou uma nota (`ao_nota(midi, certa)`).
    pub fn ao_nota(&mut self, midi: u8, certa: bool) -> Result<Vec<Pedido>, String> {
        self.chamar_com("ao_nota", |_| Ok((midi, certa)))
    }

    fn chamar_com<A: mlua::IntoLuaMulti>(
        &mut self,
        nome: &str,
        montar: impl FnOnce(&Lua) -> Result<A, ErroLua>,
    ) -> Result<Vec<Pedido>, String> {
        let valor: Value = self
            .lua
            .globals()
            .raw_get(nome)
            .map_err(|e| format!("Erro no Roteiro de Aula ({nome}): {}", self.explicar(&e)))?;
        let funcao: Function = match valor {
            Value::Nil => return Ok(Vec::new()),
            Value::Function(f) => f,
            outro => {
                return Err(format!(
                    "Erro no Roteiro de Aula: «{nome}» deveria ser uma função, mas é {}.",
                    outro.type_name()
                ));
            }
        };
        travar(&self.pedidos).clear();
        self.orcamento.iniciar();
        let resultado = montar(&self.lua).and_then(|args| funcao.call::<()>(args));
        match resultado {
            Ok(()) => Ok(std::mem::take(&mut *travar(&self.pedidos))),
            Err(e) => {
                travar(&self.pedidos).clear();
                Err(format!(
                    "Erro no Roteiro de Aula ({nome}): {}",
                    self.explicar(&e)
                ))
            }
        }
    }

    /// Texto legível (português, com a linha do script quando há) para um erro do mlua.
    fn explicar(&self, e: &ErroLua) -> String {
        if self.orcamento.estourou.load(Ordering::Relaxed) {
            return format!(
                "o Roteiro demorou demais e foi interrompido (limite de {} ms por chamada); \
                 confira se há um laço sem fim.",
                LIMITE_TEMPO.as_millis()
            );
        }
        explicar_erro(e)
    }
}

fn tabela_trecho(lua: &Lua, t: &TrechoInfo) -> Result<Table, ErroLua> {
    let tab = lua.create_table()?;
    tab.raw_set("id", t.id.as_str())?;
    tab.raw_set("titulo", t.titulo.as_str())?;
    tab.raw_set("inicio", t.inicio_s)?;
    tab.raw_set("fim", t.fim_s)?;
    tab.raw_set("indice", t.indice + 1)?;
    Ok(tab)
}

fn limitar_texto(funcao: &str, texto: String) -> Result<String, ErroLua> {
    let n = texto.chars().count();
    if n > LIMITE_TEXTO {
        return Err(ErroLua::runtime(format!(
            "{funcao}: texto com {n} caracteres; o limite é {LIMITE_TEXTO}"
        )));
    }
    Ok(texto)
}

/// Tira o traceback e traduz o que for do próprio mlua; mantém «roteiro:LINHA:» quando existir.
fn explicar_erro(e: &ErroLua) -> String {
    match e {
        ErroLua::MemoryError(_) => format!(
            "o Roteiro usou memória demais (limite de {} MB).",
            LIMITE_MEMORIA / (1024 * 1024)
        ),
        ErroLua::CallbackError { traceback, cause } => {
            let causa = explicar_erro(cause);
            // Erros levantados por funções do Rust (aula.*) não trazem linha; busca no traceback.
            if tem_posicao(&causa) {
                causa
            } else if let Some(pos) = posicao_do_traceback(traceback) {
                format!("{pos}: {causa}")
            } else {
                causa
            }
        }
        ErroLua::RuntimeError(msg) => {
            if msg.contains("not enough memory") {
                return format!(
                    "o Roteiro usou memória demais (limite de {} MB).",
                    LIMITE_MEMORIA / (1024 * 1024)
                );
            }
            limpar_traceback(msg)
        }
        ErroLua::SyntaxError { message, .. } => {
            format!("erro de sintaxe: {}", limpar_traceback(message))
        }
        outro => limpar_traceback(&outro.to_string()),
    }
}

fn limpar_traceback(msg: &str) -> String {
    msg.split("\nstack traceback:")
        .next()
        .unwrap_or(msg)
        .trim()
        .to_string()
}

/// A mensagem já começa com `nome:linha:`?
fn tem_posicao(msg: &str) -> bool {
    let mut partes = msg.splitn(3, ':');
    match (partes.next(), partes.next(), partes.next()) {
        (Some(nome), Some(linha), Some(_)) => {
            !nome.is_empty()
                && nome
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_-/".contains(c))
                && !linha.is_empty()
                && linha.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

/// Primeira linha do traceback que aponta para código do script (não `[C]`): «roteiro:12».
fn posicao_do_traceback(tb: &str) -> Option<String> {
    tb.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("[C]") && !l.starts_with("stack traceback"))
        .and_then(|l| l.split_whitespace().next())
        .map(|p| p.trim_end_matches(':'))
        .filter(|p| p.contains(':'))
        .map(str::to_string)
}

/// Um nome de módulo ou de Lição é seguro (`[a-z0-9_-]`, sem `/` nem `..`)?
fn nome_seguro(nome: &str) -> bool {
    !nome.is_empty()
        && nome.len() <= 64
        && nome
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Lê o Roteiro da Lição `licao_id` na pasta do Curso:
/// `<pasta>/roteiros-aula/<licao_id>.luau`, com os módulos comuns de
/// `<pasta>/roteiros-aula/comum/*.luau` (nome do módulo = nome do arquivo sem extensão).
///
/// `None` se a Lição não tem Roteiro (sem arquivo). `Some(Err)` se o id ou algum nome de módulo é
/// recusado (`/`, `..` ou fora de `[a-z0-9_-]`), se um arquivo não pode ser lido ou se o script
/// não carrega.
pub fn ler_roteiro_da_pasta(pasta: &Path, licao_id: &str) -> Option<Result<RoteiroAula, String>> {
    if licao_id.contains('/') || licao_id.contains('\\') || licao_id.contains("..") {
        return Some(Err(format!("Id de Lição recusado: «{licao_id}»")));
    }
    // Id fora do padrão simplesmente não tem arquivo seguro de roteiro: recusa explícita.
    let dir = pasta.join("roteiros-aula");
    if !nome_seguro(licao_id) {
        return Some(Err(format!(
            "Id de Lição recusado para Roteiro (use só a-z, 0-9, _ e -): «{licao_id}»"
        )));
    }
    let arquivo = dir.join(format!("{licao_id}.luau"));
    if !arquivo.is_file() {
        return None;
    }
    Some(ler_roteiro_existente(&dir, &arquivo))
}

fn ler_roteiro_existente(dir: &Path, arquivo: &Path) -> Result<RoteiroAula, String> {
    let principal = ler_texto(arquivo)?;
    let mut modulos = BTreeMap::new();
    let comum = dir.join("comum");
    if comum.is_dir() {
        let entradas = std::fs::read_dir(&comum)
            .map_err(|e| format!("Não consegui listar {}: {e}", comum.display()))?;
        for entrada in entradas {
            let entrada =
                entrada.map_err(|e| format!("Erro ao listar {}: {e}", comum.display()))?;
            let caminho = entrada.path();
            if caminho.extension().and_then(|e| e.to_str()) != Some("luau") {
                continue;
            }
            let nome = caminho
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            if !nome_seguro(&nome) {
                return Err(format!(
                    "Nome de módulo recusado: «{}» (use só a-z, 0-9, _ e -)",
                    caminho
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                ));
            }
            // Só arquivos comuns: atalhos (symlink) para fora da pasta do Curso não entram.
            let meta = std::fs::symlink_metadata(&caminho)
                .map_err(|e| format!("Não consegui ler {}: {e}", caminho.display()))?;
            if !meta.is_file() {
                return Err(format!(
                    "O módulo «{nome}» não é um arquivo comum (atalhos não são aceitos)."
                ));
            }
            modulos.insert(nome, ler_texto(&caminho)?);
        }
    }
    RoteiroAula::carregar(&principal, &modulos)
}

fn ler_texto(caminho: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(caminho)
        .map_err(|e| format!("Não consegui ler {}: {e}", caminho.display()))?;
    if meta.len() > LIMITE_CODIGO as u64 {
        return Err(format!(
            "{} tem {} bytes; o limite é {LIMITE_CODIGO}.",
            caminho.display(),
            meta.len()
        ));
    }
    std::fs::read_to_string(caminho)
        .map_err(|e| format!("Não consegui ler {}: {e}", caminho.display()))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn sem_modulos() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn trecho(id: &str, indice: usize) -> TrechoInfo {
        TrechoInfo {
            id: id.to_string(),
            titulo: format!("Trecho {id}"),
            inicio_s: 10.0,
            fim_s: 20.5,
            indice,
        }
    }

    fn carregar(codigo: &str) -> RoteiroAula {
        RoteiroAula::carregar(codigo, &sem_modulos()).expect("o roteiro deve carregar")
    }

    const FLUXO: &str = r#"
        function ao_abrir()
            aula.mensagem("Bem-vindo")
        end
        function ao_entrar_trecho(t)
            aula.modo("chamada")
            aula.dizer("Vamos ao trecho " .. t.titulo)
        end
        function ao_fim_trecho(t, r)
            if r.erros > 0 then
                aula.repetir()
            else
                aula.ir("proximo")
            end
        end
    "#;

    #[test]
    fn fluxo_tipico_pede_modo_repetir_e_avancar() {
        let mut r = carregar(FLUXO);
        assert_eq!(r.ao_abrir(), Ok(vec![Pedido::Mensagem("Bem-vindo".into())]));
        let t = trecho("a", 0);
        assert_eq!(
            r.ao_entrar_trecho(&t),
            Ok(vec![
                Pedido::Modo(ModoAcompanhamento::ChamadaEResposta),
                Pedido::Dizer("Vamos ao trecho Trecho a".into()),
            ])
        );
        assert_eq!(r.ao_fim_trecho(&t, 3, 2), Ok(vec![Pedido::Repetir]));
        assert_eq!(
            r.ao_fim_trecho(&t, 5, 0),
            Ok(vec![Pedido::Ir("proximo".into())])
        );
    }

    #[test]
    fn campos_do_trecho_chegam_ao_script() {
        let mut r = carregar(
            r#"
            function ao_entrar_trecho(t)
                aula.mensagem(t.id .. "|" .. t.titulo .. "|" .. t.inicio .. "|" .. t.fim .. "|" .. t.indice)
            end
            function ao_fim_trecho(t, r)
                aula.mensagem(r.acertos .. "/" .. r.erros)
            end
            function ao_nota(midi, certa)
                if certa then aula.mensagem("certa " .. midi) else aula.mensagem("errada " .. midi) end
            end
            "#,
        );
        let t = trecho("x", 2);
        assert_eq!(
            r.ao_entrar_trecho(&t),
            Ok(vec![Pedido::Mensagem("x|Trecho x|10|20.5|3".into())])
        );
        assert_eq!(
            r.ao_fim_trecho(&t, 7, 1),
            Ok(vec![Pedido::Mensagem("7/1".into())])
        );
        assert_eq!(
            r.ao_nota(60, true),
            Ok(vec![Pedido::Mensagem("certa 60".into())])
        );
        assert_eq!(
            r.ao_nota(61, false),
            Ok(vec![Pedido::Mensagem("errada 61".into())])
        );
    }

    #[test]
    fn de_trecho_converte_do_player() {
        let t = Trecho {
            exercicio_id: "cinco-notas".into(),
            titulo: "Cinco notas".into(),
            inicio_s: 1.0,
            fim_s: 2.0,
        };
        let i = TrechoInfo::de_trecho(&t, 4);
        assert_eq!(i.id, "cinco-notas");
        assert_eq!(i.indice, 4);
        assert_eq!(i.fim_s, 2.0);
    }

    #[test]
    fn callback_ausente_devolve_vazio() {
        let mut r = carregar("local x = 1");
        assert_eq!(r.ao_abrir(), Ok(vec![]));
        assert_eq!(r.ao_entrar_trecho(&trecho("a", 0)), Ok(vec![]));
        assert_eq!(r.ao_fim_trecho(&trecho("a", 0), 1, 1), Ok(vec![]));
        assert_eq!(r.ao_nota(60, true), Ok(vec![]));
    }

    #[test]
    fn callback_que_nao_e_funcao_da_erro() {
        let mut r = carregar("ao_abrir = 5");
        let e = r.ao_abrir().expect_err("não é função");
        assert!(e.contains("ao_abrir") && e.contains("função"), "{e}");
    }

    #[test]
    fn require_da_biblioteca_comum_funciona() {
        let mut mods = BTreeMap::new();
        mods.insert(
            "basico".to_string(),
            r#"
            local M = {}
            function M.elogio() return "Muito bem!" end
            return M
            "#
            .to_string(),
        );
        mods.insert(
            "outro".to_string(),
            r#"local b = require("basico"); return { dobro = function(x) return x * 2 end, e = b.elogio() }"#
                .to_string(),
        );
        let mut r = RoteiroAula::carregar(
            r#"
            local basico = require("basico")
            local outro = require("outro")
            function ao_abrir()
                aula.dizer(basico.elogio() .. " " .. outro.dobro(21) .. " " .. outro.e)
            end
            "#,
            &mods,
        )
        .expect("carrega");
        assert_eq!(
            r.ao_abrir(),
            Ok(vec![Pedido::Dizer("Muito bem! 42 Muito bem!".into())])
        );
    }

    #[test]
    fn require_e_cacheado_e_roda_o_modulo_uma_vez() {
        let mut mods = BTreeMap::new();
        mods.insert(
            "m".to_string(),
            "return { n = aula.contar('carga') }".to_string(),
        );
        let mut r = RoteiroAula::carregar(
            r#"
            local a = require("m")
            local b = require("m")
            function ao_abrir()
                aula.mensagem(a.n .. " " .. b.n .. " " .. tostring(a == b) .. " " .. aula.contador("carga"))
            end
            "#,
            &mods,
        )
        .expect("carrega");
        assert_eq!(
            r.ao_abrir(),
            Ok(vec![Pedido::Mensagem("1 1 true 1".into())])
        );
    }

    #[test]
    fn require_inexistente_falha_ao_carregar() {
        let e = RoteiroAula::carregar(r#"local x = require("nao-existe")"#, &sem_modulos())
            .expect_err("deve falhar");
        assert!(e.contains("módulo inexistente"), "{e}");
        assert!(e.contains("roteiro:1"), "deve citar a linha: {e}");
    }

    #[test]
    fn require_circular_falha() {
        let mut mods = BTreeMap::new();
        mods.insert("a".to_string(), "return require('b')".to_string());
        mods.insert("b".to_string(), "return require('a')".to_string());
        let e = RoteiroAula::carregar("require('a')", &mods).expect_err("circular");
        assert!(e.contains("circular"), "{e}");
    }

    #[test]
    fn require_de_caminho_de_arquivo_indisponivel() {
        let mut mods = BTreeMap::new();
        mods.insert("basico".to_string(), "return {}".to_string());
        for caminho in [
            "./basico",
            "../basico",
            "comum/basico",
            "/etc/passwd",
            "@self/basico",
            "basico.luau",
            "C:\\x",
        ] {
            let codigo = format!("require({caminho:?})");
            let e = RoteiroAula::carregar(&codigo, &mods).expect_err(caminho);
            assert!(e.contains("módulo inexistente"), "{caminho}: {e}");
        }
    }

    #[test]
    fn estado_persiste_entre_chamadas() {
        let mut r = carregar(
            r#"
            local tentativas = 0
            visitas = 0
            function ao_fim_trecho(t, res)
                tentativas = tentativas + 1
                visitas = visitas + 1
                local c = aula.contar("fins")
                aula.mensagem(tentativas .. "," .. visitas .. "," .. c .. "," .. aula.contador("fins"))
            end
            function ao_nota(m, c)
                aula.mensagem("fins=" .. aula.contador("fins") .. " outro=" .. aula.contador("outro"))
            end
            "#,
        );
        let t = trecho("a", 0);
        assert_eq!(
            r.ao_fim_trecho(&t, 0, 0),
            Ok(vec![Pedido::Mensagem("1,1,1,1".into())])
        );
        assert_eq!(
            r.ao_fim_trecho(&t, 0, 0),
            Ok(vec![Pedido::Mensagem("2,2,2,2".into())])
        );
        assert_eq!(
            r.ao_nota(60, true),
            Ok(vec![Pedido::Mensagem("fins=2 outro=0".into())])
        );
    }

    #[test]
    fn zerar_contador() {
        let mut r = carregar(
            r#"
            function ao_abrir()
                aula.contar("x"); aula.contar("x")
                aula.zerar("x")
                aula.mensagem(tostring(aula.contador("x")))
            end
            "#,
        );
        assert_eq!(r.ao_abrir(), Ok(vec![Pedido::Mensagem("0".into())]));
    }

    #[test]
    fn io_os_debug_e_carregadores_indisponiveis() {
        let mut r = carregar(
            r#"
            local nomes = { "io", "os", "debug", "package", "loadstring", "load", "loadfile", "dofile", "getfenv", "setfenv" }
            local presentes = {}
            for _, n in ipairs(nomes) do
                if _G[n] ~= nil then table.insert(presentes, n) end
            end
            function ao_abrir()
                aula.mensagem("presentes=" .. table.concat(presentes, ","))
            end
            "#,
        );
        assert_eq!(
            r.ao_abrir(),
            Ok(vec![Pedido::Mensagem("presentes=".into())])
        );
        for codigo in [
            "io.open('/etc/passwd')",
            "os.execute('true')",
            "os.getenv('HOME')",
            "loadfile('/etc/passwd')",
            "dofile('/etc/passwd')",
            "loadstring('return 1')()",
            "load('return 1')()",
            "debug.traceback()",
        ] {
            let e = RoteiroAula::carregar(codigo, &sem_modulos()).expect_err(codigo);
            assert!(e.contains("roteiro:1"), "{codigo}: {e}");
        }
    }

    #[test]
    fn api_aula_e_somente_leitura() {
        for codigo in [
            "aula.modo = function() end",
            "aula.nova = 1",
            "aula.versao_api = 2",
            "string.rep = nil",
        ] {
            let e = RoteiroAula::carregar(codigo, &sem_modulos()).expect_err(codigo);
            assert!(e.contains("roteiro:1"), "{codigo}: {e}");
        }
        let mut r = carregar("function ao_abrir() aula.mensagem(aula.versao_api) end");
        assert_eq!(r.ao_abrir(), Ok(vec![Pedido::Mensagem("1".into())]));
    }

    #[test]
    fn laco_infinito_e_interrompido_e_o_roteiro_continua() {
        let mut r = carregar(
            r#"
            function ao_abrir() while true do end end
            function ao_nota(m, c) aula.mensagem("vivo") end
            function ao_entrar_trecho(t)
                while true do pcall(function() while true do end end) end
            end
            function ao_fim_trecho(t, res)
                local n = 0
                repeat n = n + 1 until false
            end
            "#,
        );
        for _ in 0..2 {
            let ini = Instant::now();
            let e = r.ao_abrir().expect_err("deve interromper");
            assert!(e.contains("demorou demais"), "{e}");
            assert!(
                ini.elapsed() < Duration::from_secs(2),
                "demorou {:?}",
                ini.elapsed()
            );
            assert_eq!(
                r.ao_nota(60, true),
                Ok(vec![Pedido::Mensagem("vivo".into())])
            );
        }
        // pcall dentro do laço não escapa do orçamento
        let e = r.ao_entrar_trecho(&trecho("a", 0)).expect_err("pcall");
        assert!(e.contains("demorou demais"), "{e}");
        let e = r.ao_fim_trecho(&trecho("a", 0), 0, 0).expect_err("repeat");
        assert!(e.contains("demorou demais"), "{e}");
        assert_eq!(
            r.ao_nota(62, false),
            Ok(vec![Pedido::Mensagem("vivo".into())])
        );
    }

    #[test]
    fn laco_infinito_no_corpo_do_script_falha_ao_carregar() {
        let e = RoteiroAula::carregar("while true do end", &sem_modulos()).expect_err("laço");
        assert!(e.contains("demorou demais"), "{e}");
    }

    #[test]
    fn estouro_de_memoria_e_contido() {
        let mut r = carregar(
            r#"
            function ao_abrir()
                local s = "x"
                while true do s = s .. s end
            end
            function ao_entrar_trecho(t)
                local tabela = {}
                for i = 1, 1e9 do tabela[i] = { i, i, i } end
            end
            function ao_nota(m, c) aula.mensagem("vivo") end
            "#,
        );
        let e = r.ao_abrir().expect_err("memória");
        // Com a máquina carregada, o orçamento de tempo pode vencer antes da memória.
        assert!(
            e.contains("memória demais") || e.contains("demorou demais"),
            "{e}"
        );
        assert_eq!(
            r.ao_nota(60, true),
            Ok(vec![Pedido::Mensagem("vivo".into())])
        );
        let e = r.ao_entrar_trecho(&trecho("a", 0)).expect_err("memória 2");
        assert!(
            e.contains("memória demais") || e.contains("demorou demais"),
            "{e}"
        );
        assert_eq!(
            r.ao_nota(60, true),
            Ok(vec![Pedido::Mensagem("vivo".into())])
        );
    }

    #[test]
    fn velocidade_fora_da_faixa_da_erro() {
        let mut r = carregar(
            r#"
            function ao_entrar_trecho(t) aula.velocidade(t.indice) end
            function ao_abrir() aula.velocidade(0.5); aula.velocidade(0.25); aula.velocidade(2) end
            function ao_nota(m, c) aula.velocidade(0.24) end
            function ao_fim_trecho(t, r) aula.velocidade(0 / 0) end
            "#,
        );
        assert_eq!(
            r.ao_abrir(),
            Ok(vec![
                Pedido::Velocidade(0.5),
                Pedido::Velocidade(0.25),
                Pedido::Velocidade(2.0)
            ])
        );
        let e = r.ao_nota(60, true).expect_err("0.24");
        assert!(e.contains("fora da faixa"), "{e}");
        assert!(
            e.contains("roteiro:4: aula.velocidade"),
            "deve citar a linha: {e}"
        );
        let e = r.ao_entrar_trecho(&trecho("a", 2)).expect_err("3");
        assert!(e.contains("fora da faixa"), "{e}");
        let e = r.ao_fim_trecho(&trecho("a", 0), 0, 0).expect_err("nan");
        assert!(e.contains("fora da faixa"), "{e}");
        // um bom pedido depois do erro: a fila do erro foi descartada
        assert_eq!(
            r.ao_entrar_trecho(&trecho("a", 1)),
            Ok(vec![Pedido::Velocidade(2.0)])
        );
    }

    #[test]
    fn modos_pelo_nome_novo_e_pelo_antigo() {
        let mut r = carregar(
            r#"
            function ao_abrir()
                aula.modo("nota_a_nota")
                aula.modo("espera_tocar")
                aula.modo("espera")
            end
            "#,
        );
        assert_eq!(
            r.ao_abrir(),
            Ok(vec![
                Pedido::Modo(ModoAcompanhamento::NotaANota),
                Pedido::Modo(ModoAcompanhamento::EsperaTocar),
                // O nome antigo "espera" é o Nota a nota.
                Pedido::Modo(ModoAcompanhamento::NotaANota),
            ])
        );
    }

    #[test]
    fn modo_desconhecido_e_argumentos_invalidos_dao_erro() {
        let mut r = carregar(
            r#"
            function ao_abrir() aula.modo("turbo") end
            function ao_nota(m, c) aula.ir("") end
            function ao_entrar_trecho(t) aula.mensagem(string.rep("a", 3000)) end
            function ao_fim_trecho(t, r) aula.dizer({}) end
            "#,
        );
        assert!(r.ao_abrir().expect_err("modo").contains("desconhecido"));
        assert!(r.ao_nota(1, true).expect_err("ir").contains("vazio"));
        assert!(
            r.ao_entrar_trecho(&trecho("a", 0))
                .expect_err("texto")
                .contains("limite")
        );
        assert!(r.ao_fim_trecho(&trecho("a", 0), 0, 0).is_err());
    }

    #[test]
    fn pedidos_demais_numa_chamada_da_erro() {
        let mut r = carregar("function ao_abrir() for i = 1, 1000 do aula.pausar() end end");
        let e = r.ao_abrir().expect_err("limite");
        assert!(e.contains("pedidos demais"), "{e}");
        assert!(r.ao_abrir().is_err());
    }

    #[test]
    fn erro_em_callback_cita_a_linha_e_o_roteiro_segue() {
        let mut r = carregar(
            "local ok = true\nfunction ao_abrir()\n  local x = nil\n  return x.campo\nend\nfunction ao_nota(m, c)\n  aula.pausar()\nend\n",
        );
        let e = r.ao_abrir().expect_err("nil");
        assert!(e.contains("ao_abrir") && e.contains("roteiro:4"), "{e}");
        assert!(!e.contains("stack traceback"), "{e}");
        assert_eq!(r.ao_nota(60, true), Ok(vec![Pedido::Pausar]));
    }

    #[test]
    fn erro_de_sintaxe_ao_carregar_cita_a_linha() {
        let e = RoteiroAula::carregar("local x = 1\nlocal = = 2\n", &sem_modulos())
            .expect_err("sintaxe");
        assert!(e.contains("roteiro:2"), "{e}");
    }

    #[test]
    fn print_vai_para_o_log() {
        let mut r = carregar("function ao_abrir() print('olá', 42, true) end");
        assert_eq!(r.ao_abrir(), Ok(vec![]));
        assert_eq!(r.tirar_log(), vec!["olá\t42\ttrue".to_string()]);
        assert!(r.tirar_log().is_empty());
    }

    #[test]
    fn pedidos_do_corpo_do_script_sao_descartados() {
        let mut r = carregar("aula.pausar()\nfunction ao_abrir() aula.continuar() end");
        assert_eq!(r.ao_abrir(), Ok(vec![Pedido::Continuar]));
    }

    #[test]
    fn script_grande_demais_e_recusado() {
        let grande = "-- ".to_string() + &"a".repeat(LIMITE_CODIGO);
        assert!(RoteiroAula::carregar(&grande, &sem_modulos()).is_err());
    }

    #[test]
    fn nomes_seguros() {
        assert!(nome_seguro("s03-a14"));
        assert!(nome_seguro("basico_2"));
        for ruim in ["", "../x", "a/b", "A", "a.b", "a b", "a\\b"] {
            assert!(!nome_seguro(ruim), "{ruim}");
        }
    }

    fn pasta_com(arquivos: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().expect("tmp");
        for (rel, conteudo) in arquivos {
            let p = d.path().join(rel);
            std::fs::create_dir_all(p.parent().expect("pai")).expect("mkdir");
            std::fs::write(p, conteudo).expect("escreve");
        }
        d
    }

    #[test]
    fn le_roteiro_da_pasta_com_modulos_comuns() {
        let d = pasta_com(&[
            (
                "roteiros-aula/a1.luau",
                "local b = require('util')\nfunction ao_abrir() aula.dizer(b.oi) end",
            ),
            ("roteiros-aula/comum/util.luau", "return { oi = 'oi' }"),
            ("roteiros-aula/comum/LEIAME.md", "ignorado"),
        ]);
        let mut r = ler_roteiro_da_pasta(d.path(), "a1")
            .expect("existe")
            .expect("carrega");
        assert_eq!(r.ao_abrir(), Ok(vec![Pedido::Dizer("oi".into())]));
    }

    #[test]
    fn pasta_sem_roteiro_da_none() {
        let d = pasta_com(&[("roteiros-aula/a1.luau", "")]);
        assert!(ler_roteiro_da_pasta(d.path(), "a2").is_none());
        assert!(ler_roteiro_da_pasta(&d.path().join("nao-existe"), "a1").is_none());
    }

    #[test]
    fn pasta_recusa_ids_e_nomes_perigosos() {
        let d = pasta_com(&[
            ("roteiros-aula/a1.luau", ""),
            ("segredo.luau", ""),
            ("roteiros-aula/comum/Ruim.luau", "return 1"),
        ]);
        for id in ["../segredo", "a/b", "..", "A1", "a.b", ""] {
            let r = ler_roteiro_da_pasta(d.path(), id).expect(id);
            assert!(r.is_err(), "{id} deveria ser recusado");
        }
        let e = ler_roteiro_da_pasta(d.path(), "a1")
            .expect("existe")
            .expect_err("módulo com nome ruim");
        assert!(e.contains("Ruim.luau"), "{e}");
    }

    #[test]
    fn exemplo_da_aula_14_cinco_notas() {
        let pasta = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/exemplos");
        let mut r = ler_roteiro_da_pasta(&pasta, "s03-a14")
            .expect("o exemplo existe em docs/exemplos/roteiros-aula/s03-a14.luau")
            .expect("o exemplo carrega");
        let t = TrechoInfo {
            id: "cinco-notas".into(),
            titulo: "Cinco notas".into(),
            inicio_s: 0.0,
            fim_s: 30.0,
            indice: 0,
        };
        // Abrir não pede nada; entrar no primeiro trecho pede o modo Nota a nota.
        assert_eq!(r.ao_abrir(), Ok(vec![]));
        let entrada = r.ao_entrar_trecho(&t).expect("entra");
        assert!(
            entrada.contains(&Pedido::Modo(ModoAcompanhamento::NotaANota)),
            "{entrada:?}"
        );

        // Com erros: repete até 3 vezes e depois sugere o modo segue.
        for tentativa in 1..=3 {
            let p = r.ao_fim_trecho(&t, 3, 2).expect("fim com erros");
            assert!(p.contains(&Pedido::Repetir), "tentativa {tentativa}: {p:?}");
            assert!(!p.iter().any(|x| matches!(x, Pedido::Modo(_))), "{p:?}");
        }
        let p = r.ao_fim_trecho(&t, 3, 2).expect("depois de 3 repetições");
        assert!(!p.contains(&Pedido::Repetir), "{p:?}");
        assert!(
            p.contains(&Pedido::Modo(ModoAcompanhamento::SegueNoTempo)),
            "{p:?}"
        );
        assert!(p.iter().any(|x| matches!(x, Pedido::Mensagem(_))), "{p:?}");

        // Sem erros: elogio falado e mensagem para seguir; não repete.
        let p = r.ao_fim_trecho(&t, 5, 0).expect("fim sem erros");
        assert!(p.iter().any(|x| matches!(x, Pedido::Dizer(_))), "{p:?}");
        assert!(p.iter().any(|x| matches!(x, Pedido::Mensagem(_))), "{p:?}");
        assert!(!p.contains(&Pedido::Repetir), "{p:?}");

        // Em outro trecho, não força o modo Nota a nota.
        let outro = TrechoInfo {
            indice: 1,
            id: "outro".into(),
            ..t.clone()
        };
        assert_eq!(r.ao_entrar_trecho(&outro), Ok(vec![]));
    }
}
