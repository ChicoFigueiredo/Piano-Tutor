//! Documento da partitura fora da interface: o [`DocumentoAberto`] do serviço de documento do
//! núcleo (texto original, [`Partitura`] derivada e identidade de cada nota), os dados musicais
//! ([`Musica`]) e a exportação. Não depende de egui, áudio nem janela (seam S1): a tela de
//! Partitura só o usa. Abrir e exportar passam **somente** pelo serviço (`piano_tutor_core::
//! documento`); aqui ficam apenas a leitura e a gravação de arquivos, com os limites de tamanho.

use crate::ids_lateral::{self, IdsGravados, SituacaoDosIds};
use crate::revisao::{gravar_atomico, sincronizar_diretorio};
use crate::selecao_partitura::IndiceLeitura;
use crate::versoes::{Hash, hash_do_conteudo};
use piano_tutor_core::documento::{
    ComandoDeEdicao, DocumentoAberto, ErroDeEdicao, ErroServico, Exibicao, Formato, IdsPorNota,
    ResultadoDaEdicao, ResultadoDoHistorico, TabelaIds,
};
use piano_tutor_core::mxl::MXL_TAMANHO_MAXIMO_BYTES;
use piano_tutor_core::partitura::{
    EventoReproducao, MapaAndamento, Partitura, TAMANHO_MAXIMO_BYTES,
};
use piano_tutor_core::xml_seguro::nome_para_mensagem;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Dados musicais derivados da partitura, calculados uma vez por carga.
#[derive(Clone, Debug)]
pub struct Musica {
    pub eventos: Vec<EventoReproducao>,
    pub mapa: MapaAndamento,
    /// Início de cada compasso tocado (semínimas na execução) e seu índice.
    pub execucao: Vec<(f64, usize)>,
    pub duracao_q: f64,
    pub bpm_arquivo: Option<f64>,
}

impl Musica {
    pub fn de(p: &Partitura) -> Self {
        Self {
            // Sem uníssonos: a mesma lista (e a mesma numeração) de `esperados_da_partitura`.
            eventos: p.eventos_sem_unissono(),
            mapa: p.mapa_de_andamento(),
            execucao: p
                .compassos_na_execucao()
                .into_iter()
                .map(|(q, c)| (q.como_f64(), c))
                .collect(),
            duracao_q: p.duracao_da_execucao().como_f64(),
            bpm_arquivo: p.andamento_inicial(),
        }
    }

    /// Segundos do início da execução até `q` semínimas, no andamento `bpm_alvo`.
    pub fn segundos(&self, q: f64, bpm_alvo: f64) -> f64 {
        let base = self.bpm_arquivo.unwrap_or(bpm_alvo).max(1.0);
        self.mapa.segundos(q, base, bpm_alvo / base)
    }

    /// Instante (semínimas na execução) do primeiro compasso `c` tocado.
    pub fn inicio_do_compasso(&self, c: usize) -> Option<f64> {
        self.execucao.iter().find(|(_, i)| *i == c).map(|(q, _)| *q)
    }

    /// Compasso e tempo dentro dele para a posição `q` (semínimas na execução).
    pub fn compasso_em(&self, q: f64) -> Option<(usize, f64)> {
        self.execucao
            .iter()
            .rev()
            .find(|(ini, _)| *ini <= q + 1e-9)
            .map(|(ini, c)| (*c, (q - ini).max(0.0)))
    }
}

/// Lê os bytes de um `.musicxml`, `.xml` ou `.mxl` (com os limites de tamanho) e diz o formato.
/// Quem interpreta os bytes é o serviço de documento.
pub fn ler_arquivo(caminho: &Path) -> Result<(Vec<u8>, Formato), String> {
    let formato = caminho
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Formato::de_extensao)
        .ok_or_else(|| "use um arquivo .musicxml, .xml ou .mxl".to_owned())?;
    let nome = nome_curto(caminho);
    let meta =
        std::fs::metadata(caminho).map_err(|e| format!("{nome}: {}", erro_de_leitura(&e)))?;
    if !meta.is_file() {
        return Err(format!("{nome} não é um arquivo"));
    }
    let maximo = match formato {
        Formato::Mxl => MXL_TAMANHO_MAXIMO_BYTES,
        Formato::MusicXml => TAMANHO_MAXIMO_BYTES,
    };
    if meta.len() > maximo as u64 {
        return Err(format!(
            "arquivo grande demais ({} bytes; máximo {maximo})",
            meta.len()
        ));
    }
    // O arquivo pode crescer entre a checagem e a leitura (ou ser um dispositivo que não acaba):
    // a leitura em si também tem teto.
    let mut bytes = Vec::new();
    std::fs::File::open(caminho)
        .and_then(|f| f.take(maximo as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("{nome}: {}", erro_de_leitura(&e)))?;
    if bytes.len() > maximo {
        return Err(format!(
            "arquivo grande demais (mais de {maximo} bytes; máximo {maximo})"
        ));
    }
    Ok((bytes, formato))
}

