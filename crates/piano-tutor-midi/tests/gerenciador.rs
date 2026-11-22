//! Testes de ponta a ponta do gerenciador com a fonte falsa e relógio manual
//! (sem hardware e sem `sleep` longo).

use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::EventoMidi;
use piano_tutor_core::teclado::{EstadoTeclas, Teclado61};
use piano_tutor_midi::{
    EstadoConexao, FonteFalsa, GerenciadorMidi, Observacao, RelogioManual, SaidaMidi,
    aplicar_em_teclas,
};
use serde_json::Value;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

const KB: &str = "KB-330:KB-330 MIDI 1 20:0";
const KB_NOVO_CLIENTE: &str = "KB-330:KB-330 MIDI 1 24:0";
const THROUGH: &str = "Midi Through:Midi Through Port-0 14:0";
const PASSO: Duration = Duration::from_millis(50);

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Buffer {
    fn linhas(&self) -> Vec<Value> {
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        String::from_utf8(bytes)
            .expect("log em UTF-8")
            .lines()
            .map(|l| {
                serde_json::from_str(l).unwrap_or_else(|e| panic!("linha inválida {l:?}: {e}"))
            })
            .collect()
    }
}

struct Bancada {
    fonte: FonteFalsa,
    relogio: RelogioManual,
    g: GerenciadorMidi,
    log: Buffer,
}

fn bancada(portas: &[&str]) -> Bancada {
    let fonte = FonteFalsa::nova(portas.iter().copied());
    let relogio = RelogioManual::novo();
    let log = Buffer::default();
    let g = GerenciadorMidi::novo(Box::new(fonte.clone()), Arc::new(relogio.clone()))
        .com_registro(Box::new(log.clone()));
    Bancada {
        fonte,
        relogio,
        g,
        log,
    }
}

fn nota(n: u8) -> NotaMidi {
    NotaMidi::nova(n).expect("nota válida")
}

fn eventos(saidas: &[SaidaMidi]) -> Vec<EventoMidi> {
    saidas
        .iter()
        .filter_map(|s| match s {
            SaidaMidi::Evento(e) => Some(*e),
            _ => None,
        })
        .collect()
}

fn conectado(porta: &str) -> EstadoConexao {
    EstadoConexao::Conectado {
        porta: porta.to_string(),
    }
}

#[test]
fn selecao_manual_conecta_e_entrega_eventos_com_timestamp_do_relogio() {
    let mut b = bancada(&[THROUGH, KB]);
    let saidas = b.g.atualizar();
    assert!(saidas.is_empty());
    assert_eq!(b.g.portas(), &[THROUGH.to_string(), KB.to_string()]);
    assert_eq!(b.g.estado(), &EstadoConexao::Desconectado);

    b.g.selecionar_porta(KB);
    assert_eq!(b.g.estado(), &conectado(KB));
    assert_eq!(b.fonte.conexao_aberta().as_deref(), Some(KB));

    b.relogio.avancar(Duration::from_millis(7));
    assert!(b.fonte.injetar(&[0x90, 60, 100]));
    assert!(b.fonte.injetar(&[0x80, 60, 64]));
    let saidas = b.g.atualizar();
    assert_eq!(
        saidas[..2],
        [
            SaidaMidi::EstadoMudou(EstadoConexao::Conectando {
                porta: KB.to_string()
            }),
            SaidaMidi::EstadoMudou(conectado(KB)),
        ]
    );
    let ev = eventos(&saidas);
    assert_eq!(ev.len(), 2);
    assert!(matches!(
        ev[0],
        EventoMidi::NotaLigada {
            velocidade: 100,
            timestamp_monotonico_ns: 7_000_000,
            ..
        }
    ));
    assert!(matches!(
        ev[1],
        EventoMidi::NotaDesligada { velocidade: 64, .. }
    ));
    assert_eq!(b.g.capacidades().liberacao_de_nota(), Observacao::Observado);
}

#[test]
fn porta_ausente_vira_erro_e_conecta_sozinha_quando_aparece() {
    let mut b = bancada(&[THROUGH]);
    b.g.selecionar_porta(KB);
    assert!(matches!(b.g.estado(), EstadoConexao::Erro { .. }));
    assert!(b.g.aguardando_reconexao());

    b.fonte.adicionar_porta(KB);
    for _ in 0..30 {
        b.relogio.avancar(PASSO);
        b.g.atualizar();
        if b.g.estado() == &conectado(KB) {
            break;
        }
    }
    assert_eq!(b.g.estado(), &conectado(KB));
    assert!(b.relogio_ms() <= 1_000);
}

