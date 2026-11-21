// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Mensagem de diagnóstico só no debug (no release não há console).
macro_rules! log_debug {
    ($($t:tt)*) => {{
        // `cfg!` (e não `#[cfg]`): no release o texto some, mas as variáveis contam como usadas
        if cfg!(debug_assertions) {
            eprintln!($($t)*);
        }
    }};
}

mod atualizar;
mod bandeja;
mod comandos;
mod escolher_pasta;
mod estado_app;
mod janelas;
mod laco;
mod perfil;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, RunEvent, Window, WindowEvent};

/// Nome de exibição do app (a constante única é `nucleo::APP_NOME`; a pasta de dados é
/// `fila::caminhos::PASTA_DADOS`).
pub const APP_NOME: &str = nucleo::APP_NOME;

/// Posto em toda página antes de qualquer script dela: os textos das páginas usam o mesmo nome
/// (`ui/js/nome.js`).
fn script_do_nome() -> String {
    // o perfil de teste (rodada 17): o dock ganha a marca "teste" e a Ajuda o "Apagar o perfil…"
    format!(
        "globalThis.NOME_APP = {}; globalThis.PERFIL_TESTE = {};",
        serde_json::to_string(APP_NOME).expect("texto"),
        perfil::e_teste()
    )
}

/// O idioma da interface do Windows, como o `navigator.language` do WebView (rodada 23): "pt" para
/// qualquer português, "en" para o resto (o "Sistema" dos Ajustes → Idioma só separa os dois).
fn idioma_do_windows() -> &'static str {
    #[cfg(windows)]
    {
        // o idioma primário são os 10 bits de baixo do LANGID; LANG_PORTUGUESE = 0x16
        let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        if id & 0x3ff == 0x16 {
            return "pt";
        }
    }
    "en"
}

/// Prepara a pasta de dados (`%USERPROFILE%\.worktimer`), abre o banco, reabre o motor e põe o
/// núcleo no estado do Tauri.
fn iniciar_nucleo(app: &tauri::App) -> Result<(), String> {
    log_debug!("{} {} · © 2026 {} · {}", APP_NOME, app.package_info().version, nucleo::AUTOR, nucleo::ASSINATURA);
    // antes do banco: os nomes padrão de um banco novo nascem no idioma do Windows
    nucleo::idioma::definir_sistema(idioma_do_windows());
    let base = fila::caminhos::base();
    std::fs::create_dir_all(&base).map_err(|e| format!("não consegui criar {}: {e}", base.display()))?;
    // a cópia do hook que os comandos do settings.json chamam; em uso, fica para a próxima. No
    // perfil de teste, não: o Claude Code dele é simulado (rodada 17)
    if !perfil::e_teste() {
        if let Err(e) = comandos::copiar_hook() {
            log_debug!("worktimer: {e}");
        }
    }
    // antes de o banco migrar para o esquema deste app, uma cópia dele como estava (fora da
    // rotação): dá para voltar a uma versão anterior do app. Sem a cópia, a migração segue (ela
    // só acrescenta, numa transação só)
    let agora_local = chrono::Local::now().naive_local();
    match nucleo::dados::backup_antes_de_migrar(&fila::caminhos::banco(), &fila::caminhos::backups(), agora_local) {
        Ok(Some(p)) => log_debug!("worktimer: cópia de antes da migração em {}", p.display()),
        Ok(None) => {}
        Err(e) => log_debug!("worktimer: {e}"),
    }
    let banco = nucleo::banco::Banco::abrir(&fila::caminhos::banco()).map_err(|e| e.to_string())?;
    let (fuso, dia) = estado_app::fuso_local();
    let mut n = estado_app::Nucleo::abrir(banco, estado_app::agora_ms(), fuso, dia)?;
    if let Err(e) = n.backup_se_devido(&fila::caminhos::backups(), chrono::Local::now().naive_local(), true) {
        log_debug!("worktimer: backup ao abrir: {e}");
    }
    app.manage(Mutex::new(n));
    Ok(())
}

/// Config gravada (ou a padrão, se o banco não responder).
fn config(app: &AppHandle) -> nucleo::config::ConfigApp {
    app.try_state::<estado_app::Estado>()
        .and_then(|e| estado_app::trava(&e).banco.config_app().ok())
        .unwrap_or_default()
}