/// Só o nome do arquivo (nunca a pasta do usuário), curto e sem caracteres de controle.
fn nome_curto(caminho: &Path) -> String {
    caminho.file_name().map_or_else(
        || "arquivo".to_owned(),
        |n| nome_para_mensagem(&n.to_string_lossy()),
    )
}

/// Motivo de uma falha de E/S em português, sem o texto do sistema (que vem em inglês e pode
/// trazer caminho).
fn erro_de_leitura(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::NotFound => "arquivo não encontrado",
        std::io::ErrorKind::PermissionDenied => "sem permissão para ler",
        _ => "não foi possível ler o arquivo",
    }
}

/// Valida o caminho de destino da exportação (acrescenta `.musicxml` se faltar extensão).
pub fn destino_de_exportacao(caminho: &str) -> Result<PathBuf, String> {
    let t = caminho.trim();
    if t.is_empty() {
        return Err("digite o caminho do arquivo de destino".into());
    }
    let mut c = PathBuf::from(t);
    if c.extension().is_none() {
        c.set_extension("musicxml");
    }
    let ext = c
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if ext != "musicxml" && ext != "xml" {
        return Err("o destino deve terminar em .musicxml ou .xml".into());
    }
    Ok(c)
}

/// Partitura carregada: nome, documento aberto pelo serviço (texto original, modelo e
/// identidades de nota) e dados musicais.
#[derive(Clone, Debug)]
pub struct DocumentoPartitura {
    pub nome: String,
    aberto: DocumentoAberto,
    pub musica: Musica,
    /// Ordem de leitura das notas para a seleção e a navegação (ticket 12).
    pub leitura: IndiceLeitura,
    /// Letra, cifra e dedilhado (só mostrados e preservados, D13) e a lista de elementos que o
    /// editor não desenha (ticket 19). Lida do texto original uma vez por abertura: os comandos
    /// de edição de hoje (altura) não tocam nesses elementos.
    exibicao: Exibicao,
    /// O que houve com o arquivo lateral de identidades ao abrir (ticket 17b).
    ids_lateral: SituacaoDosIds,
    /// Versão de que as identidades deste documento descendem, para a linhagem das Versões:
    /// o hash do arquivo aberto (vale só se ele é uma Versão registrada; quem salva confere) ou a
    /// linhagem do arquivo lateral usado. `None` quando não se sabe.
    base_de_ids: Option<Hash>,
}

impl DocumentoPartitura {
    fn de_aberto(nome: String, aberto: DocumentoAberto) -> Self {
        let musica = Musica::de(aberto.partitura());
        let leitura = IndiceLeitura::de(aberto.partitura(), aberto.ids());
        let exibicao = aberto.documento().exibicao();
        Self {
            nome,
            aberto,
            musica,
            leitura,
            exibicao,
            ids_lateral: SituacaoDosIds::Nenhum,
            base_de_ids: None,
        }
    }

    /// Documento recuperado do autosave: aberto a partir do rascunho e já tido como modificado
    /// (difere do arquivo salvo), com o histórico de desfazer vazio.
    pub fn de_recuperado(nome: String, mut aberto: DocumentoAberto) -> Self {
        aberto.marcar_modificado();
        Self::de_aberto(nome, aberto)
    }

    /// Como [`Self::de_recuperado`], guardando o que houve com o arquivo lateral do rascunho e a
    /// linhagem das identidades (ticket 17b).
    pub fn de_recuperado_com_ids(
        nome: String,
        aberto: DocumentoAberto,
        situacao: SituacaoDosIds,
        derivada_de: Option<Hash>,
    ) -> Self {
        let mut d = Self::de_recuperado(nome, aberto);
        d.ids_lateral = situacao;
        d.base_de_ids = derivada_de;
        d
    }