impl Bancada {
    fn relogio_ms(&self) -> u64 {
        use piano_tutor_midi::Relogio;
        self.relogio.agora_ns() / 1_000_000
    }
}

#[test]
fn dez_mil_pares_sem_perda_ponta_a_ponta() {
    const PARES: u32 = 10_000;
    let mut b = bancada(&[KB]);
    b.g.selecionar_porta(KB);
    let mut teclas = EstadoTeclas::novo(Teclado61::padrao_para_testes_e_simulacao());

    // Injeção em outra thread, como a thread do backend real.
    let injetor = {
        let fonte = b.fonte.clone();
        std::thread::spawn(move || {
            let mut entregues = 0u32;
            for i in 0..PARES {
                let n = 36 + u8::try_from(i % 61).expect("cabe");
                let ts = u64::from(i) * 2;
                entregues += u32::from(fonte.injetar_com_ts(&[0x90, n, 1 + (i % 127) as u8], ts));
                entregues += u32::from(fonte.injetar_com_ts(&[0x80, n, 0], ts + 1));
            }
            entregues
        })
    };

    let limite = Instant::now() + Duration::from_secs(30);
    let mut recebidos: Vec<EventoMidi> = Vec::with_capacity(2 * PARES as usize);
    let mut maximo_simultaneo = 0;
    while recebidos.len() < 2 * PARES as usize && Instant::now() < limite {
        let saidas = b.g.atualizar();
        assert_eq!(aplicar_em_teclas(&saidas, &mut teclas), 0);
        maximo_simultaneo = maximo_simultaneo.max(teclas.quantidade());
        recebidos.extend(eventos(&saidas));
    }
    assert_eq!(injetor.join().expect("injetor"), 2 * PARES);
    recebidos.extend(eventos(&b.g.atualizar()));

    assert_eq!(recebidos.len(), 2 * PARES as usize, "pares perdidos");
    for (i, par) in recebidos.chunks(2).enumerate() {
        let n = 36 + u8::try_from(i % 61).expect("cabe");
        let ts = u64::try_from(i).expect("cabe") * 2;
        assert!(
            matches!(par[0], EventoMidi::NotaLigada { nota: x, timestamp_monotonico_ns: t, .. } if x == nota(n) && t == ts),
            "par {i}: {:?}",
            par[0]
        );
        assert!(
            matches!(par[1], EventoMidi::NotaDesligada { nota: x, timestamp_monotonico_ns: t, .. } if x == nota(n) && t == ts + 1),
            "par {i}: {:?}",
            par[1]
        );
    }
    assert_eq!(teclas.quantidade(), 0, "nota presa");
    assert!(maximo_simultaneo <= 61);
    let c = b.g.contadores();
    assert_eq!(c.mensagens_recebidas, 2 * u64::from(PARES));
    assert_eq!(c.eventos, 2 * u64::from(PARES));
    assert_eq!(c.erros_de_interpretacao, 0);

    let linhas = b.log.linhas();
    let notas = linhas
        .iter()
        .filter(|l| l["tipo"] == "nota_ligada" || l["tipo"] == "nota_desligada")
        .count();
    assert_eq!(notas, 2 * PARES as usize);
    assert_eq!(b.g.registro_linhas_e_falhas().map(|x| x.1), Some(0));
    assert_eq!(
        b.g.capacidades().velocidade_variavel(),
        Observacao::Observado
    );
}

