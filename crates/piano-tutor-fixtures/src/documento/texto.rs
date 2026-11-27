//! Peças de texto: cada elemento editável é decomposto em trechos do texto original (faixas) e
//! trechos novos. Concatenar as peças de um elemento intocado devolve exatamente o seu texto.

use super::erro::ErroDocumento;
use std::ops::Range;

/// Um trecho: bytes do texto original (por faixa) ou texto gerado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Texto {
    Original(Range<usize>),
    Novo(String),
}

/// Peça de um elemento: texto solto (marca de abertura, espaços, comentários, marca de
/// fechamento) ou um elemento filho.
#[derive(Clone, Debug)]
pub enum Peca {
    Txt(Texto),
    Elem(No),
}

/// Elemento decomposto. Invariante: as peças alternam `Txt`, `Elem`, `Txt`, ..., `Txt`
/// (a primeira é a marca de abertura, a última é o resto até o fim, com a marca de fechamento);
/// um elemento não decomposto tem uma única peça `Txt(Original(faixa))`.
#[derive(Clone, Debug)]
pub struct No {
    pub nome: String,
    pub pecas: Vec<Peca>,
}

/// Onde termina a marca de abertura de um elemento que começa em `inicio`.
pub enum Abertura {
    /// Índice logo após o `>` da marca de abertura.
    Aberta(usize),
    /// A marca termina em `/>`.
    AutoFechada,
}

/// Acha o fim da marca de abertura respeitando aspas (um `>` dentro de valor de atributo é
/// legítimo). Devolve `None` se o texto acabar antes.
pub fn fim_da_abertura(base: &str, inicio: usize) -> Option<Abertura> {
    let b = base.as_bytes();
    let mut i = inicio;
    let mut aspa: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        match aspa {
            Some(q) => {
                if c == q {
                    aspa = None;
                }
            }
            None => match c {
                b'"' | b'\'' => aspa = Some(c),
                b'>' => {
                    return Some(if i > inicio && b[i - 1] == b'/' {
                        Abertura::AutoFechada
                    } else {
                        Abertura::Aberta(i + 1)
                    });
                }
                _ => {}
            },
        }
        i += 1;
    }
    None
}

impl No {
    /// Elemento não decomposto: uma única peça com a faixa inteira.
    pub fn bruto(nome: &str, faixa: Range<usize>) -> Self {
        Self {
            nome: nome.to_owned(),
            pecas: vec![Peca::Txt(Texto::Original(faixa))],
        }
    }

    /// Elemento novo (não existe no original).
    pub fn novo(nome: &str, texto: String) -> Self {
        Self {
            nome: nome.to_owned(),
            pecas: vec![Peca::Txt(Texto::Novo(texto))],
        }
    }

    /// Escreve o texto do elemento em `saida`.
    pub fn escrever(&self, base: &str, saida: &mut String) {
        for p in &self.pecas {
            match p {
                Peca::Txt(Texto::Original(r)) => saida.push_str(&base[r.clone()]),
                Peca::Txt(Texto::Novo(s)) => saida.push_str(s),
                Peca::Elem(n) => n.escrever(base, saida),
            }
        }
    }

    pub fn filho(&self, nome: &str) -> Option<&No> {
        self.pecas.iter().find_map(|p| match p {
            Peca::Elem(n) if n.nome == nome => Some(n),
            _ => None,
        })
    }

    pub fn filho_mut(&mut self, nome: &str) -> Option<&mut No> {
        self.pecas.iter_mut().find_map(|p| match p {
            Peca::Elem(n) if n.nome == nome => Some(n),
            _ => None,
        })
    }

    /// Elementos filhos, em ordem, com o índice da peça de cada um.
    pub fn filhos(&self) -> impl Iterator<Item = (usize, &No)> {
        self.pecas.iter().enumerate().filter_map(|(i, p)| match p {
            Peca::Elem(n) => Some((i, n)),
            Peca::Txt(_) => None,
        })
    }

    /// Texto do elemento (para ler o conteúdo de uma folha).
    pub fn texto(&self, base: &str) -> String {
        let mut s = String::new();
        self.escrever(base, &mut s);
        s
    }

    /// Conteúdo de uma folha (`<nome>conteudo</nome>`), sem decodificar entidades.
    pub fn conteudo_folha(&self, base: &str) -> Option<String> {
        let t = self.texto(base);
        let ini = match fim_da_abertura(&t, 0)? {
            Abertura::Aberta(i) => i,
            Abertura::AutoFechada => return None,
        };
        let fim = t.rfind("</")?;
        (fim >= ini).then(|| t[ini..fim].to_owned())
    }

