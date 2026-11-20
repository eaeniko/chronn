// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Janelas: a principal (título, mostrar, fechar e recriar) e o dock (posição, tamanho e etapa).
//! As contas de posição são funções puras (pixels físicos), testadas sem Tauri.
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use nucleo::config::{
    ConfigApp, AO_FECHAR_BANDEJA, AO_FECHAR_SAIR, AO_MINIMIZAR_BANDEJA, AO_MINIMIZAR_BARRA, AO_MINIMIZAR_DOCK, DOCK_SO_PERGUNTAS,
};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WebviewWindowBuilder, Window,
};

use crate::estado_app::{trava, Estado, PedidoNovoTimer};
use crate::bandeja;

pub const PRINCIPAL: &str = "main";
/// Evento para a janela principal: o X foi clicado (`PedidoFechar`).
pub const EVENTO_PEDIR_FECHAR: &str = "pedir-fechar";
/// Evento para a janela principal: a janela foi minimizada (`PedidoFechar`, com a ação do minimizar).
pub const EVENTO_PEDIR_MINIMIZAR: &str = "pedir-minimizar";
/// Evento para a janela principal: abrir nos Ajustes (o ⚙ do dock).
pub const EVENTO_ABRIR_AJUSTES: &str = "abrir-ajustes";
/// Evento para a janela principal: há um "+ Novo timer…" esperando (`novo_timer_pedido`).
pub const EVENTO_NOVO_TIMER: &str = "novo-timer";
/// Evento para a janela principal: o dock pediu o "Ligar a um timer" de uma sessão (rodada 16).
pub const EVENTO_LIGAR_SESSAO: &str = "ligar-sessao";
/// Quanto o X espera a página responder antes de fechar sozinho.
pub const ESPERA_DA_PAGINA: Duration = Duration::from_secs(2);

/// O que o X da janela principal faz (`ConfigApp::ao_fechar`; valor desconhecido = perguntar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AoFechar {
    Perguntar,
    /// A janela é destruída (o WebView sai da memória); a bandeja e o dock continuam.
    Bandeja,
    Sair,
}

impl AoFechar {
    pub fn de(valor: &str) -> AoFechar {
        match valor {
            AO_FECHAR_BANDEJA => AoFechar::Bandeja,
            AO_FECHAR_SAIR => AoFechar::Sair,
            _ => AoFechar::Perguntar,
        }
    }

    pub fn valor(self) -> &'static str {
        match self {
            AoFechar::Perguntar => nucleo::config::AO_FECHAR_PERGUNTAR,
            AoFechar::Bandeja => AO_FECHAR_BANDEJA,
            AoFechar::Sair => AO_FECHAR_SAIR,
        }
    }

    /// A página não respondeu ao X (ainda carregando ou quebrada): "sair" sai; o resto vai
    /// para a bandeja, que segue contando.
    pub fn sem_resposta(self) -> AoFechar {
        match self {
            AoFechar::Sair => AoFechar::Sair,
            _ => AoFechar::Bandeja,
        }
    }
}

/// O que o minimizar da janela principal faz (`ConfigApp::ao_minimizar`; valor desconhecido =
/// perguntar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AoMinimizar {
    Perguntar,
    /// Como o X com "Manter na bandeja": a janela é destruída; a bandeja e o dock continuam.
    Bandeja,
    /// Minimiza e mostra o dock.
    Dock,
    /// Minimiza na barra de tarefas, e só.
    Barra,
}

impl AoMinimizar {
    pub fn de(valor: &str) -> AoMinimizar {
        match valor {
            AO_MINIMIZAR_BANDEJA => AoMinimizar::Bandeja,
            AO_MINIMIZAR_DOCK => AoMinimizar::Dock,
            AO_MINIMIZAR_BARRA => AoMinimizar::Barra,
            _ => AoMinimizar::Perguntar,
        }
    }

    pub fn valor(self) -> &'static str {
        match self {
            AoMinimizar::Perguntar => nucleo::config::AO_MINIMIZAR_PERGUNTAR,
            AoMinimizar::Bandeja => AO_MINIMIZAR_BANDEJA,
            AoMinimizar::Dock => AO_MINIMIZAR_DOCK,
            AoMinimizar::Barra => AO_MINIMIZAR_BARRA,
        }
    }

    /// A página não respondeu: "bandeja" fecha sem ela; "perguntar" minimiza na barra.
    pub fn sem_resposta(self) -> AoMinimizar {
        match self {
            AoMinimizar::Bandeja => AoMinimizar::Bandeja,
            _ => AoMinimizar::Barra,
        }
    }
}

// ---- X e minimizar da janela principal ----
// Nenhum dos dois age direto: a página grava o que estiver pendente (um campo dos Ajustes ainda
// sem `change`) e responde com `fechar_principal` / `minimizar_principal` (ou com
// `fechar_visto` / `minimizar_visto`, quando vai perguntar). Sem resposta em
// `ESPERA_DA_PAGINA`, o Rust faz sozinho (`sem_resposta`).

/// O que vai para a página nos eventos `pedir-fechar` e `pedir-minimizar`.
#[derive(Clone, Debug, Serialize)]
pub struct PedidoFechar {
    pub id: u64,
    /// "perguntar", "bandeja", "sair" (fechar) ou "barra" (minimizar).
    pub acao: &'static str,
}

/// Um pedido esperando a página (0 = nenhum).
pub struct Pedido(AtomicU64);

static PROXIMO_PEDIDO: AtomicU64 = AtomicU64::new(1);

impl Pedido {
    const fn novo() -> Pedido {
        Pedido(AtomicU64::new(0))
    }

    /// Abre um pedido e devolve o id dele.
    pub fn abrir(&self) -> u64 {
        let id = PROXIMO_PEDIDO.fetch_add(1, Ordering::SeqCst);
        self.0.store(id, Ordering::SeqCst);
        id
    }

    /// A página respondeu (ou vai perguntar).
    pub fn responder(&self) {
        self.0.store(0, Ordering::SeqCst);
    }

