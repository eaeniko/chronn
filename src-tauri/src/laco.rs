// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Laço de 5 s: fila dos hooks → motor, nomes das sessões, amostra do Windows (com a conversa
//! aberta no app do Claude e os sinais do Chat), tique.
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Duration;

use nucleo::claude_registros::titulo_da_sessao;
use nucleo::motor::{Amostra, ChatAberto, Conversa, SinaisDoChat};
use nucleo::tempo::Ms;
use sinais::conversa::{e_app_do_claude, ConversaLida, LeitorConversa, SinaisChat, LIMITE_CONSULTA};
use sinais::JanelaInfo;
use tauri::{AppHandle, Emitter, Manager};

use crate::estado_app::{agora_ms, fuso_local, texto, trava, Estado};
use crate::{bandeja, janelas};

pub const INTERVALO: Duration = Duration::from_secs(5);
/// Com o app aberto, a cópia do hook (`.worktimer\bin`) é conferida de tempos em tempos: se
/// estava em uso na abertura, a versão nova entra sem esperar o app reabrir.
pub const CONFERIR_HOOK: Duration = Duration::from_secs(30 * 60);
pub const EVENTO_MUDOU: &str = "estado-mudou";

/// A amostra do motor a partir da janela em uso. As janelas do próprio app (principal e dock)
/// são neutras; no app do Claude, a conversa aberta e o "escrevendo" (o foco na caixa de mensagem
/// dela) vêm da mesma consulta de UI Automation (`ler_conversa`, que só roda nesse caso). O
/// título da conversa fica só em memória; o texto da caixa nunca é lido.
pub fn amostra_de(
    agora: Ms,
    ocioso_ms: Ms,
    janela: Option<JanelaInfo>,
    ler_conversa: impl FnOnce(isize, bool) -> Option<ConversaLida>,
) -> Amostra {
    let Some(j) = janela else {
        return Amostra { t: agora, ocioso_ms, ..Default::default() };
    };
    let neutra = j.pid == std::process::id();
    let (conversa, escrevendo) = if !neutra && e_app_do_claude(&j) {
        // o elemento com foco só é consultado com a janela do Claude em foco
        let lida = ler_conversa(j.hwnd, j.em_foco);
        let escrevendo = lida.as_ref().is_some_and(|c| c.escrevendo);
        let conversa = match lida {
            // a conversa do Chat: o id do endereço e o título (rodada 27)
            Some(ConversaLida { titulo, chat: Some(id), .. }) => Conversa::Chat { id, titulo },
            lida => Conversa::App(lida.map(|c| c.titulo)),
        };
        (conversa, escrevendo)
    } else {
        (Conversa::PeloTitulo, false)
    };
    Amostra {
        t: agora,
        ocioso_ms,
        processo: Some(j.processo),
        titulo: Some(j.titulo),
        conversa,
        escrevendo,
        neutra,
        em_foco: j.em_foco,
        reuniao_ativa: false,
        chat: None,
    }
}

/// Os sinais do Chat lidos na janela do app do Claude, para o motor (os títulos ficam em memória).
pub fn sinais_para_o_motor(s: SinaisChat) -> SinaisDoChat {
    SinaisDoChat {
        aba_do_chat: s.aba_do_chat,
        aberta: s.aberta.map(|a| ChatAberto { id: a.id, titulo: a.titulo, parando: a.parando }),
        rodando: s.rodando,
        barra: s.barra,
        repetidos: s.repetidos,
    }
}

