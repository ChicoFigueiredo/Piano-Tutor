//! Retratos das opções de cada tela para o perfil (ADR-0010): `retrato_*` tira o texto atual e
//! `aplicar_*` devolve um texto gravado ao estado. O app compara os retratos a cada quadro e
//! grava na hora o que mudou.
//!
//! Cada tela tem a sua seção `tela.*`: Teclado, Acordes e escalas, Ritmos (e o padrão de
//! bateria), Gravação, Partitura (modo de prática, tolerância e retomada), Player de Aula (modo,
//! tolerância, retomada, divisão da tela e velocidade), Cursos (o Curso escolhido), Revisão do
//! tutor (aba, filtros e zooms), Sons e timbres (a família do seletor de instrumento) e o app
//! (a última tela aberta, `tela.app`).
//!
//! Já gravadas por conta própria: som e timbres (`som`, inclusive o timbre das demonstrações),
//! dedilhado (`exibicao`), exibição da partitura (`partitura`: rolagem, papel branco, seguir e
//! zoom), tema/janela (`casca`), configuração dos exercícios (`progresso.txt`) e a calibração
//! (`teclado.txt`).
//!
//! Fica de fora, de propósito: estado de sessão (prática em andamento, gravação em curso, texto
//! digitado, resultados, peças mudas e em solo do ritmo), a partitura aberta e o andamento dela
//! (vêm do arquivo), a porta MIDI (a calibração do teclado a escolhe) e o SoundFont em uso.

use crate::acordes::{Conjunto, EstadoAcordes, FonteTreino, Modo, tonalidades_disponiveis};
use crate::cursos::EstadoCursos;
use crate::espelho::EstadoEspelho;
use crate::gravacao::EstadoGravacao;
use crate::opcoes::Kv;
use crate::player_aula::ModoAcompanhamento;
use crate::revisao::{ESTADOS, Estado};
use crate::ritmo::EstadoRitmo;
use crate::seletor_instrumento::{EstadoSeletor, FAMILIAS};
use crate::teclado_visual::Rotulos;
use crate::tela_partitura::{
    EstadoPartitura, TOLERANCIA_PRATICA_MAX_MS, TOLERANCIA_PRATICA_MIN_MS,
    TOLERANCIA_PRATICA_PADRAO_MS,
};
use crate::tela_player_aula::OpcoesPlayer;
use crate::tela_revisao::{AbaRevisao, EstadoRevisao};
use crate::treino_andamento::Ordem;
use piano_tutor_core::pratica::{ModoPratica, Retomada};
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
    pub const PLAYER_AULA: &str = "tela.player_aula";
    pub const CURSOS: &str = "tela.cursos";
    pub const REVISAO: &str = "tela.revisao";
    pub const TIMBRES: &str = "tela.timbres";
    /// A última tela aberta. Fica por último: o app só a grava depois das outras, para que uma
    /// mudança que cria "Meu perfil" já a leve junto.
    pub const APP: &str = "tela.app";
    pub const TODAS: [&str; 11] = [
        TECLADO,
        ACORDES,
        RITMOS,
        RITMOS_PADRAO,
        GRAVACAO,
        PARTITURA,
        PLAYER_AULA,
        CURSOS,
        REVISAO,
        TIMBRES,
        APP,
    ];
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
    Kv::novo()
        .por("modo_pratica", e.modo_pratica.chave())
        .por("tolerancia_pratica_ms", e.tolerancia_pratica_ms)
        .por("retomada_pratica", e.retomada_pratica.chave())
        .texto()
}

