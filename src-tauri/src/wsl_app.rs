// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O Claude Code no WSL no app (rodada 28): os comandos da seção "Claude Code no WSL" e o laço do
//! WSL, numa thread própria, que acha e lê as filas das distros ligadas e rodando. A lógica fica em
//! `nucleo::wsl` (testada com o `WslFalso`); aqui só o WSL de verdade (`WslReal`), o recurso do
//! app e a config. O perfil de teste nunca toca no WSL: a seção não aparece e os comandos recusam.
//!
//! 0.6.1 (revisão C1 e I1): cada passo pergunta quais distros rodam e lê a fila de cada uma **logo
//! em seguida** (até `VALIDADE_LEITURA` depois do `wsl -l --running`), fora da trava do estado;
//! a trava só serve para aplicar os eventos. O laço principal nunca espera o `wsl.exe` nem o 9P.
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use nucleo::wsl::{self, DistroDto, Homes, Wsl};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::estado_app::{texto, trava, Estado};
use crate::wsl_real::WslReal;

/// A cada quanto o laço do WSL pergunta quais distros rodam (o `wsl -l --running -q`, ~60 ms; era
/// 30 s) e lê as filas delas. Sem distro ligada, o `wsl.exe` nunca é chamado.
pub const CONSULTA_WSL: Duration = Duration::from_secs(10);
/// O passo com alguma distro "quente" (0.6.1): a consulta e a leitura a cada 2 s.
pub const LEITURA_RAPIDA: Duration = Duration::from_secs(2);
/// A distro "quente": teve evento há menos disto, e o último não foi o fim da sessão (0.6.1).
pub const QUENTE: Duration = Duration::from_secs(3 * 60);
/// A fila (e o `~/.claude.json`) de uma distro só é lida até este tempo depois do `rodando()` que
/// disse que ela roda (revisão C1): mais que isso, ela pode ter parado, e ler a ligaria de novo.
pub const VALIDADE_LEITURA: Duration = Duration::from_secs(2);
/// A cada quanto os rótulos das contas são conferidos (só a data e o tamanho do `~/.claude.json`;
/// o arquivo é lido de novo só quando muda; revisão M2).
pub const ROTULOS_A_CADA: Duration = Duration::from_secs(60);

/// As pastas do usuário de cada distro e os hooks confirmados nesta execução (comandos e laço).
static HOMES: Mutex<Option<Homes>> = Mutex::new(None);
/// Pedido de conferir de novo o hook de cada distro (a cada 30 min, do laço principal).
static CONFERIR: AtomicBool = AtomicBool::new(false);

fn com_homes<T>(f: impl FnOnce(&mut Homes) -> T) -> T {
    let mut g = HOMES.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Homes::default))
}

/// O hook para Linux que vem com o app (`bundle.resources`), pelo caminho dos recursos do Tauri.
pub fn recurso_do_hook(app: &AppHandle) -> Option<PathBuf> {
    app.path().resolve(wsl::RECURSO_HOOK_LINUX, tauri::path::BaseDirectory::Resource).ok()
}

/// O hook de cada distro é conferido de novo no próximo passo. Só marca: nunca espera.
pub fn conferir_de_novo() {
    CONFERIR.store(true, Ordering::SeqCst);
}

/// Os rótulos das contas lidos de cada `~/.claude.json` (0.6.1, revisão M2): o arquivo só é lido
/// de novo quando a data ou o tamanho mudam; o último resultado vale até lá.
#[derive(Default)]
pub struct LeitorDeRotulos {
    vistos: HashMap<PathBuf, ((u64, Option<SystemTime>), Option<(String, String)>)>,
}

impl LeitorDeRotulos {
    /// O resumo do `accountUuid` e o rótulo do arquivo (`nucleo::claude_registros::rotulo_da_conta`).
    pub fn ler(&mut self, caminho: &Path) -> Option<(String, String)> {
        let Ok(m) = std::fs::metadata(caminho) else {
            self.vistos.remove(caminho);
            return None;
        };
        let assinatura = (m.len(), m.modified().ok());
        if let Some((a, r)) = self.vistos.get(caminho) {
            if *a == assinatura {
                return r.clone();
            }
        }
        let r = nucleo::claude_registros::rotulo_da_conta(caminho);
        self.vistos.insert(caminho.to_path_buf(), (assinatura, r.clone()));
        r
    }
}