    /// Interpreta o texto MusicXML.
    pub fn de_texto(nome: String, texto: String) -> Result<Self, ErroServico> {
        let aberto = DocumentoAberto::abrir_texto(&nome, &texto)?;
        Ok(Self::de_aberto(nome, aberto))
    }

    /// Abre um arquivo `.musicxml`, `.xml` ou `.mxl`; o nome é o do arquivo.
    pub fn de_arquivo(caminho: &Path, nome_se_faltar: &str) -> Result<Self, String> {
        let (bytes, formato) = ler_arquivo(caminho)?;
        let nome = caminho
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| nome_se_faltar.to_owned());
        // O arquivo lateral de identidades (`<arquivo>.ids`) é tolerante: ausente, de outro texto
        // ou ilegível, o texto abre sem ele (ticket 17b).
        let lateral = ids_lateral::ler(&ids_lateral::caminho_lateral(caminho));
        let (aberto, situacao, derivada_de) = ids_lateral::abrir_tolerante(lateral, |t| {
            DocumentoAberto::abrir_bytes_com_ids(&nome, &bytes, formato, t)
        })
        .map_err(|e| e.to_string())?;
        let mut d = Self::de_aberto(nome, aberto);
        d.base_de_ids = match situacao {
            SituacaoDosIds::Usado => derivada_de,
            // Sem tabela as identidades vêm do próprio texto: se o arquivo é uma Versão
            // registrada (a Hipótese), é dela que descendem.
            _ => Some(hash_do_conteudo(&bytes)),
        };
        d.ids_lateral = situacao;
        Ok(d)
    }

    /// Abre o conteúdo de uma Versão como documento **editável** ("Continuar a partir desta
    /// Versão"). As identidades guardadas com a Versão (`ids`) são usadas se servem (senão o texto
    /// abre sem elas, como sempre, sem erro fatal). A base do documento é a própria Versão: a
    /// próxima Versão salva é filha dela.
    pub fn de_versao(
        nome: String,
        texto: &str,
        ids: Option<IdsGravados>,
        hash: &str,
    ) -> Result<Self, ErroServico> {
        let (aberto, situacao, _) = ids_lateral::abrir_tolerante(ids.map(Ok), |t| {
            DocumentoAberto::abrir_texto_com_ids(&nome, texto, t)
        })?;
        let mut d = Self::de_aberto(nome, aberto);
        d.ids_lateral = situacao;
        d.base_de_ids = Some(hash.to_owned());
        Ok(d)
    }

    /// O que houve com o arquivo lateral de identidades ao abrir (ticket 17b).
    pub fn situacao_dos_ids(&self) -> &SituacaoDosIds {
        &self.ids_lateral
    }

    /// Versão de que as identidades deste documento descendem, se se sabe (ticket 17b).
    pub fn base_de_ids(&self) -> Option<&str> {
        self.base_de_ids.as_deref()
    }

    /// Depois de salvar uma Versão com as identidades atuais: o documento passa a descender dela.
    pub fn definir_base_de_ids(&mut self, hash: Hash) {
        self.base_de_ids = Some(hash);
    }

    /// O texto exportado agora e a tabela de identidades que vale para ele (a que vai no arquivo
    /// lateral, na Versão e no rascunho): reabrir o texto com a tabela devolve as mesmas
    /// identidades.
    pub fn exportado_com_ids(&self) -> Result<(String, TabelaIds), ErroServico> {
        let texto = self.aberto.exportar()?;
        let tabela = self.aberto.tabela_ids_para(&texto);
        Ok((texto, tabela))
    }

    /// O modelo para tocar e desenhar.
    pub fn partitura(&self) -> &Partitura {
        self.aberto.partitura()
    }

    /// Identidade de cada nota do modelo, para o layout.
    pub fn ids(&self) -> &IdsPorNota {
        self.aberto.ids()
    }

    /// Letra, cifra, dedilhado e elementos não suportados, para desenhar e listar.
    pub fn exibicao(&self) -> &Exibicao {
        &self.exibicao
    }

    /// Aplica um comando de edição do serviço de documento (ticket 14) e recalcula o que deriva
    /// da partitura: os dados musicais (tocar) e a ordem de leitura. Só o serviço edita; em erro
    /// nada muda, nem aqui.
    pub fn aplicar(&mut self, comando: ComandoDeEdicao) -> Result<ResultadoDaEdicao, ErroDeEdicao> {
        let r = self.aberto.aplicar(comando)?;
        if r.alterou {
            self.recalcular();
        }
        Ok(r)
    }

    /// Desfaz a última edição (ou gesto) pelo histórico do serviço (ticket 15) e recalcula o que
    /// deriva da partitura. `None` se não há o que desfazer.
    pub fn desfazer(&mut self) -> Option<ResultadoDoHistorico> {
        let r = self.aberto.desfazer()?;
        self.recalcular();
        Some(r)
    }

    /// Refaz a edição desfeita (ticket 15). `None` se não há o que refazer.
    pub fn refazer(&mut self) -> Option<ResultadoDoHistorico> {
        let r = self.aberto.refazer()?;
        self.recalcular();
        Some(r)
    }

    /// O documento difere do estado salvo? (volta a falso ao desfazer até ele)
    pub fn modificado(&self) -> bool {
        self.aberto.modificado()
    }

    /// Registra o estado de agora como o salvo (depois de gravar uma Versão; ticket 15).
    pub fn marcar_salvo(&mut self) {
        self.aberto.marcar_salvo();
    }

    /// Há o que desfazer?
    pub fn pode_desfazer(&self) -> bool {
        self.aberto.pode_desfazer()
    }

    /// Há o que refazer?
    pub fn pode_refazer(&self) -> bool {
        self.aberto.pode_refazer()
    }

    /// Dados musicais e ordem de leitura de novo, depois que o modelo mudou.
    fn recalcular(&mut self) {
        self.musica = Musica::de(self.aberto.partitura());
        self.leitura = IndiceLeitura::de(self.aberto.partitura(), self.aberto.ids());
        // Letra, cifra e dedilhado seguem a nota por IdNota; recalcular mantém a exibição fiel
        // depois de editar, desfazer ou refazer (e quando o ticket 17 inserir ou remover notas).
        self.exibicao = self.aberto.documento().exibicao();
    }

    /// Há compasso editado (sujo)? Abrir e navegar nunca editam.
    pub fn editado(&self) -> bool {
        self.aberto.editado()
    }

    /// Posição (parte e compasso) no formato dos comandos de edição, a partir dos índices do
    /// modelo de tocar (que o cursor de entrada usa). `None` fora dos limites.
    pub fn pos_compasso(
        &self,
        parte: usize,
        compasso: usize,
    ) -> Option<piano_tutor_core::documento::PosCompasso> {
        let p = self.aberto.documento().partes().get(parte)?;
        let c = p.compassos.get(compasso)?;
        Some(piano_tutor_core::documento::PosCompasso {
            parte: p.id.clone(),
            indice: compasso,
            numero: c.numero.clone(),
        })
    }

    /// Compassos editados (sujos), os únicos que a exportação regenera.
    pub fn compassos_editados(&self) -> Vec<piano_tutor_core::documento::PosCompasso> {
        self.aberto.documento().compassos_sujos()
    }

    /// O texto MusicXML exportado agora (com as edições; sem edição, idêntico ao original).
    pub fn exportado(&self) -> Result<String, ErroServico> {
        self.aberto.exportar()
    }

    /// O texto MusicXML original, que a exportação sem edição devolve idêntico.
    pub fn xml(&self) -> &str {
        self.aberto.texto_original()
    }

    /// Grava o MusicXML exportado pelo serviço em `destino` e confere relendo o arquivo.
    ///
    /// A gravação é atômica: o texto vai a um temporário vizinho e entra no lugar por `rename`
    /// (com `Sobrescrever`) ou por `hard_link`, que falha se o destino já existe (sem
    /// `Sobrescrever`). Uma queda no meio deixa o arquivo anterior inteiro, nunca pela metade. O
    /// texto é conferido contra o documento **antes** de gravar, então um export que não confere
    /// não troca arquivo nenhum. Quem chama confere o destino com
    /// [`conferir_destino_de_exportacao`] (este método não sabe o que é Hipótese).
    pub fn gravar(&self, destino: &Path, sobrescrever: bool) -> Result<(), String> {
        conferir_destino_gravavel(destino, sobrescrever)?;
        let texto = self.aberto.exportar().map_err(|e| e.to_string())?;
        self.aberto
            .conferir_exportacao(&texto)
            .map_err(|e| e.to_string())?;
        if sobrescrever {
            gravar_atomico(destino, &texto).map_err(|e| e.to_string())?;
        } else {
            gravar_sem_substituir(destino, &texto).map_err(|e| {
                // A dica de "Sobrescrever" só vale quando o arquivo já existe.
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    format!(
                        "{}: {e} (marque \"Sobrescrever\" para trocar um arquivo)",
                        destino.display()
                    )
                } else {
                    format!("{}: {e}", destino.display())
                }
            })?;
        }
        let lido =
            std::fs::read_to_string(destino).map_err(|e| format!("{}: {e}", destino.display()))?;
        if lido != texto {
            return Err(format!(
                "{}: o arquivo gravado não confere com o exportado",
                destino.display()
            ));
        }
        // O arquivo lateral de identidades vai ao lado do texto gravado (ticket 17b). Falhar aqui
        // não desfaz a exportação, mas também não passa em silêncio.
        let lateral = ids_lateral::caminho_lateral(destino);
        let ids = IdsGravados::nova(self.aberto.tabela_ids_para(&lido), None);
        ids_lateral::gravar(&lateral, &ids).map_err(|e| {
            format!(
                "{} foi gravado, mas não o arquivo de identidades das notas ao lado dele: {e}",
                destino.display()
            )
        })
    }
}

