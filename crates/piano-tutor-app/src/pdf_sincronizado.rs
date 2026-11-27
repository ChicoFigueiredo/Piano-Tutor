//! PDF original ao lado da partitura, sincronizado por compasso (ticket 20, A17).
//!
//! Módulo puro (sem janela, sem E/S, sem rede): o mapa **compasso → página** e a ação de rolar.
//! O mapa vem do que a Oficina já tem no `estado.toml` (páginas PNG e, opcionalmente, regiões);
//! ver [`crate::revisao::ItemPartitura`]. Sem região mapeada, a ligação é **por página**.
//!
//! Numeração: aqui compasso e página são índices a partir de 0; o `estado.toml` usa números a
//! partir de 1 (como o revisor lê). A região é fração da página (0 a 1, origem no canto superior
//! esquerdo). O PDF e as páginas nunca são escritos por este módulo.

/// Máximo de páginas aceitas por partitura (limite de memória: só a página em foco vira textura).
pub const MAX_PAGINAS: usize = 400;

/// Retângulo na página, em frações (0 a 1) da largura e da altura.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regiao {
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
}

impl Regiao {
    /// Região válida: finita, dentro da página e com área.
    pub fn nova(x: f32, y: f32, largura: f32, altura: f32) -> Option<Self> {
        let ok = [x, y, largura, altura].iter().all(|v| v.is_finite())
            && x >= 0.0
            && y >= 0.0
            && largura > 0.0
            && altura > 0.0
            && x + largura <= 1.0 + 1e-6
            && y + altura <= 1.0 + 1e-6;
        ok.then_some(Self {
            x,
            y,
            largura,
            altura,
        })
    }
}

/// Uma linha do mapa: o compasso começa nesta página (e, se souber, nesta região).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntradaDoMapa {
    pub compasso: usize,
    pub pagina: usize,
    pub regiao: Option<Regiao>,
}

/// Para onde o painel do PDF vai ao selecionar um compasso.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Destino {
    pub pagina: usize,
    /// Região a destacar; `None` = página inteira.
    pub regiao: Option<Regiao>,
    /// A ligação é só por página (sem região mapeada): a tela informa isso.
    pub por_pagina: bool,
}

/// Aviso quando o mapa é só por página.
pub const AVISO_POR_PAGINA: &str = "Mapa por página: sem região mapeada para este compasso.";
/// Aviso quando não há como ligar o compasso a uma página.
pub const AVISO_SEM_MAPA: &str = "Sem mapa de compassos para este PDF de várias páginas.";
/// Aviso discreto quando a partitura não tem PDF/páginas de origem.
pub const AVISO_SEM_PDF: &str = "Sem PDF de origem: a Oficina segue só com a partitura.";

/// Mapa compasso → página (e região, quando existe) de uma partitura.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapaCompassos {
    paginas: usize,
    entradas: Vec<EntradaDoMapa>,
}

impl MapaCompassos {
    /// Monta o mapa para um PDF de `paginas` páginas. Entradas com página fora do PDF são
    /// descartadas; ordena por compasso e mantém a última de cada compasso.
    pub fn novo(paginas: usize, entradas: impl IntoIterator<Item = EntradaDoMapa>) -> Self {
        let paginas = paginas.min(MAX_PAGINAS);
        let mut v: Vec<EntradaDoMapa> = entradas
            .into_iter()
            .filter(|e| e.pagina < paginas)
            .collect();
        v.sort_by_key(|e| e.compasso);
        v.reverse();
        v.dedup_by_key(|e| e.compasso);
        v.reverse();
        Self {
            paginas,
            entradas: v,
        }
    }

    pub fn paginas(&self) -> usize {
        self.paginas
    }

    pub fn entradas(&self) -> &[EntradaDoMapa] {
        &self.entradas
    }

    pub fn tem_pdf(&self) -> bool {
        self.paginas > 0
    }

