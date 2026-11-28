// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Laço de 5 s: fila dos hooks → motor, nomes das sessões, amostra do Windows (com a conversa
//! aberta no app do Claude e os sinais do Chat), tique.
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Duration;

use nucleo::claude_registros::registro_da_sessao;
use nucleo::diagnostico_leitura::Lentas;
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
        // o HWND (rodada 28): só em memória, para ligar a sessão do terminal à janela
        janela: Some(j.hwnd),
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

/// Um ciclo do laço (sem Tauri, testável), sem as filas do WSL. Devolve se algo mudou.
#[cfg(test)]
pub fn ciclo(estado: &Estado, fila: &Path, amostra: Amostra) -> Result<bool, String> {
    ciclo_com_wsl(estado, fila, &[], amostra)
}

/// Um ciclo do laço com as filas das distros do WSL ligadas e rodando (`filas_wsl`: a distro e o
/// caminho da fila dela pelo Windows; rodada 28), lidas logo depois da do Windows. Devolve se algo
/// mudou. A trava é solta enquanto os títulos são lidos do disco.
pub fn ciclo_com_wsl(estado: &Estado, fila: &Path, filas_wsl: &[(String, std::path::PathBuf)], amostra: Amostra) -> Result<bool, String> {
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
        // as filas do WSL (rodada 28): a distro que parou no meio só não é lida, sem erro na tela
        for (distro, caminho) in filas_wsl {
            match n.ler_fila_da_distro(distro, caminho) {
                Ok(m) => mudou |= m,
                Err(e) => log_debug!("worktimer: erro lendo a fila do WSL ({distro}): {e}"),
            }
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
    // o nome e, sem a conta do hook, a conta (0.6.1); o transcript do WSL só com a distro rodando
    // (`titulos_a_ler` confere o `pode_tocar`)
    let (lidos, contas): (Vec<(String, Option<String>)>, Vec<(String, Option<String>)>) = ler
        .into_iter()
        .map(|(id, caminho, quer_conta)| {
            let r = registro_da_sessao(&caminho, quer_conta);
            ((id.clone(), r.titulo), (id, if quer_conta { r.conta } else { None }))
        })
        .unzip();

    // 3–5. amostra, tique e gravação
    let mut n = trava(estado);
    let escrevia = n.motor.escrevendo_agora();
    let mut mud = n.aplicar_contas(contas);
    mud.extend(n.aplicar_titulos(lidos, agora));
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

/// Spec 001: a amostra (já aplicada no motor) e as consultas lentas dela vão para o diagnóstico
/// da leitura no app do Claude; o arquivo `diagnostico-leitura.json` é gravado fora da trava, no
/// máximo uma vez por minuto (ou na hora, se o estado mudou). Devolve se o estado mudou.
pub fn diagnosticar(estado: &Estado, a: &Amostra, lentas: Lentas, arquivo: &Path) -> bool {
    let (mudou, texto) = {
        let mut n = trava(estado);
        let mudou = n.registrar_leitura_app(a, lentas);
        (mudou, n.diag_leitura.para_gravar(a.t, mudou))
    };
    if let Some(t) = texto {
        // grava ao lado e troca, para nunca deixar o arquivo pela metade
        let tmp = arquivo.with_extension("json.tmp");
        if let Err(e) = std::fs::write(&tmp, t).and_then(|_| std::fs::rename(&tmp, arquivo)) {
            log_debug!("worktimer: diagnóstico da leitura: {e}");
        }
    }
    mudou
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
        // o WSL (rodada 28) tem o laço dele, numa thread própria (0.6.1, revisão C1): ele pergunta
        // quais distros rodam e lê as filas logo em seguida
        crate::wsl_app::iniciar(app.clone());
        // o rótulo das contas do `~/.claude.json` do Windows (0.6.1); o das distros, no laço do WSL
        let mut rotulos = crate::wsl_app::LeitorDeRotulos::default();
        let mut rotulos_em: Option<std::time::Instant> = None;
        let claude_json = fila::caminhos::pasta_do_usuario().join(".claude.json");
        let arquivo_diag = fila::caminhos::base().join("diagnostico-leitura.json");
        // as horas gravadas antes (revisão I3): o PC é religado quase todo dia
        if let Ok(texto) = std::fs::read_to_string(&arquivo_diag) {
            trava(&app.state::<Estado>()).diag_leitura.carregar(&texto, agora_ms());
        }
        loop {
            if hook_conferido.elapsed() >= CONFERIR_HOOK {
                hook_conferido = std::time::Instant::now();
                if let Err(e) = crate::comandos::copiar_hook() {
                    log_debug!("worktimer: {e}");
                }
                // o hook das distros também é conferido de novo
                crate::wsl_app::conferir_de_novo();
            }
            let r = catch_unwind(AssertUnwindSafe(|| {
                let estado = app.state::<Estado>();
                let (fuso, dia) = fuso_local();
                trava(&estado).atualizar_fuso(fuso, dia);
                let agora = agora_ms();
                let (ocioso, janela) = amostrador.amostrar(agora);
                let (atrasadas, seguradas, recomecos) = (conversa.atrasadas, conversa.seguradas, conversa.recomecos);
                let mut a = amostra_de(agora, ocioso, janela, |hwnd, em_foco| conversa.ler(hwnd, em_foco, LIMITE_CONSULTA));
                if conversa.atrasadas != atrasadas {
                    log_debug!("worktimer: a consulta da conversa passou de 300 ms ({} vezes até agora)", conversa.atrasadas);
                }
                let lentas = Lentas {
                    atrasadas: conversa.atrasadas.saturating_sub(atrasadas) as u32,
                    seguradas: conversa.seguradas.saturating_sub(seguradas) as u32,
                    recomecos: conversa.recomecos.saturating_sub(recomecos),
                };
                let atrasadas = chat.atrasadas;
                a.chat = sinais::win::janela_do_app_do_claude()
                    .and_then(|hwnd| chat.ler(hwnd, false, LIMITE_CONSULTA))
                    .map(sinais_para_o_motor);
                if chat.atrasadas != atrasadas {
                    log_debug!("worktimer: os sinais do Chat passaram de 300 ms ({} vezes até agora)", chat.atrasadas);
                }
                let a_diag = a.clone();
                let r = ciclo_com_wsl(&estado, &fila, &[], a);
                let diag = diagnosticar(&estado, &a_diag, lentas, &arquivo_diag);
                let r = r.map(|x| x || diag);
                // os rótulos das contas, fora da trava (o arquivo tem uns 200 KB; só relido quando
                // muda; revisão M2)
                let r = if rotulos_em.is_none_or(|u| u.elapsed() >= crate::wsl_app::ROTULOS_A_CADA) {
                    rotulos_em = Some(std::time::Instant::now());
                    let lidos: Vec<(String, String)> = rotulos.ler(&claude_json).into_iter().collect();
                    let mudou = match trava(&estado).aplicar_rotulos(&lidos) {
                        Ok(m) => m,
                        Err(e) => {
                            log_debug!("worktimer: rótulos das contas: {e}");
                            false
                        }
                    };
                    r.map(|x| x || mudou)
                } else {
                    r
                };
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
            // a saúde do dock e o lugar da janela principal (specs 005 e 006), sem a trava
            let r = catch_unwind(AssertUnwindSafe(|| {
                janelas::cuidar_do_dock(&app);
                janelas::guardar_principal_se_mexeu(&app);
            }));
            if r.is_err() {
                log_debug!("worktimer: pânico ao cuidar do dock; seguindo");
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

    /// 0.6.1: a sessão do terminal (sem o `CLAUDE_CODE_ACCOUNT_UUID`) ganha o nome pelo `aiTitle` e
    /// a conta pelo `session-owner` do transcript; o rótulo vem do `~/.claude.json` em que o resumo
    /// bate. O id nunca vai para o banco.
    #[test]
    fn a_sessao_do_terminal_ganha_nome_conta_e_rotulo_pelo_transcript() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("fila").join("eventos.jsonl");
        let tr = d.path().join("s.jsonl");
        let uuid = "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0";
        std::fs::write(
            &tr,
            format!(
                "{}\n{}\n",
                serde_json::json!({"type": "session-owner", "accountUuid": uuid, "sessionId": "t1"}),
                serde_json::json!({"type": "ai-title", "aiTitle": "Arrumar o login", "sessionId": "t1"})
            ),
        )
        .unwrap();
        let e = estado(h(9, 0));
        linha(&f, serde_json::json!({"t": h(9, 1), "evento": "UserPromptSubmit", "sessao": "t1", "cwd": "D:\\Novo", "transcript": tr.to_string_lossy(), "origem": "cli"}));
        assert!(ciclo(&e, &f, am(h(9, 1) + 5_000, 0)).unwrap());
        let resumo = fila::resumo_da_conta(Some(uuid)).unwrap();
        {
            let n = trava(&e);
            let s = &n.motor.sessoes["t1"];
            assert_eq!((s.nome.as_deref(), s.conta.as_deref()), (Some("Arrumar o login"), Some(resumo.as_str())));
            let contas = n.banco.contas().unwrap();
            assert_eq!(contas.iter().map(|c| (c.id.as_str(), c.origem.as_deref())).collect::<Vec<_>>(), [(resumo.as_str(), Some("cli"))]);
            let gravada = n.banco.sessoes_desde(0).unwrap().into_iter().find(|x| x.id == "t1").unwrap();
            assert_eq!(gravada.conta.as_deref(), Some(resumo.as_str()));
        }
        // o rótulo: o `~/.claude.json` de outra conta não muda nada; o da mesma conta, sim
        let cj = d.path().join(".claude.json");
        let oauth = |id: &str| serde_json::json!({"oauthAccount": {"accountUuid": id, "displayName": "Ana", "emailAddress": "ana@exemplo.com", "organizationType": "claude_pro"}}).to_string();
        std::fs::write(&cj, oauth("11111111-2222-3333-4444-555555555555")).unwrap();
        let ler = |p: &Path| crate::wsl_app::LeitorDeRotulos::default().ler(p).into_iter().collect::<Vec<_>>();
        assert!(!trava(&e).aplicar_rotulos(&ler(&cj)).unwrap());
        std::fs::write(&cj, oauth(uuid)).unwrap();
        assert!(trava(&e).aplicar_rotulos(&ler(&cj)).unwrap());
        let n = trava(&e);
        let s = n.snapshot(h(9, 2)).unwrap();
        assert_eq!(s.contas[0].rotulo.as_deref(), Some("Ana · an•••@exemplo.com · Pro"));
        let banco = serde_json::to_string(&n.banco.contas().unwrap()).unwrap();
        assert!(!banco.contains(uuid) && !banco.contains("ana@"), "nem o id nem o e-mail inteiro");
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
        assert_eq!(a.janela, Some(42), "a janela vai junto (rodada 28)");
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

    /// Spec 001: 30 min de uso do app do Claude sem achar conversa, com uma sessão do app
    /// trabalhando, deixa a leitura "falhando"; a conversa achada volta a "ok". O arquivo tem só
    /// contagens.
    #[test]
    fn diagnostico_da_leitura_no_app_avisa_a_falha() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("eventos.jsonl");
        let tr = d.path().join("s.jsonl");
        let arquivo = d.path().join("diagnostico-leitura.json");
        titulo(&tr, "Plano do app");
        let e = estado(h(9, 0));
        let t = trava(&e).banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        trava(&e).criar_regra("C:\\p", t).unwrap();
        let app = |t: Ms, conversa: Option<&str>| Amostra {
            t,
            ocioso_ms: 1_000,
            processo: Some("claude".into()),
            titulo: Some("Claude".into()),
            conversa: Conversa::App(conversa.map(str::to_string)),
            em_foco: true,
            ..Default::default()
        };
        assert_eq!(trava(&e).snapshot(h(9, 0)).unwrap().leitura_app.estado, "sem_uso");
        linha(&f, serde_json::json!({"t": h(9, 0), "evento": "UserPromptSubmit", "sessao": "s1", "cwd": "C:\\p", "transcript": tr, "origem": "claude-desktop"}));
        ciclo(&e, &f, am(h(9, 0) + 1_000, 0)).unwrap();
        let mut a = app(h(9, 1), Some("Plano do app"));
        assert!(!diagnosticar(&e, &a, Lentas::default(), &arquivo));
        assert_eq!(trava(&e).snapshot(h(9, 1)).unwrap().leitura_app.estado, "ok");
        // o Claude segue trabalhando na sessão do app, e a conversa some da consulta
        let mut mudou = false;
        let mut ms = h(9, 2);
        while ms <= h(9, 40) {
            if ms % (5 * 60_000) == 0 {
                linha(&f, serde_json::json!({"t": ms, "evento": "PostToolUse", "sessao": "s1", "cwd": "C:\\p", "transcript": tr, "origem": "claude-desktop"}));
                ciclo(&e, &f, am(ms, 0)).unwrap();
            }
            a = app(ms, None);
            mudou |= diagnosticar(&e, &a, Lentas { atrasadas: 1, ..Default::default() }, &arquivo);
            ms += 5_000;
        }
        assert!(mudou, "o estado mudou: a tela precisa saber");
        let s = trava(&e).snapshot(h(9, 40)).unwrap().leitura_app;
        assert_eq!((s.estado.as_str(), s.desde), ("falhando", Some(h(9, 2))));
        let texto = std::fs::read_to_string(&arquivo).unwrap();
        assert!(texto.contains("\"falhando\"") && !texto.contains("Plano") && !texto.contains("s1"), "{texto}");
        // duas conversas reconhecidas seguidas voltam a "ok" (revisão M4)
        assert!(!diagnosticar(&e, &app(h(9, 41), Some("plano do app")), Lentas::default(), &arquivo));
        assert!(diagnosticar(&e, &app(h(9, 41) + 5_000, Some("plano do app")), Lentas::default(), &arquivo));
        assert_eq!(trava(&e).snapshot(h(9, 41)).unwrap().leitura_app.estado, "ok");
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

    #[test]
    fn ciclo_le_a_fila_de_cada_distro_com_o_proprio_offset_e_o_cwd_traduzido() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("win").join("eventos.jsonl");
        let ubuntu = d.path().join("ubuntu").join("eventos.jsonl");
        let debian = d.path().join("debian").join("eventos.jsonl");
        let e = estado(h(9, 0));
        let t = trava(&e).banco.criar_timer(None, "Projeto", "#a00").unwrap();
        trava(&e).criar_regra(r"\\wsl.localhost\Ubuntu\home\u\proj", t).unwrap();
        {
            let mut n = trava(&e);
            let mut cfg = n.banco.config_app().unwrap();
            cfg.pastas_ignoradas = vec![r"\\wsl$\Ubuntu\home\u\pessoal".into()];
            n.salvar_config(&cfg, &d.path().join("hook.json")).unwrap();
        }
        linha(&ubuntu, serde_json::json!({"t": h(9, 1), "evento": "UserPromptSubmit", "sessao": "w1", "cwd": "/home/u/proj/api", "origem": "cli"}));
        // a defesa em dobro: o hook.json da distro estava velho e deixou passar a pasta ignorada
        linha(&ubuntu, serde_json::json!({"t": h(9, 1), "evento": "UserPromptSubmit", "sessao": "w2", "cwd": "/home/u/pessoal/x", "origem": "cli"}));
        linha(&debian, serde_json::json!({"t": h(9, 2), "evento": "UserPromptSubmit", "sessao": "d1", "cwd": "/mnt/c/Projetos/x", "origem": "claude-vscode"}));
        let filas = vec![("Ubuntu".to_string(), ubuntu.clone()), ("Debian".to_string(), debian.clone())];
        assert!(ciclo_com_wsl(&e, &f, &filas, am(h(9, 3), 0)).unwrap());
        {
            let n = trava(&e);
            let w1 = &n.motor.sessoes["w1"];
            assert_eq!((w1.cwd.as_str(), w1.origem.as_deref(), w1.timer_id), (r"\\wsl.localhost\Ubuntu\home\u\proj\api", Some("wsl"), Some(t)));
            assert!(!n.motor.sessoes.contains_key("w2"), "a pasta ignorada fica de fora");
            let d1 = &n.motor.sessoes["d1"];
            assert_eq!((d1.cwd.as_str(), d1.origem.as_deref()), (r"C:\Projetos\x", Some("claude-vscode")));
            let tam = |p: &Path| std::fs::metadata(p).unwrap().len().to_string();
            assert_eq!(n.banco.valor("fila_offset_wsl:Ubuntu").unwrap(), Some(tam(&ubuntu)));
            assert_eq!(n.banco.valor("fila_offset_wsl:Debian").unwrap(), Some(tam(&debian)));
            assert_eq!(n.banco.valor("fila_offset").unwrap(), None, "a do Windows é outra");
        }
        // a mesma linha não é relida; a distro que parou (sem a fila) não dá erro
        assert!(!ciclo_com_wsl(&e, &f, &filas, am(h(9, 4), 0)).unwrap());
        let parada = vec![("Ubuntu".to_string(), d.path().join("sumiu").join("eventos.jsonl"))];
        assert!(ciclo_com_wsl(&e, &f, &parada, am(h(9, 5), 0)).is_ok());
        // o offset segue depois de reabrir: o do banco
        trava(&e).offsets_wsl.clear();
        assert!(!ciclo_com_wsl(&e, &f, &filas, am(h(9, 6), 0)).unwrap());
        let novas = trava(&e).motor.perguntas.iter().filter(|(_, p)| matches!(p, nucleo::modelo::Pergunta::SessaoNova { .. })).count();
        assert_eq!(novas, 1, "só a d1 pergunta (a w1 tem regra)");
        // 0.6.1: a distro com evento recente fica "quente" (o laço do WSL roda a cada 2 s), até o
        // fim da sessão
        let quente = crate::wsl_app::QUENTE;
        assert!(trava(&e).distro_quente("Ubuntu", quente) && trava(&e).distro_quente("Debian", quente));
        assert!(!trava(&e).distro_quente("Kali", quente), "sem evento nenhum");
        linha(&ubuntu, serde_json::json!({"t": h(9, 7), "evento": "Stop", "sessao": "w1", "cwd": "/home/u/proj/api", "origem": "cli"}));
        assert!(trava(&e).ler_fila_da_distro("Ubuntu", &ubuntu).unwrap());
        assert_eq!(trava(&e).motor.sessoes["w1"].estado, nucleo::modelo::EstadoSessao::EsperandoVoce);
        assert!(trava(&e).distro_quente("Ubuntu", quente));
        linha(&ubuntu, serde_json::json!({"t": h(9, 8), "evento": "SessionEnd", "sessao": "w1", "cwd": "/home/u/proj/api", "origem": "cli"}));
        trava(&e).ler_fila_da_distro("Ubuntu", &ubuntu).unwrap();
        assert!(!trava(&e).distro_quente("Ubuntu", quente), "depois do fim da sessão, não");
        assert!(!trava(&e).ler_fila_da_distro("Ubuntu", &ubuntu).unwrap(), "nada novo");
    }
}
