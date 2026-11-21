// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Comandos invocáveis pela UI. Argumentos em snake_case (`{timer_id: 1}`).
//! Cada comando trava o núcleo, age, grava, monta o snapshot, solta a trava e só então avisa.
use nucleo::idioma::{ta, tfa};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nucleo::config::ConfigApp;
use nucleo::dados::{self, Backup};
use nucleo::hooks_claude::{self, EstadoHooks};
use nucleo::modelo::{Atividade, Grupo, Mudanca, Timer};
use nucleo::snapshot::{SessaoDto, Snapshot};
use nucleo::tempo::Ms;
use serde_json::{json, Value};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::estado_app::{
    agora_ms, fuso_local, texto, trava, Descartado, Estado, Movido, Nucleo, PedidoNovoTimer, RegraDto, Zerado,
};
use crate::janelas::{self, Lado};
use crate::laco::avisar;

fn agir(
    app: &AppHandle,
    estado: &Estado,
    f: impl FnOnce(&mut Nucleo, Ms) -> Result<Vec<Mudanca>, String>,
) -> Result<Snapshot, String> {
    let snap = {
        let mut n = trava(estado);
        // sem banco (a troca falhou de vez): nada vai para o banco em memória
        n.checar_banco()?;
        let agora = agora_ms();
        let mud = f(&mut n, agora)?;
        n.aplicar(&mud)?;
        n.snapshot(agora)?
    };
    avisar(app);
    Ok(snap)
}

fn nome_valido(nome: &str) -> Result<&str, String> {
    let n = nome.trim();
    if n.is_empty() {
        Err(ta("erro.nome_vazio").into())
    } else {
        Ok(n)
    }
}

// ---- estado e timers ----

#[tauri::command(rename_all = "snake_case")]
pub fn obter_estado(estado: State<'_, Estado>) -> Result<Snapshot, String> {
    trava(&estado).snapshot(agora_ms())
}

#[tauri::command(rename_all = "snake_case")]
pub fn play(app: AppHandle, estado: State<'_, Estado>, timer_id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| {
        n.checar_timer(timer_id)?;
        Ok(n.motor.play(timer_id, agora))
    })
}

/// 👥 (reunião manual) e qualquer play manual numa atividade: o manual é um por vez por timer, e
/// trocar de atividade fecha um trecho e abre o outro no mesmo instante.
#[tauri::command(rename_all = "snake_case")]
pub fn play_atividade(app: AppHandle, estado: State<'_, Estado>, timer_id: i64, atividade_id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| {
        n.checar_timer(timer_id)?;
        n.checar_atividade(atividade_id)?;
        Ok(n.motor.play_atividade(timer_id, atividade_id, agora))
    })
}

/// 👥 apertado de novo: a reunião manual para (e o ▶ que ela substituiu volta, se havia); o
/// Claude trabalhando e a leitura seguem.
#[tauri::command(rename_all = "snake_case")]
pub fn parar_atividade(app: AppHandle, estado: State<'_, Estado>, timer_id: i64, atividade_id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| Ok(n.motor.parar_atividade(timer_id, atividade_id, agora)))
}

/// O ⏸ da linha de uma atividade no painel do timer: para só essa atividade (a reunião segue
/// quando é o trabalho que pausa).
#[tauri::command(rename_all = "snake_case")]
pub fn pausar_atividade(app: AppHandle, estado: State<'_, Estado>, timer_id: i64, atividade_id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| Ok(n.motor.pausar_atividade(timer_id, atividade_id, agora)))
}

#[tauri::command(rename_all = "snake_case")]
pub fn pausar(app: AppHandle, estado: State<'_, Estado>, timer_id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| Ok(n.motor.pausar(timer_id, agora)))
}

/// Encerrar (■): pausa e tira o timer do dock recolhido e do "esperando você" até a próxima
/// mensagem numa sessão dele, o play ou a virada do dia.
#[tauri::command(rename_all = "snake_case")]
pub fn encerrar(app: AppHandle, estado: State<'_, Estado>, timer_id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| {
        n.checar_timer(timer_id)?;
        Ok(n.motor.encerrar(timer_id, agora))
    })
}

/// "Desfazer" da faixa "Encerrado": tira a marca e reabre, sem buraco, o que o ■ fechou
/// (o play manual e o Claude que ainda trabalha). Vencido ou já desfeito, não faz nada.
#[tauri::command(rename_all = "snake_case")]
pub fn desfazer_encerrar(app: AppHandle, estado: State<'_, Estado>, timer_id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| Ok(n.motor.desfazer_encerrar(timer_id, agora)))
}

/// "Pausar tudo": com `espaco_id` (a aba de um espaço, rodada 20), só os timers dele.
#[tauri::command(rename_all = "snake_case")]
pub fn pausar_tudo(app: AppHandle, estado: State<'_, Estado>, espaco_id: Option<i64>) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| n.pausar_tudo(espaco_id, agora))
}

/// "Pausar tudo" do menu da bandeja.
pub fn pausar_tudo_agora(app: &AppHandle) {
    if let Some(estado) = app.try_state::<Estado>() {
        let _ = agir(app, &estado, |n, agora| Ok(n.motor.pausar_tudo(agora)));
    }
}

// ---- zerar e descartar ----

/// ↺ "Zerar": o número do timer recomeça de 0:00 agora (nada é apagado; rodando, segue rodando).
/// Devolve o que o `desfazer_zerar` precisa.
#[tauri::command(rename_all = "snake_case")]
pub fn zerar(app: AppHandle, estado: State<'_, Estado>, timer_id: i64) -> Result<Zerado, String> {
    let z = trava(&estado).zerar(timer_id, agora_ms())?;
    avisar(&app);
    Ok(z)
}

/// ↺ "Só <atividade>" (rodada 19): só aquela atividade do timer recomeça de 0:00 agora; as outras
/// seguem. Devolve o que o `desfazer_zerar` precisa.
#[tauri::command(rename_all = "snake_case")]
pub fn zerar_atividade(app: AppHandle, estado: State<'_, Estado>, timer_id: i64, atividade_id: i64) -> Result<Zerado, String> {
    let z = trava(&estado).zerar_atividade(timer_id, atividade_id, agora_ms())?;
    avisar(&app);
    Ok(z)
}

/// "Zerar todos" (barra do topo): todos os timers, os arquivados também, no mesmo instante.
#[tauri::command(rename_all = "snake_case")]
pub fn zerar_todos(app: AppHandle, estado: State<'_, Estado>, espaco_id: Option<i64>) -> Result<Zerado, String> {
    let z = trava(&estado).zerar_todos(espaco_id, agora_ms())?;
    avisar(&app);
    Ok(z)
}

/// `zerado`: o que o `zerar`, o `zerar_atividade` ou o `zerar_todos` devolveu.
#[tauri::command(rename_all = "snake_case")]
pub fn desfazer_zerar(app: AppHandle, estado: State<'_, Estado>, zerado: Zerado) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        n.desfazer_zerar(&zerado)?;
        Ok(vec![])
    })
}