/// Um ciclo do laço (sem Tauri, testável). Devolve se algo mudou.
/// A trava é solta enquanto os títulos são lidos do disco.
pub fn ciclo(estado: &Estado, fila: &Path, amostra: Amostra) -> Result<bool, String> {
    let agora = amostra.t;
    let mut mudou = false;

    // 1. fila de eventos dos hooks
    let ler = {
        let mut n = trava(estado);
        // sem banco (a troca falhou de vez): nada é lido nem gravado
        n.checar_banco()?;
        // Arquivo recriado/encolhido: recomeça do início.
        if std::fs::metadata(fila).map(|m| m.len()).unwrap_or(0) < n.fila_offset {
            n.fila_offset = 0;
        }
        match fila::ler_novos(fila, n.fila_offset) {
            Ok((evs, novo)) => {
                let mut mud = vec![];
                for ev in &evs {
                    mud.extend(n.motor.evento(ev));
                }
                n.aplicar(&mud)?;
                // Fila lida inteira e grande demais: esvazia e recomeça do zero.
                let novo = match fila::esvaziar_se_lida(fila, novo, fila::LIMITE_FILA) {
                    Ok(true) => 0,
                    Ok(false) => novo,
                    Err(e) => {
                        log_debug!("worktimer: erro esvaziando a fila: {e}");
                        novo
                    }
                };
                if novo != n.fila_offset {
                    n.fila_offset = novo;
                    n.banco.gravar_valor("fila_offset", &novo.to_string()).map_err(texto)?;
                }
                mudou |= !mud.is_empty();
            }
            Err(e) => log_debug!("worktimer: erro lendo a fila: {e}"),
        }
        (n.titulos_a_ler(agora), n.transcripts_a_conferir(agora))
    };
    let (ler, fins) = ler;

    // 2. nomes das sessões (fora da trava; cache no Nucleo)
    // o fim dos transcripts (o stop na conversa e as tarefas em segundo plano), também fora da
    // trava: cada leitor lê para a frente o que foi acrescentado
    let fins: Vec<(String, nucleo::fim_do_transcript::LeitorDoTranscript, Option<Option<Ms>>)> = fins
        .into_iter()
        .map(|(id, caminho, mut leitor)| {
            let r = leitor.ler(&caminho, agora);
            (id, leitor, r)
        })
        .collect();
    let lidos: Vec<(String, Option<String>)> =
        ler.into_iter().map(|(id, caminho)| (id, titulo_da_sessao(&caminho))).collect();

    // 3–5. amostra, tique e gravação
    let mut n = trava(estado);
    let escrevia = n.motor.escrevendo_agora();
    let mut mud = n.aplicar_titulos(lidos, agora);
    let segundo_plano = n.motor.segundo_plano.clone();
    mud.extend(n.aplicar_fins(fins, agora));
    // entrar ou sair do "segundo plano" só muda a etiqueta: a tela precisa saber
    let etiqueta_fim = n.motor.segundo_plano != segundo_plano;
    mud.extend(n.motor.amostra(&amostra));
    mud.extend(n.motor.tique(agora));
    n.aplicar(&mud)?;
    n.gravar_tique(agora)?;
    // "Zerar sozinho na virada" (ligado): o corte do dia zera os timers
    let zerou = n.zerar_na_virada(agora)?;
    // o "esperando você" vence sem evento nenhum (1 h depois de terminar): a tela precisa saber
    let esperas = n.esperas_mudaram(agora);
    // "lendo" ↔ "escrevendo" só muda a etiqueta: nenhuma `Mudanca`, mas a tela precisa saber
    let etiqueta = n.motor.escrevendo_agora() != escrevia;
    Ok(mudou || !mud.is_empty() || esperas || etiqueta || zerou || etiqueta_fim)
}

/// Avisa as janelas e atualiza a bandeja e o título. Chamar sem a trava.
pub fn avisar(app: &AppHandle) {
    let rodando = match app.try_state::<Estado>() {
        Some(e) => trava(&e).rodando(),
        None => 0,
    };
    let _ = app.emit(EVENTO_MUDOU, ());
    bandeja::atualizar(app, rodando);
    janelas::atualizar_titulo(app, rodando);
    // com o dock "Só quando houver uma pergunta", uma pergunta nova o mostra e a última respondida o esconde
    janelas::aplicar_dock(app);
}