#[test]
fn desconexao_muda_status_em_ate_3_s_e_limpa_notas() {
    let mut b = bancada(&[KB]);
    b.g.selecionar_porta(KB);
    let mut teclas = EstadoTeclas::novo(Teclado61::padrao_para_testes_e_simulacao());
    aplicar_em_teclas(&b.g.atualizar(), &mut teclas);

    // Acorde de 3 notas pressionado.
    for n in [60, 64, 67] {
        b.fonte.injetar(&[0x90, n, 90]);
    }
    b.relogio.avancar(Duration::from_millis(370));
    aplicar_em_teclas(&b.g.atualizar(), &mut teclas);
    assert_eq!(teclas.quantidade(), 3);

    // Última mensagem chega e o cabo é puxado em seguida.
    b.fonte.injetar(&[0x90, 72, 90]);
    let desplugado_ns = {
        use piano_tutor_midi::Relogio;
        b.relogio.agora_ns()
    };
    b.fonte.remover_porta(KB);
    assert!(!b.fonte.injetar(&[0x90, 74, 90]), "porta morta não entrega");

    let mut todas = Vec::new();
    let mut detectado_ns = None;
    for _ in 0..200 {
        b.relogio.avancar(PASSO);
        let saidas = b.g.atualizar();
        aplicar_em_teclas(&saidas, &mut teclas);
        todas.extend(saidas);
        if b.g.estado() == &EstadoConexao::Desconectado {
            use piano_tutor_midi::Relogio;
            detectado_ns = Some(b.relogio.agora_ns());
            break;
        }
    }
    let atraso = detectado_ns.expect("desconexão detectada") - desplugado_ns;
    assert!(atraso <= 3_000_000_000, "atraso {atraso} ns > 3 s");
    assert!(
        atraso <= 1_000_000_000 + 50_000_000,
        "atraso {atraso} ns > intervalo + passo"
    );

    // A nota que chegou antes do desplugue é entregue antes da limpeza.
    let pos_evento = todas
        .iter()
        .position(|s| matches!(s, SaidaMidi::Evento(EventoMidi::NotaLigada { nota: n, .. }) if *n == nota(72)))
        .expect("nota 72 entregue");
    let pos_limpeza = todas
        .iter()
        .position(|s| *s == SaidaMidi::LimparNotas)
        .expect("limpeza emitida");
    assert!(pos_evento < pos_limpeza);
    assert_eq!(teclas.quantidade(), 0, "notas pendentes limpas");
    assert!(b.g.aguardando_reconexao());
    assert_eq!(b.g.contadores().desconexoes_detectadas, 1);
    assert_eq!(b.fonte.conexao_aberta(), None, "conexão antiga fechada");
}

#[test]
fn desconexao_detectada_em_ate_2_intervalos_com_atualizar_a_cada_1_s() {
    let mut b = bancada(&[KB]);
    b.g.selecionar_porta(KB);
    b.relogio.avancar(Duration::from_millis(999));
    b.g.atualizar();
    b.fonte.remover_porta(KB);
    let mut passos = 0;
    while b.g.estado() != &EstadoConexao::Desconectado && passos < 10 {
        b.relogio.avancar(Duration::from_secs(1));
        b.g.atualizar();
        passos += 1;
    }
    assert!(passos <= 2, "{passos} s");
    assert!(b.g.proxima_varredura_em() <= Duration::from_secs(1));
}

#[test]
fn reconexao_restaura_operacao_mesmo_com_novo_cliente_alsa() {
    let mut b = bancada(&[THROUGH, KB]);
    b.g.selecionar_porta(KB);
    b.g.atualizar();
    b.fonte.injetar(&[0x90, 60, 100]);
    b.fonte.remover_porta(KB);
    b.relogio.avancar(Duration::from_secs(1));
    let saidas = b.g.atualizar();
    assert!(saidas.contains(&SaidaMidi::LimparNotas));
    assert_eq!(b.g.estado(), &EstadoConexao::Desconectado);

    // Volta com outro número de cliente ALSA.
    b.fonte.adicionar_porta(KB_NOVO_CLIENTE);
    b.relogio.avancar(Duration::from_secs(1));
    let saidas = b.g.atualizar();
    assert!(saidas.contains(&SaidaMidi::EstadoMudou(conectado(KB_NOVO_CLIENTE))));
    assert_eq!(b.g.porta_selecionada(), Some(KB_NOVO_CLIENTE));
    assert_eq!(b.g.contadores().reconexoes, 1);
    assert_eq!(b.fonte.conexoes_realizadas(), 2);

    // O running status da conexão anterior não vaza para a nova.
    assert!(b.fonte.injetar(&[62, 100]));
    assert!(b.fonte.injetar(&[0x90, 64, 100]));
    let ev = eventos(&b.g.atualizar());
    assert_eq!(ev.len(), 1);
    assert!(matches!(ev[0], EventoMidi::NotaLigada { nota: n, .. } if n == nota(64)));
    assert_eq!(b.g.contadores().erros_de_interpretacao, 1);

    let linhas = b.log.linhas();
    let conexoes: Vec<&Value> = linhas.iter().filter(|l| l["tipo"] == "conectado").collect();
    assert_eq!(conexoes.len(), 2);
    assert_eq!(conexoes[1]["reconexao"], true);
    assert!(
        linhas
            .iter()
            .any(|l| l["tipo"] == "desconectado" && l["motivo"] == "porta_ausente")
    );
    assert!(linhas.iter().any(|l| l["tipo"] == "erro_midi"));
}