    /// O prazo do pedido `id` venceu sem resposta? (e fecha o pedido)
    pub fn vencer(&self, id: u64) -> bool {
        self.0.compare_exchange(id, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }

    pub fn aberto(&self) -> bool {
        self.0.load(Ordering::SeqCst) != 0
    }
}

static PEDIDO_FECHAR: Pedido = Pedido::novo();
static PEDIDO_MINIMIZAR: Pedido = Pedido::novo();
/// O próprio app minimizou a janela (a resposta "Barra de tarefas") neste instante (ms desde
/// 1970; 0 = nunca): os avisos de minimizado que chegarem logo depois não perguntam de novo.
static MINIMIZOU_SOZINHO_EM: AtomicU64 = AtomicU64::new(0);
/// Quanto tempo depois do próprio minimizar os avisos são ignorados.
pub const FOLGA_DO_MINIMIZAR: Duration = Duration::from_secs(2);

fn agora_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Um aviso de minimizado em `agora` (ms) é do minimizar que o próprio app fez em `feito`?
pub fn minimizado_pelo_app(feito: u64, agora: u64) -> bool {
    feito != 0 && agora >= feito && agora - feito < FOLGA_DO_MINIMIZAR.as_millis() as u64
}

/// Emite o pedido e, se a página não responder a tempo, faz `sem_resposta` sozinho.
fn pedir_a_pagina(app: &AppHandle, pedido: &'static Pedido, evento: &'static str, acao: &'static str, sem_resposta: impl FnOnce(&AppHandle) + Send + 'static) {
    let id = pedido.abrir();
    let _ = app.emit_to(PRINCIPAL, evento, PedidoFechar { id, acao });
    let app = app.clone();
    let _ = std::thread::Builder::new().name("worktimer-pedido".into()).spawn(move || {
        std::thread::sleep(ESPERA_DA_PAGINA);
        if pedido.vencer(id) {
            log_debug!("worktimer: a página não respondeu a {evento}; seguindo sem ela");
            sem_resposta(&app);
        }
    });
}

/// O X da janela principal, com a config `ao_fechar` já lida.
pub fn pedir_fechar(app: &AppHandle, janela: &Window, modo: AoFechar) {
    if modo == AoFechar::Perguntar {
        // fechar pela barra de tarefas com a janela minimizada (ou atrás de outras): o
        // diálogo precisa aparecer
        let _ = janela.unminimize();
        let _ = janela.show();
        let _ = janela.set_focus();
    }
    pedir_a_pagina(app, &PEDIDO_FECHAR, EVENTO_PEDIR_FECHAR, modo.valor(), move |app| {
        // sem a página: "sair" sai; o resto vai para a bandeja
        fechar_com(app, modo.sem_resposta());
    });
}

/// A página respondeu ao X (ou vai perguntar ao usuário): o Rust não fecha sozinho.
pub fn pedido_respondido() {
    PEDIDO_FECHAR.responder();
}

/// A janela principal foi minimizada (`janela.is_minimized()`), com a config `ao_minimizar` já
/// lida. "Dock" mostra o dock e "barra" deixa como está; "bandeja" e "perguntar" passam pela
/// página (com "perguntar", a janela volta para o diálogo aparecer).
pub fn ao_minimizar(app: &AppHandle, janela: &Window, modo: AoMinimizar) {
    // o minimizar que o próprio app pediu, ou um pedido ainda esperando a página
    if minimizado_pelo_app(MINIMIZOU_SOZINHO_EM.load(Ordering::SeqCst), agora_ms()) || PEDIDO_MINIMIZAR.aberto() {
        return;
    }
    match modo {
        AoMinimizar::Dock => mostrar_dock(app),
        AoMinimizar::Barra => {}
        AoMinimizar::Perguntar | AoMinimizar::Bandeja => {
            if modo == AoMinimizar::Perguntar {
                let _ = janela.unminimize();
                let _ = janela.show();
                let _ = janela.set_focus();
            }
            pedir_a_pagina(app, &PEDIDO_MINIMIZAR, EVENTO_PEDIR_MINIMIZAR, modo.valor(), move |app| {
                minimizar_com(app, modo.sem_resposta())
            });
        }
    }
}

/// A página respondeu ao minimizar (ou vai perguntar): o Rust não age sozinho.
pub fn minimizar_respondido() {
    PEDIDO_MINIMIZAR.responder();
}

/// "Bandeja": destrói a janela principal (como o X). "Barra": minimiza sem perguntar de novo.
/// "Dock": minimiza e mostra o dock (com o dock "Só quando houver uma pergunta", só minimiza:
/// `mostrar_dock`).
pub fn minimizar_com(app: &AppHandle, modo: AoMinimizar) {
    match modo {
        AoMinimizar::Bandeja => fechar_principal(app),
        AoMinimizar::Barra | AoMinimizar::Dock => {
            if let Some(j) = app.get_webview_window(PRINCIPAL) {
                MINIMIZOU_SOZINHO_EM.store(agora_ms(), Ordering::SeqCst);
                let _ = j.minimize();
            }
            if modo == AoMinimizar::Dock {
                mostrar_dock(app);
            }
        }
        AoMinimizar::Perguntar => {}
    }
}

/// "Bandeja": destrói a janela principal. "Sair": encerra o app. "Perguntar": nada.
pub fn fechar_com(app: &AppHandle, modo: AoFechar) {
    match modo {
        AoFechar::Sair => sair(app),
        AoFechar::Bandeja => fechar_principal(app),
        AoFechar::Perguntar => {}
    }
}

/// Encerra o app (o "Sair" da bandeja e o "sair" do X).
pub fn sair(app: &AppHandle) {
    app.exit(0);
}
/// Argumento que o "Iniciar com o Windows" põe na linha de comando: abre só o dock e a bandeja
/// (com o dock "Só quando houver uma pergunta", só a bandeja).
pub const ARG_BANDEJA: &str = "--bandeja";

/// O app foi aberto pelo Windows (com `--bandeja`)? Então a janela principal não é criada.
pub fn so_bandeja(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter().skip(1).any(|a| a == ARG_BANDEJA)
}

/// A janela principal é criada (e mostrada) na partida? Sempre, menos quando o Windows abriu
/// o app (`--bandeja`); mas o primeiro uso (nenhum timer ainda) precisa dela mesmo assim.
pub fn principal_na_partida(so_bandeja: bool, primeiro_uso: bool) -> bool {
    !so_bandeja || primeiro_uso
}
pub const DOCK: &str = "dock";
/// Dock expandido: tamanho padrão e mínimo (lógicos).
pub const EXPANDIDO_PADRAO: (f64, f64) = (320.0, 420.0);
pub const EXPANDIDO_MIN: (f64, f64) = (280.0, 240.0);
/// Dock recolhido antes de a página medir o conteúdo (lógico).
pub const RECOLHIDO_INICIAL: (f64, f64) = (240.0, 46.0);
/// Distância da borda da tela quando não há posição salva (lógica).
pub const MARGEM: f64 = 16.0;
/// Quanto do dock precisa aparecer num monitor para a posição salva valer (físico).
const VISIVEL_MIN: i32 = 32;

/// Retângulo em pixels físicos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ret {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Lado da tela em que o dock está: o conteúdo se alinha a ele e o tamanho cresce para longe dele.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Lado {
    pub direita: bool,
    pub embaixo: bool,
}

fn limitar(v: i32, min: i32, max: i32) -> i32 {
    if max < min {
        min
    } else {
        v.clamp(min, max)
    }
}

/// Posição de uma janela `w`×`h` em (x, y), empurrada para dentro da área.
pub fn dentro(x: i32, y: i32, w: i32, h: i32, area: Ret) -> (i32, i32) {
    (limitar(x, area.x, area.x + area.w - w), limitar(y, area.y, area.y + area.h - h))
}

/// Pelo centro da janela: está na metade direita? na de baixo?
pub fn lado(r: Ret, area: Ret) -> Lado {
    Lado { direita: 2 * r.x + r.w > 2 * area.x + area.w, embaixo: 2 * r.y + r.h > 2 * area.y + area.h }
}

/// Nova posição ao trocar o tamanho para `w`×`h`: segura a borda do lado em que o dock está
/// (direita e/ou de baixo) e não deixa sair da área.
pub fn ancorar(r: Ret, w: i32, h: i32, area: Ret) -> (i32, i32) {
    let l = lado(r, area);
    let x = if l.direita { r.x + r.w - w } else { r.x };
    let y = if l.embaixo { r.y + r.h - h } else { r.y };
    dentro(x, y, w, h, area)
}

fn intersecao(a: Ret, b: Ret) -> (i32, i32) {
    let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    (w.max(0), h.max(0))
}

/// A área (monitor) que mais contém a janela; nenhuma se ela aparece menos de 32×32 px.
pub fn area_de(r: Ret, areas: &[Ret]) -> Option<Ret> {
    areas
        .iter()
        .map(|a| (*a, intersecao(r, *a)))
        .filter(|(_, (w, h))| *w >= VISIVEL_MIN.min(r.w) && *h >= VISIVEL_MIN.min(r.h))
        .max_by_key(|(_, (w, h))| w * h)
        .map(|(a, _)| a)
}

/// Posição ao abrir: a salva, se o dock ainda aparece num monitor (empurrada para dentro dele);
/// senão o canto de cima à direita da área principal, com margem.
pub fn posicao_inicial(salva: Option<(i32, i32)>, w: i32, h: i32, areas: &[Ret], principal: Ret, margem: i32) -> (i32, i32) {
    if let Some((x, y)) = salva {
        if let Some(a) = area_de(Ret { x, y, w, h }, areas) {
            return dentro(x, y, w, h, a);
        }
    }
    dentro(principal.x + principal.w - w - margem, principal.y + margem, w, h, principal)
}

/// Título da janela do dock (não aparece na tela; o Windows mostra em listas de janelas).
pub fn titulo_dock() -> String {
    format!("{} — dock", crate::perfil::nome_na_tela())
}

/// "● N rodando · Chronn", ou só "Chronn" (o nome de exibição, `APP_NOME`).
pub fn titulo_principal(rodando: usize) -> String {
    let nome = crate::perfil::nome_na_tela();
    if rodando == 0 {
        nome
    } else {
        format!("● {rodando} rodando · {nome}")
    }
}

// ---- com Tauri ----

/// O primeiro ajuste do recolhido depois de restaurar uma posição salva não ancora:
/// o canto salvo é o de cima à esquerda do dock como ele estava.
static PRIMEIRO_SEM_ANCORA: AtomicBool = AtomicBool::new(false);

fn ret_de(j: &WebviewWindow) -> Option<Ret> {
    let p = j.outer_position().ok()?;
    let s = j.outer_size().ok()?;
    Some(Ret { x: p.x, y: p.y, w: s.width as i32, h: s.height as i32 })
}