pub fn aplicar_partitura(e: &mut EstadoPartitura, t: &str) {
    let kv = Kv::de_texto(t);
    // Lê também o nome antigo ("Espera" = Nota a nota).
    e.modo_pratica = kv
        .ler::<String>("modo_pratica")
        .and_then(|v| ModoPratica::de_chave(&v))
        .unwrap_or(ModoPratica::NotaANota);
    e.tolerancia_pratica_ms = kv
        .ler::<f32>("tolerancia_pratica_ms")
        .filter(|v| (TOLERANCIA_PRATICA_MIN_MS..=TOLERANCIA_PRATICA_MAX_MS).contains(v))
        .unwrap_or(TOLERANCIA_PRATICA_PADRAO_MS);
    e.retomada_pratica = kv
        .ler::<String>("retomada_pratica")
        .and_then(|v| Retomada::de_chave(&v))
        .unwrap_or_default();
}

// --- Player de Aula --------------------------------------------------------------------------

/// Retrato das escolhas do Player de Aula. Com um player aberto, a vista dele manda: as opções
/// dele são copiadas para `opcoes_player` antes de tirar o texto (como o pulso do Ritmos).
pub fn retrato_player_aula(c: &mut EstadoCursos) -> String {
    if let Some(p) = &c.player {
        c.opcoes_player = p.opcoes();
    }
    let o = c.opcoes_player;
    Kv::novo()
        .por("modo", o.modo.como_texto())
        .por("tolerancia_ms", o.tolerancia_ms)
        .por("retomada", o.retomada.chave())
        .por("fracao_video", o.fracao_video)
        .por("altura_topo", o.altura_topo)
        .por("video_a_direita", o.video_a_direita)
        .por("velocidade", o.velocidade)
        .texto()
}

pub fn aplicar_player_aula(c: &mut EstadoCursos, t: &str) {
    let kv = Kv::de_texto(t);
    let p = OpcoesPlayer::default();
    let o = OpcoesPlayer {
        modo: kv
            .ler::<String>("modo")
            .and_then(|v| ModoAcompanhamento::de_texto(&v))
            .unwrap_or(p.modo),
        tolerancia_ms: kv
            .ler::<f32>("tolerancia_ms")
            .filter(|v| (20.0..=500.0).contains(v))
            .unwrap_or(p.tolerancia_ms),
        retomada: kv
            .ler::<String>("retomada")
            .and_then(|v| Retomada::de_chave(&v))
            .unwrap_or(p.retomada),
        fracao_video: kv
            .ler::<f32>("fracao_video")
            .filter(|v| (0.15..=0.85).contains(v))
            .unwrap_or(p.fracao_video),
        altura_topo: kv
            .ler::<f32>("altura_topo")
            .filter(|v| (0.0..=900.0).contains(v))
            .unwrap_or(p.altura_topo),
        video_a_direita: kv.ler("video_a_direita").unwrap_or(p.video_a_direita),
        velocidade: kv
            .ler::<f64>("velocidade")
            .and_then(|v| {
                crate::tela_player_aula::VELOCIDADES
                    .into_iter()
                    .find(|x| (x - v).abs() < 1e-9)
            })
            .unwrap_or(p.velocidade),
    };
    c.opcoes_player = o;
    if let Some(player) = c.player.as_mut() {
        player.aplicar_opcoes(o);
    }
}

// --- Cursos ----------------------------------------------------------------------------------

/// O Curso escolhido no seletor da tela Cursos (pelo `id`).
pub fn retrato_cursos(c: &EstadoCursos) -> String {
    Kv::novo().por("curso", &c.curso_selecionado().id).texto()
}

/// Volta ao Curso gravado; sem ele (ou se não estiver mais instalado), ao primeiro. Não mexe
/// no Curso enquanto há Exercício ou Aula aberta.
pub fn aplicar_cursos(c: &mut EstadoCursos, t: &str) {
    if c.aberto.is_some() || c.player.is_some() {
        return;
    }
    let kv = Kv::de_texto(t);
    c.selecionar(0);
    if let Some(id) = kv.ler::<String>("curso") {
        c.selecionar_por_id(&id);
    }
}

// --- Revisão do tutor ------------------------------------------------------------------------