#[test]
fn falha_de_conexao_vira_erro_e_tenta_de_novo() {
    let mut b = bancada(&[KB]);
    b.fonte.falhar_proximas_conexoes(1);
    b.g.selecionar_porta(KB);
    assert!(matches!(b.g.estado(), EstadoConexao::Erro { .. }));
    b.relogio.avancar(Duration::from_secs(1));
    b.g.atualizar();
    assert_eq!(b.g.estado(), &conectado(KB));
    assert!(b.log.linhas().iter().any(|l| l["tipo"] == "erro_conexao"));
}

#[test]
fn falha_de_listagem_nao_derruba_conexao_viva() {
    let mut b = bancada(&[KB]);
    b.g.selecionar_porta(KB);
    b.fonte.definir_falha_de_listagem(true);
    for _ in 0..5 {
        b.relogio.avancar(Duration::from_secs(1));
        b.g.atualizar();
    }
    assert_eq!(b.g.estado(), &conectado(KB));
    assert!(b.g.ultimo_erro_de_listagem().is_some());
    assert_eq!(b.g.contadores().falhas_de_listagem, 5);
    b.fonte.definir_falha_de_listagem(false);
    b.relogio.avancar(Duration::from_secs(1));
    b.g.atualizar();
    assert!(b.g.ultimo_erro_de_listagem().is_none());
}

#[test]
fn desconectar_manual_limpa_e_nao_reconecta() {
    let mut b = bancada(&[KB]);
    b.g.selecionar_porta(KB);
    b.g.atualizar();
    b.g.desconectar();
    let saidas = b.g.atualizar();
    assert!(saidas.contains(&SaidaMidi::LimparNotas));
    assert_eq!(b.g.estado(), &EstadoConexao::Desconectado);
    b.relogio.avancar(Duration::from_secs(3));
    b.g.atualizar();
    assert_eq!(b.g.estado(), &EstadoConexao::Desconectado);
    assert_eq!(b.fonte.conexao_aberta(), None);
    assert!(!b.g.aguardando_reconexao());
}

#[test]
fn troca_de_porta_limpa_notas_e_capacidades() {
    let mut b = bancada(&[THROUGH, KB]);
    b.g.selecionar_porta(KB);
    b.fonte.injetar(&[0xB0, 64, 127]);
    b.g.atualizar();
    assert_eq!(b.g.capacidades().pedal_sustain(), Observacao::Observado);
    b.g.selecionar_porta(THROUGH);
    let saidas = b.g.atualizar();
    assert!(saidas.contains(&SaidaMidi::LimparNotas));
    assert_eq!(b.g.estado(), &conectado(THROUGH));
    assert_eq!(b.g.capacidades().pedal_sustain(), Observacao::NaoObservado);
}

#[test]
fn reconectar_forcado_reabre_a_mesma_porta() {
    let mut b = bancada(&[KB]);
    b.g.selecionar_porta(KB);
    b.g.reconectar();
    let saidas = b.g.atualizar();
    assert!(saidas.contains(&SaidaMidi::LimparNotas));
    assert_eq!(b.g.estado(), &conectado(KB));
    assert_eq!(b.fonte.conexoes_realizadas(), 2);
}

#[test]
fn velocity_fixa_e_sem_pedal_ficam_nao_observados() {
    let mut b = bancada(&[KB]);
    b.g.selecionar_porta(KB);
    for n in 48..72 {
        b.fonte.injetar(&[0x90, n, 100]);
        b.fonte.injetar(&[0x90, n, 0]);
    }
    b.g.atualizar();
    let c = b.g.capacidades();
    assert_eq!(c.velocidade_variavel(), Observacao::NaoObservado);
    assert_eq!(c.faixa_velocidade(), Some((100, 100)));
    assert_eq!(c.pedal_sustain(), Observacao::NaoObservado);
    assert_eq!(c.mudanca_de_programa(), Observacao::NaoObservado);
    assert_eq!(c.liberacao_de_nota(), Observacao::Observado);
}