fn area_monitor(m: &tauri::Monitor) -> Ret {
    let a = m.work_area();
    Ret { x: a.position.x, y: a.position.y, w: a.size.width as i32, h: a.size.height as i32 }
}

fn areas(app: &AppHandle) -> Vec<Ret> {
    app.available_monitors().map(|v| v.iter().map(area_monitor).collect()).unwrap_or_default()
}

/// Área de trabalho e escala do monitor principal (o primeiro, se não houver principal).
fn principal(app: &AppHandle) -> Option<(Ret, f64)> {
    let m = app.primary_monitor().ok().flatten().or_else(|| app.available_monitors().ok()?.into_iter().next())?;
    Some((area_monitor(&m), m.scale_factor()))
}

/// Área onde a janela está (ou a principal).
fn area_da_janela(app: &AppHandle, r: Ret) -> Option<Ret> {
    area_de(r, &areas(app)).or_else(|| principal(app).map(|(a, _)| a))
}

pub fn dock(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(DOCK)
}

/// Mostra a janela principal; se ela foi fechada para a bandeja, recria. Foi a pessoa: a janela
/// não se esconde de novo depois de um diálogo do dock.
pub fn mostrar_principal(app: &AppHandle) {
    serie_do_dock().zerar();
    abrir_principal(app, None);
}

/// Mostra a janela principal nos Ajustes (recria se preciso).
pub fn mostrar_ajustes(app: &AppHandle) {
    serie_do_dock().zerar();
    abrir_principal(app, Some(PEDIDO_AJUSTES));
}

/// O "Sobre o …" da bandeja (rodada 18): a janela principal em Ajustes → Sobre.
pub fn mostrar_sobre(app: &AppHandle) {
    serie_do_dock().zerar();
    abrir_principal(app, Some(PEDIDO_SOBRE));
}

/// Uma recriação por vez (dois cliques rápidos na bandeja não criam duas).
static RECRIANDO: AtomicBool = AtomicBool::new(false);
/// O ⚙ do dock pediu os Ajustes e a página ainda não buscou (`ajustes_pedidos`). Guardado
/// aqui, não num script da página: vale também para o ⚙ clicado enquanto a janela nasce, e
/// um recarregamento não volta para os Ajustes.
/// 0: nada; `PEDIDO_AJUSTES`: os Ajustes; `PEDIDO_SOBRE`: Ajustes → Sobre (rodada 18).
static AJUSTES_PEDIDOS: AtomicU8 = AtomicU8::new(0);
const PEDIDO_AJUSTES: u8 = 1;
const PEDIDO_SOBRE: u8 = 2;

/// A página pergunta (ao abrir e ao receber `abrir-ajustes`) se deve ir para os Ajustes:
/// "ajustes", "sobre" (a seção) ou nada.
pub fn ajustes_pedidos() -> Option<&'static str> {
    match AJUSTES_PEDIDOS.swap(0, Ordering::SeqCst) {
        PEDIDO_AJUSTES => Some("ajustes"),
        PEDIDO_SOBRE => Some("sobre"),
        _ => None,
    }
}

/// O "+ Novo timer…" da pergunta do dock, esperando a página buscar (como o ⚙): vale também se a
/// janela principal ainda vai nascer. Um pedido novo troca o anterior.
static NOVO_TIMER: Mutex<Option<PedidoNovoTimer>> = Mutex::new(None);

/// Guarda o pedido, mostra (ou recria) a janela principal e avisa a página.
pub fn pedir_novo_timer(app: &AppHandle, pedido: PedidoNovoTimer) {
    *NOVO_TIMER.lock().unwrap_or_else(|p| p.into_inner()) = Some(pedido);
    lembrar_antes_do_dialogo(app);
    abrir_principal(app, None);
    let _ = app.emit_to(PRINCIPAL, EVENTO_NOVO_TIMER, ());
}

// ---- diálogos do dock na janela principal (rodada 16) ----
// Nenhum diálogo abre dentro do dock: o "Escolher timer…", o "Outro timer…", o "Trocar" e o
// "+ Novo timer…" abrem na janela principal, que aparece na frente com o foco. Se ela estava
// escondida (ou fechada para a bandeja), volta a ficar assim quando o diálogo fecha.

/// O "Ligar a um timer" pedido pelo dock: a sessão, o que mostrar se ela não estiver na lista
/// das recentes, e se a lista oferece o "+ Novo timer…" (só com a pergunta da sessão).
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct PedidoLigar {
    pub sessao_id: String,
    pub pergunta_id: Option<u64>,
    pub nome: Option<String>,
    pub cwd: Option<String>,
    pub timer_id: Option<i64>,
    pub com_novo: bool,
}

static LIGAR: Mutex<Option<PedidoLigar>> = Mutex::new(None);

/// Guarda o pedido do dock, mostra (ou recria) a janela principal na frente e avisa a página.
pub fn pedir_ligar_sessao(app: &AppHandle, pedido: PedidoLigar) {
    *LIGAR.lock().unwrap_or_else(|p| p.into_inner()) = Some(pedido);
    lembrar_antes_do_dialogo(app);
    abrir_principal(app, None);
    let _ = app.emit_to(PRINCIPAL, EVENTO_LIGAR_SESSAO, ());
}

/// A página busca o pedido (ao abrir e ao receber `ligar-sessao`); vale uma vez.
pub fn ligar_sessao_pedido() -> Option<PedidoLigar> {
    LIGAR.lock().unwrap_or_else(|p| p.into_inner()).take()
}

/// Como a janela principal estava quando o dock pediu um diálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AntesDoDialogo {
    /// À vista (também minimizada): fica como o diálogo a deixar.
    AVista,
    /// Existia, escondida: volta a se esconder.
    Escondida,
    /// Fechada para a bandeja (o WebView fora da memória): é recriada e volta a fechar.
    Fechada,
}

/// O que fazer com a janela principal quando o diálogo pedido pelo dock fecha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepoisDoDialogo {
    Nada,
    Esconder,
    Fechar,
}

pub fn antes_do_dialogo(existe: bool, visivel: bool) -> AntesDoDialogo {
    match (existe, visivel) {
        (false, _) => AntesDoDialogo::Fechada,
        (true, false) => AntesDoDialogo::Escondida,
        (true, true) => AntesDoDialogo::AVista,
    }
}

pub fn depois_do_dialogo(antes: Option<AntesDoDialogo>) -> DepoisDoDialogo {
    match antes {
        Some(AntesDoDialogo::Escondida) => DepoisDoDialogo::Esconder,
        Some(AntesDoDialogo::Fechada) => DepoisDoDialogo::Fechar,
        Some(AntesDoDialogo::AVista) | None => DepoisDoDialogo::Nada,
    }
}

/// Os diálogos pedidos pelo dock abertos agora (cada pedido conta um, e a página avisa quando o
/// dele acaba) e como a janela estava quando o primeiro abriu (revisão da rodada 16: um controle
/// explícito, sem prazo).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SerieDoDock {
    pub abertos: u32,
    pub antes: Option<AntesDoDialogo>,
}

impl SerieDoDock {
    /// O dock pediu um diálogo; `agora`: como a janela está neste instante.
    pub fn abriu(&mut self, agora: AntesDoDialogo) {
        if self.abertos == 0 {
            self.antes = Some(agora);
        }
        self.abertos += 1;
    }

    /// O diálogo de um pedido acabou. Quando acaba o último, a janela volta como estava.
    pub fn fechou(&mut self) -> DepoisDoDialogo {
        if self.abertos == 0 {
            return DepoisDoDialogo::Nada;
        }
        self.abertos -= 1;
        if self.abertos > 0 {
            return DepoisDoDialogo::Nada;
        }
        depois_do_dialogo(self.antes.take())
    }

    /// A pessoa mostrou a janela (bandeja, duplo clique no dock, ⚙) ou ela foi fechada: o estado de
    /// antes não vale mais (ela fica como você a deixou).
    pub fn zerar(&mut self) {
        *self = SerieDoDock::default();
    }
}

static SERIE_DO_DOCK: Mutex<SerieDoDock> = Mutex::new(SerieDoDock { abertos: 0, antes: None });