/// A fila de uma distro lida fora da trava (revisão I1): os eventos novos desde `offset` e até onde
/// foi lida (a fila encolhida recomeça do zero). Sem a fila (ainda sem evento, ou a distro parou no
/// meio), nada. Quem chama esvazia a fila grande depois de aplicar os eventos.
pub fn ler_fila_fora(fila: &Path, offset: u64) -> Result<Option<(Vec<::fila::EventoHook>, u64)>, String> {
    let Ok(tamanho) = std::fs::metadata(fila).map(|m| m.len()) else { return Ok(None) };
    let offset = if tamanho < offset { 0 } else { offset };
    let (evs, novo) = ::fila::ler_novos(fila, offset).map_err(texto)?;
    Ok(Some((evs, novo)))
}

/// O que um passo do laço do WSL fez.
#[derive(Debug, Default, PartialEq)]
pub struct PassoWsl {
    /// Algo mudou no estado (a tela precisa saber).
    pub mudou: bool,
    /// Alguma distro ficou quente: o próximo passo vem em `LEITURA_RAPIDA`.
    pub quente: bool,
}

/// Um passo do laço do WSL: as distros ligadas e rodando (`ciclo_das_distros`, que pergunta ao
/// WSL e confere o hook), e, logo em seguida, a fila de cada uma (só se o `rodando()` ainda vale:
/// `VALIDADE_LEITURA`), lida fora da trava. Com `rotulos` (a cada `ROTULOS_A_CADA`), também o
/// `~/.claude.json` de cada uma e se os hooks seguem no `settings.json` de lá (`wsl_sem_hooks`).
/// Sem distro ligada, o WSL nem é consultado.
pub fn passo<W: Wsl>(
    w: &W,
    estado: &Estado,
    homes: &dyn Fn(&mut dyn FnMut(&mut Homes)),
    conferidas: &mut BTreeSet<String>,
    recurso: Option<&Path>,
    rotulos: Option<&mut LeitorDeRotulos>,
) -> PassoWsl {
    passo_com_limite(w, estado, homes, conferidas, recurso, rotulos, ::fila::LIMITE_FILA)
}