/// "Descartar o tempo de hoje…" (no "Ajustar hoje"): pausa o timer e descarta o tempo de hoje
/// (trechos e ajustes à mão); devolve os ids de cada um para `desfazer_descartar`.
#[tauri::command(rename_all = "snake_case")]
pub fn descartar_hoje(app: AppHandle, estado: State<'_, Estado>, timer_id: i64) -> Result<Descartado, String> {
    let d = trava(&estado).descartar_hoje(timer_id, agora_ms())?;
    avisar(&app);
    Ok(d)
}

/// `trechos` e `ajustes`: os ids que o `descartar_hoje` devolveu.
#[tauri::command(rename_all = "snake_case")]
pub fn desfazer_descartar(
    app: AppHandle,
    estado: State<'_, Estado>,
    trechos: Vec<i64>,
    ajustes: Vec<i64>,
) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        n.desfazer_descartar(&trechos, &ajustes)?;
        Ok(vec![])
    })
}

// ---- ajuste à mão ----

/// "Ajustar hoje": soma (ou tira, com `delta_ms` negativo) do tempo de hoje do timer, na atividade
/// (sem ela, a padrão do timer), com motivo opcional. Devolve o id para `desfazer_ajuste`.
#[tauri::command(rename_all = "snake_case")]
pub fn ajustar_hoje(
    app: AppHandle,
    estado: State<'_, Estado>,
    timer_id: i64,
    atividade_id: Option<i64>,
    delta_ms: Ms,
    motivo: Option<String>,
) -> Result<i64, String> {
    let id = trava(&estado).ajustar_hoje(timer_id, atividade_id, delta_ms, motivo.as_deref(), agora_ms())?;
    avisar(&app);
    Ok(id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn desfazer_ajuste(app: AppHandle, estado: State<'_, Estado>, id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        n.desfazer_ajuste(id)?;
        Ok(vec![])
    })
}

// ---- grupos e timers ----

/// `espaco_id`: o espaço do grupo novo (rodada 20; sem ele, o primeiro).
#[tauri::command(rename_all = "snake_case")]
pub fn criar_grupo(app: AppHandle, estado: State<'_, Estado>, nome: String, cor: String, espaco_id: Option<i64>) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        let espaco = match espaco_id {
            Some(e) => e,
            None => n.banco.espaco_padrao().map_err(texto)?,
        };
        n.checar_espaco(espaco, true)?;
        let nome = nome_valido(&nome)?;
        n.checar_nome_de_grupo(nome, None)?;
        n.banco.criar_grupo_em(nome, &cor, espaco).map_err(texto)?;
        Ok(vec![])
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn editar_grupo(app: AppHandle, estado: State<'_, Estado>, grupo: Grupo) -> Result<Snapshot, String> {
    nome_valido(&grupo.nome)?;
    agir(&app, &estado, |n, agora| n.editar_grupo(&grupo, agora))
}

#[tauri::command(rename_all = "snake_case")]
pub fn criar_timer(
    app: AppHandle,
    estado: State<'_, Estado>,
    grupo_id: Option<i64>,
    nome: String,
    cor: String,
    espaco_id: Option<i64>,
) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| {
        // o timer novo começa zerado no início do dia de trabalho (a sessão ligada a ele "desde o
        // início" conta no número também o que veio antes da criação); sem grupo, no "Sem grupo"
        // do espaço dado (rodada 20)
        let fuso = n.fuso_min;
        let espaco = match espaco_id {
            Some(e) => e,
            None => n.banco.espaco_padrao().map_err(texto)?,
        };
        if grupo_id.is_none() {
            n.checar_espaco(espaco, true)?;
        }
        n.banco.criar_timer_no(grupo_id, espaco, nome_valido(&nome)?, &cor, agora, fuso).map_err(texto)?;
        Ok(vec![])
    })
}

/// O "Ajustar hoje" que o diálogo do timer vai gravar (para conferir antes).
#[derive(serde::Deserialize)]
pub struct AjustePedido {
    pub atividade_id: Option<i64>,
    pub delta_ms: Ms,
    pub motivo: Option<String>,
}

/// Confere o editar timer e o ajuste junto, sem gravar nada (revisão da rodada 21): uma recusa
/// não deixa nada gravado pela metade. `grupo_novo_espaco`: o espaço do "+ Novo grupo…" (o
/// timer vem no lugar de agora).
#[tauri::command(rename_all = "snake_case")]
pub fn checar_editar_timer(
    estado: State<'_, Estado>,
    timer: Timer,
    ocultas: Option<Vec<i64>>,
    ajuste: Option<AjustePedido>,
    grupo_novo_espaco: Option<i64>,
) -> Result<(), String> {
    nome_valido(&timer.nome)?;
    let n = trava(&estado);
    n.checar_banco()?;
    if let Some(e) = grupo_novo_espaco {
        n.checar_destino(e)?;
    }
    if let Some(a) = &ajuste {
        n.checar_ajuste(timer.id, a.atividade_id, a.delta_ms, a.motivo.as_deref())?;
    }
    n.checar_editar_timer(&timer, ocultas.as_deref())?;
    Ok(())
}

/// `ocultas`: as atividades que o painel do timer não mostra (rodada 20; sem elas, ficam as de
/// antes).
#[tauri::command(rename_all = "snake_case")]
pub fn editar_timer(app: AppHandle, estado: State<'_, Estado>, timer: Timer, ocultas: Option<Vec<i64>>) -> Result<Snapshot, String> {
    nome_valido(&timer.nome)?;
    agir(&app, &estado, |n, agora| n.editar_timer_com(&timer, agora, ocultas.as_deref()))
}

// ---- mover entre espaços (rodada 21) ----

/// "Mover para outro espaço…" do grupo. Devolve o que o `desfazer_mover` precisa.
#[tauri::command(rename_all = "snake_case")]
pub fn mover_grupo(app: AppHandle, estado: State<'_, Estado>, grupo_id: i64, espaco_id: i64) -> Result<Movido, String> {
    let m = trava(&estado).mover_grupo(grupo_id, espaco_id)?;
    avisar(&app);
    Ok(m)
}

/// "Mover para…" do timer: para um grupo (de qualquer espaço) ou para o "Sem grupo" de um espaço.
#[tauri::command(rename_all = "snake_case")]
pub fn mover_timer(
    app: AppHandle,
    estado: State<'_, Estado>,
    timer_id: i64,
    grupo_id: Option<i64>,
    espaco_id: i64,
) -> Result<Movido, String> {
    let m = trava(&estado).mover_timer(timer_id, grupo_id, espaco_id)?;
    avisar(&app);
    Ok(m)
}

/// O "Desfazer" do aviso do mover.
#[tauri::command(rename_all = "snake_case")]
pub fn desfazer_mover(app: AppHandle, estado: State<'_, Estado>, movido: Movido) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        n.desfazer_mover(&movido)?;
        Ok(vec![])
    })
}

// ---- espaços (rodada 20) ----

/// "+ Novo → Espaço" e Ajustes → Espaços: acrescenta um no fim da lista.
#[tauri::command(rename_all = "snake_case")]
pub fn criar_espaco(app: AppHandle, estado: State<'_, Estado>, nome: String, cor: String) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        n.criar_espaco(&nome, &cor)?;
        Ok(vec![])
    })
}

