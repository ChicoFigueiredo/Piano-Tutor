//! Editor de sincronia do Player de Aula (ADR-0011): Âncoras sobre o Alinhamento.
//!
//! Tempo± desloca e acelerar± estica ou comprime as notas do trecho; marcar uma Âncora fixa uma
//! nota num instante e reinterpola as notas entre Âncoras vizinhas pela proporção da
//! partitura. Salvar grava o Alinhamento como Hipótese (`metodo = "editor"`) na oficina do
//! tutor, de onde o `scripts/atualizar-pacote-curso.sh` o leva ao pacote.

use piano_tutor_curso::{Alinhamento, EstadoConteudo, NotaAlinhada};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Passo de tempo± (s) e de acelerar± (fração).
pub const PASSO_TEMPO_S: f64 = 0.05;
pub const PASSO_ACELERAR: f64 = 0.01;
/// Distância máxima (s) para "marcar Âncora aqui" pegar uma nota.
pub const ALCANCE_MARCAR_S: f64 = 0.6;

/// Uma edição em curso do Alinhamento de um Exercício.
#[derive(Clone, Debug, PartialEq)]
pub struct EdicaoSincronia {
    original: Alinhamento,
    pub atual: Alinhamento,
    /// Índices (do Alinhamento) das notas fixadas à mão.
    pub ancoras: BTreeSet<u32>,
    desfazer: Vec<(Alinhamento, BTreeSet<u32>)>,
}

impl EdicaoSincronia {
    pub fn nova(al: &Alinhamento) -> Self {
        Self {
            original: al.clone(),
            atual: al.clone(),
            ancoras: BTreeSet::new(),
            desfazer: Vec::new(),
        }
    }

    pub fn alterada(&self) -> bool {
        self.atual != self.original
    }

    pub fn original(&self) -> &Alinhamento {
        &self.original
    }

    fn guardar(&mut self) {
        self.desfazer
            .push((self.atual.clone(), self.ancoras.clone()));
        if self.desfazer.len() > 200 {
            self.desfazer.remove(0);
        }
    }

    pub fn desfazer(&mut self) -> bool {
        match self.desfazer.pop() {
            Some((a, an)) => {
                self.atual = a;
                self.ancoras = an;
                true
            }
            None => false,
        }
    }

    /// Desloca as notas `indices` (em geral, as do trecho) em `ds` segundos.
    pub fn deslocar(&mut self, indices: &[u32], ds: f64) {
        if indices.is_empty() || !ds.is_finite() {
            return;
        }
        self.guardar();
        for n in self
            .atual
            .notas
            .iter_mut()
            .filter(|n| indices.contains(&n.indice))
        {
            n.t_audio_s = (n.t_audio_s + ds).max(0.0);
        }
    }

    /// Estica (`fator` > 1) ou comprime (< 1) as notas `indices` em volta da primeira delas.
    pub fn escalar(&mut self, indices: &[u32], fator: f64) {
        if !(fator.is_finite() && fator > 0.0) {
            return;
        }
        let Some(t0) = self
            .atual
            .notas
            .iter()
            .filter(|n| indices.contains(&n.indice))
            .map(|n| n.t_audio_s)
            .reduce(f64::min)
        else {
            return;
        };
        self.guardar();
        for n in self
            .atual
            .notas
            .iter_mut()
            .filter(|n| indices.contains(&n.indice))
        {
            n.t_audio_s = t0 + (n.t_audio_s - t0) * fator;
        }
    }

    /// Fixa a nota `indice` em `t_audio` como Âncora e reinterpola as vizinhas.
    pub fn fixar_ancora(&mut self, indice: u32, t_audio: f64) {
        if !t_audio.is_finite() || !self.atual.notas.iter().any(|n| n.indice == indice) {
            return;
        }
        self.guardar();
        self.ancoras.insert(indice);
        let antes = self
            .atual
            .notas
            .iter()
            .find(|n| n.indice == indice)
            .map_or(t_audio, |n| n.t_audio_s);
        if self.ancoras.len() == 1 {
            // A primeira Âncora leva todas as notas junto (é um deslocamento).
            let ds = t_audio - antes;
            for n in &mut self.atual.notas {
                n.t_audio_s = (n.t_audio_s + ds).max(0.0);
            }
        } else {
            if let Some(n) = self.atual.notas.iter_mut().find(|n| n.indice == indice) {
                n.t_audio_s = t_audio.max(0.0);
            }
            reinterpolar(&mut self.atual.notas, &self.ancoras);
        }
    }