fn serie_do_dock() -> std::sync::MutexGuard<'static, SerieDoDock> {
    SERIE_DO_DOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn lembrar_antes_do_dialogo(app: &AppHandle) {
    let j = app.get_webview_window(PRINCIPAL);
    let agora = antes_do_dialogo(j.is_some(), j.is_some_and(|j| j.is_visible().unwrap_or(false)));
    serie_do_dock().abriu(agora);
}

/// A página avisa que o diálogo de um pedido do dock acabou (de qualquer jeito): com o último, a
/// janela volta a ficar como estava.
pub fn dialogo_do_dock_fechou(app: &AppHandle) {
    let depois = serie_do_dock().fechou();
    match depois {
        DepoisDoDialogo::Nada => {}
        DepoisDoDialogo::Esconder => {
            if let Some(j) = app.get_webview_window(PRINCIPAL) {
                let _ = j.hide();
            }
            // escondida, ela não segura a vez das dicas
            crate::comandos::liberar_dicas_da_janela(app, PRINCIPAL);
        }
        DepoisDoDialogo::Fechar => fechar_principal(app),
    }
}

/// A página busca o pedido (ao abrir e ao receber `novo-timer`); vale uma vez.
pub fn novo_timer_pedido() -> Option<PedidoNovoTimer> {
    NOVO_TIMER.lock().unwrap_or_else(|p| p.into_inner()).take()
}

fn abrir_principal(app: &AppHandle, pedido: Option<u8>) {
    let ajustes = pedido.is_some();
    if let Some(p) = pedido {
        AJUSTES_PEDIDOS.store(p, Ordering::SeqCst);
    }
    if let Some(j) = app.get_webview_window(PRINCIPAL) {
        let _ = j.show();
        let _ = j.unminimize();
        let _ = j.set_focus();
        if ajustes {
            let _ = app.emit_to(PRINCIPAL, EVENTO_ABRIR_AJUSTES, ());
        }
        return;
    }
    if RECRIANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    // Criar janela no laço de eventos (bandeja, comando síncrono) trava o WebView2 no Windows:
    // a criação vai numa thread própria.
    let app = app.clone();
    let r = std::thread::Builder::new().name("worktimer-janela".into()).spawn(move || {
        if let Err(e) = criar_principal(&app) {
            log_debug!("worktimer: não consegui recriar a janela principal: {e}");
        }
        RECRIANDO.store(false, Ordering::SeqCst);
    });
    if r.is_err() {
        RECRIANDO.store(false, Ordering::SeqCst);
    }
}

/// Cria a janela principal a partir do `tauri.conf.json` (mesmo label, tamanho, opções e
/// argumentos do WebView2) e mostra. Lá ela tem `"create": false`: o Tauri não a cria na
/// partida; quem cria é o setup (fora do `--bandeja`) ou `mostrar_principal`, quando precisa.
/// Fora do setup, chamar numa thread própria (no laço de eventos o WebView2 trava).
/// Só falha se a janela não nasce; mostrar e focar são tentados sem derrubar nada.
pub fn criar_principal(app: &AppHandle) -> tauri::Result<()> {
    let mut conf = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == PRINCIPAL)
        .cloned()
        .ok_or(tauri::Error::WindowNotFound)?;
    // o título vem do nome de exibição (no tauri.conf.json fica vazio)
    conf.title = crate::perfil::nome_na_tela();
    let j = WebviewWindowBuilder::from_config(app, &conf)?.build()?;
    // título com a contagem de rodando, como a janela que foi fechada
    crate::laco::avisar(app);
    let _ = j.show();
    let _ = j.set_focus();
    Ok(())
}

/// "Manter na bandeja": destrói a janela principal (o WebView dela sai da memória). Os diálogos
/// do dock que estavam nela acabaram junto.
pub fn fechar_principal(app: &AppHandle) {
    serie_do_dock().zerar();
    crate::comandos::liberar_dicas_da_janela(app, PRINCIPAL);
    if let Some(j) = app.get_webview_window(PRINCIPAL) {
        let _ = j.destroy();
    }
}

pub fn atualizar_titulo(app: &AppHandle, rodando: usize) {
    if let Some(j) = app.get_webview_window(PRINCIPAL) {
        let _ = j.set_title(&titulo_principal(rodando));
    }
}

// ---- dock opcional (rodada 12) ----
// Em Ajustes, o dock fica "Sempre à vista" (o padrão) ou "Só quando houver uma pergunta": aí ele
// fica escondido e aparece sozinho com uma pergunta do núcleo (sessão nova, volta da inatividade,
// app fechado; os lembretes "esperando você" não contam), sem roubar o foco, e se esconde de novo
// quando não sobra nenhuma. A bandeja tem "Mostrar o dock agora". Quem decide é `dock_visivel`.

/// Quando o dock aparece (`ConfigApp::dock_modo`; valor desconhecido = sempre).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModoDock {
    /// "Sempre à vista": o dock de sempre (a bandeja pode escondê-lo).
    Sempre,
    /// "Só quando houver uma pergunta".
    SoPerguntas,
}

impl ModoDock {
    pub fn de(valor: &str) -> ModoDock {
        if valor == DOCK_SO_PERGUNTAS {
            ModoDock::SoPerguntas
        } else {
            ModoDock::Sempre
        }
    }
}

/// O "Mostrar o dock agora" (modo escondido) vale até um clique fora ou até isto sem o mouse em
/// cima do dock, o que vier primeiro.
pub const MOSTRAR_AGORA_MS: u64 = 30_000;

/// O que a bandeja pediu do dock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PedidoDock {
    #[default]
    Nenhum,
    /// "Sempre à vista", escondido pelo "Mostrar/esconder dock" da bandeja.
    Escondido,
    /// "Mostrar o dock agora" no modo escondido: `desde` é o pedido ou a última saída do mouse
    /// (ms); `clicou_fora`, o dock perdeu o foco.
    MostrarAgora { desde: u64, mouse_em_cima: bool, clicou_fora: bool },
}

/// O dock aparece? Com pergunta pendente, sempre, nos dois modos (nada se perde; revisão da
/// rodada 12, I1: também com o dock escondido pela bandeja, que volta a se esconder quando não
/// sobra pergunta). Sem pergunta: "Sempre à vista", sim, menos escondido pela bandeja; "Só quando
/// houver uma pergunta", só pelo "Mostrar o dock agora" ainda valendo (sem clique fora, e com o
/// mouse em cima ou há menos de 30 s).
pub fn dock_visivel(modo: ModoDock, perguntas: usize, pedido: PedidoDock, agora: u64) -> bool {
    match modo {
        ModoDock::Sempre => perguntas > 0 || pedido != PedidoDock::Escondido,
        ModoDock::SoPerguntas => {
            perguntas > 0
                || match pedido {
                    PedidoDock::MostrarAgora { desde, mouse_em_cima, clicou_fora } => {
                        !clicou_fora && (mouse_em_cima || agora.saturating_sub(desde) < MOSTRAR_AGORA_MS)
                    }
                    _ => false,
                }
        }
    }
}

static PEDIDO_DOCK: Mutex<PedidoDock> = Mutex::new(PedidoDock::Nenhum);

fn pedido_dock() -> std::sync::MutexGuard<'static, PedidoDock> {
    PEDIDO_DOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/// O modo do dock e as perguntas pendentes agora (sem núcleo: sempre à vista, sem perguntas).
/// Chamar sem a trava do núcleo.
fn modo_e_perguntas(app: &AppHandle) -> (ModoDock, usize) {
    match app.try_state::<Estado>() {
        Some(e) => {
            let n = trava(&e);
            let modo = n.banco.config_app().map(|c| ModoDock::de(&c.dock_modo)).unwrap_or(ModoDock::Sempre);
            (modo, n.motor.perguntas.len())
        }
        None => (ModoDock::Sempre, 0),
    }
}

/// Mostra ou esconde o dock conforme `dock_visivel` (a cada mudança de estado, a cada ciclo do
/// laço e a cada pedido da bandeja). Ao aparecer sozinho, não pede o foco. Chamar sem a trava do
/// núcleo.
pub fn aplicar_dock(app: &AppHandle) {
    let (modo, perguntas) = modo_e_perguntas(app);
    let agora = agora_ms();
    let quer = {
        let mut p = pedido_dock();
        let quer = dock_visivel(modo, perguntas, *p, agora);
        // o "Mostrar o dock agora" que venceu sai (uma pergunta depois não o revive)
        if matches!(*p, PedidoDock::MostrarAgora { .. }) && !dock_visivel(modo, 0, *p, agora) {
            *p = PedidoDock::Nenhum;
        }
        quer
    };
    let Some(d) = dock(app) else { return };
    if d.is_visible().unwrap_or(false) != quer {
        if quer {
            mostrar_sem_ativar(&d);
        } else {
            let _ = d.hide();
            // o aviso do dock não fica sozinho na tela
            esconder_aviso(app);
        }
    }
    bandeja::atualizar_item_dock(app, modo == ModoDock::SoPerguntas);
}