/// Ajustes → Espaços: renomear, cor, ordem, arquivar (nunca o último).
#[tauri::command(rename_all = "snake_case")]
pub fn editar_espaco(app: AppHandle, estado: State<'_, Estado>, espaco: nucleo::modelo::Espaco) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| n.editar_espaco(&espaco, agora))
}

/// Ajustes → Espaços: apaga um espaço sem grupo nem timer à vista; os arquivados dele vão para o
/// primeiro outro espaço. Com algo à vista, recusa.
#[tauri::command(rename_all = "snake_case")]
pub fn apagar_espaco(app: AppHandle, estado: State<'_, Estado>, id: i64) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        n.apagar_espaco(id)?;
        Ok(vec![])
    })
}

// ---- atividades ----

/// Ajustes → Atividades: acrescenta uma no fim da lista.
#[tauri::command(rename_all = "snake_case")]
pub fn criar_atividade(app: AppHandle, estado: State<'_, Estado>, nome: String, cor: String) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        n.criar_atividade(&nome, &cor)?;
        Ok(vec![])
    })
}

/// Ajustes → Atividades: renomear, cor, arquivar. `confirmar`: a pessoa aceitou arquivar a padrão
/// de algum timer (eles passam a usar a primeira que sobra). A última que sobra nunca arquiva.
#[tauri::command(rename_all = "snake_case")]
pub fn editar_atividade(
    app: AppHandle,
    estado: State<'_, Estado>,
    atividade: Atividade,
    confirmar: bool,
) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, _| {
        n.editar_atividade(&atividade, confirmar)?;
        Ok(vec![])
    })
}

// ---- sessões ----

/// Sessões dos últimos 7 dias, a mais recente primeiro.
#[tauri::command(rename_all = "snake_case")]
pub fn sessoes_recentes(estado: State<'_, Estado>) -> Result<Vec<SessaoDto>, String> {
    trava(&estado).sessoes_recentes(agora_ms())
}

#[tauri::command(rename_all = "snake_case")]
pub fn ligar_sessao(
    app: AppHandle,
    estado: State<'_, Estado>,
    sessao_id: String,
    timer_id: i64,
    desde_inicio: bool,
    criar_regra: bool,
    so_hoje: Option<bool>,
    pasta: Option<String>,
) -> Result<Snapshot, String> {
    let so_hoje = so_hoje.unwrap_or(false);
    // `pasta` (rodada 27): onde fica a regra do "sempre nesta pasta" (uma das pastas da sessão)
    agir(&app, &estado, |n, agora| n.ligar_sessao_com(&sessao_id, timer_id, desde_inicio, so_hoje, criar_regra, pasta.as_deref(), agora))
}

#[tauri::command(rename_all = "snake_case")]
pub fn desligar_sessao(app: AppHandle, estado: State<'_, Estado>, sessao_id: String) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| Ok(n.motor.desligar_sessao(&sessao_id, agora)))
}

// ---- perguntas ----

/// `valor`: "contar" | "retomar" | "pausado" | "nao" | "ligar:{id}" | "ligar:{id}:regra" |
/// "ligar:{id}:pasta:{caminho}" (rodada 27).
#[tauri::command(rename_all = "snake_case")]
pub fn responder(app: AppHandle, estado: State<'_, Estado>, pergunta_id: u64, valor: String) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| n.responder(pergunta_id, &valor, agora))
}

// ---- configuração ----

#[tauri::command(rename_all = "snake_case")]
pub fn salvar_config(app: AppHandle, estado: State<'_, Estado>, config: ConfigApp) -> Result<Snapshot, String> {
    agir(&app, &estado, |n, agora| {
        n.salvar_config(&config, &fila::caminhos::hook_json())?;
        // "Zerar sozinho na virada" ligado agora: conta a partir do corte de hoje
        n.zerar_na_virada(agora)?;
        Ok(vec![])
    })
}

/// O que a tela de Ajustes mostra além da config: versão (Sobre), onde fica o
/// `settings.json` do Claude Code e o corte do dia em `HH:MM`.
#[derive(Serialize)]
pub struct AjustesInfo {
    pub versao: String,
    pub settings: String,
    pub corte: String,
    /// A marca de autoria (rodada 18), no "Sobre".
    pub assinatura: &'static str,
    /// O canal da versão ("alpha", rodada 25): "Versão 0.4.0 · alpha".
    pub canal: &'static str,
}

#[tauri::command(rename_all = "snake_case")]
pub fn ajustes_info(app: AppHandle, estado: State<'_, Estado>) -> Result<AjustesInfo, String> {
    let cfg = trava(&estado).banco.config_app().map_err(texto)?;
    Ok(AjustesInfo {
        versao: app.package_info().version.to_string(),
        settings: caminho_settings().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        corte: nucleo::config::texto_de_corte(cfg.corte_min),
        assinatura: nucleo::ASSINATURA,
        canal: nucleo::CANAL,
    })
}

/// Corte do dia lido de `HH:MM`: os minutos e o texto arrumado ("3:00" → 180, "03:00").
#[derive(Serialize)]
pub struct Corte {
    pub min: u32,
    pub texto: String,
}

#[tauri::command(rename_all = "snake_case")]
pub fn ler_corte(texto: String) -> Result<Corte, String> {
    let min = nucleo::config::corte_de_texto(&texto).ok_or(ta("erro.corte"))?;
    Ok(Corte { min, texto: nucleo::config::texto_de_corte(min) })
}

// ---- regras de pasta ----

#[tauri::command(rename_all = "snake_case")]
pub fn regras_pasta(estado: State<'_, Estado>) -> Result<Vec<RegraDto>, String> {
    trava(&estado).regras()
}

/// Confere uma regra de pasta antes de gravar (rodada 15): a pasta onde ela fica, se é uma
/// pasta-mãe, se a regra já é de outro timer e se pode ficar ali.
#[tauri::command(rename_all = "snake_case")]
pub fn conferir_regra(
    estado: State<'_, Estado>,
    pasta: String,
    timer_id: Option<i64>,
) -> Result<crate::estado_app::ConferenciaRegra, String> {
    trava(&estado).conferir_regra(&pasta, timer_id)
}

/// Grava uma regra de pasta no timer (ou troca a pasta de uma dele, `trocar`) e devolve a lista
/// nova (rodada 15).
#[tauri::command(rename_all = "snake_case")]
pub fn salvar_regra(
    app: AppHandle,
    estado: State<'_, Estado>,
    pasta: String,
    timer_id: i64,
    trocar: Option<i64>,
) -> Result<Vec<RegraDto>, String> {
    let v = {
        let mut n = trava(&estado);
        n.salvar_regra(&pasta, timer_id, trocar)?;
        n.regras()?
    };
    avisar(&app);
    Ok(v)
}