/// `passo` com o tamanho a partir do qual a fila lida inteira é esvaziada (os testes usam um menor).
pub fn passo_com_limite<W: Wsl>(
    w: &W,
    estado: &Estado,
    homes: &dyn Fn(&mut dyn FnMut(&mut Homes)),
    conferidas: &mut BTreeSet<String>,
    recurso: Option<&Path>,
    rotulos: Option<&mut LeitorDeRotulos>,
    limite_fila: u64,
) -> PassoWsl {
    let cfg = trava(estado).banco.config_app().unwrap_or_default();
    if cfg.wsl_distros.is_empty() {
        // ninguém pergunta mais quais rodam: a lista fica vazia (re-revisão N1)
        wsl::definir_rodando(&[]);
        conferidas.clear();
        trava(estado).wsl_ultimo_evento.clear();
        return PassoWsl::default();
    }
    if CONFERIR.swap(false, Ordering::SeqCst) {
        conferidas.clear();
    }
    let mut resultado = (vec![], vec![]);
    homes(&mut |h| resultado = wsl::ciclo_das_distros(w, h, &cfg.wsl_distros, &cfg.pastas_ignoradas, recurso, conferidas));
    let (filas, erros) = resultado;
    for e in erros {
        log_debug!("worktimer: WSL: {e}");
    }
    // a distro que parou deixa de ser quente (revisão C1)
    trava(estado).wsl_ultimo_evento.retain(|d, _| filas.iter().any(|(x, _)| x == d));
    let mut mudou = false;
    let mut lidos = vec![];
    let conferir_hooks = rotulos.is_some();
    let mut rotulos = rotulos;
    for (distro, caminho) in &filas {
        if !wsl::pode_tocar_recente(&caminho.to_string_lossy(), VALIDADE_LEITURA) {
            continue;
        }
        let offset = match trava(estado).offset_da_distro(distro) {
            Ok(o) => o,
            Err(e) => {
                log_debug!("worktimer: WSL: {e}");
                continue;
            }
        };
        match ler_fila_fora(caminho, offset) {
            Ok(Some((evs, novo))) => {
                // a trava só para aplicar: solta antes de esvaziar a fila pelo 9P (re-revisão R1:
                // segurada no `match`, o `trava` de baixo esperava por ela mesma para sempre)
                let aplicado = trava(estado).aplicar_fila_da_distro(distro, evs, novo);
                match aplicado {
                    Ok(m) => {
                        mudou |= m;
                        // a fila lida inteira e grande demais é esvaziada (como a do Windows)
                        if let Ok(true) = ::fila::esvaziar_se_lida(caminho, novo, limite_fila) {
                            let zerado = trava(estado).definir_offset_da_distro(distro, 0);
                            if let Err(e) = zerado {
                                log_debug!("worktimer: WSL: {e}");
                            }
                        }
                    }
                    Err(e) => log_debug!("worktimer: erro aplicando a fila do WSL ({distro}): {e}"),
                }
            }
            Ok(None) => {}
            Err(e) => log_debug!("worktimer: erro lendo a fila do WSL ({distro}): {e}"),
        }
        if let (Some(r), Some(cj)) = (rotulos.as_deref_mut(), wsl::claude_json_da_fila(caminho)) {
            if wsl::pode_tocar_recente(&cj.to_string_lossy(), VALIDADE_LEITURA) {
                lidos.extend(r.ler(&cj));
            }
        }
    }
    // os hooks de cada distro rodando seguem no `settings.json` de lá? (0.6.1: outro programa pode
    // regravar o arquivo sem eles.) Junto com os rótulos (a cada `ROTULOS_A_CADA`), logo depois
    // do `rodando()`; a distro parada fica como estava
    if conferir_hooks {
        let mut sem = vec![];
        let mut com = vec![];
        for (distro, caminho) in &filas {
            if !wsl::pode_tocar_recente(&caminho.to_string_lossy(), VALIDADE_LEITURA) {
                continue;
            }
            let mut estado_hooks = None;
            homes(&mut |h| estado_hooks = h.de(w, distro).map(|p| wsl::estado_dos_hooks(w, distro, &p)));
            match estado_hooks {
                Some(nucleo::hooks_claude::EstadoHooks::Ligado) => com.push(distro.clone()),
                Some(_) => sem.push(distro.clone()),
                None => {}
            }
        }
        let mut n = trava(estado);
        let antes = n.wsl_sem_hooks.clone();
        for d in com {
            n.wsl_sem_hooks.remove(&d);
        }
        n.wsl_sem_hooks.extend(sem);
        // a distro desligada (saiu da config) não avisa mais
        n.wsl_sem_hooks.retain(|d| cfg.wsl_distros.contains(d));
        if n.wsl_sem_hooks != antes {
            if !antes.is_superset(&n.wsl_sem_hooks) {
                log_debug!("worktimer: WSL: os hooks saíram do settings.json de {:?}", n.wsl_sem_hooks);
            }
            mudou = true;
        }
    }
    if !lidos.is_empty() {
        match trava(estado).aplicar_rotulos(&lidos) {
            Ok(m) => mudou |= m,
            Err(e) => log_debug!("worktimer: rótulos das contas do WSL: {e}"),
        }
    }
    let n = trava(estado);
    let quente = filas.iter().any(|(d, _)| n.distro_quente(d, QUENTE));
    PassoWsl { mudou, quente }
}