    /// Destino do compasso `c`: a região mapeada, ou a página do último compasso mapeado até
    /// ele (por página); com uma só página, ela; sem nada a que se agarrar, `None`.
    pub fn destino_do_compasso(&self, c: usize) -> Option<Destino> {
        if self.paginas == 0 {
            return None;
        }
        if let Some(e) = self.entradas.iter().rfind(|e| e.compasso <= c) {
            let exata = e.compasso == c;
            return Some(Destino {
                pagina: e.pagina,
                regiao: if exata { e.regiao } else { None },
                por_pagina: !(exata && e.regiao.is_some()),
            });
        }
        // Antes do primeiro compasso mapeado, ou sem mapa algum.
        if self.entradas.is_empty() && self.paginas > 1 {
            return None;
        }
        Some(Destino {
            pagina: 0,
            regiao: None,
            por_pagina: true,
        })
    }

    /// Aviso para a tela sobre o destino (ou a falta dele).
    pub fn aviso(&self, destino: Option<Destino>) -> Option<&'static str> {
        match destino {
            Some(d) if d.por_pagina => Some(AVISO_POR_PAGINA),
            Some(_) => None,
            None if self.paginas > 0 => Some(AVISO_SEM_MAPA),
            None => Some(AVISO_SEM_PDF),
        }
    }

    /// Compasso que o PDF indica ao rolar: `pagina` visível e `y` (0 a 1) da posição de leitura
    /// nela. Com regiões na página, o compasso cuja região contém ou segue `y`; sem regiões, o
    /// primeiro compasso mapeado na página; página sem entrada continua o último compasso
    /// mapeado antes dela.
    pub fn compasso_na_pagina(&self, pagina: usize, y: f32) -> Option<usize> {
        if pagina >= self.paginas {
            return None;
        }
        let y = if y.is_finite() {
            y.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let na_pagina: Vec<&EntradaDoMapa> = self
            .entradas
            .iter()
            .filter(|e| e.pagina == pagina)
            .collect();
        let mut com_regiao: Vec<(&EntradaDoMapa, Regiao)> = na_pagina
            .iter()
            .filter_map(|e| e.regiao.map(|r| (*e, r)))
            .collect();
        if !com_regiao.is_empty() {
            com_regiao.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
            let achada = com_regiao
                .iter()
                .find(|(_, r)| r.y + r.altura >= y)
                .or_else(|| com_regiao.last());
            return achada.map(|(e, _)| e.compasso);
        }
        if let Some(e) = na_pagina.iter().min_by_key(|e| e.compasso) {
            return Some(e.compasso);
        }
        self.entradas
            .iter()
            .filter(|e| e.pagina < pagina)
            .max_by_key(|e| e.compasso)
            .map(|e| e.compasso)
    }
}

/// Pedido de rolagem do painel do PDF, gerado ao selecionar um compasso.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PedidoDeRolagem {
    pub pagina: usize,
    /// Fração (0 a 1) da altura da página que deve ficar no topo do painel.
    pub y: f32,
}