/// O "Desfazer" de Tirar ou Apagar uma regra: põe de volta, a não ser que a pasta já seja de
/// outro timer (o erro diz de quem). Devolve a lista nova.
#[tauri::command(rename_all = "snake_case")]
pub fn restaurar_regra(app: AppHandle, estado: State<'_, Estado>, pasta: String, timer_id: i64) -> Result<Vec<RegraDto>, String> {
    let v = {
        let mut n = trava(&estado);
        n.restaurar_regra(&pasta, timer_id)?;
        n.regras()?
    };
    avisar(&app);
    Ok(v)
}

/// "Trocar por uma subpasta…" do ✎ da sessão: a regra passa a valer só na subpasta escolhida
/// (rodada 15). Devolve a lista nova.
#[tauri::command(rename_all = "snake_case")]
pub fn trocar_regra_por_subpasta(
    app: AppHandle,
    estado: State<'_, Estado>,
    regra_id: i64,
    pasta: String,
    sessao_id: String,
) -> Result<Vec<RegraDto>, String> {
    let v = {
        let mut n = trava(&estado);
        n.trocar_por_subpasta(regra_id, &pasta, &sessao_id)?;
        n.regras()?
    };
    avisar(&app);
    Ok(v)
}

/// O seletor de pasta do Windows, modal à janela principal: a pasta escolhida, ou nada se a
/// pessoa cancelou (rodada 15).
#[tauri::command(rename_all = "snake_case")]
pub async fn escolher_pasta(app: AppHandle, inicial: Option<String>, titulo: Option<String>) -> Result<Option<String>, String> {
    #[cfg(windows)]
    let dono = app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize);
    #[cfg(not(windows))]
    let dono = {
        let _ = &app;
        None
    };
    let titulo = titulo.unwrap_or_else(|| ta("janela.escolher_pasta").into());
    tauri::async_runtime::spawn_blocking(move || crate::escolher_pasta::escolher_pasta(dono, inicial, titulo))
        .await
        .map_err(texto)?
}

/// Remove a regra e devolve a lista nova.
#[tauri::command(rename_all = "snake_case")]
pub fn remover_regra_pasta(app: AppHandle, estado: State<'_, Estado>, id: i64) -> Result<Vec<RegraDto>, String> {
    let v = {
        let mut n = trava(&estado);
        n.remover_regra(id)?;
        n.regras()?
    };
    avisar(&app);
    Ok(v)
}

// ---- janelas e dock ----

#[tauri::command(rename_all = "snake_case")]
pub fn abrir_app(app: AppHandle) {
    janelas::mostrar_principal(&app);
}

/// Mostra a janela principal na tela de Ajustes (o ⚙ do dock).
#[tauri::command(rename_all = "snake_case")]
pub fn abrir_ajustes(app: AppHandle) {
    janelas::mostrar_ajustes(&app);
}

/// Resposta do "Fechar o Work Timer?": `acao` = "bandeja" (fecha a janela; a bandeja e o dock
/// continuam) ou "sair" (encerra o app). `lembrar` grava a escolha ("Não perguntar de novo").
/// Assíncrono: destruir a janela de dentro de um comando síncrono dela travaria o WebView2.
#[tauri::command(rename_all = "snake_case")]
pub async fn fechar_principal(app: AppHandle, acao: String, lembrar: bool) -> Result<(), String> {
    let escolha = janelas::AoFechar::de(&acao);
    if escolha == janelas::AoFechar::Perguntar {
        return Err(format!("escolha desconhecida ao fechar: {acao}"));
    }
    janelas::pedido_respondido();
    if lembrar {
        if let Some(estado) = app.try_state::<Estado>() {
            trava(&estado).guardar_ao_fechar(&acao)?;
        }
        avisar(&app);
    }
    janelas::fechar_com(&app, escolha);
    Ok(())
}

/// A página recebeu o X e vai perguntar ("Fechar o Work Timer?"): o Rust não fecha sozinho.
#[tauri::command(rename_all = "snake_case")]
pub fn fechar_visto() {
    janelas::pedido_respondido();
}

/// Resposta do "Minimizar para a bandeja?" (ou o minimizar com `ao_minimizar = "bandeja"`, depois
/// de a página gravar o pendente): `acao` = "bandeja" (fecha a janela, como o X) ou "barra"
/// (minimiza na barra de tarefas). `lembrar` grava a escolha. Assíncrono, como o fechar.
#[tauri::command(rename_all = "snake_case")]
pub async fn minimizar_principal(app: AppHandle, acao: String, lembrar: bool) -> Result<(), String> {
    let escolha = janelas::AoMinimizar::de(&acao);
    if !matches!(escolha, janelas::AoMinimizar::Bandeja | janelas::AoMinimizar::Barra) {
        return Err(format!("escolha desconhecida ao minimizar: {acao}"));
    }
    janelas::minimizar_respondido();
    if lembrar {
        if let Some(estado) = app.try_state::<Estado>() {
            trava(&estado).guardar_ao_minimizar(&acao)?;
        }
        avisar(&app);
    }
    janelas::minimizar_com(&app, escolha);
    Ok(())
}

/// A página recebeu o minimizar e vai perguntar: o Rust não minimiza sozinho.
#[tauri::command(rename_all = "snake_case")]
pub fn minimizar_visto() {
    janelas::minimizar_respondido();
}

/// "+ Novo timer…" da pergunta de sessão nova (dock): abre a janela principal com o "Novo timer"
/// já preenchido pela pasta da sessão; ao salvar, a página liga a sessão ao timer criado.
/// `pasta` e `sempre` (rodada 27): o "Sempre que for na pasta" da pergunta, com a pasta escolhida.
#[tauri::command(rename_all = "snake_case")]
pub fn pedir_novo_timer(
    app: AppHandle,
    estado: State<'_, Estado>,
    sessao_id: String,
    pasta: Option<String>,
    sempre: Option<bool>,
) -> Result<(), String> {
    let pedido = trava(&estado).pedido_novo_timer(&sessao_id, pasta.as_deref(), sempre)?;
    janelas::pedir_novo_timer(&app, pedido);
    Ok(())
}

/// O "Ligar a um timer" pedido pelo dock (o "Escolher timer…", o "Outro timer…", o "Trocar"): abre
/// na janela principal, nunca dentro do dock (rodada 16).
#[tauri::command(rename_all = "snake_case")]
pub fn pedir_ligar_sessao(app: AppHandle, pedido: janelas::PedidoLigar) {
    janelas::pedir_ligar_sessao(&app, pedido);
}

/// O "Ligar a um timer" esperando a página (vale uma vez).
#[tauri::command(rename_all = "snake_case")]
pub fn ligar_sessao_pedido() -> Option<janelas::PedidoLigar> {
    janelas::ligar_sessao_pedido()
}

/// O diálogo pedido pelo dock fechou: a janela principal volta a ficar como estava.
/// Assíncrono (re-revisão da rodada 16): quem chama é a própria janela principal, que pode ser
/// escondida ou destruída aqui; de dentro de um comando síncrono dela, travaria o WebView2 (como
/// no `fechar_principal`).
#[tauri::command(rename_all = "snake_case")]
pub async fn dialogo_do_dock_fechou(app: AppHandle) {
    janelas::dialogo_do_dock_fechou(&app);
}