    /// "Marcar Âncora aqui": a nota de `candidatas` mais perto de `t_audio` vira Âncora nesse
    /// instante. Devolve o índice marcado.
    pub fn marcar_aqui(&mut self, candidatas: &[u32], t_audio: f64) -> Option<u32> {
        let n = self
            .atual
            .notas
            .iter()
            .filter(|n| candidatas.contains(&n.indice))
            .filter(|n| (n.t_audio_s - t_audio).abs() <= ALCANCE_MARCAR_S)
            .min_by(|a, b| {
                (a.t_audio_s - t_audio)
                    .abs()
                    .total_cmp(&(b.t_audio_s - t_audio).abs())
            })?
            .indice;
        self.fixar_ancora(n, t_audio);
        Some(n)
    }

    /// Corrige a altura (MIDI) da nota `indice` (auditoria: o vídeo mostra outra tecla).
    pub fn trocar_nota(&mut self, indice: u32, midi: u8) {
        if self
            .atual
            .notas
            .iter()
            .any(|n| n.indice == indice && n.midi != midi)
        {
            self.guardar();
            if let Some(n) = self.atual.notas.iter_mut().find(|n| n.indice == indice) {
                n.midi = midi;
            }
        }
    }

    pub fn soltar_ancora(&mut self, indice: u32) {
        if self.ancoras.contains(&indice) {
            self.guardar();
            self.ancoras.remove(&indice);
        }
    }

    /// O Alinhamento a salvar: Hipótese, feita no editor.
    pub fn para_salvar(&self) -> Alinhamento {
        let mut a = self.atual.clone();
        a.estado = EstadoConteudo::Hipotese;
        a.metodo = "editor".to_owned();
        a.notas.sort_by_key(|x| x.indice);
        a
    }
}

/// Reposiciona as notas que não são Âncora (com duas Âncoras ou mais): entre duas, pela
/// proporção do tempo da partitura; antes da primeira e depois da última, com o mesmo
/// deslocamento da Âncora mais próxima.
fn reinterpolar(notas: &mut [NotaAlinhada], ancoras: &BTreeSet<u32>) {
    let mut fixas: Vec<(f64, f64)> = notas
        .iter()
        .filter(|n| ancoras.contains(&n.indice))
        .map(|n| (n.t_partitura_s, n.t_audio_s))
        .collect();
    fixas.sort_by(|a, b| a.0.total_cmp(&b.0));
    fixas.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
    if fixas.len() < 2 {
        return;
    }
    let (primeira, ultima) = (fixas[0], fixas[fixas.len() - 1]);
    for n in notas.iter_mut().filter(|n| !ancoras.contains(&n.indice)) {
        let p = n.t_partitura_s;
        n.t_audio_s = if p <= primeira.0 {
            p - primeira.0 + primeira.1
        } else if p >= ultima.0 {
            p - ultima.0 + ultima.1
        } else {
            let j = fixas
                .iter()
                .position(|f| f.0 >= p)
                .unwrap_or(fixas.len() - 1);
            let (a, b) = (fixas[j - 1], fixas[j]);
            let r = (p - a.0) / (b.0 - a.0).max(1e-9);
            a.1 + r * (b.1 - a.1)
        }
        .max(0.0);
    }
}

/// Onde a oficina do tutor guarda o Alinhamento do Exercício `id`.
pub fn arquivo_na_oficina(dados: &Path, tutor: &str, exercicio: &str) -> PathBuf {
    dados
        .join("fontes")
        .join(tutor)
        .join("oficina")
        .join("alinhamentos")
        .join(format!("{exercicio}.json"))
}

/// Grava o Alinhamento em `destino`, guardando antes uma cópia do que havia lá
/// (`<id>.json.antes-<n>`). Devolve o caminho da cópia, se houve.
pub fn salvar(destino: &Path, al: &Alinhamento) -> Result<Option<PathBuf>, String> {
    let texto = serde_json::to_string_pretty(al).map_err(|e| e.to_string())?;
    salvar_texto(destino, &texto)
}

