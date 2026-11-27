//! Durações exatas: `divisions` é o MMC dos denominadores; o que não couber vira erro explícito.

use super::erro::ErroDocumento;
use crate::partitura::Fracao;

fn mdc(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.abs()
}

/// MMC de dois inteiros positivos, com erro se não couber em `i64`.
pub(crate) fn mmc(a: i64, b: i64) -> Result<i64, ErroDocumento> {
    if a <= 0 || b <= 0 {
        return Err(ErroDocumento::DuracaoInvalida(format!(
            "MMC de {a} e {b}: só vale para positivos"
        )));
    }
    let (a, b) = (i128::from(a), i128::from(b));
    let m = a / mdc(a, b) * b;
    i64::try_from(m).map_err(|_| ErroDocumento::Overflow(format!("MMC de {a} e {b}")))
}

/// MMC dos denominadores: o menor `divisions` que representa todos os valores sem arredondar.
pub fn mmc_dos_denominadores(valores: &[Fracao]) -> Result<i64, ErroDocumento> {
    valores
        .iter()
        .try_fold(1_i64, |acc, v| mmc(acc, v.denominador()))
}

/// Valor em unidades de `1/divisions`. Erro explícito se não for inteiro positivo (nunca
/// arredonda) ou se estourar.
pub(crate) fn para_divisions(valor: Fracao, divisions: i64) -> Result<i64, ErroDocumento> {
    if valor.numerador() <= 0 {
        return Err(ErroDocumento::DuracaoInvalida(format!(
            "{valor}: a duração precisa ser positiva"
        )));
    }
    let prod = i128::from(valor.numerador()) * i128::from(divisions);
    let den = i128::from(valor.denominador());
    if prod % den != 0 {
        return Err(ErroDocumento::DuracaoNaoRepresentavel {
            valor,
            divisions,
            necessarias: mmc(divisions, valor.denominador())?,
        });
    }
    i64::try_from(prod / den).map_err(|_| ErroDocumento::Overflow(format!("{valor} × {divisions}")))
}
