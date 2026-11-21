// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Atualizar pelo app (rodada 25), com o `tauri-plugin-updater`: ele lê o `latest.json` da última
//! release (o endpoint e a chave pública ficam no `tauri.conf.json`), baixa o instalador, confere
//! a assinatura e roda o instalador no modo passivo, que reabre o app no fim. É a única consulta
//! à internet do app.
//!
//! As regras sem rede (quando a busca automática roda, o rótulo da versão) ficam em
//! `nucleo::atualizacao`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use nucleo::idioma::{ta, tfa};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::estado_app::{self, Estado};

/// A versão nova à vista: o ícone no cabeçalho e o diálogo de atualizar.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Novidade {
    pub versao: String,
    pub notas: String,
}

/// O que a última busca achou (a atualização inteira, para instalar sem buscar de novo).
#[derive(Default)]
pub struct Atualizacao {
    pub achada: Option<Update>,
}

pub type EstadoAtualizacao = Mutex<Atualizacao>;

/// Evento com a `Option<Novidade>` depois de cada busca: o ícone aparece ou some.
pub const EVENTO: &str = "atualizacao";
/// Evento do download: `{ baixado, total }` em bytes (`total` nulo se o servidor não diz).
pub const EVENTO_PROGRESSO: &str = "atualizacao-progresso";

#[derive(Clone, Serialize)]
struct Progresso {
    baixado: u64,
    total: Option<u64>,
}

fn novidade(u: &Update) -> Novidade {
    Novidade { versao: u.version.clone(), notas: u.body.clone().unwrap_or_default() }
}

fn trava(app: &AppHandle) -> std::sync::MutexGuard<'_, Atualizacao> {
    app.state::<EstadoAtualizacao>().inner().lock().unwrap_or_else(|p| p.into_inner())
}

/// O erro do plugin é de rede (sem internet, servidor fora)? A frase do `reqwest` é técnica e em
/// inglês: na tela vai só "sem internet?", e o detalhe fica no log.
fn e_de_rede(e: &tauri_plugin_updater::Error) -> bool {
    use tauri_plugin_updater::Error as E;
    matches!(e, E::Reqwest(_) | E::Network(_))
}

/// O erro da busca numa frase para a tela (revisão da rodada 25: a rede numa frase curta).
fn erro_ao_procurar(e: &tauri_plugin_updater::Error) -> String {
    log_debug!("worktimer: atualização: a busca falhou: {e}");
    if e_de_rede(e) {
        ta("erro.atualizar_procurar_rede").into()
    } else {
        tfa("erro.atualizar_procurar", &[&e.to_string()])
    }
}

/// O erro do download e da instalação numa frase para a tela: a assinatura que não confere à
/// parte (também a versão assinada que não é a anunciada), porque ali nada foi instalado e a
/// pessoa precisa saber que não é falta de internet; a rede numa frase curta.
fn erro_ao_instalar(e: &tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error as E;
    log_debug!("worktimer: atualização: a instalação falhou: {e}");
    match e {
        E::Minisign(_) | E::Base64(_) | E::SignatureUtf8(_) | E::SignedVersionMismatch { .. } | E::MissingSignedVersion => {
            ta("erro.atualizar_assinatura").into()
        }
        e if e_de_rede(e) => ta("erro.atualizar_baixar_rede").into(),
        outro => tfa("erro.atualizar_baixar", &[&outro.to_string()]),
    }
}

/// Grava o horário da busca que deu certo (a regra das 24 h conta dele) e avisa as janelas, que
/// mostram a data nos Ajustes.
fn marcar_busca(app: &AppHandle) {
    if let Some(e) = app.try_state::<Estado>() {
        let agora = estado_app::agora_ms();
        if let Err(err) = estado_app::trava(&e).guardar_dock(|c| c.atualizacao_ultima = Some(agora)) {
            log_debug!("worktimer: atualização: não gravei a hora da busca: {err}");
        }
    }
    crate::laco::avisar(app);
}

/// O GitHub respondeu, mas sem `latest.json` (404): a última release não tem o arquivo, por
/// exemplo uma de antes do atualizador. O plugin chama isso de `ReleaseNotFound`. Para a pessoa,
/// é "não há versão nova", e não falta de internet (a falha de conexão vem como `Reqwest`).
fn sem_versao_publicada(e: &tauri_plugin_updater::Error) -> bool {
    matches!(e, tauri_plugin_updater::Error::ReleaseNotFound)
}

/// Busca agora. Achou ou não, guarda o resultado e avisa as janelas pelo `EVENTO`.
async fn procurar_agora(app: &AppHandle) -> Result<Option<Novidade>, String> {
    let updater = app.updater().map_err(|e| erro_ao_procurar(&e))?;
    let achada = match updater.check().await {
        Ok(a) => a,
        Err(e) if sem_versao_publicada(&e) => {
            log_debug!("worktimer: atualização: a última release não tem latest.json; nada novo");
            None
        }
        Err(e) => return Err(erro_ao_procurar(&e)),
    };
    let n = achada.as_ref().map(novidade);
    trava(app).achada = achada;
    marcar_busca(app);
    let _ = app.emit(EVENTO, &n);
    Ok(n)
}