/// Confere, antes de gravar, o que a gravação por temporário e `rename` não deve fazer em
/// silêncio (achados N4 e N9 da segunda revisão independente da E01):
///
/// - a pasta do destino precisa existir (mensagem própria, sem a dica de "Sobrescrever");
/// - um link simbólico como destino é recusado: o `rename` o trocaria por um arquivo comum sem
///   tocar o alvo, e escrever "através" dele alteraria um arquivo que o usuário não escolheu. A
///   escolha mais segura é recusar e pedir o arquivo real;
/// - com "Sobrescrever", um arquivo somente leitura (ou que o processo não pode abrir para
///   escrita) é recusado: o `rename` o substituiria sem aviso, contornando a permissão. As
///   permissões de um arquivo gravável são preservadas pela gravação ([`gravar_atomico`]).
fn conferir_destino_gravavel(destino: &Path, sobrescrever: bool) -> Result<(), String> {
    if let Some(dir) = destino.parent().filter(|d| !d.as_os_str().is_empty())
        && !dir.is_dir()
    {
        return Err(format!("a pasta de destino não existe: {}", dir.display()));
    }
    let Ok(meta) = std::fs::symlink_metadata(destino) else {
        return Ok(());
    };
    if meta.file_type().is_symlink() {
        return Err(format!(
            "{}: o destino é um atalho (link simbólico): a exportação não escreve em atalhos \
             nem os substitui. Escolha o arquivo real.",
            destino.display()
        ));
    }
    if sobrescrever
        && meta.is_file()
        && (meta.permissions().readonly()
            || std::fs::OpenOptions::new()
                .write(true)
                .open(destino)
                .is_err())
    {
        return Err(format!(
            "{}: o destino é somente leitura (ou sem permissão de escrita): a exportação não o \
             substitui. Mude a permissão do arquivo ou escolha outro destino.",
            destino.display()
        ));
    }
    Ok(())
}