/// Grava `texto` em `destino` do mesmo jeito (cópia do anterior, troca atômica).
pub fn salvar_texto(destino: &Path, texto: &str) -> Result<Option<PathBuf>, String> {
    if let Some(pai) = destino.parent() {
        fs::create_dir_all(pai).map_err(|e| format!("{}: {e}", pai.display()))?;
    }
    let mut copia = None;
    if destino.is_file() {
        let nome = destino
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let c = (1..10_000)
            .map(|i| destino.with_file_name(format!("{nome}.antes-{i}")))
            .find(|c| !c.exists())
            .ok_or("muitas cópias antigas do Alinhamento")?;
        fs::copy(destino, &c).map_err(|e| format!("{}: {e}", c.display()))?;
        copia = Some(c);
    }
    let tmp = destino.with_extension("json.tmp");
    fs::write(&tmp, format!("{texto}\n")).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, destino).map_err(|e| format!("{}: {e}", destino.display()))?;
    Ok(copia)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn nota(indice: u32, tp: f64, ta: f64) -> NotaAlinhada {
        NotaAlinhada {
            indice,
            compasso: 1,
            midi: 60,
            t_partitura_s: tp,
            t_audio_s: ta,
            confianca: 1.0,
        }
    }

    fn al() -> Alinhamento {
        Alinhamento {
            formato_versao: 1,
            exercicio: "x".into(),
            audio: None,
            estado: EstadoConteudo::Aprovada,
            metodo: "dtw-croma".into(),
            offset_aula_s: 100.0,
            notas: (0..5)
                .map(|i| nota(i, f64::from(i), 1.0 + f64::from(i)))
                .collect(),
        }
    }

    fn tempos(e: &EdicaoSincronia) -> Vec<f64> {
        e.atual
            .notas
            .iter()
            .map(|n| (n.t_audio_s * 1000.0).round() / 1000.0)
            .collect()
    }

    #[test]
    fn deslocar_e_desfazer() {
        let mut e = EdicaoSincronia::nova(&al());
        e.deslocar(&[1, 2], 0.05);
        assert_eq!(tempos(&e), vec![1.0, 2.05, 3.05, 4.0, 5.0]);
        assert!(e.alterada());
        assert!(e.desfazer());
        assert!(!e.alterada());
    }

    #[test]
    fn escalar_em_volta_da_primeira() {
        let mut e = EdicaoSincronia::nova(&al());
        e.escalar(&[0, 1, 2, 3, 4], 1.1);
        assert_eq!(tempos(&e), vec![1.0, 2.1, 3.2, 4.3, 5.4]);
    }

    #[test]
    fn duas_ancoras_reinterpolam_o_meio_e_deslocam_as_pontas() {
        let mut e = EdicaoSincronia::nova(&al());
        e.fixar_ancora(1, 2.5); // partitura 1 s; desloca tudo +0,5
        e.fixar_ancora(3, 6.5); // partitura 3 s: entre elas, 2 s de partitura viram 4 s
        assert_eq!(tempos(&e), vec![1.5, 2.5, 4.5, 6.5, 7.5]);
        assert_eq!(e.ancoras.iter().copied().collect::<Vec<_>>(), vec![1, 3]);
    }

    #[test]
    fn marcar_aqui_pega_a_nota_mais_perto_dentro_do_alcance() {
        let mut e = EdicaoSincronia::nova(&al());
        assert_eq!(e.marcar_aqui(&[0, 1, 2], 2.2), Some(1));
        // A primeira Âncora desloca tudo junto.
        assert_eq!(tempos(&e), vec![1.2, 2.2, 3.2, 4.2, 5.2]);
        assert_eq!(e.marcar_aqui(&[0, 1, 2], 9.5), None);
    }

    #[test]
    fn salvar_vira_hipotese_do_editor_e_guarda_copia() {
        let d = tempfile::tempdir().expect("tmp");
        let destino = arquivo_na_oficina(d.path(), "t", "x");
        let mut e = EdicaoSincronia::nova(&al());
        assert_eq!(salvar(&destino, &e.para_salvar()).expect("1ª"), None);
        e.deslocar(&[0], 0.1);
        let copia = salvar(&destino, &e.para_salvar())
            .expect("2ª")
            .expect("cópia");
        assert!(copia.is_file());
        let lido = Alinhamento::de_json(&fs::read(&destino).expect("lê")).expect("json");
        assert_eq!(lido.estado, EstadoConteudo::Hipotese);
        assert_eq!(lido.metodo, "editor");
        assert!((lido.notas[0].t_audio_s - 1.1).abs() < 1e-9);
    }
}