/// O tour foi concluído, fechado ou recusado (rodada 16): não volta sozinho.
#[tauri::command(rename_all = "snake_case")]
pub fn marcar_tour_visto(app: AppHandle, estado: State<'_, Estado>) -> Result<(), String> {
    trava(&estado).guardar_dock(|c| c.tour_visto = true)?;
    avisar(&app);
    Ok(())
}

/// As dicas na memória: a que está à vista e se a janela principal está ocupada (revisão da
/// rodada 16: uma por vez no app inteiro).
pub type EstadoDicas = Mutex<nucleo::dicas::Dicas>;

fn dicas_trava(app: &AppHandle) -> std::sync::MutexGuard<'_, nucleo::dicas::Dicas> {
    app.state::<EstadoDicas>().inner().lock().unwrap_or_else(|p| p.into_inner())
}

/// Uma janela quer mostrar a dica `id`: "mostrar" (ela fica à vista até `dica_saiu`), "esperar"
/// (outra dica à vista, o tour ou um diálogo na janela principal) ou "nao" (desligadas ou vista).
#[tauri::command(rename_all = "snake_case")]
pub fn dica_reivindicar(
    app: AppHandle,
    janela: tauri::WebviewWindow,
    estado: State<'_, Estado>,
    id: String,
) -> Result<nucleo::dicas::Vez, String> {
    let cfg = trava(&estado).banco.config_app().map_err(texto)?;
    Ok(dicas_trava(&app).pedir(&cfg, &id, janela.label()))
}

/// A janela `label` sumiu (escondida, fechada ou recarregada): a dica dela sai sem contar como
/// vista e, sendo a principal, ela deixa de estar ocupada (re-revisão da rodada 16).
pub fn liberar_dicas_da_janela(app: &AppHandle, label: &str) {
    if let Some(e) = app.try_state::<EstadoDicas>() {
        e.inner().lock().unwrap_or_else(|p| p.into_inner()).janela_saiu(label, label == janelas::PRINCIPAL);
    }
}

/// A página (re)começou: o que ela mostrava, ou o ocupado dela, não vale mais. Ela informa o
/// estado de novo logo depois.
#[tauri::command(rename_all = "snake_case")]
pub fn dicas_pagina_comecou(app: AppHandle, janela: tauri::WebviewWindow) {
    liberar_dicas_da_janela(&app, janela.label());
}

/// A dica `id` saiu da tela: `vista` pela pessoa ou pelo prazo (não volta); fechada no meio, não.
#[tauri::command(rename_all = "snake_case")]
pub fn dica_saiu(
    app: AppHandle,
    janela: tauri::WebviewWindow,
    estado: State<'_, Estado>,
    id: String,
    vista: bool,
) -> Result<(), String> {
    let mut mudou = false;
    {
        let mut d = dicas_trava(&app);
        trava(&estado).guardar_dock(|c| mudou = d.saiu(c, &id, vista, janela.label()))?;
    }
    if mudou {
        avisar(&app);
    }
    Ok(())
}

/// A janela principal ficou ocupada (o tour, um diálogo) ou livre. Com uma dica à vista, ela
/// fecha (evento `dica-fechar`, para a janela que a mostra) e volta depois.
#[tauri::command(rename_all = "snake_case")]
pub fn principal_ocupada(app: AppHandle, ocupada: bool) {
    use tauri::Emitter;
    if let Some(id) = dicas_trava(&app).ocupar(ocupada) {
        let _ = app.emit("dica-fechar", id);
    }
}

/// "Dicas na tela" ligadas ou não ("Não mostrar mais dicas" desliga).
#[tauri::command(rename_all = "snake_case")]
pub fn dicas_ligar(app: AppHandle, estado: State<'_, Estado>, ligadas: bool) -> Result<(), String> {
    trava(&estado).guardar_dock(|c| c.dicas_ligadas = ligadas)?;
    avisar(&app);
    Ok(())
}

/// "Mostrar as dicas de novo": esquece as vistas (e liga as dicas).
#[tauri::command(rename_all = "snake_case")]
pub fn dicas_de_novo(app: AppHandle, estado: State<'_, Estado>) -> Result<(), String> {
    trava(&estado).guardar_dock(|c| {
        c.dicas_vistas.clear();
        c.dicas_ligadas = true;
    })?;
    avisar(&app);
    Ok(())
}

/// O passo do dock no tour: o dock pisca uma borda suave, sem pegar o foco (rodada 16).
#[tauri::command(rename_all = "snake_case")]
pub fn dock_piscar(app: AppHandle, ligado: bool) {
    use tauri::Emitter;
    let _ = app.emit_to(janelas::DOCK, "piscar", ligado);
}

/// O "+ Novo timer…" esperando a página (vale uma vez).
#[tauri::command(rename_all = "snake_case")]
pub fn novo_timer_pedido() -> Option<PedidoNovoTimer> {
    janelas::novo_timer_pedido()
}

/// A página abre nos Ajustes? (o ⚙ do dock pediu: "ajustes"; o "Sobre" da bandeja: "sobre";
/// nada: nulo; vale uma vez)
#[tauri::command(rename_all = "snake_case")]
pub fn ajustes_pedidos() -> Option<&'static str> {
    janelas::ajustes_pedidos()
}

/// O endereço de um link dos créditos (rodada 18): só os dois fixos, nunca um vindo da página.
pub fn link_dos_creditos(qual: &str) -> Option<&'static str> {
    match qual {
        "autor" => Some(nucleo::LINK_AUTOR),
        "codigo" => Some(nucleo::LINK_CODIGO),
        _ => None,
    }
}

/// Abre um link dos créditos ("autor" ou "codigo") no navegador padrão, fora do app (rodada 18).
#[tauri::command(rename_all = "snake_case")]
pub fn abrir_link(qual: String) -> Result<(), String> {
    let url = link_dos_creditos(&qual).ok_or(ta("erro.link"))?;
    // o Explorer entrega o https ao navegador padrão (como o "Abrir pasta" dos backups)
    let programa = if cfg!(windows) { "explorer" } else { "open" };
    std::process::Command::new(programa).arg(url).spawn().map_err(|e| tfa("erro.abrir_link", &[&e.to_string()]))?;
    Ok(())
}

/// Recolhe ou expande o dock (guarda a etapa). Devolve o lado da tela em que ele está.
#[tauri::command(rename_all = "snake_case")]
pub fn dock_etapa(app: AppHandle, estado: State<'_, Estado>, expandido: bool) -> Result<Lado, String> {
    let cfg = trava(&estado).guardar_dock(|c| c.dock_expandido = expandido)?;
    Ok(janelas::mudar_etapa(&app, &cfg))
}

/// Posição e tamanho do dock em pixels físicos (o tamanho só vale no expandido).
#[tauri::command(rename_all = "snake_case")]
pub fn dock_guardar(app: AppHandle, estado: State<'_, Estado>, x: i32, y: i32, larg: i32, alt: i32) -> Result<Lado, String> {
    trava(&estado).guardar_posicao_dock(x, y, larg, alt)?;
    Ok(janelas::lado_do_dock(&app))
}

// ---- avisos fora do dock (rodada 15) ----