/// Mostra o dock **sem ativá-lo**: a pergunta chega logo depois do Enter no Claude, e o foco não
/// pode sair de lá (revisão da rodada 12). O `show()` do Tauri não serve sozinho: no Windows, o
/// tao só usa `SW_SHOWNOACTIVATE` na criação da janela; depois, `show()` é `ShowWindow(SW_SHOW)`,
/// que ativa. Aqui a janela aparece com `SW_SHOWNOACTIVATE`, e o `show()` em seguida só acerta o
/// estado do tao (sem ele, o próximo `set_size`/`set_position` a esconderia de novo); numa janela
/// já visível, o `SW_SHOW` não faz nada. É o único lugar em que o dock aparece (`aplicar_dock`);
/// o "Mostrar o dock agora" da bandeja pede o foco depois, de propósito.
fn mostrar_sem_ativar(d: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(h) = d.hwnd() {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
        // SAFETY: a janela é do próprio app e existe (o `hwnd` acabou de vir dela)
        unsafe {
            let _ = ShowWindow(HWND(h.0), SW_SHOWNOACTIVATE);
        }
    }
    let _ = d.show();
}

/// O item do dock na bandeja, um só (revisão da rodada 12), com o texto do modo
/// (`bandeja::texto_do_item_dock`). "Sempre à vista": "Mostrar/esconder dock" esconde ou mostra
/// (com uma pergunta pendente, o escondido aparece mesmo assim). "Só quando houver uma pergunta":
/// "Mostrar o dock agora", até o próximo clique fora ou 30 s sem o mouse em cima; aí o dock pega o
/// foco (foi pedido), para o clique fora valer.
pub fn item_dock(app: &AppHandle) {
    let (modo, _) = modo_e_perguntas(app);
    match modo {
        ModoDock::Sempre => {
            let visivel = dock(app).is_some_and(|d| d.is_visible().unwrap_or(false));
            *pedido_dock() = if visivel { PedidoDock::Escondido } else { PedidoDock::Nenhum };
            aplicar_dock(app);
        }
        ModoDock::SoPerguntas => {
            *pedido_dock() = PedidoDock::MostrarAgora { desde: agora_ms(), mouse_em_cima: false, clicou_fora: false };
            aplicar_dock(app);
            if let Some(d) = dock(app) {
                let _ = d.set_focus();
            }
        }
    }
}

/// O minimizar "Dock": mostra o dock se ele está "Sempre à vista" (tira o escondido pela
/// bandeja); no modo escondido, nada (só minimiza).
pub fn mostrar_dock(app: &AppHandle) {
    let (modo, _) = modo_e_perguntas(app);
    if modo == ModoDock::Sempre {
        *pedido_dock() = PedidoDock::Nenhum;
        aplicar_dock(app);
    }
}

/// O dock perdeu o foco (um clique fora): o "Mostrar o dock agora" acaba.
pub fn dock_perdeu_foco(app: &AppHandle) {
    // o clique no "Desfazer" do aviso tira o foco do dock: não é um clique fora
    if aviso_visivel(app) {
        return;
    }
    {
        let mut p = pedido_dock();
        if let PedidoDock::MostrarAgora { clicou_fora, .. } = &mut *p {
            *clicou_fora = true;
        } else {
            return;
        }
    }
    aplicar_dock(app);
}

/// O mouse entrou no dock ou saiu dele (a página avisa): os 30 s do "Mostrar o dock agora"
/// contam da saída.
pub fn dock_mouse(app: &AppHandle, dentro: bool) {
    {
        let mut p = pedido_dock();
        if let PedidoDock::MostrarAgora { desde, mouse_em_cima, .. } = &mut *p {
            *mouse_em_cima = dentro;
            if !dentro {
                *desde = agora_ms();
            }
        } else {
            return;
        }
    }
    aplicar_dock(app);
}

/// Lado da tela em que o dock está agora.
pub fn lado_do_dock(app: &AppHandle) -> Lado {
    dock(app).and_then(|d| ret_de(&d)).and_then(|r| Some(lado(r, area_da_janela(app, r)?))).unwrap_or_default()
}

fn tamanho_expandido(d: &WebviewWindow, cfg: &ConfigApp) -> (i32, i32) {
    let escala = d.scale_factor().unwrap_or(1.0);
    match (cfg.dock_larg, cfg.dock_alt) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => ((EXPANDIDO_PADRAO.0 * escala) as i32, (EXPANDIDO_PADRAO.1 * escala) as i32),
    }
}

/// Troca o tamanho do dock segurando a borda do lado em que ele está (ou o canto de cima
/// à esquerda, com `ancorar = false`). Devolve o lado.
fn redimensionar(app: &AppHandle, d: &WebviewWindow, w: i32, h: i32, ancorar_borda: bool) -> Lado {
    let Some(r) = ret_de(d) else { return Lado::default() };
    let Some(area) = area_da_janela(app, r) else { return Lado::default() };
    let (x, y) = if ancorar_borda { ancorar(r, w, h, area) } else { dentro(r.x, r.y, w, h, area) };
    if (x, y) != (r.x, r.y) {
        let _ = d.set_position(PhysicalPosition::new(x, y));
    }
    if (w, h) != (r.w, r.h) {
        let _ = d.set_size(PhysicalSize::new(w as u32, h as u32));
    }
    lado(Ret { x, y, w, h }, area)
}

fn ajustar_limites(d: &WebviewWindow, expandido: bool) {
    let _ = d.set_resizable(expandido);
    let min = expandido.then(|| LogicalSize::new(EXPANDIDO_MIN.0, EXPANDIDO_MIN.1));
    let _ = d.set_min_size(min);
}

/// Põe o dock na posição, no tamanho e na etapa salvos e mostra (se o modo dele deixa).
pub fn restaurar_dock(app: &AppHandle, cfg: &ConfigApp) {
    let Some(d) = dock(app) else { return };
    let (area, escala) = principal(app).unwrap_or((Ret { x: 0, y: 0, w: 1920, h: 1080 }, 1.0));
    let (w, h) = if cfg.dock_expandido {
        tamanho_expandido(&d, cfg)
    } else {
        ((RECOLHIDO_INICIAL.0 * escala) as i32, (RECOLHIDO_INICIAL.1 * escala) as i32)
    };
    let salva = cfg.dock_x.zip(cfg.dock_y);
    let (x, y) = posicao_inicial(salva, w, h, &areas(app), area, (MARGEM * escala) as i32);
    let valeu_salva = salva.is_some_and(|s| area_de(Ret { x: s.0, y: s.1, w, h }, &areas(app)).is_some());
    PRIMEIRO_SEM_ANCORA.store(valeu_salva && !cfg.dock_expandido, Ordering::SeqCst);
    ajustar_limites(&d, cfg.dock_expandido);
    let _ = d.set_size(PhysicalSize::new(w as u32, h as u32));
    let _ = d.set_position(PhysicalPosition::new(x, y));
    // "Sempre à vista" aparece já; no modo escondido, só com uma pergunta (o "Iniciar com o
    // Windows" abre então só a bandeja)
    aplicar_dock(app);
}

/// Troca a etapa. Expandido: o tamanho salvo (ou 320×420). Recolhido: o tamanho chega
/// depois, pelo `ajustar_recolhido`, quando a página mede o conteúdo.
pub fn mudar_etapa(app: &AppHandle, cfg: &ConfigApp) -> Lado {
    let Some(d) = dock(app) else { return Lado::default() };
    PRIMEIRO_SEM_ANCORA.store(false, Ordering::SeqCst);
    ajustar_limites(&d, cfg.dock_expandido);
    if cfg.dock_expandido {
        let (w, h) = tamanho_expandido(&d, cfg);
        redimensionar(app, &d, w, h, true)
    } else {
        lado_do_dock(app)
    }
}