fn estados_com_todos() -> Vec<Option<Estado>> {
    std::iter::once(None).chain(ESTADOS.map(Some)).collect()
}

pub fn retrato_revisao(e: &EstadoRevisao) -> String {
    Kv::novo()
        .por_enum("aba", e.aba)
        .por_enum("filtro_partitura", e.filtro_de_partitura)
        .por_enum("filtro_fala", e.filtro_de_fala)
        .por("zoom_pagina", e.zoom_da_pagina)
        .por("ajustar_pagina", e.ajustar_pagina)
        .por("zoom_partitura", e.zoom_partitura())
        .texto()
}

pub fn aplicar_revisao(e: &mut EstadoRevisao, t: &str) {
    let kv = Kv::de_texto(t);
    let filtros = estados_com_todos();
    e.aba = kv
        .ler_enum("aba", &[AbaRevisao::Partituras, AbaRevisao::Roteiro])
        .unwrap_or(AbaRevisao::Partituras);
    e.filtro_de_partitura = kv.ler_enum("filtro_partitura", &filtros).unwrap_or(None);
    e.filtro_de_fala = kv.ler_enum("filtro_fala", &filtros).unwrap_or(None);
    e.zoom_da_pagina = kv
        .ler::<f32>("zoom_pagina")
        .filter(|z| (0.1..=4.0).contains(z))
        .unwrap_or(1.0);
    e.ajustar_pagina = kv.ler("ajustar_pagina").unwrap_or(true);
    e.definir_zoom_partitura(kv.ler("zoom_partitura").unwrap_or(1.0));
}

// --- Sons e timbres (seletor de instrumento do Ribbon) ---------------------------------------

pub fn retrato_timbres(s: &EstadoSeletor) -> String {
    Kv::novo()
        .por(
            "familia",
            s.familia
                .map_or_else(|| "todas".to_owned(), |f| f.to_string()),
        )
        .texto()
}

pub fn aplicar_timbres(s: &mut EstadoSeletor, t: &str) {
    s.familia = Kv::de_texto(t)
        .ler::<usize>("familia")
        .filter(|f| *f < FAMILIAS.len());
}

// --- O app: a última tela aberta -------------------------------------------------------------

pub fn retrato_app(rota: &str) -> String {
    Kv::novo().por("rota", rota).texto()
}