/// O dock mostra um aviso (uma faixa, talvez com "Desfazer") na janela dos avisos, abaixo dele;
/// com o dock escondido, na janela principal. Devolve se ficou no dock.
#[tauri::command(rename_all = "snake_case")]
pub fn aviso_mostrar(app: AppHandle, aviso: janelas::AvisoDoDock) -> bool {
    janelas::mostrar_aviso(&app, aviso)
}

/// A página do aviso mediu o aviso (lógico): a janela aparece, do tamanho dele.
#[tauri::command(rename_all = "snake_case")]
pub fn aviso_tamanho(app: AppHandle, id: u64, larg: f64, alt: f64) {
    janelas::tamanho_do_aviso(&app, id, larg, alt);
}

/// O aviso venceu (ou o "Desfazer" deu certo): a janela dele some.
#[tauri::command(rename_all = "snake_case")]
pub fn aviso_fechar(app: AppHandle, id: u64) {
    janelas::fechar_aviso(&app, id);
}

/// Um botão do aviso foi clicado (na janela do aviso ou na principal): o dock faz o resto.
/// `qual`: 1 (o padrão) ou 2 (o segundo botão, o "Não mostrar mais dicas" de uma dica).
#[tauri::command(rename_all = "snake_case")]
pub fn aviso_clicado(app: AppHandle, id: u64, qual: Option<u8>) {
    janelas::aviso_clicado(&app, id, qual.unwrap_or(1));
}

/// O aviso a mostrar agora (a página do aviso pergunta ao abrir).
#[tauri::command(rename_all = "snake_case")]
pub fn aviso_atual() -> Option<janelas::AvisoDoDock> {
    janelas::aviso_atual()
}

/// O mouse entrou no dock ou saiu dele: o "Mostrar o dock agora" conta 30 s da saída.
#[tauri::command(rename_all = "snake_case")]
pub fn dock_mouse(app: AppHandle, dentro: bool) {
    janelas::dock_mouse(&app, dentro);
}

/// Tamanho do conteúdo do dock recolhido (lógico); a janela acompanha.
#[tauri::command(rename_all = "snake_case")]
pub fn dock_ajustar(app: AppHandle, estado: State<'_, Estado>, larg: f64, alt: f64) -> Result<Lado, String> {
    let cfg = trava(&estado).banco.config_app().map_err(texto)?;
    Ok(janelas::ajustar_recolhido(&app, &cfg, larg, alt))
}

// ---- hooks do Claude Code ----
// Só a UI chama estes comandos; nenhum caminho automático instala hooks.

/// `%USERPROFILE%\.claude\settings.json`.
/// O `settings.json` do Claude Code do perfil (rodada 17: no perfil de teste, o simulado; o ponto
/// único é `fila::caminhos::settings_claude`).
pub fn caminho_settings() -> Result<PathBuf, String> {
    Ok(fila::caminhos::settings_claude())
}

/// O hook que vem com o app: o `work-timer-hook.exe` ao lado do executável.
pub fn hook_instalado() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(texto)?;
    let pasta = exe.parent().ok_or("pasta do executável desconhecida")?;
    Ok(pasta.join(format!("{}{}", fila::caminhos::HOOK_NOME, std::env::consts::EXE_SUFFIX)))
}

/// Copia o hook da instalação para a pasta de dados (`.worktimer\bin`), se faltar ou mudou.
/// Os comandos do `settings.json` chamam essa cópia: ela fica fora do AppData, que o pacote
/// do app do Claude virtualiza.
pub fn copiar_hook() -> Result<bool, String> {
    // o perfil de teste nunca copia o exe do hook: o Claude Code dele é simulado (rodada 17;
    // revisão: o laço também passa por aqui)
    if !deve_copiar_hook(crate::perfil::atual()) {
        return Ok(false);
    }
    dados::atualizar_hook(&hook_instalado()?, &fila::caminhos::hook_exe())
}

/// O perfil copia o exe do hook? Só o normal.
pub fn deve_copiar_hook(p: crate::perfil::Perfil) -> bool {
    p == crate::perfil::Perfil::Normal
}

/// Comando do hook: a cópia em `%USERPROFILE%\.worktimer\bin`.
pub fn comando_hook() -> String {
    hooks_claude::comando_do_hook(&fila::caminhos::hook_exe())
}

/// Antes e depois do bloco `hooks`, sem gravar nada.
pub fn previa_em(settings: &Path, comando: &str) -> Result<String, String> {
    let antes: Value = match std::fs::read_to_string(settings) {
        Ok(t) if t.trim().is_empty() => json!({}),
        Ok(t) => serde_json::from_str(&t).map_err(|e| tfa("erro.json", &[&e.to_string()]))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(tfa("erro.ler", &[&e.to_string()])),
    };
    let depois = hooks_claude::mesclar(antes.clone(), comando);
    Ok(hooks_claude::previa(&antes, &depois))
}

/// Instala; devolve o caminho da cópia de segurança ("" se não havia arquivo).
pub fn instalar_em(settings: &Path, comando: &str) -> Result<String, String> {
    Ok(hooks_claude::instalar(settings, comando)?.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default())
}

/// "desligado", "antigo" (hooks nossos que não são os de agora: a pessoa decide se atualiza;
/// o app nunca regrava sozinho) ou "ligado".
#[tauri::command(rename_all = "snake_case")]
pub fn hooks_estado() -> Result<EstadoHooks, String> {
    Ok(hooks_claude::estado(&caminho_settings()?, &comando_hook()))
}

/// A conexão do Claude Code (rodada 18): o estado dos hooks e se o Claude Code existe neste PC
/// (a pasta `~\\.claude`; só olha se ela existe).
#[derive(Serialize)]
pub struct ConexaoClaude {
    pub estado: EstadoHooks,
    pub encontrado: bool,
}

#[tauri::command(rename_all = "snake_case")]
pub fn conexao_claude() -> Result<ConexaoClaude, String> {
    Ok(ConexaoClaude {
        estado: hooks_claude::estado(&caminho_settings()?, &comando_hook()),
        encontrado: fila::caminhos::pasta_claude().is_dir(),
    })
}

/// Os projetos recentes deste PC (rodada 18): as pastas em que o Claude Code trabalhou nos últimos
/// 30 dias, lidas de `~\\.claude\\projects` (só ler; vale também no perfil de teste), sem as que
/// nunca contam, as já ligadas por uma regra, a raiz de um drive e a pasta do usuário. Fica só em
/// memória, para o passo "Pastas dos projetos". Assíncrono: lê o disco fora da janela.
#[tauri::command(rename_all = "snake_case")]
pub async fn projetos_recentes(estado: State<'_, Estado>) -> Result<Vec<nucleo::recentes::ProjetoRecente>, String> {
    let (regras, ignoradas) = {
        let n = trava(&estado);
        (n.motor.regras_pasta.clone(), n.motor.cfg.pastas_ignoradas.clone())
    };
    let pasta = fila::caminhos::pasta_claude().join("projects");
    tauri::async_runtime::spawn_blocking(move || {
        let perfil = std::env::var("USERPROFILE").ok();
        let filtro = nucleo::recentes::Filtro { regras: &regras, ignoradas: &ignoradas, perfil: perfil.as_deref() };
        nucleo::recentes::projetos_recentes(
            &pasta,
            agora_ms(),
            nucleo::recentes::DIAS_RECENTES,
            nucleo::recentes::MAX_RECENTES,
            &filtro,
        )
    })
    .await
    .map_err(texto)
}