/// Tamanho do recolhido medido pela página (lógico). Ignorado com o dock expandido.
pub fn ajustar_recolhido(app: &AppHandle, cfg: &ConfigApp, larg: f64, alt: f64) -> Lado {
    let Some(d) = dock(app) else { return Lado::default() };
    if cfg.dock_expandido || !(larg > 0.0 && alt > 0.0) {
        return lado_do_dock(app);
    }
    let escala = d.scale_factor().unwrap_or(1.0);
    let (w, h) = ((larg * escala).ceil() as i32, (alt * escala).ceil() as i32);
    let ancorar_borda = !PRIMEIRO_SEM_ANCORA.swap(false, Ordering::SeqCst);
    redimensionar(app, &d, w, h, ancorar_borda)
}

// ---- avisos fora do dock (rodada 15) ----
// As faixas do dock (o "Flow encerrado. Desfazer", um erro) aparecem numa janela pequena só delas,
// sem borda, sempre por cima e sem pegar o foco, logo abaixo do dock e centralizada nele (acima,
// se embaixo não couber na área do monitor do dock); ela acompanha o dock quando ele se move. O
// dock não muda de tamanho por causa de um aviso (no lugar do crescer da rodada 14). Com o dock
// escondido, o aviso vai para a janela principal.

/// A janela dos avisos do dock (`tauri.conf.json`).
pub const AVISO: &str = "aviso";
/// Evento para a janela do aviso: o aviso a mostrar (`AvisoDoDock`).
pub const EVENTO_AVISO_MOSTRAR: &str = "aviso-mostrar";
/// Evento para o dock: o botão do aviso (o id) foi clicado, na janela do aviso ou na principal.
pub const EVENTO_AVISO_CLICADO: &str = "aviso-clicado";
/// Evento para a janela principal: um aviso do dock com o dock escondido (`AvisoDoDock`).
pub const EVENTO_AVISO_DO_DOCK: &str = "aviso-do-dock";
/// Distância entre o dock e o aviso (lógica).
pub const MARGEM_AVISO: f64 = 6.0;

/// Um aviso do dock: o texto, o botão (o "Desfazer"), quanto tempo fica e o tema do dock.
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct AvisoDoDock {
    pub id: u64,
    pub texto: String,
    pub botao: Option<String>,
    /// O segundo botão (rodada 16: o "Não mostrar mais dicas" de uma dica).
    #[serde(default)]
    pub botao2: Option<String>,
    pub ms: u64,
    pub tema: Option<String>,
}

/// Onde fica o aviso de `w`×`h`: logo abaixo do dock, centralizado nele; se não couber embaixo
/// na área (o monitor do dock), logo acima; e sempre dentro da área nas laterais. Se não couber
/// nem embaixo nem em cima sem tapar o dock (um expandido alto), ao lado dele, do lado com mais
/// espaço, centralizado na altura (revisão da rodada 15).
pub fn posicao_do_aviso(dock: Ret, w: i32, h: i32, area: Ret, margem: i32) -> (i32, i32) {
    let x = dock.x + (dock.w - w) / 2;
    let abaixo = dock.y + dock.h + margem;
    if abaixo + h <= area.y + area.h {
        return dentro(x, abaixo, w, h, area);
    }
    let acima = dock.y - margem - h;
    if acima >= area.y {
        return dentro(x, acima, w, h, area);
    }
    let (a_direita, a_esquerda) = (area.x + area.w - (dock.x + dock.w), dock.x - area.x);
    let x = if a_direita >= a_esquerda { dock.x + dock.w + margem } else { dock.x - margem - w };
    dentro(x, dock.y + (dock.h - h) / 2, w, h, area)
}

/// O aviso à vista (ou a caminho) e o tamanho medido pela página do aviso (lógico).
type AvisoMedido = (AvisoDoDock, Option<(f64, f64)>);

static AVISO_ATUAL: Mutex<Option<AvisoMedido>> = Mutex::new(None);

fn aviso_atual_trava() -> std::sync::MutexGuard<'static, Option<AvisoMedido>> {
    AVISO_ATUAL.lock().unwrap_or_else(|p| p.into_inner())
}

/// O aviso que a página do aviso precisa mostrar (ela pergunta ao abrir: o evento pode ter
/// chegado antes dela).
pub fn aviso_atual() -> Option<AvisoDoDock> {
    aviso_atual_trava().as_ref().map(|(a, _)| a.clone())
}

/// Mostra um aviso do dock (um de cada vez: o novo toma o lugar). Com o dock escondido, ele vai
/// para a janela principal. Devolve se ficou no dock.
pub fn mostrar_aviso(app: &AppHandle, aviso: AvisoDoDock) -> bool {
    let no_dock = dock(app).is_some_and(|d| d.is_visible().unwrap_or(false));
    if !no_dock {
        esconder_aviso(app);
        let _ = app.emit_to(PRINCIPAL, EVENTO_AVISO_DO_DOCK, aviso);
        return false;
    }
    *aviso_atual_trava() = Some((aviso.clone(), None));
    // o prazo conta aqui, no núcleo: a janela fecha mesmo se a página do dock recarregar no meio
    // (revisão da rodada 15); um aviso novo, com outro id, não é fechado por este
    let (app2, id, prazo) = (app.clone(), aviso.id, prazo_do_aviso(aviso.ms));
    std::thread::spawn(move || {
        std::thread::sleep(prazo);
        fechar_aviso(&app2, id);
    });
    let _ = app.emit_to(AVISO, EVENTO_AVISO_MOSTRAR, aviso);
    true
}

/// Quanto tempo a janela do aviso fica, no máximo: o prazo do aviso, entre 1 s e 1 min.
pub fn prazo_do_aviso(ms: u64) -> Duration {
    Duration::from_millis(ms.clamp(1_000, 60_000))
}

/// A página do aviso mediu o aviso `id`: a janela fica desse tamanho, no lugar dela, e aparece
/// sem pegar o foco.
pub fn tamanho_do_aviso(app: &AppHandle, id: u64, larg: f64, alt: f64) {
    {
        let mut a = aviso_atual_trava();
        match a.as_mut() {
            Some((v, t)) if v.id == id && larg > 0.0 && alt > 0.0 => *t = Some((larg, alt)),
            _ => return,
        }
    }
    if posicionar_aviso(app) {
        if let Some(j) = app.get_webview_window(AVISO) {
            mostrar_sem_ativar(&j);
        }
    }
}

/// Põe a janela do aviso no lugar (abaixo ou acima do dock) e no tamanho medido. Falso se não há
/// aviso medido ou janela.
fn posicionar_aviso(app: &AppHandle) -> bool {
    let Some((larg, alt)) = aviso_atual_trava().as_ref().and_then(|(_, t)| *t) else { return false };
    let (Some(j), Some(d)) = (app.get_webview_window(AVISO), dock(app)) else { return false };
    let Some(r) = ret_de(&d) else { return false };
    let Some(area) = area_da_janela(app, r) else { return false };
    let escala = d.scale_factor().unwrap_or(1.0);
    let (w, h) = ((larg * escala).ceil() as i32, (alt * escala).ceil() as i32);
    let (x, y) = posicao_do_aviso(r, w, h, area, (MARGEM_AVISO * escala).round() as i32);
    let _ = j.set_size(PhysicalSize::new(w as u32, h as u32));
    let _ = j.set_position(PhysicalPosition::new(x, y));
    true
}

/// O dock se moveu ou mudou de tamanho: o aviso à vista vai junto.
pub fn reposicionar_aviso(app: &AppHandle) {
    if aviso_visivel(app) {
        posicionar_aviso(app);
    }
}

/// Fecha o aviso `id` (venceu, ou o "Desfazer" deu certo); outro aviso que tomou o lugar fica.
pub fn fechar_aviso(app: &AppHandle, id: u64) {
    if aviso_atual_trava().as_ref().is_some_and(|(a, _)| a.id == id) {
        esconder_aviso(app);
    }
}

fn esconder_aviso(app: &AppHandle) {
    *aviso_atual_trava() = None;
    if let Some(j) = app.get_webview_window(AVISO) {
        let _ = j.hide();
    }
}

/// O aviso está à vista (o "clique fora" do "Mostrar o dock agora" não vale enquanto isso: o
/// clique no "Desfazer" tira o foco do dock).
pub fn aviso_visivel(app: &AppHandle) -> bool {
    app.get_webview_window(AVISO).is_some_and(|j| j.is_visible().unwrap_or(false))
}