/// Eventos da janela principal (a do `tauri.conf.json` e a recriada depois de ir para a bandeja).
fn eventos_da_principal(janela: &Window, evento: &WindowEvent) {
    // o dock perdeu o foco (um clique fora): acaba o "Mostrar o dock agora" da bandeja
    if janela.label() == janelas::DOCK {
        match evento {
            WindowEvent::Focused(false) => janelas::dock_perdeu_foco(janela.app_handle()),
            // o aviso do dock acompanha o dock (rodada 15)
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => janelas::reposicionar_aviso(janela.app_handle()),
            _ => {}
        }
        return;
    }
    if janela.label() != janelas::PRINCIPAL {
        return;
    }
    let app = janela.app_handle();
    // a janela principal destruída (o X para a bandeja, o minimizar para a bandeja): a dica dela
    // e o ocupado não prendem a vez das dicas (re-revisão da rodada 16)
    if let WindowEvent::Destroyed = evento {
        comandos::liberar_dicas_da_janela(app, janelas::PRINCIPAL);
    }
    match evento {
        // a página grava o pendente e fecha (ou pergunta); "bandeja" destrói a janela e o
        // WebView dela, e a bandeja e o dock ficam
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            janelas::pedir_fechar(app, janela, janelas::AoFechar::de(&config(app).ao_fechar));
        }
        // minimizar: perguntar, bandeja (fecha como o X), dock ou barra (`ao_minimizar`)
        WindowEvent::Resized(_) if janela.is_minimized().unwrap_or(false) => {
            janelas::ao_minimizar(app, janela, janelas::AoMinimizar::de(&config(app).ao_minimizar));
        }
        _ => {}
    }
}

/// `work-timer.exe --remover-hooks` (o desinstalador, rodada 16).
pub const ARG_REMOVER_HOOKS: &str = "--remover-hooks";

/// Tira os hooks do Claude Code e a cópia do hook, sem janela, sem bandeja e sem tocar no banco.
/// Roda antes de tudo do Tauri (a instância única acordaria o app aberto). Sai com 0 sempre: o
/// desinstalador segue de qualquer jeito.
fn remover_hooks_e_sair() -> ! {
    match comandos::caminho_settings() {
        Ok(settings) => {
            let r = nucleo::hooks_claude::remover_ao_desinstalar(&settings, &fila::caminhos::hook_exe());
            log_debug!("worktimer: --remover-hooks: {r:?}");
        }
        Err(e) => log_debug!("worktimer: --remover-hooks: {e}"),
    }
    std::process::exit(0)
}

/// A linha de comando pede o `--remover-hooks`? (o primeiro argumento é o próprio exe)
fn pede_remover_hooks(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter().skip(1).any(|a| a == ARG_REMOVER_HOOKS)
}

/// Um aviso para o log (o release não tem console): uma linha no `avisos.log` da pasta de dados.
fn avisar_no_log(texto: &str) {
    log_debug!("worktimer: {texto}");
    let p = fila::caminhos::base().join("avisos.log");
    if std::fs::create_dir_all(fila::caminhos::base()).is_ok() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{} {texto}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        }
    }
}