#[tauri::command(rename_all = "snake_case")]
pub fn hooks_previa() -> Result<String, String> {
    previa_em(&caminho_settings()?, &comando_hook())
}

/// Instala (ou atualiza) os hooks. Antes, garante a cópia do hook que eles vão chamar.
#[tauri::command(rename_all = "snake_case")]
pub fn hooks_instalar() -> Result<String, String> {
    // o perfil de teste não copia o exe do hook: o Claude Code dele é simulado (rodada 17)
    if !crate::perfil::e_teste() {
        if let Err(e) = copiar_hook() {
            if !fila::caminhos::hook_exe().is_file() {
                return Err(e);
            }
        }
    }
    instalar_em(&caminho_settings()?, &comando_hook())
}

// ---- backups ----

fn agora_local() -> chrono::NaiveDateTime {
    chrono::Local::now().naive_local()
}

/// Backups da pasta, o mais recente primeiro.
#[tauri::command(rename_all = "snake_case")]
pub fn backups_listar() -> Vec<Backup> {
    dados::listar(&fila::caminhos::backups())
}

/// "Fazer backup agora"; devolve a lista nova.
#[tauri::command(rename_all = "snake_case")]
pub fn backup_agora(estado: State<'_, Estado>) -> Result<Vec<Backup>, String> {
    let dir = fila::caminhos::backups();
    trava(&estado).backup_agora(&dir, agora_local())?;
    Ok(dados::listar(&dir))
}

/// "Restaurar" (a tela já pediu a confirmação): faz um backup do estado de agora e reabre o
/// banco a partir do escolhido, sem reiniciar o app.
#[tauri::command(rename_all = "snake_case")]
pub fn backup_restaurar(app: AppHandle, estado: State<'_, Estado>, nome: String) -> Result<Snapshot, String> {
    let snap = {
        let mut n = trava(&estado);
        let (fuso, dia) = fuso_local();
        let agora = agora_ms();
        n.restaurar_backup(&fila::caminhos::backups(), &nome, &fila::caminhos::banco(), agora_local(), agora, fuso, dia)?;
        n.gravar_hook_json(&fila::caminhos::hook_json())?;
        n.snapshot(agora)?
    };
    avisar(&app);
    Ok(snap)
}

/// "Abrir pasta": a pasta dos backups no Explorer.
#[tauri::command(rename_all = "snake_case")]
pub fn backups_abrir_pasta() -> Result<(), String> {
    let dir = fila::caminhos::backups();
    std::fs::create_dir_all(&dir).map_err(texto)?;
    let programa = if cfg!(windows) { "explorer" } else { "open" };
    std::process::Command::new(programa).arg(&dir).spawn().map_err(|e| tfa("erro.abrir_pasta", &[&e.to_string()]))?;
    Ok(())
}

// ---- iniciar com o Windows ----

/// O "Iniciar com o Windows" do perfil: lê (`ligar = None`) ou liga/desliga. No normal, o registro;
/// no de teste, só a config do perfil (rodada 17).
pub fn inicio_windows_do_perfil(app: &AppHandle, ligar: Option<bool>) -> Result<bool, String> {
    crate::perfil::aplicar_inicio(
        crate::perfil::atual(),
        ligar,
        |ligar| {
            use tauri_plugin_autostart::ManagerExt;
            let a = app.autolaunch();
            match ligar {
                Some(true) => a.enable().map_err(texto)?,
                Some(false) => a.disable().map_err(texto)?,
                None => {}
            }
            a.is_enabled().map_err(texto)
        },
        |ligar| {
            let estado = app.state::<Estado>();
            let n = trava(&estado);
            match ligar {
                Some(v) => Ok(n.guardar_dock(|c| c.iniciar_com_windows = v)?.iniciar_com_windows),
                None => Ok(n.banco.config_app().map_err(texto)?.iniciar_com_windows),
            }
        },
    )
}

/// O "Iniciar com o Windows" da página (Ajustes, primeiro uso): `ligar` nulo só lê.
#[tauri::command(rename_all = "snake_case")]
pub fn inicio_windows(app: AppHandle, ligar: Option<bool>) -> Result<bool, String> {
    inicio_windows_do_perfil(&app, ligar)
}

/// "Apagar o perfil de teste e recomeçar" (Ajustes → Ajuda, só no perfil de teste): fecha o app de
/// teste e, depois que ele sai, apaga a pasta do perfil. Fora dele, recusa. Só apaga uma pasta
/// que é a do perfil de teste (o nome confere), nunca a de uso.
#[tauri::command(rename_all = "snake_case")]
pub fn apagar_perfil_teste(app: AppHandle) -> Result<(), String> {
    if !crate::perfil::e_teste() {
        return Err(ta("erro.so_perfil_teste").into());
    }
    let pasta = fila::caminhos::base();
    if !pasta_do_perfil_teste(&pasta) {
        return Err(ta("erro.nao_e_perfil_teste").into());
    }
    let mut pastas = vec![pasta];
    // a pasta do WebView2 do perfil (o `localStorage` da página), pelo identificador do perfil
    // (revisão): só se ela for mesmo a do teste, dentro do %LOCALAPPDATA%
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        let webview = local.join(&app.config().identifier);
        if pasta_do_webview_do_teste(&webview, &local) {
            pastas.push(webview);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const SEM_JANELA: u32 = 0x0800_0000;
        // um processo destacado espera este app terminar (o banco e o WebView2 abertos não
        // deixam apagar) e apaga as pastas
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &linha_de_apagar(std::process::id(), &pastas)])
            .creation_flags(SEM_JANELA)
            .spawn()
            .map_err(|e| tfa("erro.apagar_perfil", &[&e.to_string()]))?;
    }
    app.exit(0);
    Ok(())
}

/// O PowerShell que espera o processo `pid` terminar e apaga as `pastas` (duas tentativas: os
/// processos do WebView2 saem logo depois do app).
pub fn linha_de_apagar(pid: u32, pastas: &[PathBuf]) -> String {
    let lista = pastas.iter().map(|p| format!("'{}'", p.display())).collect::<Vec<_>>().join(",");
    format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; foreach ($v in 1..3) {{ Start-Sleep -Seconds 2; \
         Remove-Item -LiteralPath {lista} -Recurse -Force -ErrorAction SilentlyContinue }}"
    )
}

/// A pasta do WebView2 é a do perfil de teste? Termina em `.perfil-teste`, fica direto dentro do
/// `%LOCALAPPDATA%` e não tem nada que quebre a linha do PowerShell. Nunca a do perfil normal.
pub fn pasta_do_webview_do_teste(pasta: &Path, localappdata: &Path) -> bool {
    let texto = pasta.to_string_lossy();
    pasta.parent() == Some(localappdata)
        && pasta.file_name().is_some_and(|n| n.to_string_lossy().ends_with(".perfil-teste"))
        && !texto.contains(['"', '\'', '&', '%', '$', '`'])
}