/// Começa o laço do WSL numa thread própria (o perfil de teste nunca toca no WSL). A cada
/// `CONSULTA_WSL` (com uma distro quente, `LEITURA_RAPIDA`), um `passo`; o que mudou avisa as janelas.
pub fn iniciar(app: AppHandle) {
    if crate::perfil::e_teste() {
        return;
    }
    let r = std::thread::Builder::new().name("worktimer-wsl".into()).spawn(move || {
        let recurso = recurso_do_hook(&app);
        let mut conferidas = BTreeSet::new();
        let mut rotulos = LeitorDeRotulos::default();
        let mut rotulos_em: Option<Instant> = None;
        let homes = |f: &mut dyn FnMut(&mut Homes)| com_homes(|h| f(h));
        loop {
            let ler_rotulos = rotulos_em.is_none_or(|u| u.elapsed() >= ROTULOS_A_CADA);
            if ler_rotulos {
                rotulos_em = Some(Instant::now());
            }
            let feito = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let estado = app.state::<Estado>();
                passo(&WslReal, &estado, &homes, &mut conferidas, recurso.as_deref(), ler_rotulos.then_some(&mut rotulos))
            }));
            let espera = match feito {
                Ok(p) => {
                    if p.mudou {
                        crate::laco::avisar(&app);
                    }
                    if p.quente { LEITURA_RAPIDA } else { CONSULTA_WSL }
                }
                Err(_) => {
                    log_debug!("worktimer: pânico no laço do WSL; seguindo");
                    CONSULTA_WSL
                }
            };
            std::thread::sleep(espera);
        }
    });
    if let Err(e) = r {
        log_debug!("worktimer: não consegui iniciar o laço do WSL: {e}");
    }
}

/// As distros do registro com o estado de cada uma (vazia no perfil de teste: a seção some).
/// Sem resposta do WSL, erro: a tela diz que não deu para ler.
#[tauri::command(rename_all = "snake_case")]
pub async fn wsl_estado(app: AppHandle, estado: State<'_, Estado>) -> Result<Vec<DistroDto>, String> {
    if crate::perfil::e_teste() {
        return Ok(vec![]);
    }
    let ligadas = trava(&estado).banco.config_app().map_err(texto)?.wsl_distros;
    let recurso = recurso_do_hook(&app);
    tauri::async_runtime::spawn_blocking(move || com_homes(|h| wsl::estado_das_distros(&WslReal, h, &ligadas, recurso.as_deref())))
        .await
        .map_err(texto)?
}

/// A prévia do `settings.json` da distro.
#[derive(Serialize)]
pub struct PreviaWsl {
    pub previa: String,
    pub settings: String,
}

fn so_no_normal() -> Result<(), String> {
    if crate::perfil::e_teste() {
        Err(nucleo::idioma::ta("erro.so_perfil_normal").into())
    } else {
        Ok(())
    }
}

/// A prévia do "Ligar". `subir` (0.6.1): a pessoa clicou no "Ligar" de uma distro parada, e o
/// Chronn liga a distro antes (só neste clique; nunca sozinho).
#[tauri::command(rename_all = "snake_case")]
pub async fn wsl_previa(distro: String, subir: Option<bool>) -> Result<PreviaWsl, String> {
    so_no_normal()?;
    tauri::async_runtime::spawn_blocking(move || {
        if subir == Some(true) {
            wsl::subir_se_parada(&WslReal, &distro)?;
        }
        com_homes(|h| wsl::previa(&WslReal, h, &distro)).map(|(previa, settings)| PreviaWsl { previa, settings })
    })
    .await
    .map_err(texto)?
}