fn main() {
    if pede_remover_hooks(std::env::args()) {
        remover_hooks_e_sair();
    }
    // o perfil vem antes de qualquer caminho (rodada 17): todos passam por `fila::caminhos`
    let (perfil, aviso) = fila::caminhos::perfil_da_linha(std::env::args());
    fila::caminhos::definir_perfil(perfil);
    if let Some(a) = aviso {
        avisar_no_log(&a);
    }
    // o identificador do perfil de teste separa a instância única e os dados do WebView2
    let mut contexto = tauri::generate_context!();
    let id = perfil::identificador(&contexto.config().identifier, perfil);
    contexto.config_mut().identifier = id;
    tauri::Builder::default()
        .manage::<comandos::EstadoDicas>(Mutex::new(nucleo::dicas::Dicas::default()))
        .manage::<atualizar::EstadoAtualizacao>(Mutex::new(atualizar::Atualizacao::default()))
        // o nome de exibição chega às páginas (principal e dock) antes dos scripts delas
        .plugin(tauri::plugin::Builder::<tauri::Wry>::new("nome-app").js_init_script(script_do_nome()).build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            janelas::mostrar_principal(app);
        }))
        // o lançador do macOS é ignorado no Windows; ao iniciar com o Windows o app abre só na bandeja
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![janelas::ARG_BANDEJA]),
        ))
        // atualizar pelo app (rodada 25): o endpoint, a chave pública e o modo passivo ficam no
        // tauri.conf.json; a página não chama o plugin direto (os comandos em `atualizar`)
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            iniciar_nucleo(app)?;

            bandeja::criar(app.handle())?;
            if let Some(d) = app.get_webview_window(janelas::DOCK) {
                let _ = d.set_title(&janelas::titulo_dock());
            }
            janelas::restaurar_dock(app.handle(), &config(app.handle()));

            // a principal não nasce com o app (tauri.conf.json): aberto pelo Windows, nem é
            // criada (o WebView dela não ocupa memória) até alguém pedir; no primeiro uso, sim
            let primeiro_uso =
                app.try_state::<estado_app::Estado>().is_none_or(|e| estado_app::trava(&e).primeiro_uso());
            if janelas::principal_na_partida(janelas::so_bandeja(std::env::args()), primeiro_uso) {
                // sem a janela, o dock e a bandeja seguem contando; ela é recriada ao abrir
                if let Err(e) = janelas::criar_principal(app.handle()) {
                    log_debug!("worktimer: não consegui criar a janela principal: {e}");
                }
            }

            laco::iniciar(app.handle().clone());
            laco::avisar(app.handle());
            // uma vez por dia, ao abrir (fora do perfil de teste e com a opção ligada)
            atualizar::buscar_ao_abrir(app.handle());
            Ok(())
        })
        .on_window_event(eventos_da_principal)
        .invoke_handler(tauri::generate_handler![
            comandos::obter_estado,
            comandos::play,
            comandos::play_atividade,
            comandos::parar_atividade,
            comandos::pausar_atividade,
            comandos::pausar,
            comandos::encerrar,
            comandos::desfazer_encerrar,
            comandos::pausar_tudo,
            comandos::zerar,
            comandos::zerar_atividade,
            comandos::zerar_todos,
            comandos::desfazer_zerar,
            comandos::descartar_hoje,
            comandos::desfazer_descartar,
            comandos::ajustar_hoje,
            comandos::desfazer_ajuste,
            comandos::criar_grupo,
            comandos::editar_grupo,
            comandos::criar_timer,
            comandos::editar_timer,
            comandos::checar_editar_timer,
            comandos::mover_grupo,
            comandos::mover_timer,
            comandos::desfazer_mover,
            comandos::criar_espaco,
            comandos::editar_espaco,
            comandos::apagar_espaco,
            comandos::criar_atividade,
            comandos::editar_atividade,
            comandos::sessoes_recentes,
            comandos::ligar_sessao,
            comandos::desligar_sessao,
            comandos::responder,
            comandos::salvar_config,
            comandos::ajustes_info,
            comandos::ler_corte,
            comandos::regras_pasta,
            comandos::conferir_regra,
            comandos::salvar_regra,
            comandos::restaurar_regra,
            comandos::trocar_regra_por_subpasta,
            comandos::escolher_pasta,
            comandos::remover_regra_pasta,
            comandos::abrir_ajustes,
            comandos::hooks_estado,
            comandos::hooks_previa,
            comandos::hooks_instalar,
            comandos::hooks_remover,
            comandos::abrir_app,
            comandos::fechar_principal,
            comandos::fechar_visto,
            comandos::minimizar_principal,
            comandos::minimizar_visto,
            comandos::backups_listar,
            comandos::backup_agora,
            comandos::backup_restaurar,
            comandos::backups_abrir_pasta,
            comandos::ajustes_pedidos,
            comandos::pedir_novo_timer,
            comandos::novo_timer_pedido,
            comandos::dock_etapa,
            comandos::dock_guardar,
            comandos::dock_ajustar,
            comandos::dock_mouse,
            comandos::aviso_mostrar,
            comandos::pedir_ligar_sessao,
            comandos::marcar_tour_visto,
            comandos::dica_reivindicar,
            comandos::dica_saiu,
            comandos::inicio_windows,
            comandos::conexao_claude,
            comandos::projetos_recentes,
            comandos::abrir_link,
            comandos::apagar_perfil_teste,
            comandos::dicas_pagina_comecou,
            comandos::principal_ocupada,
            comandos::dicas_ligar,
            comandos::dicas_de_novo,
            comandos::dock_piscar,
            comandos::ligar_sessao_pedido,
            comandos::dialogo_do_dock_fechou,
            comandos::aviso_tamanho,
            comandos::aviso_fechar,
            comandos::aviso_clicado,
            comandos::aviso_atual,
            atualizar::atualizacao_estado,
            atualizar::atualizacao_procurar,
            atualizar::atualizacao_instalar,
        ])
        .build(contexto)
        .unwrap_or_else(|e| panic!("erro ao executar o {APP_NOME}: {e}"))
        .run(|_app, evento| {
            // sem janela aberta (a principal foi para a bandeja) o app continua: só sai pelo
            // "Sair" (bandeja ou diálogo), que pede a saída com código
            if let RunEvent::ExitRequested { code: None, api, .. } = evento {
                api.prevent_exit();
            }
        });
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O modelo do instalador (sem teste do NSIS em si): o desinstalador só tira os hooks na
    /// desinstalação de verdade, e o instalador, ao chamar o desinstalador antigo, sempre passa
    /// `/MANTERHOOKS` (revisão da rodada 16: reinstalar a mesma versão não tira os hooks).
    #[test]
    fn instalador_so_tira_os_hooks_na_desinstalacao_de_verdade() {
        let nsi = include_str!("../nsis/instalador.nsi").replace("\r\n", "\n");
        assert!(!nsi.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')), "caractere de controle no modelo");
        assert!(nsi.contains("Var ManterHooks\n"));
        // o instalador chama o desinstalador antigo sempre com /MANTERHOOKS (antes do ExecWait)
        let reinst = &nsi[nsi.find("reinst_uninstall:").unwrap()..nsi.find("reinst_done:").unwrap()];
        let manter = reinst.find(r#"StrCpy $R1 "$R1 /MANTERHOOKS""#).expect("/MANTERHOOKS no reinst_uninstall");
        let exec = reinst.rfind("ExecWait '$R1' $0").unwrap();
        assert!(manter < exec);
        // o desinstalador lê o flag e só roda o --remover-hooks sem ele e sem /UPDATE
        assert!(nsi.contains("${GetOptions} $CMDLINE \"/MANTERHOOKS\" $ManterHooks"));
        let des = &nsi[nsi.find("Section Uninstall").unwrap()..];
        let trecho = "  ${If} $UpdateMode <> 1\n  ${AndIf} $ManterHooks <> 1\n    ExecWait '\"$INSTDIR\\${MAINBINARYNAME}.exe\" --remover-hooks'\n  ${EndIf}";
        let i = des.find(trecho).expect("o --remover-hooks guardado pelos dois flags");
        assert!(i < des.find("Delete \"$INSTDIR\\${MAINBINARYNAME}.exe\"").unwrap(), "antes de apagar os arquivos");
        assert!(i > des.find("CheckIfAppIsRunning").unwrap(), "depois de fechar o app");
        assert_eq!(nsi.matches("--remover-hooks").count(), 1);
    }

    /// 0.4.1 (teste do atualizador; antes, rodada 23: 0.3.1 → 0.4.0): a versão 0.4.1 no `tauri.conf.json` e nos
    /// `Cargo.toml`, e o instalador que o `build.ps1` copia com o nome de exibição:
    /// `Chronn_0.4.1_x64-setup.exe`.
    #[test]
    fn versao_0_4_1_e_o_nome_do_instalador() {
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.4.1");
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["version"], "0.4.1");
        for toml in [
            include_str!("../../crates/fila/Cargo.toml"),
            include_str!("../../crates/hook/Cargo.toml"),
            include_str!("../../crates/nucleo/Cargo.toml"),
            include_str!("../../crates/sinais/Cargo.toml"),
        ] {
            assert!(toml.replace("\r\n", "\n").contains("\nversion = \"0.4.1\"\n"), "{toml}");
        }
        // o Cargo.lock acompanha os pacotes do workspace
        let lock = include_str!("../../Cargo.lock").replace("\r\n", "\n");
        for nome in ["fila", "nucleo", "sinais", "work-timer", "work-timer-hook"] {
            assert!(lock.contains(&format!("name = \"{nome}\"\nversion = \"0.4.1\"\n")), "{nome} no Cargo.lock");
        }
        // o build.ps1 pega o instalador da versão do tauri.conf.json (revisão da rodada 25) e faz a
        // cópia com o nome de exibição
        let ps1 = include_str!("../../scripts/build.ps1");
        assert!(ps1.contains(r#"$versao = $conf.version"#));
        assert!(ps1.contains(r#"(Join-Path $pasta "Work Timer_${versao}_x64-setup.exe")"#));
        assert!(ps1.contains(r#"$copia = Join-Path $pasta "${nome}_${versao}_x64-setup.exe""#));
        let versao = conf["version"].as_str().unwrap();
        assert_eq!(format!("{APP_NOME}_{versao}_x64-setup.exe"), "Chronn_0.4.1_x64-setup.exe");
        // as variáveis da chave saem do ambiente no fim, com ou sem erro
        assert!(ps1.contains("Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD"));
    }

    /// Rodada 23: o instalador em inglês e em português, escolhido pelo idioma do Windows (sem a
    /// caixa de escolha; o inglês vem primeiro, para os outros idiomas). Os nossos textos
    /// (`LangString`) nos dois idiomas, com as mesmas chaves, e toda chave que o modelo lê existe
    /// nos dois.
    #[test]
    fn instalador_em_ingles_e_em_portugues() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let nsis = &conf["bundle"]["windows"]["nsis"];
        assert_eq!(nsis["languages"], serde_json::json!(["English", "PortugueseBR"]));
        assert_eq!(nsis["displayLanguageSelector"], false);
        assert_eq!(nsis["customLanguageFiles"]["English"], "nsis/idiomas/English.nsh");
        assert_eq!(nsis["customLanguageFiles"]["PortugueseBR"], "nsis/idiomas/PortugueseBR.nsh");
        let chaves = |texto: &str, lang: &str| -> Vec<String> {
            texto
                .lines()
                .filter(|l| l.starts_with("LangString "))
                .map(|l| {
                    let mut p = l.split_whitespace();
                    p.next();
                    let k = p.next().unwrap().to_string();
                    assert_eq!(p.next(), Some(lang), "{l}");
                    assert!(l.ends_with('"') && l.matches('"').count() >= 2, "{l}");
                    k
                })
                .collect()
        };
        let en = chaves(include_str!("../nsis/idiomas/English.nsh"), "${LANG_ENGLISH}");
        let pt = chaves(include_str!("../nsis/idiomas/PortugueseBR.nsh"), "${LANG_PORTUGUESEBR}");
        assert_eq!(en, pt, "as mesmas chaves, na mesma ordem");
        // o que o modelo lê com $(chave) existe nos dois (menos o choowHowToInstall, que ele mesmo põe)
        let nsi = include_str!("../nsis/instalador.nsi");
        for parte in nsi.split("$(").skip(1) {
            let k = &parte[..parte.find(')').unwrap()];
            if k.starts_with('^') || k == "choowHowToInstall" {
                continue;
            }
            assert!(en.iter().any(|x| x == k), "{k} falta nos textos do instalador");
        }
        // as do utils.nsh do bundler (fechar o app aberto) também
        for k in ["appRunning", "appRunningOkKill", "failedToKillApp", "noPermissionToCloseApp"] {
            assert!(en.iter().any(|x| x == k), "{k}");
        }
        // o nome de exibição em todos (nunca o {{product_name}} do bundler, que é o nome interno)
        for f in [include_str!("../nsis/idiomas/English.nsh"), include_str!("../nsis/idiomas/PortugueseBR.nsh")] {
            assert!(!f.contains("{{product_name}}"));
            assert!(f.contains("${PRODUCTNAME}"));
        }
    }

    /// Revisão da rodada 23 (M1): a instalação grava o idioma do instalador sempre, também na
    /// silenciosa, para o desinstalador sair no mesmo idioma sem perguntar.
    #[test]
    fn instalador_grava_o_idioma_para_o_desinstalador() {
        let nsi = include_str!("../nsis/instalador.nsi").replace("\r\n", "\n");
        let sec = &nsi[nsi.find("Section Install\n").unwrap()..];
        let sec = &sec[..sec.find("SectionEnd").unwrap()];
        assert!(sec.contains(r#"WriteRegStr HKCU "${MANUPRODUCTKEY}" "Installer Language" $LANGUAGE"#));
        assert!(nsi.contains(r#"!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language""#));
        assert!(nsi.contains("!insertmacro MUI_UNGETLANGUAGE"));
    }

    /// Rodada 25: o atualizador no tauri.conf.json: a chave pública do usuário, o endpoint da
    /// última release, o instalador passivo e os artefatos assinados na build.
    #[test]
    fn atualizador_configurado() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let u = &conf["plugins"]["updater"];
        assert_eq!(
            u["pubkey"],
            "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEFDMENFOTcwODk0QTdFQUEKUldTcWZrcUpjT2tNcklZV1dhMGZpNVBzaUtrcGsyM2lkMm45ZmNvTHBPUmhTRHZZL1lWNUJkSlEK"
        );
        assert_eq!(u["endpoints"], serde_json::json!(["https://github.com/eaeniko/chronn/releases/latest/download/latest.json"]));
        assert_eq!(u["windows"]["installMode"], "passive");
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], true);
        // revisão da rodada 25: a versão do latest.json tem de ser a assinada no .sig (o latest.json
        // não é assinado; sem isto, ele poderia apontar uma versão nova para um instalador velho)
        assert_eq!(u["requireSignedVersion"], true);
        // e o plugin lê a opção como está escrita
        let lida: tauri_plugin_updater::Config = serde_json::from_value(u.clone()).unwrap();
        assert!(lida.require_signed_version);
        // o instalador do Chronn aceita o que o plugin passa (/P /UPDATE /R /ARGS) e reabre o app
        let nsi = include_str!("../nsis/instalador.nsi").replace("\r\n", "\n");
        for opcao in [r#"${GetOptions} $CMDLINE "/P" $PassiveMode"#, r#"${GetOptions} $CMDLINE "/UPDATE" $UpdateMode"#, r#"${GetOptions} $CMDLINE "/R" $R0"#, r#"${GetOptions} $CMDLINE "/ARGS" $R0"#] {
            assert!(nsi.contains(opcao), "{opcao}");
        }
        let fim = &nsi[nsi.find("Function .onInstSuccess").unwrap()..];
        let fim = &fim[..fim.find("FunctionEnd").unwrap()];
        assert!(fim.contains(r#"nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "$R0""#));
    }

    /// Rodada 18: o publisher "eaeniko" vai para o "Aplicativos instalados" e muda a chave do
    /// fabricante; a pasta de uma instalação de antes (chave "github") ainda é achada.
    #[test]
    fn instalador_com_o_publisher_e_a_chave_antiga() {
        let nsi = include_str!("../nsis/instalador.nsi").replace("\r\n", "\n");
        assert!(nsi.contains("!define MANUFACTURER \"{{manufacturer}}\""));
        assert!(nsi.contains("WriteRegStr SHCTX \"${UNINSTKEY}\" \"Publisher\" \"${MANUFACTURER}\""));
        assert!(nsi.contains("VIAddVersionKey \"LegalCopyright\" \"${COPYRIGHT}\""));
        assert!(nsi.contains(r#"!define MANUPRODUCTKEYANTIGA "Software\github\${NOMEINTERNO}""#));
        let ler = "ReadRegStr $4 SHCTX \"${MANUPRODUCTKEY}\" \"\"\n";
        assert_eq!(nsi.matches(ler).count(), 2);
        for (i, _) in nsi.match_indices(ler) {
            let depois = nsi[i + ler.len()..].trim_start();
            assert!(depois.starts_with("${If} $4 == \"\""), "a chave antiga logo depois");
            assert!(depois.contains("ReadRegStr $4 SHCTX \"${MANUPRODUCTKEYANTIGA}\" \"\""));
        }
    }

    /// Os comandos que a janela principal chama e que podem escondê-la ou destruí-la são
    /// assíncronos: de dentro de um comando síncrono dela, travaria o WebView2 (re-revisão da
    /// rodada 16).
    #[test]
    fn comandos_que_fecham_a_principal_sao_assincronos() {
        let c = include_str!("comandos.rs").replace("\r\n", "\n");
        for nome in ["dialogo_do_dock_fechou", "fechar_principal", "minimizar_principal"] {
            assert!(c.contains(&format!("pub async fn {nome}(")), "{nome} precisa ser async");
        }
    }

    #[test]
    fn remover_hooks_so_pelo_argumento() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(pede_remover_hooks(args(&["work-timer.exe", "--remover-hooks"])));
        assert!(!pede_remover_hooks(args(&["work-timer.exe"])));
        assert!(!pede_remover_hooks(args(&["work-timer.exe", janelas::ARG_BANDEJA])));
        assert!(!pede_remover_hooks(args(&["--remover-hooks"])), "o nome do exe não conta");
    }
}