/// Um botão do aviso foi clicado: o dock faz o que ele faz (o "Desfazer" é dele). `qual`: 1 ou 2.
pub fn aviso_clicado(app: &AppHandle, id: u64, qual: u8) {
    let _ = app.emit_to(DOCK, EVENTO_AVISO_CLICADO, serde_json::json!({ "id": id, "qual": qual }));
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::APP_NOME;

    #[test]
    fn serie_de_dialogos_do_dock_conta_e_zera() {
        use AntesDoDialogo::*;
        let mut s = SerieDoDock::default();
        // escondida, dois pedidos (o "Ligar a um timer" e, de dentro dele, o "+ Novo timer…")
        s.abriu(Escondida);
        s.abriu(AVista);
        assert_eq!(s.fechou(), DepoisDoDialogo::Nada, "o primeiro acabou, o segundo está aberto");
        assert_eq!(s.fechou(), DepoisDoDialogo::Esconder, "com o último, volta como estava antes do primeiro");
        assert_eq!(s, SerieDoDock::default());
        // um aviso a mais (uma página recarregada) não esconde nada
        assert_eq!(s.fechou(), DepoisDoDialogo::Nada);
        // fechada para a bandeja: volta a fechar
        s.abriu(Fechada);
        assert_eq!(s.fechou(), DepoisDoDialogo::Fechar);
        // a pessoa mostrou a janela no meio (ou ela foi fechada): fica como está
        s.abriu(Escondida);
        s.zerar();
        assert_eq!(s.fechou(), DepoisDoDialogo::Nada);
        // à vista: fica
        s.abriu(AVista);
        assert_eq!(s.fechou(), DepoisDoDialogo::Nada);
    }

    #[test]
    fn dialogo_do_dock_deixa_a_janela_como_estava() {
        use AntesDoDialogo::*;
        assert_eq!(antes_do_dialogo(true, true), AVista);
        assert_eq!(antes_do_dialogo(true, false), Escondida);
        assert_eq!(antes_do_dialogo(false, false), Fechada);
        assert_eq!(antes_do_dialogo(false, true), Fechada);
        // escondida volta a se esconder; fechada para a bandeja volta a fechar; à vista fica
        assert_eq!(depois_do_dialogo(Some(Escondida)), DepoisDoDialogo::Esconder);
        assert_eq!(depois_do_dialogo(Some(Fechada)), DepoisDoDialogo::Fechar);
        assert_eq!(depois_do_dialogo(Some(AVista)), DepoisDoDialogo::Nada);
        assert_eq!(depois_do_dialogo(None), DepoisDoDialogo::Nada, "um diálogo que não veio do dock");
    }

    #[test]
    fn o_aviso_fecha_sozinho_no_prazo_dele() {
        assert_eq!(prazo_do_aviso(10_000), Duration::from_secs(10));
        assert_eq!(prazo_do_aviso(4_000), Duration::from_secs(4));
        assert_eq!(prazo_do_aviso(0), Duration::from_secs(1), "nunca some antes de dar para ler");
        assert_eq!(prazo_do_aviso(u64::MAX), Duration::from_secs(60), "nunca fica para sempre");
    }

    #[test]
    fn aviso_abaixo_do_dock_centralizado_ou_acima_se_nao_couber() {
        let dock = |x, y| Ret { x, y, w: 300, h: 46 };
        // embaixo, centralizado no dock, com a margem
        assert_eq!(posicao_do_aviso(dock(800, 20), 220, 34, TELA, 6), (840, 72));
        // o aviso mais largo que o dock: centralizado também (passa dos dois lados)
        assert_eq!(posicao_do_aviso(dock(800, 20), 420, 34, TELA, 6), (740, 72));
        // o dock embaixo da tela: o aviso vai para cima
        assert_eq!(posicao_do_aviso(dock(800, 990), 220, 34, TELA, 6), (840, 950));
        // cabe embaixo bem no limite: fica embaixo
        assert_eq!(posicao_do_aviso(dock(800, 954), 220, 34, TELA, 6), (840, 1006));
        // nas bordas laterais, não sai da tela
        assert_eq!(posicao_do_aviso(dock(0, 20), 420, 34, TELA, 6), (0, 72));
        assert_eq!(posicao_do_aviso(dock(1620, 20), 420, 34, TELA, 6), (1500, 72));
        // o dock expandido embaixo à direita: acima e encostado na borda
        assert_eq!(posicao_do_aviso(Ret { x: 1600, y: 620, w: 320, h: 420 }, 420, 34, TELA, 6), (1500, 580));
        // o expandido quase da altura da área: nem embaixo nem em cima sem tapar o dock, então ao
        // lado, do lado com mais espaço, centralizado na altura (revisão)
        let alto = |x| Ret { x, y: 20, w: 320, h: 1000 };
        assert_eq!(posicao_do_aviso(alto(1500), 420, 34, TELA, 6), (1074, 503), "à esquerda");
        assert_eq!(posicao_do_aviso(alto(100), 420, 34, TELA, 6), (426, 503), "à direita");
        // no segundo monitor, a área é a dele
        assert_eq!(posicao_do_aviso(dock(2900, 20), 420, 34, SEGUNDA, 6), (2780, 72));
        assert_eq!(posicao_do_aviso(dock(1930, 940), 220, 34, SEGUNDA, 6), (1970, 900));
    }

    #[test]
    fn dock_sempre_a_vista_ou_so_com_pergunta() {
        use ModoDock::*;
        let t0 = 1_000_000;
        let agora_mostra = |desde, mouse, fora| PedidoDock::MostrarAgora { desde, mouse_em_cima: mouse, clicou_fora: fora };
        // sempre à vista: aparece, a não ser escondido pela bandeja
        assert!(dock_visivel(Sempre, 0, PedidoDock::Nenhum, t0));
        assert!(!dock_visivel(Sempre, 0, PedidoDock::Escondido, t0));
        assert!(dock_visivel(Sempre, 2, PedidoDock::Escondido, t0), "escondido pela bandeja: a pergunta mostra");
        assert!(dock_visivel(Sempre, 0, agora_mostra(0, false, true), t0), "pedido do outro modo: vale o normal");
        // só com pergunta: escondido sem pergunta, à vista com uma
        assert!(!dock_visivel(SoPerguntas, 0, PedidoDock::Nenhum, t0));
        assert!(dock_visivel(SoPerguntas, 1, PedidoDock::Nenhum, t0));
        assert!(!dock_visivel(SoPerguntas, 0, PedidoDock::Escondido, t0));
        assert!(dock_visivel(SoPerguntas, 1, PedidoDock::Escondido, t0), "a pergunta nunca se perde");
        // "Mostrar o dock agora": 30 s sem o mouse em cima, ou até o clique fora
        assert!(dock_visivel(SoPerguntas, 0, agora_mostra(t0, false, false), t0 + 29_999));
        assert!(!dock_visivel(SoPerguntas, 0, agora_mostra(t0, false, false), t0 + 30_000));
        assert!(dock_visivel(SoPerguntas, 0, agora_mostra(t0, true, false), t0 + 120_000), "com o mouse em cima, fica");
        assert!(!dock_visivel(SoPerguntas, 0, agora_mostra(t0, true, true), t0 + 1), "clicou fora");
        assert!(dock_visivel(SoPerguntas, 1, agora_mostra(t0, false, true), t0 + 1), "clicou fora com pergunta: fica");
        // relógio que voltou: não quebra
        assert!(dock_visivel(SoPerguntas, 0, agora_mostra(t0, false, false), t0 - 5));
        // o valor da config
        assert_eq!(ModoDock::de("perguntas"), SoPerguntas);
        assert_eq!(ModoDock::de("sempre"), Sempre);
        assert_eq!(ModoDock::de("outro"), Sempre);
    }

    const TELA: Ret = Ret { x: 0, y: 0, w: 1920, h: 1040 };
    const SEGUNDA: Ret = Ret { x: 1920, y: 0, w: 1280, h: 984 };

    #[test]
    fn so_bandeja_quando_o_windows_abre_o_app() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(so_bandeja(args(&["C:\\x\\work-timer.exe", "--bandeja"])));
        assert!(!so_bandeja(args(&["C:\\x\\work-timer.exe"])));
        assert!(!so_bandeja(args(&["--bandeja"])), "o primeiro é o executável");
        // o Tauri não cria a janela principal na partida e ela nasce escondida: quem cria e
        // mostra é o setup
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let janelas = conf["app"]["windows"].as_array().unwrap();
        let main = janelas.iter().find(|w| w["label"] == PRINCIPAL).unwrap();
        assert_eq!(main["create"], false);
        assert_eq!(main["visible"], false);
        let dock = janelas.iter().find(|w| w["label"] == DOCK).unwrap();
        assert_eq!(dock["create"], serde_json::Value::Null, "o dock o Tauri cria");
    }

    #[test]
    fn principal_na_partida_so_fora_da_bandeja_ou_no_primeiro_uso() {
        assert!(principal_na_partida(false, false));
        assert!(principal_na_partida(false, true));
        assert!(!principal_na_partida(true, false), "Iniciar com o Windows: só bandeja e dock");
        assert!(principal_na_partida(true, true), "primeiro uso abre a janela mesmo assim");
    }

    #[test]
    fn sem_resposta_da_pagina_so_sair_sai() {
        assert_eq!(AoFechar::Sair.sem_resposta(), AoFechar::Sair);
        assert_eq!(AoFechar::Bandeja.sem_resposta(), AoFechar::Bandeja);
        assert_eq!(AoFechar::Perguntar.sem_resposta(), AoFechar::Bandeja);
        for m in [AoFechar::Perguntar, AoFechar::Bandeja, AoFechar::Sair] {
            assert_eq!(AoFechar::de(m.valor()), m);
        }
    }

    #[test]
    fn ajustes_pedidos_vale_uma_vez() {
        assert_eq!(ajustes_pedidos(), None);
        AJUSTES_PEDIDOS.store(PEDIDO_AJUSTES, Ordering::SeqCst);
        assert_eq!(ajustes_pedidos(), Some("ajustes"));
        assert_eq!(ajustes_pedidos(), None, "um recarregamento não volta para os Ajustes");
        // o "Sobre o …" da bandeja: a seção Sobre
        AJUSTES_PEDIDOS.store(PEDIDO_SOBRE, Ordering::SeqCst);
        assert_eq!(ajustes_pedidos(), Some("sobre"));
        assert_eq!(ajustes_pedidos(), None);
    }

    #[test]
    fn ao_fechar_desconhecido_pergunta() {
        assert_eq!(AoFechar::de("perguntar"), AoFechar::Perguntar);
        assert_eq!(AoFechar::de("bandeja"), AoFechar::Bandeja);
        assert_eq!(AoFechar::de("sair"), AoFechar::Sair);
        assert_eq!(AoFechar::de(""), AoFechar::Perguntar);
        assert_eq!(AoFechar::de(&ConfigApp::default().ao_fechar), AoFechar::Perguntar);
    }

    /// WebView2 só divide o ambiente entre janelas com os mesmos argumentos; a principal
    /// recriada usa os da config, então basta todas as janelas da config terem os mesmos.
    /// Os padrões são os que o wry 0.57 passa quando não há argumentos (o `autoplay` dele
    /// vem ligado), mais `--disable-gpu`.
    #[test]
    fn todas_as_janelas_sem_gpu_e_com_os_argumentos_padrao() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let janelas = conf["app"]["windows"].as_array().unwrap();
        // a principal, o dock e a dos avisos do dock (rodada 15)
        assert_eq!(janelas.len(), 3);
        for j in janelas {
            assert_eq!(
                j["additionalBrowserArgs"],
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required --disable-gpu",
                "{}",
                j["label"]
            );
        }
    }

    #[test]
    fn titulo_conta_os_rodando() {
        assert_eq!(titulo_principal(0), APP_NOME);
        assert_eq!(titulo_principal(2), format!("● 2 rodando · {APP_NOME}"));
        assert_eq!(titulo_dock(), format!("{APP_NOME} — dock"));
    }

    #[test]
    fn ao_minimizar_le_a_config_e_sem_resposta() {
        for m in [AoMinimizar::Perguntar, AoMinimizar::Bandeja, AoMinimizar::Dock, AoMinimizar::Barra] {
            assert_eq!(AoMinimizar::de(m.valor()), m);
        }
        assert_eq!(AoMinimizar::de(""), AoMinimizar::Perguntar);
        assert_eq!(AoMinimizar::de(&ConfigApp::default().ao_minimizar), AoMinimizar::Perguntar);
        // sem a página: "bandeja" fecha mesmo assim; "perguntar" minimiza na barra
        assert_eq!(AoMinimizar::Bandeja.sem_resposta(), AoMinimizar::Bandeja);
        assert_eq!(AoMinimizar::Perguntar.sem_resposta(), AoMinimizar::Barra);
    }

    #[test]
    fn o_proprio_minimizar_nao_pergunta_de_novo_por_2_s() {
        assert!(!minimizado_pelo_app(0, 5_000), "nunca minimizou");
        assert!(minimizado_pelo_app(10_000, 10_000));
        assert!(minimizado_pelo_app(10_000, 11_999));
        assert!(!minimizado_pelo_app(10_000, 12_000), "passou a folga: a pessoa minimizou de novo");
        assert!(!minimizado_pelo_app(10_000, 9_000), "relógio voltou");
    }

    #[test]
    fn pedido_vence_so_se_ninguem_respondeu() {
        static P: Pedido = Pedido::novo();
        let id = P.abrir();
        assert!(P.aberto());
        P.responder();
        assert!(!P.aberto() && !P.vencer(id), "respondido: não age sozinho");
        let id = P.abrir();
        let outro = P.abrir();
        assert!(!P.vencer(id), "um pedido mais novo vale");
        assert!(P.vencer(outro));
        assert!(!P.aberto());
    }

    #[test]
    fn posicao_inicial_usa_a_salva_se_aparece() {
        let areas = [TELA, SEGUNDA];
        assert_eq!(posicao_inicial(Some((100, 200)), 300, 46, &areas, TELA, 16), (100, 200));
        // no segundo monitor
        assert_eq!(posicao_inicial(Some((2500, 10)), 300, 46, &areas, TELA, 16), (2500, 10));
        // metade para fora: empurrada para dentro do monitor
        assert_eq!(posicao_inicial(Some((1800, 10)), 300, 46, &[TELA], TELA, 16), (1620, 10));
    }

    #[test]
    fn posicao_inicial_fora_da_tela_volta_ao_canto() {
        // segundo monitor desligado
        assert_eq!(posicao_inicial(Some((2500, 10)), 300, 46, &[TELA], TELA, 16), (1604, 16));
        // aparece só uma lasquinha
        assert_eq!(posicao_inicial(Some((1910, 10)), 300, 46, &[TELA], TELA, 16), (1604, 16));
        assert_eq!(posicao_inicial(None, 300, 46, &[TELA], TELA, 16), (1604, 16));
        // escala 150%: margem física maior
        assert_eq!(posicao_inicial(None, 480, 69, &[TELA], TELA, 24), (1416, 24));
    }

    #[test]
    fn ancorar_segura_a_borda_mais_perto() {
        // em cima, à direita: cresce para a esquerda e para baixo
        let r = Ret { x: 1604, y: 16, w: 300, h: 46 };
        assert_eq!(lado(r, TELA), Lado { direita: true, embaixo: false });
        assert_eq!(ancorar(r, 320, 420, TELA), (1584, 16));
        // embaixo, à esquerda: cresce para a direita e para cima
        let r = Ret { x: 20, y: 980, w: 300, h: 46 };
        assert_eq!(ancorar(r, 320, 420, TELA), (20, 606));
        // maior que o espaço até a borda: fica dentro
        let r = Ret { x: 900, y: 0, w: 100, h: 46 };
        assert_eq!(ancorar(r, 1100, 46, TELA), (820, 0));
        let r = Ret { x: 0, y: 0, w: 100, h: 46 };
        assert_eq!(ancorar(r, 400, 46, Ret { x: 0, y: 0, w: 300, h: 500 }), (0, 0));
    }

    #[test]
    fn area_de_escolhe_o_monitor_com_mais_dock() {
        let r = Ret { x: 1800, y: 0, w: 300, h: 46 };
        assert_eq!(area_de(r, &[TELA, SEGUNDA]), Some(SEGUNDA));
        let r = Ret { x: 1700, y: 0, w: 300, h: 46 };
        assert_eq!(area_de(r, &[TELA, SEGUNDA]), Some(TELA));
        assert_eq!(area_de(Ret { x: -500, y: 0, w: 300, h: 46 }, &[TELA]), None);
    }
}