/// A rota gravada no retrato do app, se houver.
pub fn rota_gravada(t: &str) -> Option<String> {
    Kv::de_texto(t)
        .ler::<String>("rota")
        .filter(|r| !r.is_empty())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::mpv::apoio_testes::midia_falsa;
    use crate::tela_player_aula::EstadoPlayer;

    fn cursos() -> EstadoCursos {
        EstadoCursos::novo(None, midia_falsa().0)
    }

    #[test]
    fn partitura_ida_e_volta_com_tolerancia_e_retomada() {
        let mut e = EstadoPartitura::novo(None);
        e.modo_pratica = ModoPratica::EsperaTocar;
        e.tolerancia_pratica_ms = 210.0;
        e.retomada_pratica = Retomada::Quatro;
        let texto = retrato_partitura(&e);
        let mut outro = EstadoPartitura::novo(None);
        aplicar_partitura(&mut outro, &texto);
        assert_eq!(outro.modo_pratica, ModoPratica::EsperaTocar);
        assert_eq!(outro.tolerancia_pratica_ms, 210.0);
        assert_eq!(outro.retomada_pratica, Retomada::Quatro);
        assert_eq!(retrato_partitura(&outro), texto);
        // Todas as retomadas e modos sobrevivem à ida e volta.
        for r in Retomada::TODAS {
            for m in ModoPratica::TODOS {
                e.modo_pratica = m;
                e.retomada_pratica = r;
                aplicar_partitura(&mut outro, &retrato_partitura(&e));
                assert_eq!((outro.modo_pratica, outro.retomada_pratica), (m, r));
            }
        }
    }

    #[test]
    fn partitura_lixo_fora_de_faixa_e_nome_antigo() {
        let mut e = EstadoPartitura::novo(None);
        for lixo in [
            "modo_pratica=xyz\ntolerancia_pratica_ms=abc\nretomada_pratica=9",
            "tolerancia_pratica_ms=5",
            "tolerancia_pratica_ms=99999",
            "tolerancia_pratica_ms=NaN",
            "",
        ] {
            e.modo_pratica = ModoPratica::Tempo;
            e.tolerancia_pratica_ms = 77.0;
            e.retomada_pratica = Retomada::Inicio;
            aplicar_partitura(&mut e, lixo);
            assert_eq!(e.modo_pratica, ModoPratica::NotaANota, "{lixo:?}");
            assert_eq!(
                e.tolerancia_pratica_ms, TOLERANCIA_PRATICA_PADRAO_MS,
                "{lixo:?}"
            );
            assert_eq!(e.retomada_pratica, Retomada::Zero, "{lixo:?}");
        }
        // O nome antigo "Espera" é o Nota a nota.
        e.modo_pratica = ModoPratica::Tempo;
        aplicar_partitura(&mut e, "modo_pratica=Espera");
        assert_eq!(e.modo_pratica, ModoPratica::NotaANota);
        // Os limites da faixa valem.
        aplicar_partitura(&mut e, "tolerancia_pratica_ms=30");
        assert_eq!(e.tolerancia_pratica_ms, 30.0);
        aplicar_partitura(&mut e, "tolerancia_pratica_ms=500");
        assert_eq!(e.tolerancia_pratica_ms, 500.0);
    }

    #[test]
    fn player_aula_ida_e_volta_e_lixo_volta_ao_padrao() {
        let mut c = cursos();
        c.opcoes_player = OpcoesPlayer {
            modo: ModoAcompanhamento::ChamadaEResposta,
            tolerancia_ms: 180.0,
            retomada: Retomada::Duas,
            fracao_video: 0.6,
            altura_topo: 333.0,
            video_a_direita: true,
            velocidade: 0.75,
        };
        let texto = retrato_player_aula(&mut c);
        let mut outro = cursos();
        aplicar_player_aula(&mut outro, &texto);
        assert_eq!(outro.opcoes_player, c.opcoes_player);
        assert_eq!(retrato_player_aula(&mut outro), texto);

        for lixo in [
            "modo=xyz\ntolerancia_ms=1\nretomada=7\nfracao_video=2\naltura_topo=-3\nvelocidade=3\nvideo_a_direita=talvez",
            "tolerancia_ms=501\nfracao_video=NaN\naltura_topo=9999\nvelocidade=0.8",
            "",
        ] {
            aplicar_player_aula(&mut outro, lixo);
            assert_eq!(outro.opcoes_player, OpcoesPlayer::default(), "{lixo:?}");
        }
        // O nome antigo do modo ("espera") é o Nota a nota.
        aplicar_player_aula(&mut outro, "modo=espera");
        assert_eq!(outro.opcoes_player.modo, ModoAcompanhamento::NotaANota);
    }

    #[test]
    fn player_aula_aberto_manda_no_retrato_e_recebe_o_aplicado() {
        let mut c = cursos();
        let mut p = EstadoPlayer::abrir("licao", std::path::PathBuf::from("/x.mp4"), 0.0, vec![]);
        let mut o = p.opcoes();
        o.tolerancia_ms = 222.0;
        o.video_a_direita = true;
        o.velocidade = 1.25;
        p.aplicar_opcoes(o);
        c.player = Some(p);
        let texto = retrato_player_aula(&mut c);
        assert_eq!(c.opcoes_player.tolerancia_ms, 222.0, "copiou do player");
        assert!(texto.contains("velocidade=1.25"));
        aplicar_player_aula(&mut c, "tolerancia_ms=90\nvelocidade=0.5");
        let visto = c.player.as_ref().expect("player").opcoes();
        assert_eq!(visto.tolerancia_ms, 90.0);
        assert_eq!(visto.velocidade, 0.5);
        assert_eq!(c.opcoes_player, visto);
    }

    #[test]
    fn cursos_ida_e_volta_e_curso_sumido_volta_ao_primeiro() {
        let mut c = cursos();
        let id = c.curso_selecionado().id.clone();
        let texto = retrato_cursos(&c);
        assert_eq!(texto, format!("curso={id}\n"));
        aplicar_cursos(&mut c, &texto);
        assert_eq!(c.indice_selecionado(), 0);
        aplicar_cursos(&mut c, "curso=nao-existe");
        assert_eq!(c.indice_selecionado(), 0);
        aplicar_cursos(&mut c, "");
        assert_eq!(c.indice_selecionado(), 0);
    }

    #[test]
    fn revisao_ida_e_volta_e_lixo_volta_ao_padrao() {
        let mut e = EstadoRevisao::novo(None);
        e.aba = AbaRevisao::Roteiro;
        e.filtro_de_partitura = Some(Estado::Aprovada);
        e.filtro_de_fala = Some(Estado::EmRevisao);
        e.zoom_da_pagina = 1.75;
        e.ajustar_pagina = false;
        e.definir_zoom_partitura(1.5);
        let texto = retrato_revisao(&e);
        let mut outro = EstadoRevisao::novo(None);
        aplicar_revisao(&mut outro, &texto);
        assert_eq!(outro.aba, AbaRevisao::Roteiro);
        assert_eq!(outro.filtro_de_partitura, Some(Estado::Aprovada));
        assert_eq!(outro.filtro_de_fala, Some(Estado::EmRevisao));
        assert_eq!(outro.zoom_da_pagina, 1.75);
        assert!(!outro.ajustar_pagina);
        assert_eq!(outro.zoom_partitura(), 1.5);
        assert_eq!(retrato_revisao(&outro), texto);

        aplicar_revisao(
            &mut outro,
            "aba=Nenhuma\nfiltro_partitura=Roxo\nfiltro_fala=x\nzoom_pagina=99\najustar_pagina=talvez\nzoom_partitura=abc",
        );
        assert_eq!(outro.aba, AbaRevisao::Partituras);
        assert_eq!(outro.filtro_de_partitura, None);
        assert_eq!(outro.filtro_de_fala, None);
        assert_eq!(outro.zoom_da_pagina, 1.0);
        assert!(outro.ajustar_pagina);
        assert_eq!(outro.zoom_partitura(), 1.0);
    }

    #[test]
    fn timbres_familia_ida_e_volta_e_fora_da_lista_vira_todas() {
        let mut s = EstadoSeletor {
            familia: Some(3),
            ..EstadoSeletor::default()
        };
        let texto = retrato_timbres(&s);
        let mut outro = EstadoSeletor::default();
        aplicar_timbres(&mut outro, &texto);
        assert_eq!(outro.familia, Some(3));
        assert_eq!(retrato_timbres(&outro), texto);
        s.familia = None;
        aplicar_timbres(&mut outro, &retrato_timbres(&s));
        assert_eq!(outro.familia, None);
        for lixo in ["familia=999", "familia=-1", "familia=abc", ""] {
            outro.familia = Some(1);
            aplicar_timbres(&mut outro, lixo);
            assert_eq!(outro.familia, None, "{lixo:?}");
        }
    }

    #[test]
    fn app_rota_ida_e_volta() {
        assert_eq!(
            rota_gravada(&retrato_app("partitura")).as_deref(),
            Some("partitura")
        );
        assert_eq!(rota_gravada(""), None);
        assert_eq!(rota_gravada("rota="), None);
        assert_eq!(rota_gravada("lixo"), None);
    }
}
