//! Exportação: patches textuais sobre o texto original, aplicados uma única vez, de trás para
//! frente. As faixas só valem para o texto-base; um texto novo tem outras posições.

use super::erro::ErroDocumento;
use super::{Documento, Parte};
use std::ops::Range;

/// Impressão digital do texto-base: tamanho e FNV-1a de 64 bits (sem dependências).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Impressao {
    pub tamanho: usize,
    pub fnv: u64,
}

pub fn impressao_de(texto: &str) -> Impressao {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Impressao {
        tamanho: texto.len(),
        fnv: h,
    }
}

/// Substituição de uma faixa do texto-base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    pub faixa: Range<usize>,
    pub novo: String,
}

/// Conjunto de patches ligado ao texto-base por impressão digital. `aplicar` consome o conjunto:
/// reaplicar é impossível por construção; clonar e aplicar sobre o texto novo é recusado.
#[derive(Clone, Debug)]
pub struct Patches {
    base: Impressao,
    itens: Vec<Patch>,
}

impl Patches {
    pub fn novo(base: &str, itens: Vec<Patch>) -> Self {
        Self {
            base: impressao_de(base),
            itens,
        }
    }

    pub fn itens(&self) -> &[Patch] {
        &self.itens
    }

    /// Aplica de trás para frente sobre `texto`, que precisa ser o texto-base.
    pub fn aplicar(mut self, texto: &str) -> Result<String, ErroDocumento> {
        if impressao_de(texto) != self.base {
            return Err(ErroDocumento::PatchesDeOutroTexto);
        }
        self.itens.sort_by_key(|p| p.faixa.start);
        let mut fim_anterior = 0;
        for p in &self.itens {
            if p.faixa.start < fim_anterior {
                return Err(ErroDocumento::PatchesInvalidos(format!(
                    "faixa {:?} sobrepõe a anterior",
                    p.faixa
                )));
            }
            if p.faixa.start > p.faixa.end
                || !texto.is_char_boundary(p.faixa.start)
                || !texto.is_char_boundary(p.faixa.end)
            {
                return Err(ErroDocumento::PatchesInvalidos(format!(
                    "faixa {:?} fora do texto ou fora de fronteira de caractere",
                    p.faixa
                )));
            }
            fim_anterior = p.faixa.end;
        }
        let mut saida = texto.to_owned();
        for p in self.itens.iter().rev() {
            saida.replace_range(p.faixa.clone(), &p.novo);
        }
        Ok(saida)
    }
}

impl Documento {
    fn patch_do_compasso(&self, parte: &Parte, i: usize) -> Patch {
        let c = &parte.compassos[i];
        let mut novo = String::with_capacity(c.faixa.len() + 64);
        c.no.escrever(&self.original, &mut novo);
        Patch {
            faixa: c.faixa.clone(),
            novo,
        }
    }

    /// Um patch por compasso sujo (e nenhum outro).
    pub fn patches(&self) -> Patches {
        let mut itens = Vec::new();
        for p in &self.partes {
            for (i, c) in p.compassos.iter().enumerate() {
                if c.sujo {
                    itens.push(self.patch_do_compasso(p, i));
                }
            }
        }
        Patches::novo(&self.original, itens)
    }

    /// Exporta: copia byte a byte tudo que não está sujo e regenera só os compassos sujos.
    pub fn exportar(&self) -> Result<String, ErroDocumento> {
        self.patches().aplicar(&self.original)
    }

    /// Regenera TODOS os compassos a partir das peças (como se todos estivessem sujos). Serve
    /// para provar que a decomposição não perde bytes e para medir o custo do pior caso.
    pub fn exportar_regenerando_tudo(&self) -> Result<String, ErroDocumento> {
        let mut itens = Vec::new();
        for p in &self.partes {
            for i in 0..p.compassos.len() {
                itens.push(self.patch_do_compasso(p, i));
            }
        }
        Patches::novo(&self.original, itens).aplicar(&self.original)
    }
}