#[test]
fn ndjson_de_sessao_e_valido_e_anonimizado() {
    let porta_com_serial = "KB-330 SN0123456789:KB-330 MIDI 1 20:0";
    let mut b = bancada(&[porta_com_serial]);
    b.g.selecionar_porta(porta_com_serial);
    b.fonte.injetar(&[0x93, 60, 100]);
    b.fonte.injetar(&[0xB3, 64, 127]);
    b.fonte.injetar(&[0xC3, 4]);
    b.fonte.injetar(&[0xF0, 0x41, 0x10, 0x42, 0x12, 0x34, 0xF7]);
    b.fonte.injetar(&[0x83, 60, 0]);
    b.g.atualizar();
    b.g.desconectar();

    let linhas = b.log.linhas();
    let tipos: Vec<&str> = linhas
        .iter()
        .map(|l| l["tipo"].as_str().expect("tipo"))
        .collect();
    assert_eq!(
        tipos,
        [
            "sessao",
            "conectado",
            "nota_ligada",
            "controle",
            "programa",
            "desconhecido",
            "nota_desligada",
            "desconectado"
        ]
    );
    for l in &linhas {
        assert!(l["ts_ns"].is_u64(), "{l}");
    }
    assert_eq!(linhas[2]["canal"], 4);
    assert_eq!(linhas[3]["valor"], 127);
    assert_eq!(linhas[4]["programa"], 4);
    assert_eq!(linhas[1]["porta"], "KB-330 [removido]:KB-330 MIDI 1 20:0");
    let bruto = String::from_utf8(b.log.0.lock().expect("log").clone()).expect("UTF-8");
    assert!(!bruto.contains("0123456789"), "serial vazou no log");
}

#[test]
fn log_liga_e_desliga_em_tempo_de_execucao_e_marca_a_porta_atual() {
    let fonte = FonteFalsa::nova([KB]);
    let relogio = RelogioManual::novo();
    let mut g = GerenciadorMidi::novo(Box::new(fonte.clone()), Arc::new(relogio.clone()));
    assert!(!g.registrando());
    g.selecionar_porta(KB);
    assert!(fonte.injetar(&[0x90, 60, 100]));
    let _ = g.atualizar();

    let log = Buffer::default();
    g.iniciar_registro(Box::new(log.clone()));
    assert!(g.registrando());
    assert!(fonte.injetar(&[0x80, 60, 0]));
    let _ = g.atualizar();
    let (linhas, falhas) = g.parar_registro().expect("havia log");
    assert!(!g.registrando());
    assert_eq!(falhas, 0);

    let l = log.linhas();
    assert_eq!(l.len() as u64, linhas);
    let tipos: Vec<&str> = l.iter().filter_map(|v| v["tipo"].as_str()).collect();
    // O NoteOn anterior ao log não entra; o log diz de qual porta vêm os eventos.
    assert_eq!(tipos, vec!["sessao", "conectado", "nota_desligada"]);
    assert_eq!(l[1]["reconexao"], Value::Bool(false));

    // Depois de parar, nada mais é gravado.
    assert!(fonte.injetar(&[0x90, 62, 100]));
    let _ = g.atualizar();
    assert_eq!(log.linhas().len(), 3);
    assert_eq!(g.parar_registro(), None);
}

#[test]
fn aviso_de_chegada_e_chamado_a_cada_mensagem_inclusive_em_conexao_ja_aberta() {
    use std::sync::atomic::{AtomicU32, Ordering};
    let mut b = bancada(&[KB]);
    b.g.selecionar_porta(KB);
    let contagem = Arc::new(AtomicU32::new(0));
    let c = Arc::clone(&contagem);
    b.g.definir_aviso_de_chegada(Arc::new(move || {
        c.fetch_add(1, Ordering::SeqCst);
    }));
    assert!(b.fonte.injetar(&[0x90, 60, 100]));
    assert!(b.fonte.injetar(&[0x80, 60, 0]));
    assert_eq!(contagem.load(Ordering::SeqCst), 2);
    assert_eq!(eventos(&b.g.atualizar()).len(), 2);
}
