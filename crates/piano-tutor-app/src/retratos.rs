//! Retratos das opções de cada tela para o perfil (ADR-0010): `retrato_*` tira o texto atual e
//! `aplicar_*` devolve um texto gravado ao estado. O app compara os retratos a cada quadro e
//! grava na hora o que mudou.
//!
//! Já gravadas por conta própria: som e timbres (`som`), dedilhado (`exibicao`), partitura
//! (`partitura`), tema/janela (`casca`), configuração dos exercícios (`progresso.txt`) e a
//! calibração (`teclado.txt`).

use crate::acordes::{Conjunto, EstadoAcordes, FonteTreino, Modo, tonalidades_disponiveis};
use crate::espelho::EstadoEspelho;
use crate::gravacao::EstadoGravacao;
use crate::opcoes::Kv;
use crate::ritmo::EstadoRitmo;
use crate::teclado_visual::Rotulos;
use crate::tela_partitura::EstadoPartitura;
use crate::treino_andamento::Ordem;
use piano_tutor_core::pratica::ModoPratica;
use piano_tutor_core::ritmo::{Padrao, Subdivisao};
use piano_tutor_core::teclado::{Acidente, ConvencaoOitava, Idioma};
use piano_tutor_core::teoria::{ClasseAltura, Qualidade, TipoEscala, Tonalidade};

/// Seções gravadas por retrato.
pub mod secoes {
    pub const TECLADO: &str = "tela.teclado";
    pub const ACORDES: &str = "tela.acordes";
    pub const RITMOS: &str = "tela.ritmos";
    pub const RITMOS_PADRAO: &str = "tela.ritmos.padrao";
    pub const GRAVACAO: &str = "tela.gravacao";
    pub const PARTITURA: &str = "tela.partitura";
    pub const TODAS: [&str; 6] = [TECLADO, ACORDES, RITMOS, RITMOS_PADRAO, GRAVACAO, PARTITURA];
}

const ROTULOS: [Rotulos; 4] = [
    Rotulos::Nenhum,
    Rotulos::SoGuias,
    Rotulos::SoDo,
    Rotulos::Todas,
];

// --- Teclado (nomes, zoom, ajustar à largura, rótulos, pulso) ------------------------------

pub fn retrato_teclado(e: &EstadoEspelho) -> String {
    let o = e.opcoes_tela;
    Kv::novo()
        .por_enum("idioma", e.nomes.idioma)
        .por_enum("acidente", e.nomes.acidente)
        .por_enum("convencao", e.nomes.convencao)
        .por("zoom", e.zoom())
        .por("ajustar_a_largura", o.ajustar_a_largura)
        .por_enum("rotulos", o.rotulos)
        .por("pulso", o.pulso)
        .texto()
}

pub fn aplicar_teclado(e: &mut EstadoEspelho, t: &str) {
    let kv = Kv::de_texto(t);
    let padrao = EstadoEspelho::novo();
    e.nomes = padrao.nomes;
    e.opcoes_tela = padrao.opcoes_tela;
    kv.em_enum(
        "idioma",
        &[Idioma::PtBr, Idioma::Internacional],
        &mut e.nomes.idioma,
    );
    kv.em_enum(
        "acidente",
        &[Acidente::Sustenido, Acidente::Bemol],
        &mut e.nomes.acidente,
    );
    kv.em_enum(
        "convencao",
        &[ConvencaoOitava::Do4, ConvencaoOitava::Do3],
        &mut e.nomes.convencao,
    );
    e.definir_zoom(kv.ler("zoom").unwrap_or(1.0));
    kv.em("ajustar_a_largura", &mut e.opcoes_tela.ajustar_a_largura);
    kv.em_enum("rotulos", &ROTULOS, &mut e.opcoes_tela.rotulos);
    kv.em("pulso", &mut e.opcoes_tela.pulso);
}

// --- Acordes e escalas -----------------------------------------------------------------------

fn tonalidades() -> Vec<Option<Tonalidade>> {
    std::iter::once(None)
        .chain(tonalidades_disponiveis().into_iter().map(Some))
        .collect()
}

pub fn retrato_acordes(a: &EstadoAcordes) -> String {
    let c = &a.treino.config;
    Kv::novo()
        .por_enum("modo", a.modo)
        .por_enum("tonalidade", a.tonalidade)
        .por("fundamental", a.fundamental.valor())
        .por_enum("qualidade", a.qualidade)
        .por("inversao", a.inversao)
        .por("base", a.base)
        .por_enum("escala", a.escala)
        .por_enum("conjunto", a.conjunto)
        .por("de_ouvido", a.de_ouvido)
        .por("mostrar_dica", a.mostrar_dica)
        .por_enum("fonte_treino", a.fonte_treino)
        .por_enum("treino.ordem", c.ordem)
        .por("treino.app_toca", c.app_toca)
        .por("treino.compassos_por_item", c.compassos_por_item)
        .por("treino.contagem", c.contagem)
        .por_enum("treino.voltas", c.voltas)
        .por("treino.mostrar_teclas", c.mostrar_teclas)
        .por("treino.tolerancia_ms", c.tolerancia_ms)
        .texto()
}