/// A atualização já achada nesta abertura (o ícone, ao abrir ou recarregar uma janela).
#[tauri::command(rename_all = "snake_case")]
pub fn atualizacao_estado(app: AppHandle) -> Option<Novidade> {
    trava(&app).achada.as_ref().map(novidade)
}

/// "Verificar atualizações" (Ajustes → Sobre). O erro chega à tela ("Não foi possível
/// verificar").
#[tauri::command(rename_all = "snake_case")]
pub async fn atualizacao_procurar(app: AppHandle) -> Result<Option<Novidade>, String> {
    procurar_agora(&app).await
}

/// "Atualizar e reiniciar": baixa (com o progresso no `EVENTO_PROGRESSO`), confere a assinatura e
/// instala. No Windows, o plugin roda o instalador no modo passivo (`/P /UPDATE /R`) e encerra o
/// app; o instalador reabre o app no fim. Fora do Windows, o app reinicia aqui. Com erro, nada
/// muda e o app segue como está.
///
/// No perfil de teste, não instala (revisão da rodada 25): o exe é o mesmo do uso, então a
/// instalação de verdade mudaria, e o app reabriria no perfil de teste. A página nem oferece.
#[tauri::command(rename_all = "snake_case")]
pub async fn atualizacao_instalar(app: AppHandle) -> Result<(), String> {
    if crate::perfil::e_teste() {
        return Err(ta("erro.atualizar_teste").into());
    }
    let achada = trava(&app).achada.clone();
    let update = match achada {
        Some(u) => u,
        None => {
            procurar_agora(&app).await?;
            trava(&app).achada.clone().ok_or_else(|| ta("erro.atualizar_nenhuma").to_string())?
        }
    };
    // o total baixado, para o aviso do fim: sem o tamanho total (o servidor não disse), o fim do
    // download vai como `total = baixado`, e o diálogo passa a "Instalando…"
    let baixado = Arc::new(AtomicU64::new(0));
    let (janelas, contado) = (app.clone(), baixado.clone());
    let (fim, ate_aqui) = (app.clone(), baixado.clone());
    update
        .download_and_install(
            move |n, total| {
                let baixado = contado.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
                let _ = janelas.emit(EVENTO_PROGRESSO, Progresso { baixado, total });
            },
            move || {
                let baixado = ate_aqui.load(Ordering::Relaxed);
                let _ = fim.emit(EVENTO_PROGRESSO, Progresso { baixado, total: Some(baixado) });
            },
        )
        .await
        .map_err(|e| erro_ao_instalar(&e))?;
    app.restart()
}

/// Ao abrir o app: a busca automática, se a regra das 24 h deixar (ligada, fora do perfil de
/// teste). Roda em segundo plano; falha de rede é silenciosa (só não aparece o ícone).
pub fn buscar_ao_abrir(app: &AppHandle) {
    let cfg = app
        .try_state::<Estado>()
        .and_then(|e| estado_app::trava(&e).banco.config_app().ok())
        .unwrap_or_default();
    let devida = nucleo::atualizacao::busca_automatica_devida(
        cfg.atualizar_auto,
        cfg.atualizacao_ultima,
        estado_app::agora_ms(),
        crate::perfil::e_teste(),
    );
    if !devida {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = procurar_agora(&app).await {
            log_debug!("worktimer: busca automática de atualização: {e}");
        }
    });
}

#[cfg(test)]
mod testes {
    use super::*;
    use tauri_plugin_updater::Error as E;

    /// Revisão da rodada 25: a versão assinada que não confere é erro de assinatura (nada foi
    /// instalado), e a rede vira uma frase curta, sem o texto técnico do `reqwest`.
    #[test]
    fn erros_do_atualizador_na_tela() {
        let assinatura = ta("erro.atualizar_assinatura");
        assert_eq!(erro_ao_instalar(&E::MissingSignedVersion), assinatura);
        assert_eq!(
            erro_ao_instalar(&E::SignedVersionMismatch { signed: "0.4.0".into(), announced: "9.9.9".into() }),
            assinatura
        );
        assert_eq!(erro_ao_instalar(&E::Network("error sending request for url (…)".into())), ta("erro.atualizar_baixar_rede"));
        assert_eq!(erro_ao_procurar(&E::Network("error sending request".into())), ta("erro.atualizar_procurar_rede"));
        // o resto leva o detalhe
        // a release sem latest.json (404) é "nada novo"; a falha de conexão continua erro
        assert!(sem_versao_publicada(&E::ReleaseNotFound));
        assert!(!sem_versao_publicada(&E::Network("error sending request".into())));
        let outro = erro_ao_instalar(&E::ReleaseNotFound);
        assert!(outro.contains(&E::ReleaseNotFound.to_string()), "{outro}");
        // nenhuma frase começa com "não": o diálogo diz "Não foi possível atualizar: …"
        for chave in ["erro.atualizar_procurar", "erro.atualizar_procurar_rede", "erro.atualizar_baixar", "erro.atualizar_baixar_rede", "erro.atualizar_assinatura", "erro.atualizar_teste"] {
            for idioma in [nucleo::idioma::Idioma::Pt, nucleo::idioma::Idioma::En] {
                let texto = nucleo::idioma::t(idioma, chave).to_lowercase();
                assert!(!texto.starts_with("não") && !texto.starts_with("couldn"), "{chave}: {texto}");
            }
        }
    }
}