impl Destino {
    /// Rolagem que leva o painel a este destino (um pouco acima da região, para dar contexto).
    pub fn rolagem(&self) -> PedidoDeRolagem {
        let y = self.regiao.map_or(0.0, |r| (r.y - 0.03).max(0.0));
        PedidoDeRolagem {
            pagina: self.pagina,
            y,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn r(y: f32) -> Option<Regiao> {
        Regiao::nova(0.1, y, 0.8, 0.1)
    }

    fn e(compasso: usize, pagina: usize, regiao: Option<Regiao>) -> EntradaDoMapa {
        EntradaDoMapa {
            compasso,
            pagina,
            regiao,
        }
    }

    #[test]
    fn regiao_invalida_e_recusada() {
        assert!(Regiao::nova(0.0, 0.0, 1.0, 1.0).is_some());
        assert!(Regiao::nova(-0.1, 0.0, 0.5, 0.5).is_none());
        assert!(Regiao::nova(0.5, 0.5, 0.6, 0.1).is_none());
        assert!(Regiao::nova(0.0, 0.0, 0.0, 0.5).is_none());
        assert!(Regiao::nova(f32::NAN, 0.0, 0.5, 0.5).is_none());
    }

    #[test]
    fn com_regiao_mapeada_o_destino_traz_a_regiao_e_nao_e_por_pagina() {
        let m = MapaCompassos::novo(2, [e(0, 0, r(0.1)), e(4, 1, r(0.5))]);
        let d = m.destino_do_compasso(4).expect("destino");
        assert_eq!(d.pagina, 1);
        assert_eq!(d.regiao, r(0.5));
        assert!(!d.por_pagina);
        assert_eq!(m.aviso(Some(d)), None);
    }

    #[test]
    fn sem_regiao_leva_a_pagina_inteira_e_informa_mapa_por_pagina() {
        let m = MapaCompassos::novo(2, [e(0, 0, None), e(4, 1, None)]);
        let d = m.destino_do_compasso(4).expect("destino");
        assert_eq!((d.pagina, d.regiao, d.por_pagina), (1, None, true));
        assert_eq!(m.aviso(Some(d)), Some(AVISO_POR_PAGINA));
        // Compasso sem entrada herda a página do último mapeado antes dele.
        let d6 = m.destino_do_compasso(6).expect("destino");
        assert_eq!((d6.pagina, d6.regiao, d6.por_pagina), (1, None, true));
        // Compasso vizinho de um com região, mas sem entrada própria: vai por página.
        let m2 = MapaCompassos::novo(1, [e(0, 0, r(0.2))]);
        let d2 = m2.destino_do_compasso(3).expect("destino");
        assert!(d2.por_pagina && d2.regiao.is_none());
    }

    #[test]
    fn uma_pagina_sem_mapa_serve_todos_os_compassos_e_varias_sem_mapa_nao() {
        let uma = MapaCompassos::novo(1, []);
        let d = uma.destino_do_compasso(7).expect("destino");
        assert_eq!((d.pagina, d.por_pagina), (0, true));
        let varias = MapaCompassos::novo(3, []);
        let nenhum = varias.destino_do_compasso(7);
        assert!(nenhum.is_none());
        assert_eq!(varias.aviso(nenhum), Some(AVISO_SEM_MAPA));
    }

    #[test]
    fn sem_pdf_nao_ha_destino_e_o_aviso_e_discreto() {
        let m = MapaCompassos::default();
        assert!(!m.tem_pdf());
        assert_eq!(m.destino_do_compasso(0), None);
        assert_eq!(m.aviso(None), Some(AVISO_SEM_PDF));
    }

    #[test]
    fn entradas_fora_do_pdf_sao_descartadas_e_a_ultima_do_compasso_vale() {
        let m = MapaCompassos::novo(2, [e(3, 5, None), e(2, 0, None), e(2, 1, None)]);
        assert_eq!(m.entradas(), &[e(2, 1, None)]);
        let enorme = MapaCompassos::novo(MAX_PAGINAS + 50, []);
        assert_eq!(enorme.paginas(), MAX_PAGINAS);
    }

    #[test]
    fn rolar_o_pdf_indica_o_compasso() {
        let m = MapaCompassos::novo(
            2,
            [
                e(0, 0, r(0.1)),
                e(1, 0, r(0.6)),
                e(2, 1, None),
                e(5, 1, None),
            ],
        );
        assert_eq!(m.compasso_na_pagina(0, 0.0), Some(0));
        assert_eq!(m.compasso_na_pagina(0, 0.5), Some(1));
        assert_eq!(m.compasso_na_pagina(0, 0.99), Some(1));
        // Página sem regiões: o primeiro compasso dela.
        assert_eq!(m.compasso_na_pagina(1, 0.9), Some(2));
        assert_eq!(m.compasso_na_pagina(9, 0.0), None);
        // Página sem entrada continua o último compasso mapeado antes.
        let m2 = MapaCompassos::novo(3, [e(0, 0, None), e(4, 0, None)]);
        assert_eq!(m2.compasso_na_pagina(2, 0.0), Some(4));
        assert_eq!(m2.compasso_na_pagina(1, f32::NAN), Some(4));
    }

    #[test]
    fn rolagem_leva_ao_topo_da_pagina_ou_um_pouco_acima_da_regiao() {
        let d = Destino {
            pagina: 1,
            regiao: None,
            por_pagina: true,
        };
        assert_eq!(d.rolagem(), PedidoDeRolagem { pagina: 1, y: 0.0 });
        let d = Destino {
            pagina: 0,
            regiao: r(0.5),
            por_pagina: false,
        };
        let p = d.rolagem();
        assert_eq!(p.pagina, 0);
        assert!((p.y - 0.47).abs() < 1e-6);
    }
}