/// "Ligar" (a pessoa viu a prévia): o hook, o `hook.json`, os hooks no `settings.json` da distro
/// e a distro na config. Devolve o caminho Linux da cópia de segurança ("" sem arquivo antes).
#[tauri::command(rename_all = "snake_case")]
pub async fn wsl_ligar(app: AppHandle, estado: State<'_, Estado>, distro: String, subir: Option<bool>) -> Result<String, String> {
    so_no_normal()?;
    let ignoradas = trava(&estado).banco.config_app().map_err(texto)?.pastas_ignoradas;
    let recurso = recurso_do_hook(&app).ok_or_else(|| nucleo::idioma::tfa("erro.wsl_sem_hook", &[wsl::RECURSO_HOOK_LINUX]))?;
    let d = distro.clone();
    // a distro que a pessoa ligou para ver a prévia pode ter parado enquanto ela lia (0.6.1)
    let backup = tauri::async_runtime::spawn_blocking(move || {
        if subir == Some(true) {
            wsl::subir_se_parada(&WslReal, &d)?;
        }
        com_homes(|h| wsl::ligar(&WslReal, h, &d, &recurso, &ignoradas))
    })
        .await
        .map_err(texto)??;
    {
        let mut n = trava(&estado);
        n.guardar_dock(|c| {
            if !c.wsl_distros.contains(&distro) {
                c.wsl_distros.push(distro.clone());
            }
        })?;
        // os hooks voltaram: o aviso da janela principal sai na hora (0.6.1)
        n.wsl_sem_hooks.remove(&distro);
    }
    crate::laco::avisar(&app);
    Ok(backup)
}

/// "Desligar": só os nossos hooks saem do `settings.json` da distro, e ela sai da config.
#[tauri::command(rename_all = "snake_case")]
pub async fn wsl_desligar(estado: State<'_, Estado>, distro: String) -> Result<(), String> {
    so_no_normal()?;
    let d = distro.clone();
    tauri::async_runtime::spawn_blocking(move || com_homes(|h| wsl::desligar(&WslReal, h, &d))).await.map_err(texto)??;
    trava(&estado).guardar_dock(|c| c.wsl_distros.retain(|x| x != &distro))?;
    Ok(())
}

/// As pastas ignoradas mudaram: o `hook.json` das distros ligadas e rodando é regravado (fora da
/// trava; os erros vão só para o log). As paradas recebem quando voltarem (o ciclo, revisão M5).
pub fn regravar_hook_json_das_distros(ligadas: Vec<String>, ignoradas: Vec<String>) {
    if crate::perfil::e_teste() || ligadas.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        for e in com_homes(|h| wsl::regravar_hook_json(&WslReal, h, &ligadas, &ignoradas)) {
            log_debug!("worktimer: WSL: {e}");
        }
    });
}
#[cfg(test)]
mod testes {
    use super::*;
    use crate::estado_app::Nucleo;
    use chrono::NaiveDate;
    use nucleo::banco::Banco;
    use nucleo::tempo::ms_local;
    use nucleo::wsl::WslFalso;
    use std::io::Write;
    use std::sync::atomic::AtomicUsize;

    /// Um WSL que conta as vezes que perguntaram quais distros rodam.
    struct Contado(WslFalso, AtomicUsize);
    impl Wsl for Contado {
        fn distros(&self) -> Vec<String> {
            self.0.distros()
        }
        fn rodando(&self) -> Option<Vec<String>> {
            self.1.fetch_add(1, Ordering::SeqCst);
            self.0.rodando()
        }
        fn home(&self, d: &str) -> Option<String> {
            self.0.home(d)
        }
        fn chmod(&self, d: &str, c: &str) -> Result<(), String> {
            self.0.chmod(d, c)
        }
        fn raiz(&self, d: &str) -> PathBuf {
            self.0.raiz(d)
        }
    }

    fn estado() -> Estado {
        let dia = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        Mutex::new(Nucleo::abrir(Banco::abrir_memoria().unwrap(), ms_local(2026, 11, 17, 9, 0, -180), -180, dia).unwrap())
    }
    fn linha(f: &Path, j: serde_json::Value) {
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        let mut a = std::fs::OpenOptions::new().create(true).append(true).open(f).unwrap();
        writeln!(a, "{j}").unwrap();
    }