    /// Troca o conteúdo de uma folha mantendo a marca de abertura e a de fechamento originais
    /// (atributos, espaços e prefixo de nome intocados).
    pub fn definir_conteudo(&mut self, base: &str, novo: &str) -> Result<(), ErroDocumento> {
        // Já editado antes: só a peça do meio muda.
        if self.pecas.len() == 3
            && matches!(self.pecas[1], Peca::Txt(Texto::Novo(_)))
            && matches!(self.pecas[0], Peca::Txt(_))
            && matches!(self.pecas[2], Peca::Txt(_))
        {
            self.pecas[1] = Peca::Txt(Texto::Novo(novo.to_owned()));
            return Ok(());
        }
        let atual = self.texto(base);
        let vazio = || {
            ErroDocumento::NaoSuportado(format!(
                "<{}> vazio, autofechado ou não é folha",
                self.nome
            ))
        };
        let ini = match fim_da_abertura(&atual, 0) {
            Some(Abertura::Aberta(i)) => i,
            _ => return Err(vazio()),
        };
        let fim = atual.rfind("</").filter(|&f| f >= ini).ok_or_else(vazio)?;
        if self.pecas.iter().any(|p| matches!(p, Peca::Elem(_))) {
            return Err(vazio());
        }
        self.pecas = match self.pecas.as_slice() {
            // Texto original: mantém as faixas das marcas (atributos, espaços e prefixo intocados).
            [Peca::Txt(Texto::Original(r))] => vec![
                Peca::Txt(Texto::Original(r.start..r.start + ini)),
                Peca::Txt(Texto::Novo(novo.to_owned())),
                Peca::Txt(Texto::Original(r.start + fim..r.end)),
            ],
            // Elemento criado nesta sessão: tudo é texto novo.
            _ => vec![
                Peca::Txt(Texto::Novo(atual[..ini].to_owned())),
                Peca::Txt(Texto::Novo(novo.to_owned())),
                Peca::Txt(Texto::Novo(atual[fim..].to_owned())),
            ],
        };
        Ok(())
    }

    /// Insere `novo` em ordem do esquema: antes do primeiro filho cujo posto seja maior que o de
    /// `novo`; filhos desconhecidos (posto `None`) ficam onde estão. O espaço que precedia o
    /// filho de referência é copiado para o novo.
    pub fn inserir_em_ordem(
        &mut self,
        novo: No,
        posto: &dyn Fn(&str) -> Option<u32>,
    ) -> Result<(), ErroDocumento> {
        let meu = posto(&novo.nome)
            .ok_or_else(|| ErroDocumento::NaoSuportado(format!("<{}> sem posto", novo.nome)))?;
        let alvo = self
            .filhos()
            .find(|(_, n)| posto(&n.nome).is_some_and(|p| p > meu))
            .map(|(i, _)| i);
        let (pos, ref_gap) = match alvo {
            Some(i) => (i, i.checked_sub(1)),
            None => {
                let ultimo = self.filhos().last().map(|(i, _)| i);
                (self.pecas.len() - 1, ultimo.and_then(|i| i.checked_sub(1)))
            }
        };
        let gap = ref_gap
            .and_then(|g| match &self.pecas[g] {
                Peca::Txt(t) => Some(t.clone()),
                Peca::Elem(_) => None,
            })
            .ok_or_else(|| ErroDocumento::NaoSuportado("elemento sem filhos".into()))?;
        if alvo.is_some() {
            // ... gap, NOVO, gap(copia), ref ...
            self.pecas.insert(pos, Peca::Elem(novo));
            self.pecas.insert(pos + 1, Peca::Txt(gap));
        } else {
            // ... ultimo, gap(copia), NOVO, resto ...
            self.pecas.insert(pos, Peca::Txt(gap));
            self.pecas.insert(pos + 1, Peca::Elem(novo));
        }
        Ok(())
    }

    /// Remove o filho `nome` com o espaço que o precedia.
    pub fn remover_filho(&mut self, nome: &str) -> bool {
        let Some(i) = self.filhos().find(|(_, n)| n.nome == nome).map(|(i, _)| i) else {
            return false;
        };
        self.pecas.remove(i);
        self.pecas.remove(i - 1);
        true
    }
}