pub fn aplicar_acordes(a: &mut EstadoAcordes, t: &str) {
    let kv = Kv::de_texto(t);
    let p = EstadoAcordes::default();
    kv.em_enum(
        "modo",
        &[Modo::Dicionario, Modo::Escalas, Modo::Desafio, Modo::Treino],
        &mut a.modo,
    );
    a.tonalidade = kv
        .ler_enum("tonalidade", &tonalidades())
        .unwrap_or(p.tonalidade);
    a.fundamental = kv
        .ler::<u8>("fundamental")
        .filter(|v| *v < 12)
        .map_or(p.fundamental, ClasseAltura::nova);
    a.qualidade = kv
        .ler_enum("qualidade", &Qualidade::TODAS)
        .unwrap_or(p.qualidade);
    a.inversao = kv.ler::<usize>("inversao").filter(|i| *i < 5).unwrap_or(0);
    a.base = kv
        .ler::<u8>("base")
        .filter(|b| (36..=96).contains(b))
        .unwrap_or(p.base);
    a.escala = kv
        .ler_enum("escala", &TipoEscala::TODAS)
        .unwrap_or(p.escala);
    a.conjunto = kv
        .ler_enum("conjunto", &Conjunto::TODOS)
        .unwrap_or(p.conjunto);
    a.de_ouvido = kv.ler("de_ouvido").unwrap_or(p.de_ouvido);
    a.mostrar_dica = kv.ler("mostrar_dica").unwrap_or(p.mostrar_dica);
    a.fonte_treino = kv
        .ler_enum("fonte_treino", &FonteTreino::todas())
        .unwrap_or(p.fonte_treino);
    let pc = p.treino.config;
    let c = &mut a.treino.config;
    c.ordem = kv
        .ler_enum("treino.ordem", &[Ordem::Sequencial, Ordem::Aleatoria])
        .unwrap_or(pc.ordem);
    c.app_toca = kv.ler("treino.app_toca").unwrap_or(pc.app_toca);
    c.compassos_por_item = kv
        .ler::<u8>("treino.compassos_por_item")
        .filter(|v| (1..=4).contains(v))
        .unwrap_or(pc.compassos_por_item);
    c.contagem = kv
        .ler::<u8>("treino.contagem")
        .filter(|v| *v <= 4)
        .unwrap_or(pc.contagem);
    let voltas: Vec<Option<u32>> = std::iter::once(None).chain((1..=100).map(Some)).collect();
    c.voltas = kv.ler_enum("treino.voltas", &voltas).unwrap_or(pc.voltas);
    c.mostrar_teclas = kv.ler("treino.mostrar_teclas").unwrap_or(pc.mostrar_teclas);
    c.tolerancia_ms = kv
        .ler::<u32>("treino.tolerancia_ms")
        .filter(|v| (20..=2000).contains(v))
        .unwrap_or(pc.tolerancia_ms);
}

// --- Ritmos ----------------------------------------------------------------------------------

/// Retrato das opções de Ritmos. Atualiza a lembrança do exercício de pulso enquanto ele existe.
pub fn retrato_ritmos(r: &mut EstadoRitmo) -> String {
    if let Some(e) = &r.exercicio {
        r.pulso_salvo = (e.tolerancia_ms, e.compensacao_ms);
    }
    Kv::novo()
        .por("metronomo", r.config.metronomo)
        .por("bateria", r.config.bateria)
        .por_enum("subdivisao", r.config.subdivisao)
        .por("contagem", r.config.contagem)
        .por("volume", r.volume)
        .por("pulso.tolerancia_ms", r.pulso_salvo.0)
        .por("pulso.compensacao_ms", r.pulso_salvo.1)
        .texto()
}

pub fn aplicar_ritmos(r: &mut EstadoRitmo, t: &str) {
    let kv = Kv::de_texto(t);
    let p = EstadoRitmo::novo(None);
    r.config = p.config;
    kv.em("metronomo", &mut r.config.metronomo);
    kv.em("bateria", &mut r.config.bateria);
    kv.em_enum("subdivisao", &Subdivisao::TODAS, &mut r.config.subdivisao);
    r.config.contagem = kv.ler::<u8>("contagem").filter(|c| *c <= 2).unwrap_or(0);
    r.volume = kv.ler::<u8>("volume").map_or(p.volume, |v| v.min(127));
    r.pulso_salvo = (
        kv.ler("pulso.tolerancia_ms").unwrap_or(p.pulso_salvo.0),
        kv.ler::<i32>("pulso.compensacao_ms")
            .map_or(p.pulso_salvo.1, |c| c.clamp(-100, 100)),
    );
}

/// O padrão de bateria em uso (passos, fórmula, andamento), no formato dos arquivos `.ritmo`.
pub fn retrato_padrao(r: &EstadoRitmo) -> String {
    r.padrao.para_texto()
}

pub fn aplicar_padrao(r: &mut EstadoRitmo, t: &str) {
    let p = if t.trim().is_empty() {
        None
    } else {
        Padrao::de_texto(t).ok()
    };
    let p = p.unwrap_or_else(|| EstadoRitmo::novo(None).padrao);
    if !r.tocando() {
        r.escolher_padrao(p);
    }
}

// --- Gravação e Partitura --------------------------------------------------------------------

pub fn retrato_gravacao(g: &EstadoGravacao) -> String {
    Kv::novo()
        .por("contagem", g.contagem)
        .por("bpm_contagem", g.bpm_contagem)
        .texto()
}

pub fn aplicar_gravacao(g: &mut EstadoGravacao, t: &str) {
    let kv = Kv::de_texto(t);
    let p = EstadoGravacao::novo(None);
    g.contagem = kv.ler("contagem").unwrap_or(p.contagem);
    g.bpm_contagem = kv
        .ler::<u16>("bpm_contagem")
        .filter(|b| (40..=200).contains(b))
        .unwrap_or(p.bpm_contagem);
}

pub fn retrato_partitura(e: &EstadoPartitura) -> String {
    Kv::novo().por_enum("modo_pratica", e.modo_pratica).texto()
}

pub fn aplicar_partitura(e: &mut EstadoPartitura, t: &str) {
    let kv = Kv::de_texto(t);
    e.modo_pratica = kv
        .ler_enum("modo_pratica", &ModoPratica::TODOS)
        .unwrap_or(ModoPratica::Espera);
}