/// A pasta é a do perfil de teste? (o último nome é `.worktimer-perfil-teste`, sem aspas nem `&`)
pub fn pasta_do_perfil_teste(pasta: &Path) -> bool {
    let texto = pasta.to_string_lossy();
    pasta.file_name().is_some_and(|n| n == fila::caminhos::PASTA_PERFIL_TESTE)
        && !texto.contains(['"', '\'', '&', '%', '$', '`'])
}

#[tauri::command(rename_all = "snake_case")]
pub fn hooks_remover(estado: State<'_, Estado>) -> Result<Snapshot, String> {
    hooks_claude::desinstalar(&caminho_settings()?)?;
    trava(&estado).snapshot(agora_ms())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn os_links_dos_creditos_sao_so_os_dois_fixos() {
        assert_eq!(link_dos_creditos("autor"), Some("https://eaeniko.okin.club"));
        assert_eq!(link_dos_creditos("codigo"), Some("https://github.com/eaeniko/chronn"));
        assert_eq!(link_dos_creditos("https://exemplo.com"), None, "nunca um endereço vindo da página");
        assert_eq!(link_dos_creditos(""), None);
    }

    #[test]
    fn a_assinatura_vai_no_sobre() {
        assert_eq!(nucleo::ASSINATURA, "chronn-eaeniko-7f3a9c");
        let info = AjustesInfo {
            versao: "0.4.0".into(),
            settings: String::new(),
            corte: "00:00".into(),
            assinatura: nucleo::ASSINATURA,
            canal: nucleo::CANAL,
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["assinatura"], "chronn-eaeniko-7f3a9c");
        // rodada 25: o canal vem da constante única
        assert_eq!(v["canal"], "alpha");
    }

    #[test]
    fn o_perfil_de_teste_nao_copia_o_hook() {
        assert!(deve_copiar_hook(crate::perfil::Perfil::Normal));
        assert!(!deve_copiar_hook(crate::perfil::Perfil::Teste));
        // o laço e a abertura passam pelo `copiar_hook`, que confere o perfil
        let laco = include_str!("laco.rs");
        assert!(laco.contains("crate::comandos::copiar_hook()"));
    }

    #[test]
    fn apagar_o_perfil_leva_a_pasta_do_webview_do_teste_e_nunca_a_do_normal() {
        let local = Path::new("C:/Users/usuario/AppData/Local");
        let teste = local.join("io.github.eaeniko.worktimer.perfil-teste");
        assert!(pasta_do_webview_do_teste(&teste, local));
        assert!(!pasta_do_webview_do_teste(&local.join("io.github.eaeniko.worktimer"), local), "nunca a do normal");
        assert!(!pasta_do_webview_do_teste(&local.join("x").join("a.perfil-teste"), local), "só direto no LOCALAPPDATA");
        assert!(!pasta_do_webview_do_teste(Path::new("D:/a.perfil-teste"), local));
        assert!(!pasta_do_webview_do_teste(&local.join("o'x.perfil-teste"), local), "nada que quebre a linha");
        let l = linha_de_apagar(4242, &[PathBuf::from("C:/u/.worktimer-perfil-teste"), teste.clone()]);
        assert!(l.starts_with("Wait-Process -Id 4242 "), "espera o app sair: {l}");
        assert!(l.contains(&format!("Remove-Item -LiteralPath 'C:/u/.worktimer-perfil-teste','{}' -Recurse -Force", teste.display())), "{l}");
    }

    #[test]
    fn so_apaga_a_pasta_do_perfil_de_teste() {
        let u = Path::new("C:/Users/usuario");
        assert!(pasta_do_perfil_teste(&u.join(".worktimer-perfil-teste")));
        assert!(!pasta_do_perfil_teste(&u.join(".worktimer")), "nunca a de uso");
        assert!(!pasta_do_perfil_teste(&u.join(".claude")));
        assert!(!pasta_do_perfil_teste(u));
        assert!(!pasta_do_perfil_teste(Path::new("C:/a&b/.worktimer-perfil-teste")), "nada que quebre o cmd");
        assert!(!pasta_do_perfil_teste(Path::new("C:/%x%/.worktimer-perfil-teste")));
        // fora do perfil de teste, o comando nem existe (os testes rodam no normal)
        assert!(!crate::perfil::e_teste());
    }

    #[test]
    fn ler_corte_arruma_e_recusa() {
        let c = ler_corte("3:00".into()).unwrap();
        assert_eq!((c.min, c.texto.as_str()), (180, "03:00"));
        assert_eq!(ler_corte("25:00".into()).err().unwrap(), "Use HH:MM, de 00:00 a 23:59");
    }

    /// Único teste que mexe em USERPROFILE (e na pasta de dados): aponta para uma pasta
    /// temporária (nunca o perfil real) e passa pelo mesmo caminho dos comandos.
    #[test]
    fn hooks_no_perfil_temporario() {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("USERPROFILE", d.path());
        std::env::remove_var(fila::caminhos::VAR_BASE);
        let s = caminho_settings().unwrap();
        assert_eq!(s, d.path().join(".claude").join("settings.json"));
        assert_eq!(fila::caminhos::base(), d.path().join(".worktimer"));
        let cmd = comando_hook();
        assert_eq!(hooks_claude::estado(&s, &cmd), EstadoHooks::Desligado);
        let esperado = format!("\"{}/.worktimer/bin/work-timer-hook.exe\"", d.path().to_string_lossy().replace('\\', "/"));
        assert_eq!(cmd, esperado);
        let p = previa_em(&s, &cmd).unwrap();
        assert!(p.starts_with("Antes:\n(sem hooks)"), "{p}");
        assert!(p.contains("work-timer-hook.exe"));
        assert!(!s.exists(), "prévia não grava");
        assert_eq!(instalar_em(&s, &cmd).unwrap(), "", "sem arquivo antes, sem cópia");
        assert_eq!(hooks_claude::estado(&s, &cmd), EstadoHooks::Ligado);
        let copia = instalar_em(&s, &cmd).unwrap();
        assert!(Path::new(&copia).exists());
        hooks_claude::desinstalar(&s).unwrap();
        assert_eq!(hooks_claude::estado(&s, &cmd), EstadoHooks::Desligado);
        std::fs::write(&s, "{ quebrado").unwrap();
        assert!(previa_em(&s, &cmd).unwrap_err().contains("JSON inválido"));
        // o hook da instalação (ao lado do executável de teste) vai para .worktimer\bin
        let instalado = hook_instalado().unwrap();
        assert_eq!(instalado.file_name().unwrap(), format!("work-timer-hook{}", std::env::consts::EXE_SUFFIX).as_str());
        if instalado.is_file() {
            assert!(copiar_hook().unwrap());
            assert!(fila::caminhos::hook_exe().is_file());
            assert!(!copiar_hook().unwrap(), "igual: não copia de novo");
        }
    }
}