pub fn iniciar(app: AppHandle) {
    let r = std::thread::Builder::new().name("worktimer-laco".into()).spawn(move || {
        let mut amostrador = sinais::Amostrador::novo();
        // a UI Automation roda numa thread própria e espera no máximo 300 ms por amostra
        let mut conversa = LeitorConversa::novo();
        // os sinais do Chat (rodada 27): outra thread, o mesmo teto, a cada ciclo, com a janela do
        // app do Claude em uso ou não
        let mut chat = LeitorConversa::do_chat();
        let fila = fila::caminhos::fila();
        let backups = fila::caminhos::backups();
        let mut hook_conferido = std::time::Instant::now();
        loop {
            if hook_conferido.elapsed() >= CONFERIR_HOOK {
                hook_conferido = std::time::Instant::now();
                if let Err(e) = crate::comandos::copiar_hook() {
                    log_debug!("worktimer: {e}");
                }
            }
            let r = catch_unwind(AssertUnwindSafe(|| {
                let estado = app.state::<Estado>();
                let (fuso, dia) = fuso_local();
                trava(&estado).atualizar_fuso(fuso, dia);
                let agora = agora_ms();
                let (ocioso, janela) = amostrador.amostrar(agora);
                let atrasadas = conversa.atrasadas;
                let mut a = amostra_de(agora, ocioso, janela, |hwnd, em_foco| conversa.ler(hwnd, em_foco, LIMITE_CONSULTA));
                if conversa.atrasadas != atrasadas {
                    log_debug!("worktimer: a consulta da conversa passou de 300 ms ({} vezes até agora)", conversa.atrasadas);
                }
                let atrasadas = chat.atrasadas;
                a.chat = sinais::win::janela_do_app_do_claude()
                    .and_then(|hwnd| chat.ler(hwnd, false, LIMITE_CONSULTA))
                    .map(sinais_para_o_motor);
                if chat.atrasadas != atrasadas {
                    log_debug!("worktimer: os sinais do Chat passaram de 300 ms ({} vezes até agora)", chat.atrasadas);
                }
                let r = ciclo(&estado, &fila, a);
                // backup a cada 6 h com o app aberto
                if let Err(e) = trava(&estado).backup_se_devido(&backups, chrono::Local::now().naive_local(), false) {
                    log_debug!("worktimer: erro no backup: {e}");
                }
                r
            }));
            match r {
                Ok(Ok(true)) => avisar(&app),
                // o "Mostrar o dock agora" vence sem mudança nenhuma no núcleo
                Ok(Ok(false)) => janelas::aplicar_dock(&app),
                Ok(Err(e)) => log_debug!("worktimer: erro no laço: {e}"),
                Err(_) => log_debug!("worktimer: pânico no laço; seguindo"),
            }
            std::thread::sleep(INTERVALO);
        }
    });
    if let Err(e) = r {
        log_debug!("worktimer: não consegui iniciar o laço: {e}");
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::estado_app::Nucleo;
    use chrono::NaiveDate;
    use nucleo::banco::Banco;
    use nucleo::tempo::ms_local;
    use std::io::Write;
    use std::sync::Mutex;

    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms {
        ms_local(2026, 11, 17, hh, mm, F)
    }
    fn estado(agora: Ms) -> Estado {
        let dia = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        Mutex::new(Nucleo::abrir(Banco::abrir_memoria().unwrap(), agora, F, dia).unwrap())
    }
    fn linha(f: &Path, j: serde_json::Value) {
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        let mut a = std::fs::OpenOptions::new().create(true).append(true).open(f).unwrap();
        writeln!(a, "{j}").unwrap();
    }
    /// Amostra sem janela em uso.
    fn am(t: Ms, ocioso_ms: Ms) -> Amostra {
        Amostra { t, ocioso_ms, ..Default::default() }
    }
    fn titulo(p: &Path, t: &str) {
        std::fs::write(p, format!("{}\n", serde_json::json!({"type": "custom-title", "customTitle": t}))).unwrap();
    }

    #[test]
    fn ciclo_le_a_fila_pergunta_e_grava_offset_e_tique() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("fila").join("eventos.jsonl");
        let e = estado(h(9, 0));
        assert!(!ciclo(&e, &f, am(h(9, 0), 0)).unwrap(), "sem fila, nada muda");
        linha(&f, serde_json::json!({"t": h(9, 1), "evento": "UserPromptSubmit", "sessao": "t1", "cwd": "D:\\Novo"}));
        assert!(ciclo(&e, &f, am(h(9, 1) + 5_000, 0)).unwrap());
        let n = trava(&e);
        let s = n.snapshot(h(9, 2)).unwrap();
        assert_eq!(s.perguntas.len(), 1);
        assert_eq!(s.perguntas[0].texto, "Sessão nova do Claude na pasta D:\\Novo. Em qual timer contar?");
        let tamanho = std::fs::metadata(&f).unwrap().len();
        assert_eq!(n.fila_offset, tamanho);
        assert_eq!(n.banco.valor("fila_offset").unwrap(), Some(tamanho.to_string()));
        assert_eq!(n.banco.valor("ultimo_tique").unwrap(), Some((h(9, 1) + 5_000).to_string()));
        drop(n);
        // a mesma linha não é relida
        assert!(!ciclo(&e, &f, am(h(9, 2), 0)).unwrap());
        assert_eq!(trava(&e).motor.perguntas.len(), 1);
    }

    #[test]
    fn ciclo_recomeca_se_a_fila_encolher() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("eventos.jsonl");
        let e = estado(h(9, 0));
        trava(&e).fila_offset = 10_000;
        linha(&f, serde_json::json!({"t": h(9, 1), "evento": "UserPromptSubmit", "sessao": "t1", "cwd": "D:\\Novo"}));
        ciclo(&e, &f, am(h(9, 2), 0)).unwrap();
        assert_eq!(trava(&e).motor.perguntas.len(), 1);
    }

    #[test]
    fn ciclo_amostra_e_pausa_por_inatividade() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("eventos.jsonl");
        let e = estado(h(9, 0));
        let t = {
            let mut n = trava(&e);
            let t = n.banco.criar_timer(None, "Cliente F", "#a00").unwrap();
            let mud = n.motor.play(t, h(9, 0));
            n.aplicar(&mud).unwrap();
            t
        };
        ciclo(&e, &f, am(h(9, 5), 0)).unwrap();
        assert!(ciclo(&e, &f, am(h(9, 20), 11 * 60_000)).unwrap());
        assert!(trava(&e).banco.abertos().unwrap().is_empty());
        ciclo(&e, &f, am(h(9, 35), 0)).unwrap();
        let s = trava(&e).snapshot(h(9, 35)).unwrap();
        assert_eq!(s.perguntas[0].texto, "Você ficou 26 min sem mexer no PC. Retomar Cliente F?");
        assert_eq!(s.grupos[0].timers[0].id, t);
    }

    #[test]
    fn ciclo_nomeia_sessao_com_cache_de_um_minuto() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("eventos.jsonl");
        let tr = d.path().join("s.jsonl");
        titulo(&tr, "Primeiro nome");
        let e = estado(h(9, 0));
        linha(
            &f,
            serde_json::json!({"t": h(9, 0), "evento": "UserPromptSubmit", "sessao": "s1", "cwd": "C:\\p", "transcript": tr}),
        );
        assert!(ciclo(&e, &f, am(h(9, 0), 0)).unwrap());
        let nome = |e: &Estado| trava(e).motor.sessoes["s1"].nome.clone();
        assert_eq!(nome(&e).as_deref(), Some("Primeiro nome"));
        let banco_nome = |e: &Estado| trava(e).banco.sessoes_desde(0).unwrap()[0].nome.clone();
        assert_eq!(banco_nome(&e).as_deref(), Some("Primeiro nome"));
        titulo(&tr, "Segundo nome");
        ciclo(&e, &f, am(h(9, 0) + 30_000, 0)).unwrap();
        assert_eq!(nome(&e).as_deref(), Some("Primeiro nome"), "não relê antes de 1 min");
        assert!(ciclo(&e, &f, am(h(9, 1) + 1_000, 0)).unwrap());
        assert_eq!(nome(&e).as_deref(), Some("Segundo nome"));
        assert_eq!(banco_nome(&e).as_deref(), Some("Segundo nome"));
        // parada: não relê mais
        linha(&f, serde_json::json!({"t": h(9, 2), "evento": "SessionEnd", "sessao": "s1", "cwd": "C:\\p"}));
        ciclo(&e, &f, am(h(9, 3), 0)).unwrap();
        titulo(&tr, "Terceiro");
        ciclo(&e, &f, am(h(9, 10), 0)).unwrap();
        assert_eq!(nome(&e).as_deref(), Some("Segundo nome"));
        let s = trava(&e).snapshot(h(9, 10)).unwrap();
        assert!(s.grupos.is_empty());
    }

    fn janela(processo: &str, classe: &str, pid: u32) -> Option<JanelaInfo> {
        Some(JanelaInfo { processo: processo.into(), titulo: "Claude".into(), classe: classe.into(), pid, hwnd: 42, em_foco: true })
    }

    #[test]
    fn amostra_le_a_conversa_so_no_app_do_claude() {
        let outro_pid = std::process::id() + 1;
        let lida = |hwnd: isize, em_foco: bool| {
            assert!(em_foco, "a janela em foco vai junto");
            Some(ConversaLida { titulo: format!("conversa da janela {hwnd}"), escrevendo: false, chat: None })
        };
        let a = amostra_de(h(9, 0), 1_000, janela("claude", "Chrome_WidgetWin_1", outro_pid), lida);
        assert_eq!(a.conversa, Conversa::App(Some("conversa da janela 42".into())));
        assert!(!a.neutra && a.em_foco && !a.escrevendo);
        // o foco na caixa de mensagem da conversa: a mesma consulta diz "escrevendo"
        let escrevendo = |_: isize, _: bool| Some(ConversaLida { titulo: "x".into(), escrevendo: true, chat: None });
        let a = amostra_de(h(9, 0), 1_000, janela("claude", "Chrome_WidgetWin_1", outro_pid), escrevendo);
        assert!(a.escrevendo);
        let a = amostra_de(h(9, 0), 1_000, janela("claude", "Chrome_WidgetWin_1", outro_pid), |_, _| None);
        assert_eq!((a.conversa, a.escrevendo), (Conversa::App(None), false));
        // a conversa do Chat: o id e o título
        let chat = |_: isize, _: bool| Some(ConversaLida { titulo: "Receita".into(), escrevendo: true, chat: Some("0a1b".into()) });
        let a = amostra_de(h(9, 0), 1_000, janela("claude", "Chrome_WidgetWin_1", outro_pid), chat);
        assert_eq!((a.conversa, a.escrevendo), (Conversa::Chat { id: "0a1b".into(), titulo: "Receita".into() }, true));
        // terminal e VS Code: pelo título, sem UI Automation
        let nunca = |_: isize, _: bool| -> Option<ConversaLida> { panic!("não devia consultar") };
        let a = amostra_de(h(9, 0), 1_000, janela("windowsterminal", "CASCADIA_HOSTING_WINDOW_CLASS", outro_pid), nunca);
        assert_eq!((a.conversa, a.processo.as_deref()), (Conversa::PeloTitulo, Some("windowsterminal")));
        let a = amostra_de(h(9, 0), 1_000, janela("claude", "ConsoleWindowClass", outro_pid), nunca);
        assert_eq!(a.conversa, Conversa::PeloTitulo);
        // janela do próprio app: neutra
        let a = amostra_de(h(9, 0), 1_000, janela("work-timer", "Chrome_WidgetWin_1", std::process::id()), nunca);
        assert!(a.neutra);
        let a = amostra_de(h(9, 0), 1_000, None, nunca);
        assert_eq!((a.processo, a.neutra), (None, false));
    }

    #[test]
    fn ciclo_conta_leitura_da_conversa_aberta() {
        use nucleo::modelo::Origem;
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("eventos.jsonl");
        let tr = d.path().join("s.jsonl");
        titulo(&tr, "Plano do app");
        let e = estado(h(9, 0));
        let t = trava(&e).banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        trava(&e).criar_regra("C:\\p", t).unwrap();
        for (min, ev) in [(0, "UserPromptSubmit"), (5, "Stop")] {
            linha(&f, serde_json::json!({"t": h(9, min), "evento": ev, "sessao": "s1", "cwd": "C:\\p", "transcript": tr}));
        }
        ciclo(&e, &f, am(h(9, 6), 0)).unwrap();
        let app = |t: Ms, conversa: &str| Amostra {
            t,
            ocioso_ms: 1_000,
            processo: Some("claude".into()),
            titulo: Some("Claude".into()),
            conversa: Conversa::App(Some(conversa.into())),
            ..Default::default()
        };
        ciclo(&e, &f, app(h(9, 7), "Outra conversa")).unwrap();
        assert!(trava(&e).motor.lendo_agora().is_none());
        // troca suave: a primeira amostra só começa a espera (a tela não muda); 10 s depois vale,
        // desde quando a conversa apareceu
        let suave = nucleo::motor::TROCA_SUAVE_MS;
        assert!(!ciclo(&e, &f, app(h(9, 8), "plano do app")).unwrap());
        assert!(trava(&e).motor.lendo_agora().is_none());
        assert!(ciclo(&e, &f, app(h(9, 8) + suave, "plano do app")).unwrap());
        let n = trava(&e);
        assert_eq!(n.motor.lendo_agora(), Some("s1"));
        assert_eq!(n.snapshot(h(9, 8) + suave).unwrap().lendo.as_deref(), Some("s1"));
        let tr = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert!(tr.iter().any(|x| x.origem == Origem::Leitura && x.inicio == h(9, 8) && x.timer_id == Some(t)));
        assert!(!n.snapshot(h(9, 8)).unwrap().escrevendo);
        drop(n);
        // o foco vai para a caixa de mensagem: muda só a etiqueta, quando se mantém (o laço avisa a tela)
        assert!(!ciclo(&e, &f, Amostra { escrevendo: true, ..app(h(9, 9), "plano do app") }).unwrap());
        assert!(ciclo(&e, &f, Amostra { escrevendo: true, ..app(h(9, 9) + suave, "plano do app") }).unwrap());
        let n = trava(&e);
        let s = n.snapshot(h(9, 9) + suave).unwrap();
        assert!(s.escrevendo);
        assert_eq!(s.grupos[0].timers[0].situacao.as_deref(), Some("escrevendo"));
        assert_eq!(n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap().iter().filter(|x| x.origem == Origem::Leitura).count(), 1);
    }

    #[test]
    fn ciclo_conta_o_chat_pelos_sinais_sem_gravar_o_titulo() {
        use sinais::conversa::ChatAberto as Lido;
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("eventos.jsonl");
        let e = estado(h(9, 0));
        let sinais = |parando: bool, rodando: &[&str]| {
            Some(sinais_para_o_motor(SinaisChat {
                aba_do_chat: true,
                aberta: Some(Lido { id: "0a1b".into(), titulo: "Receita de bolo".into(), parando }),
                rodando: rodando.iter().map(|t| t.to_string()).collect(),
                barra: true,
                repetidos: vec![],
            }))
        };
        // o Claude responde no Chat, com a janela fora de uso: a sessão nasce e pergunta
        assert!(ciclo(&e, &f, Amostra { chat: sinais(true, &["Receita de bolo"]), ..am(h(9, 0), 0) }).unwrap());
        {
            let n = trava(&e);
            let s = &n.motor.sessoes["chat:0a1b"];
            assert_eq!((s.estado, s.nome.as_deref()), (nucleo::modelo::EstadoSessao::Trabalhando, Some("Receita de bolo")));
            assert_eq!(n.motor.perguntas.len(), 1);
            assert_eq!(n.banco.sessoes_desde(0).unwrap()[0].nome, None, "o título fica só em memória");
            assert_eq!(n.banco.abertos().unwrap().len(), 1, "o trecho do Claude");
        }
        // sem sinal (sem a janela): nada muda
        assert!(!ciclo(&e, &f, am(h(9, 1), 0)).unwrap());
        // a resposta acabou
        assert!(ciclo(&e, &f, Amostra { chat: sinais(false, &[]), ..am(h(9, 2), 0) }).unwrap());
        let n = trava(&e);
        assert!(n.banco.abertos().unwrap().is_empty());
        assert_eq!(n.motor.sessoes["chat:0a1b"].estado, nucleo::modelo::EstadoSessao::EsperandoVoce);
    }
}