/// Cria `destino` com `texto` por temporário vizinho e `hard_link` (que falha se o destino já
/// existe): nunca substitui um arquivo e nunca deixa o destino pela metade.
fn gravar_sem_substituir(destino: &Path, texto: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let nome = destino
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = destino.with_file_name(format!(".{nome}.{}.tmp", std::process::id()));
    let r = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(texto.as_bytes())?;
        f.sync_all()?;
        match std::fs::hard_link(&tmp, destino) {
            Ok(()) => Ok(()),
            // Sem suporte a link físico: `rename` só se o destino ainda não existe.
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists && !destino.exists() => {
                std::fs::rename(&tmp, destino)
            }
            Err(e) => Err(e),
        }
    })();
    let _ = std::fs::remove_file(&tmp);
    if r.is_ok()
        && let Some(dir) = destino.parent()
    {
        let _ = sincronizar_diretorio(if dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            dir
        });
    }
    r
}

/// Caminho canônico mesmo que o arquivo ainda não exista (canoniza a pasta e junta o nome).
fn canonico(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    let pai = p
        .parent()
        .filter(|x| !x.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match (pai.canonicalize(), p.file_name()) {
        (Ok(c), Some(n)) => c.join(n),
        _ => p.to_path_buf(),
    }
}

/// `a` e `b` são o mesmo arquivo? Pelo caminho canônico (resolve `..`, `.` e atalhos) e, no Unix,
/// também por dispositivo e inode (pega link físico e diferença de maiúsculas em disco que não
/// diferencia).
fn mesmo_arquivo(a: &Path, b: &Path) -> bool {
    if canonico(a) == canonico(b) {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) {
            return ma.dev() == mb.dev() && ma.ino() == mb.ino();
        }
    }
    false
}