    /// 0.6.1 (revisão C1 e I1): cada passo pergunta quais rodam e lê a fila logo depois, fora da
    /// trava; a distro que parou não é lida nem fica quente; sem distro ligada, o WSL nem é
    /// consultado.
    #[test]
    fn o_passo_le_a_fila_logo_depois_de_saber_que_a_distro_roda() {
        let d = tempfile::tempdir().unwrap();
        let falso = WslFalso {
            raiz: d.path().to_path_buf(),
            distros: vec!["Ubuntu".into()],
            rodando: vec!["Ubuntu".into()],
            homes: [("Ubuntu".to_string(), "/home/u".to_string())].into(),
            ..Default::default()
        };
        let mut w = Contado(falso, AtomicUsize::new(0));
        let e = estado();
        let homes_proprio = Mutex::new(Homes::default());
        let homes = |f: &mut dyn FnMut(&mut Homes)| f(&mut homes_proprio.lock().unwrap());
        let mut conferidas = BTreeSet::new();
        // sem distro ligada: nada, e o WSL nem é consultado
        assert_eq!(passo(&w, &e, &homes, &mut conferidas, None, None), PassoWsl::default());
        assert_eq!(w.1.load(Ordering::SeqCst), 0);
        trava(&e).guardar_dock(|c| c.wsl_distros.push("Ubuntu".into())).unwrap();
        let fila = d.path().join("Ubuntu/home/u/.worktimer/fila/eventos.jsonl");
        assert_eq!(passo(&w, &e, &homes, &mut conferidas, None, None), PassoWsl { mudou: false, quente: false }, "sem fila ainda");
        let t = ms_local(2026, 11, 17, 9, 1, -180);
        linha(&fila, serde_json::json!({"t": t, "evento": "UserPromptSubmit", "sessao": "w1", "cwd": "/home/u/proj", "origem": "cli"}));
        assert_eq!(passo(&w, &e, &homes, &mut conferidas, None, None), PassoWsl { mudou: true, quente: true });
        assert_eq!(w.1.load(Ordering::SeqCst), 2);
        {
            let n = trava(&e);
            assert_eq!(n.motor.sessoes["w1"].origem.as_deref(), Some("wsl"));
            assert_eq!(n.banco.valor("fila_offset_wsl:Ubuntu").unwrap(), Some(std::fs::metadata(&fila).unwrap().len().to_string()));
        }
        // a mesma linha não é relida
        assert_eq!(passo(&w, &e, &homes, &mut conferidas, None, None), PassoWsl { mudou: false, quente: true });
        // a distro parou: a fila nem é aberta, e ela deixa de ser quente
        w.0.rodando.clear();
        linha(&fila, serde_json::json!({"t": t + 60_000, "evento": "Stop", "sessao": "w1", "cwd": "/home/u/proj", "origem": "cli"}));
        assert_eq!(passo(&w, &e, &homes, &mut conferidas, None, None), PassoWsl { mudou: false, quente: false });
        assert!(trava(&e).wsl_ultimo_evento.is_empty());
        assert_eq!(trava(&e).motor.sessoes["w1"].estado, nucleo::modelo::EstadoSessao::Trabalhando, "o Stop espera a distro voltar");
        // voltou: o Stop chega; o fim da sessão tira o "quente"
        w.0.rodando.push("Ubuntu".into());
        linha(&fila, serde_json::json!({"t": t + 120_000, "evento": "SessionEnd", "sessao": "w1", "cwd": "/home/u/proj", "origem": "cli"}));
        let p = passo(&w, &e, &homes, &mut conferidas, None, None);
        assert!(p.mudou && !p.quente, "{p:?}");
        // re-revisão R1: a fila lida inteira e maior que o limite é esvaziada sem travar (a trava
        // já foi solta), e o offset volta a 0
        linha(&fila, serde_json::json!({"t": t + 180_000, "evento": "UserPromptSubmit", "sessao": "w2", "cwd": "/home/u/proj", "origem": "cli"}));
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|sc| {
            sc.spawn(|| {
                let p = passo_com_limite(&w, &e, &homes, &mut conferidas, None, None, 10);
                tx.send(p).unwrap();
            });
            let p = rx.recv_timeout(Duration::from_secs(10)).expect("o passo travou");
            assert!(p.mudou && p.quente, "{p:?}");
        });
        assert_eq!(std::fs::metadata(&fila).unwrap().len(), 0, "esvaziada");
        assert_eq!(trava(&e).banco.valor("fila_offset_wsl:Ubuntu").unwrap().as_deref(), Some("0"));
        assert!(trava(&e).motor.sessoes.contains_key("w2"));
    }

    /// 0.6.1: outro programa regravou o `settings.json` da distro sem os nossos hooks: o passo (com
    /// os rótulos) põe a distro em `wsl_sem_hooks`, e o estado da tela leva o aviso; com os hooks de
    /// volta, sai. Sem os rótulos (os passos de 2 s), o `settings.json` nem é lido.
    #[test]
    fn o_passo_percebe_os_hooks_que_sairam_do_settings_json() {
        let d = tempfile::tempdir().unwrap();
        let w = WslFalso {
            raiz: d.path().to_path_buf(),
            distros: vec!["Ubuntu".into()],
            rodando: vec!["Ubuntu".into()],
            homes: [("Ubuntu".to_string(), "/home/u".to_string())].into(),
            ..Default::default()
        };
        let settings = d.path().join("Ubuntu/home/u/.claude/settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(&settings, "{\"hooks\": {\"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"node kit.mjs\"}]}]}}\n").unwrap();
        let e = estado();
        trava(&e).guardar_dock(|c| c.wsl_distros.push("Ubuntu".into())).unwrap();
        let homes_proprio = Mutex::new(Homes::default());
        let homes = |f: &mut dyn FnMut(&mut Homes)| f(&mut homes_proprio.lock().unwrap());
        let mut conferidas = BTreeSet::new();
        passo(&w, &e, &homes, &mut conferidas, None, None);
        assert!(trava(&e).wsl_sem_hooks.is_empty(), "sem os rótulos, não confere");
        let mut r = LeitorDeRotulos::default();
        assert!(passo(&w, &e, &homes, &mut conferidas, None, Some(&mut r)).mudou);
        assert_eq!(trava(&e).snapshot(ms_local(2026, 11, 17, 9, 0, -180)).unwrap().wsl_sem_hooks, ["Ubuntu"]);
        // o "Ligar de novo" devolveu os hooks (e os do kit ficaram)
        let p = nucleo::wsl::PastasDaDistro { home: "/home/u".into() };
        nucleo::hooks_claude::instalar(&settings, &nucleo::wsl::comando_linux(&p)).unwrap();
        assert!(std::fs::read_to_string(&settings).unwrap().contains("node kit.mjs"));
        assert!(passo(&w, &e, &homes, &mut conferidas, None, Some(&mut r)).mudou);
        assert!(trava(&e).wsl_sem_hooks.is_empty());
        assert!(!passo(&w, &e, &homes, &mut conferidas, None, Some(&mut r)).mudou, "igual: nada muda");
    }

    #[test]
    fn os_rotulos_so_sao_lidos_de_novo_quando_o_arquivo_muda() {
        let d = tempfile::tempdir().unwrap();
        let cj = d.path().join(".claude.json");
        let oauth = |nome: &str| serde_json::json!({"oauthAccount": {"accountUuid": "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0", "displayName": nome, "organizationType": "claude_pro"}}).to_string();
        std::fs::write(&cj, oauth("Ana")).unwrap();
        let mut r = LeitorDeRotulos::default();
        assert_eq!(r.ler(&cj).map(|x| x.1).as_deref(), Some("Ana · Pro"));
        // o mesmo arquivo (data e tamanho iguais): o resultado guardado
        assert_eq!(r.ler(&cj).map(|x| x.1).as_deref(), Some("Ana · Pro"));
        std::fs::write(&cj, oauth("Ana Maria")).unwrap();
        assert_eq!(r.ler(&cj).map(|x| x.1).as_deref(), Some("Ana Maria · Pro"));
        std::fs::remove_file(&cj).unwrap();
        assert_eq!(r.ler(&cj), None);
    }
}