/// `<stem>.musicxml` numa pasta `partituras/` com `<stem>.estado.toml` ao lado é a Hipótese de um
/// item da Oficina (é como a Revisão e a tela Partitura reconhecem o item).
fn e_hipotese_da_oficina(arquivo: &Path) -> bool {
    let Some(pasta) = arquivo.parent() else {
        return false;
    };
    if !pasta.file_name().is_some_and(|n| n == "partituras") {
        return false;
    }
    let Some(nome) = arquivo.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    // O `.ids` lateral da Hipótese conta como a própria Hipótese.
    let nome = nome
        .strip_suffix(&format!(".{}", ids_lateral::EXTENSAO))
        .unwrap_or(nome);
    nome.strip_suffix(".musicxml")
        .filter(|id| !id.is_empty())
        .is_some_and(|id| pasta.join(format!("{id}.estado.toml")).is_file())
}

/// O destino de uma exportação é seguro? Recusa, com a razão em português, gravar sobre:
/// - um dos `protegidos` (o arquivo aberto, a Hipótese da partitura vinculada), ou o `.ids`
///   lateral deles;
/// - a Hipótese de qualquer item da Oficina (`partituras/<id>.musicxml` com `<id>.estado.toml`);
/// - qualquer arquivo dentro de uma pasta de Versões (`<id>.versoes/`): objetos, `.ids`, índice.
///
/// As comparações são por caminho canonizado (resolve `..`, `.` e atalhos) e, no Unix, por
/// inode (link físico). Vale também para o `.ids` lateral que a exportação grava ao lado do
/// destino. A Hipótese e o arquivo original nunca mudam por uma exportação (A8, ticket 21;
/// revisão independente da E01, achado R2).
pub fn conferir_destino_de_exportacao(
    destino: &Path,
    protegidos: &[(&Path, &str)],
) -> Result<(), String> {
    let recusa = |o_que: &str| {
        Err(format!(
            "o destino é {o_que}: a exportação nunca o sobrescreve (a Hipótese e os originais \
             não são alterados). Escolha outro arquivo."
        ))
    };
    let lateral = ids_lateral::caminho_lateral(destino);
    for candidato in [destino, lateral.as_path()] {
        for (protegido, o_que) in protegidos {
            let lateral_do_protegido = ids_lateral::caminho_lateral(protegido);
            if mesmo_arquivo(candidato, protegido)
                || mesmo_arquivo(candidato, &lateral_do_protegido)
            {
                return recusa(o_que);
            }
        }
        let c = canonico(candidato);
        if e_hipotese_da_oficina(&c) {
            return recusa("a Hipótese de uma partitura da Oficina");
        }
        let em_pasta_de_versoes = c.ancestors().skip(1).any(|a| {
            a.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".versoes"))
        });
        if em_pasta_de_versoes {
            return recusa("um arquivo da pasta de Versões da Oficina");
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::tela_partitura::EMBUTIDAS;
    use piano_tutor_core::mxl::empacotar_mxl;

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("pt-doc-{nome}-{}", std::process::id()));
        std::fs::create_dir_all(&p).expect("pasta");
        p
    }

    #[test]
    fn abre_texto_e_deriva_modelo_e_musica_sem_interface() {
        let d = DocumentoPartitura::de_texto("Ode".into(), EMBUTIDAS[3].1.to_owned())
            .expect("fixture válida");
        assert_eq!(d.nome, "Ode");
        assert_eq!(d.xml(), EMBUTIDAS[3].1, "o texto original é guardado");
        assert_eq!(d.partitura().compassos.len(), 16);
        assert_eq!(
            d.musica.eventos.len(),
            d.partitura().eventos_sem_unissono().len()
        );
        assert!(d.musica.duracao_q > 0.0);
        assert_eq!(d.musica.compasso_em(0.0).map(|(c, _)| c), Some(0));
    }

    #[test]
    fn texto_invalido_e_recusado_com_erro() {
        assert!(DocumentoPartitura::de_texto("x".into(), "<nao-e-musicxml/>".into()).is_err());
        assert!(DocumentoPartitura::de_texto("x".into(), String::new()).is_err());
    }

    #[test]
    fn exporta_o_texto_original_e_nao_sobrescreve_sem_pedir() {
        let d = DocumentoPartitura::de_texto("Ode".into(), EMBUTIDAS[3].1.to_owned()).expect("ok");
        let dir = pasta("exporta");
        let destino = dir.join("saida.musicxml");
        let _ = std::fs::remove_file(&destino);
        d.gravar(&destino, false).expect("grava");
        assert_eq!(
            std::fs::read_to_string(&destino).expect("lê"),
            EMBUTIDAS[3].1
        );
        assert!(
            d.gravar(&destino, false).is_err(),
            "não sobrescreve sem pedir"
        );
        assert!(d.gravar(&destino, true).is_ok());
    }

    #[test]
    fn exportar_confere_a_releitura_pelo_servico() {
        // Se o arquivo gravado deixasse de ser o documento aberto, a conferência do serviço
        // recusaria (cobertura do caminho de erro no teste do núcleo e aqui pelo mesmo contrato).
        let d = DocumentoPartitura::de_texto("Ode".into(), EMBUTIDAS[3].1.to_owned()).expect("ok");
        let outro =
            DocumentoPartitura::de_texto("B".into(), EMBUTIDAS[0].1.to_owned()).expect("ok");
        let e = d
            .aberto
            .conferir_exportacao(outro.xml())
            .expect_err("difere");
        assert!(e.to_string().contains("releitura"), "{e}");
    }

    #[test]
    fn abrir_sem_editar_nao_grava_nada_em_disco() {
        let dir = pasta("sem-gravar");
        let xml = dir.join("ode.musicxml");
        std::fs::write(&xml, EMBUTIDAS[3].1).expect("grava");
        let mxl = dir.join("ode.mxl");
        std::fs::write(
            &mxl,
            empacotar_mxl("ode.musicxml", EMBUTIDAS[3].1).expect("zip"),
        )
        .expect("grava");
        let instantaneo = || {
            let mut v: Vec<_> = std::fs::read_dir(&dir)
                .expect("lê")
                .map(|e| {
                    let e = e.expect("entrada");
                    let m = e.metadata().expect("meta");
                    (e.file_name(), m.len(), m.modified().expect("mtime"))
                })
                .collect();
            v.sort();
            v
        };
        let antes = instantaneo();
        for c in [&xml, &mxl] {
            let d = DocumentoPartitura::de_arquivo(c, "?").expect("abre");
            assert!(!d.aberto.editado());
            // Exportar devolve o texto, sem tocar o disco (quem grava é `gravar`).
            let _ = d.aberto.exportar().expect("exporta");
        }
        assert_eq!(instantaneo(), antes, "nenhum arquivo criado nem alterado");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rota_de_abrir_e_exportar_so_usa_o_servico_sem_parse_direto() {
        // Ticket 10: nenhum parse direto de MusicXML/MXL na rota da tela (só o serviço do núcleo).
        for (nome, fonte) in [
            (
                "documento_partitura.rs",
                include_str!("documento_partitura.rs"),
            ),
            ("tela_partitura.rs", include_str!("tela_partitura.rs")),
        ] {
            let producao = fonte
                .split("#[cfg(test)]\nmod testes")
                .next()
                .unwrap_or_default();
            for proibido in ["de_musicxml", "extrair_musicxml", "de_mxl", "roxmltree"] {
                assert!(!producao.contains(proibido), "{nome} usa {proibido}");
            }
        }
    }

    #[test]
    fn erros_de_leitura_nao_vazam_o_caminho_completo_e_estao_em_portugues() {
        let dir = pasta("sem-vazar");
        let segredo = dir.join("pasta-do-usuario-secreta");
        std::fs::create_dir_all(&segredo).expect("pasta");
        let ausente = segredo.join("some.mxl");
        let falso = segredo.join("lixo.musicxml");
        std::fs::write(&falso, "isto não é xml").expect("grava");
        let diretorio = segredo.join("pasta.musicxml");
        std::fs::create_dir_all(&diretorio).expect("pasta");
        for c in [&ausente, &falso, &diretorio] {
            let e = DocumentoPartitura::de_arquivo(c, "?").expect_err("recusa");
            assert!(!e.contains("pasta-do-usuario-secreta"), "vaza caminho: {e}");
            assert!(!e.contains(&*dir.to_string_lossy()), "vaza caminho: {e}");
            let baixa = e.to_lowercase();
            for ingles in ["no such file", "os error", "not a directory", "permission"] {
                assert!(!baixa.contains(ingles), "mensagem em inglês: {e}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn identidade_de_cada_nota_chega_do_servico() {
        let d = DocumentoPartitura::de_texto("Ode".into(), EMBUTIDAS[3].1.to_owned()).expect("ok");
        assert_eq!(d.ids().total(), d.partitura().total_de_notas());
        assert!(d.ids().id(0, 0, 0).is_some());
    }

    #[test]
    fn destino_valida_extensao_e_completa_a_que_falta() {
        assert!(destino_de_exportacao("").is_err());
        assert!(destino_de_exportacao("  ").is_err());
        assert!(destino_de_exportacao("a.txt").is_err());
        assert_eq!(
            destino_de_exportacao("a/b").expect("ok"),
            PathBuf::from("a/b.musicxml")
        );
        assert!(destino_de_exportacao("a.XML").is_ok());
    }

    #[test]
    fn abre_arquivo_xml_e_mxl_e_recusa_o_que_nao_serve() {
        let dir = pasta("abre");
        let xml = dir.join("ode.musicxml");
        std::fs::write(&xml, EMBUTIDAS[3].1).expect("grava");
        let d = DocumentoPartitura::de_arquivo(&xml, "?").expect("abre xml");
        assert_eq!(d.nome, "ode.musicxml");
        assert_eq!(d.xml(), EMBUTIDAS[3].1);

        let mxl = dir.join("ode.mxl");
        std::fs::write(
            &mxl,
            empacotar_mxl("ode.musicxml", EMBUTIDAS[3].1).expect("zip"),
        )
        .expect("grava");
        let m = DocumentoPartitura::de_arquivo(&mxl, "?").expect("abre mxl");
        assert_eq!(m.xml(), EMBUTIDAS[3].1, "o .mxl entrega o mesmo XML");
        assert_eq!(m.partitura(), d.partitura());

        let ruim = dir.join("a.txt");
        std::fs::write(&ruim, "x").expect("grava");
        assert!(DocumentoPartitura::de_arquivo(&ruim, "?").is_err());
        assert!(DocumentoPartitura::de_arquivo(&dir.join("nao-existe.xml"), "?").is_err());
        let falso = dir.join("falso.mxl");
        std::fs::write(&falso, "não é zip").expect("grava");
        assert!(DocumentoPartitura::de_arquivo(&falso, "?").is_err());
    }

    #[test]
    fn r4_ids_lateral_hostil_e_ignorado_sem_panico_e_o_texto_abre() {
        let dir = pasta("ids-hostil");
        let xml = dir.join("hostil.musicxml");
        std::fs::write(&xml, EMBUTIDAS[3].1).expect("grava");
        let lateral = ids_lateral::caminho_lateral(&xml);
        for hostil in [
            "piano-tutor-ids v1\nproximo=18446744073709551615\n",
            "piano-tutor-ids v1\nproximo=10\npos\tP1\t0\t0\t1\npos\tP1\t0\t1\t1\n",
            "piano-tutor-ids v1\nxml\ta\t7\nproximo=2\n",
        ] {
            std::fs::write(&lateral, hostil).expect("grava");
            let d = DocumentoPartitura::de_arquivo(&xml, "?").expect("abre sem a tabela");
            assert!(
                matches!(d.situacao_dos_ids(), SituacaoDosIds::Recusado(_)),
                "{:?}",
                d.situacao_dos_ids()
            );
            let sem =
                DocumentoPartitura::de_texto("x".into(), EMBUTIDAS[3].1.to_owned()).expect("abre");
            assert_eq!(
                d.partitura(),
                sem.partitura(),
                "o mesmo texto, as mesmas notas"
            );
        }
    }
}
