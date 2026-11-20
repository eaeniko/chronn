// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Conversa aberta no app do Claude. A UI Automation da janela do app tem um `Document` por
//! conversa à vista, com o Name `"<título da sessão> - Claude Code"`; o título é o mesmo nome
//! que o app guarda por sessão. Privacidade: os títulos só vão para o chamador comparar.
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::JanelaInfo;

/// Final do Name do `Document` de uma conversa do Claude Code no app do Claude.
pub const SUFIXO: &str = " - Claude Code";

/// Consulta que demora mais que isto conta como "conversa não reconhecida" naquela amostra.
pub const LIMITE_CONSULTA: Duration = Duration::from_millis(300);
/// Consulta pendente há mais que isto: a thread travou; o leitor abandona ela e começa outra.
pub const LIMITE_TRAVADA: Duration = Duration::from_secs(30);

/// Um `Document` da janela e se o elemento com foco está dentro dele.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Documento {
    pub nome: String,
    pub tem_foco: bool,
    /// O foco do teclado está num campo de texto (`Edit`) dentro deste documento: no app do
    /// Claude, é a caixa de mensagem da conversa.
    pub campo_com_foco: bool,
}

/// A conversa aberta lida na janela: o título e se você está escrevendo nela (o foco do teclado
/// na caixa de mensagem dessa conversa). O texto da caixa nunca é lido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversaLida {
    pub titulo: String,
    pub escrevendo: bool,
}

/// `ControlType` da UI Automation de um campo de texto (`UIA_EditControlTypeId`). A caixa de
/// mensagem do app do Claude (um editor ProseMirror) aparece assim.
pub const TIPO_CAMPO: i32 = 50004;

/// O elemento com foco é um campo de texto?
pub fn e_campo_de_texto(tipo: i32) -> bool {
    tipo == TIPO_CAMPO
}

/// A janela é a do app do Claude (Electron), e não um terminal rodando o `claude.exe`.
pub fn e_app_do_claude(j: &JanelaInfo) -> bool {
    j.processo == "claude" && j.classe.starts_with("Chrome_WidgetWin")
}

/// Título da sessão a partir do Name do `Document`: sem o sufixo `" - Claude Code"` e sem
/// espaços nas pontas. `None` se o documento não é de uma conversa do Claude Code.
pub fn titulo_da_conversa(nome_documento: &str) -> Option<&str> {
    let t = nome_documento.trim_end().strip_suffix(SUFIXO)?.trim();
    (!t.is_empty()).then_some(t)
}

/// Índice (em `docs`) da conversa aberta: a única conversa à vista; com várias (vista dividida),
/// a que contém o elemento com foco; sem foco dentro de nenhuma, nenhuma.
fn indice_da_conversa(docs: &[Documento]) -> Option<usize> {
    let conversas: Vec<(usize, bool)> = docs
        .iter()
        .enumerate()
        .filter(|(_, d)| titulo_da_conversa(&d.nome).is_some())
        .map(|(i, d)| (i, d.tem_foco))
        .collect();
    match conversas.as_slice() {
        [] => None,
        [(i, _)] => Some(*i),
        varias => varias.iter().find(|(_, foco)| *foco).map(|(i, _)| *i),
    }
}

/// Título da conversa aberta (`indice_da_conversa`).
pub fn conversa_aberta(docs: &[Documento]) -> Option<String> {
    indice_da_conversa(docs).and_then(|i| titulo_da_conversa(&docs[i].nome)).map(str::to_string)
}

/// A conversa aberta e se o foco do teclado está num campo de texto dela ("escrevendo"). O campo
/// de outro documento (outra conversa, um painel lateral) não conta.
pub fn ler_conversa(docs: &[Documento]) -> Option<ConversaLida> {
    let i = indice_da_conversa(docs)?;
    Some(ConversaLida { titulo: titulo_da_conversa(&docs[i].nome)?.to_string(), escrevendo: docs[i].campo_com_foco })
}

/// Subindo do foco (`acima`: do mais perto ao mais longe, `(é Document, nome)`), o título do
/// primeiro `Document` que é uma conversa. Um `Document` interno da conversa (prévia, diff) fica
/// no caminho e não conta: o foco nele conta para a conversa que o contém (spec 5.5; até a
/// rodada 5, com duas conversas à vista, dava "não sei").
pub fn conversa_acima(acima: &[(bool, String)]) -> Option<&str> {
    acima.iter().find_map(|(doc, nome)| if *doc { titulo_da_conversa(nome) } else { None })
}

/// Por onde a consulta acha a conversa aberta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caminho {
    /// Subindo do elemento com foco até a conversa (barato).
    PeloFoco,
    /// Os `Document`s de conversa guardados da última busca, conferidos (barato).
    Guardados,
    /// A busca na árvore inteira (cara numa conversa longa).
    BuscaInteira,
}

/// A escolha do caminho. Com a janela em foco: a conversa acima do foco, se há; senão a busca
/// inteira (o foco na barra lateral ou num botão é quando se troca de conversa ou se abre outra
/// ao lado: uma lista guardada estaria velha). Com a janela fora de foco (só debaixo do mouse, a
/// conversa não troca): os guardados, se ainda valem; senão a busca inteira.
pub fn caminho_da_consulta(em_foco: bool, conversa_acima_do_foco: bool, guardados_validos: bool) -> Caminho {
    match (em_foco, conversa_acima_do_foco, guardados_validos) {
        (true, true, _) => Caminho::PeloFoco,
        (true, false, _) => Caminho::BuscaInteira,
        (false, _, true) => Caminho::Guardados,
        (false, _, false) => Caminho::BuscaInteira,
    }
}

/// A thread da consulta e os canais dela.
struct Canais<T> {
    pedidos: Sender<(u64, isize, bool)>,
    respostas: Receiver<(u64, Option<T>)>,
    thread: JoinHandle<()>,
}

/// Faz a consulta numa thread própria e espera no máximo `limite` pela resposta. Enquanto uma
/// consulta atrasada não volta, as próximas amostras nem perguntam (e ficam sem conversa). Se ela
/// não volta em `limite_travada`, a thread é abandonada e outra começa (sem reiniciar o app). No
/// máximo uma abandonada fica viva: enquanto ela não volta, não nasce outra e a amostra é "não sei".
pub struct LeitorConversa<T = ConversaLida> {
    iniciar: Box<dyn Fn() -> Option<Canais<T>> + Send>,
    canais: Option<Canais<T>>,
    abandonada: Option<JoinHandle<()>>,
    prox: u64,
    pendente: Option<(u64, Instant)>,
    pub limite_travada: Duration,
    /// Quantas vezes a thread da consulta foi trocada (travou ou caiu).
    pub recomecos: u32,
    /// Quantas consultas passaram do limite (viraram "não sei").
    pub atrasadas: u64,
}

impl<T: Send + 'static> LeitorConversa<T> {
    /// `fabrica` roda na thread da consulta (ex.: inicia o COM e o cliente de UI Automation)
    /// e devolve a função que acha a conversa aberta na janela `hwnd`. Uma thread nova chama de novo.
    pub fn com<F, C>(fabrica: F) -> LeitorConversa<T>
    where
        F: Fn() -> C + Send + Sync + 'static,
        C: FnMut(isize, bool) -> Option<T>,
    {
        let fabrica = Arc::new(fabrica);
        let iniciar = move || -> Option<Canais<T>> {
            let (tx_pedido, rx_pedido) = mpsc::channel::<(u64, isize, bool)>();
            let (tx_resposta, respostas) = mpsc::channel();
            let fabrica = Arc::clone(&fabrica);
            let thread = std::thread::Builder::new()
                .name("worktimer-conversa".into())
                .spawn(move || {
                    let mut consultar = fabrica();
                    for (id, hwnd, em_foco) in rx_pedido {
                        if tx_resposta.send((id, consultar(hwnd, em_foco))).is_err() {
                            break;
                        }
                    }
                })
                .ok()?;
            Some(Canais { pedidos: tx_pedido, respostas, thread })
        };
        let canais = iniciar();
        LeitorConversa {
            iniciar: Box::new(iniciar),
            canais,
            abandonada: None,
            prox: 1,
            pendente: None,
            limite_travada: LIMITE_TRAVADA,
            recomecos: 0,
            atrasadas: 0,
        }
    }

    /// Troca a thread da consulta. A antiga fica como a abandonada: se um dia voltar, sai sozinha
    /// (ninguém mais escuta).
    fn recomecar(&mut self) {
        if let Some(c) = self.canais.take() {
            self.abandonada = Some(c.thread);
        }
        self.canais = (self.iniciar)();
        self.pendente = None;
        self.recomecos += 1;
    }

    /// Ainda há uma consulta abandonada presa.
    fn abandonada_viva(&self) -> bool {
        self.abandonada.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Conversa aberta na janela, ou `None` (não reconhecida, falhou ou passou do `limite`).
    /// `em_foco`: a janela é a que tem o foco (só então o elemento com foco é consultado).
    pub fn ler(&mut self, hwnd: isize, em_foco: bool, limite: Duration) -> Option<T> {
        if let Some((p, desde)) = self.pendente {
            // resposta atrasada de uma amostra anterior: só libera a thread
            if let Some(c) = &self.canais {
                while let Ok((id, _)) = c.respostas.try_recv() {
                    if id == p {
                        self.pendente = None;
                    }
                }
            }
            if self.pendente.is_some() {
                if desde.elapsed() < self.limite_travada || self.abandonada_viva() {
                    return None;
                }
                self.recomecar();
            }
        }
        if self.canais.is_none() {
            self.recomecar();
        }
        let id = self.prox;
        self.prox += 1;
        let c = self.canais.as_ref()?;
        if c.pedidos.send((id, hwnd, em_foco)).is_err() {
            self.canais = None;
            return None;
        }
        match c.respostas.recv_timeout(limite) {
            Ok((rid, r)) if rid == id => r,
            Ok(_) => None,
            Err(RecvTimeoutError::Timeout) => {
                self.pendente = Some((id, Instant::now()));
                self.atrasadas += 1;
                None
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.canais = None;
                None
            }
        }
    }
}

#[cfg(windows)]
impl LeitorConversa<ConversaLida> {
    /// Leitor com a UI Automation do Windows.
    pub fn novo() -> LeitorConversa<ConversaLida> {
        LeitorConversa::com(|| {
            let mut uia = crate::uia::Uia::novo();
            move |hwnd: isize, em_foco: bool| uia.as_mut().and_then(|u| u.ler(hwnd, em_foco))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(nome: &str, tem_foco: bool) -> Documento {
        Documento { nome: nome.into(), tem_foco, campo_com_foco: false }
    }

    fn escrevendo(nome: &str) -> Documento {
        Documento { nome: nome.into(), tem_foco: true, campo_com_foco: true }
    }

    #[test]
    fn escrevendo_so_com_o_foco_no_campo_da_conversa_aberta() {
        let lida = |t: &str, e: bool| Some(ConversaLida { titulo: t.into(), escrevendo: e });
        // uma conversa à vista, foco na caixa de mensagem dela
        let docs = [doc("", false), escrevendo("Flow: nomes - Claude Code")];
        assert_eq!(ler_conversa(&docs), lida("Flow: nomes", true));
        // foco fora do campo (na lista de mensagens, ou em outro programa): lendo
        let docs = [doc("", false), doc("Flow: nomes - Claude Code", true)];
        assert_eq!(ler_conversa(&docs), lida("Flow: nomes", false));
        // vista dividida: escrevendo na B
        let docs = [doc("A - Claude Code", false), escrevendo("B - Claude Code")];
        assert_eq!(ler_conversa(&docs), lida("B", true));
        // o campo de um documento que não é conversa (um painel lateral) não marca nada
        let docs = [escrevendo("Notas"), doc("A - Claude Code", false)];
        assert_eq!(ler_conversa(&docs), lida("A", false));
        assert_eq!(ler_conversa(&[]), None);
        assert!(e_campo_de_texto(TIPO_CAMPO));
        assert!(!e_campo_de_texto(50030), "Document não é campo");
    }

    #[test]
    fn extrai_o_titulo_do_name_do_documento() {
        assert_eq!(
            titulo_da_conversa("Projeto: tela de login - Claude Code"),
            Some("Projeto: tela de login")
        );
        assert_eq!(titulo_da_conversa("  Título com espaço   - Claude Code "), Some("Título com espaço"));
        // o documento raiz da página não tem nome; uma conversa comum do Claude não é Claude Code
        assert_eq!(titulo_da_conversa(""), None);
        assert_eq!(titulo_da_conversa("Receita de bolo - Claude"), None);
        assert_eq!(titulo_da_conversa(" - Claude Code"), None);
        // o sufixo é o do fim; um título que contém o sufixo no meio fica inteiro
        assert_eq!(titulo_da_conversa("A - Claude Code e B - Claude Code"), Some("A - Claude Code e B"));
    }

    #[test]
    fn uma_conversa_a_vista_vale_sem_foco() {
        let docs = [doc("", false), doc("Flow: nomes - Claude Code", false)];
        assert_eq!(conversa_aberta(&docs), Some("Flow: nomes".into()));
        assert_eq!(conversa_aberta(&[doc("", true)]), None);
        assert_eq!(conversa_aberta(&[]), None);
    }

    #[test]
    fn vista_dividida_usa_a_conversa_com_foco() {
        let docs = [doc("", false), doc("A - Claude Code", false), doc("B - Claude Code", true)];
        assert_eq!(conversa_aberta(&docs), Some("B".into()));
        // o foco no documento raiz (ou em outro programa) não escolhe nenhuma
        let docs = [doc("", true), doc("A - Claude Code", false), doc("B - Claude Code", false)];
        assert_eq!(conversa_aberta(&docs), None);
    }

    #[test]
    fn app_do_claude_e_o_electron_e_nao_o_terminal() {
        let j = |processo: &str, classe: &str| JanelaInfo {
            processo: processo.into(),
            titulo: "Claude".into(),
            classe: classe.into(),
            pid: 1,
            hwnd: 1,
            em_foco: true,
        };
        assert!(e_app_do_claude(&j("claude", "Chrome_WidgetWin_1")));
        assert!(!e_app_do_claude(&j("claude", "ConsoleWindowClass")));
        assert!(!e_app_do_claude(&j("code", "Chrome_WidgetWin_1")));
    }

    #[test]
    fn caminho_da_consulta_nunca_usa_os_guardados_com_a_janela_em_foco() {
        use Caminho::*;
        assert_eq!(caminho_da_consulta(true, true, true), PeloFoco);
        assert_eq!(caminho_da_consulta(true, true, false), PeloFoco);
        // em foco, o foco fora de uma conversa (barra lateral, botão da vista dividida): busca inteira
        assert_eq!(caminho_da_consulta(true, false, true), BuscaInteira);
        assert_eq!(caminho_da_consulta(true, false, false), BuscaInteira);
        // fora de foco: os guardados, se ainda valem
        assert_eq!(caminho_da_consulta(false, false, true), Guardados);
        assert_eq!(caminho_da_consulta(false, false, false), BuscaInteira);
    }

    #[test]
    fn foco_num_documento_interno_sobe_ate_a_conversa() {
        let a = |doc: bool, nome: &str| (doc, nome.to_string());
        // do foco para cima: um grupo, um Document interno (prévia), outro grupo, a conversa B, a raiz
        let acima = [a(false, ""), a(true, "Prévia do diff"), a(false, ""), a(true, "B - Claude Code"), a(true, "")];
        assert_eq!(conversa_acima(&acima), Some("B"), "a prévia fica no caminho");
        // o próprio foco é o Document da conversa
        assert_eq!(conversa_acima(&[a(true, "A - Claude Code")]), Some("A"));
        // um elemento que não é Document, com o nome de uma conversa, não conta
        assert_eq!(conversa_acima(&[a(false, "A - Claude Code"), a(true, "")]), None);
        assert_eq!(conversa_acima(&[]), None, "foco fora da janela");
    }

    #[test]
    fn leitor_travado_recomeca_com_thread_nova() {
        let (tx, rx) = mpsc::channel::<()>();
        let rx = std::sync::Mutex::new(Some(rx));
        // a primeira thread trava para sempre; as seguintes respondem
        let mut l = LeitorConversa::com(move || {
            let trava = rx.lock().unwrap().take();
            move |_: isize, _: bool| {
                if let Some(r) = &trava {
                    let _ = r.recv();
                }
                Some("ok".to_string())
            }
        });
        l.limite_travada = Duration::from_millis(30);
        assert_eq!(l.ler(1, true, Duration::from_millis(20)), None);
        assert_eq!(l.ler(1, true, Duration::from_millis(20)), None, "pendente, ainda dentro do prazo");
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(l.ler(1, true, Duration::from_secs(5)), Some("ok".into()));
        assert_eq!(l.recomecos, 1);
        drop(tx);
    }

    #[test]
    fn leitor_deixa_no_maximo_uma_consulta_abandonada_viva() {
        // toda consulta espera uma liberação; as liberações saem na ordem em que as threads pedem
        let (tx, rx) = mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));
        let mut l = LeitorConversa::com(move || {
            let rx = Arc::clone(&rx);
            move |_: isize, _: bool| {
                let _ = rx.lock().unwrap().recv();
                Some("ok".to_string())
            }
        });
        l.limite_travada = Duration::from_millis(20);
        let curto = Duration::from_millis(5);
        assert_eq!(l.ler(1, true, curto), None);
        // muitos prazos com a primeira presa: só uma troca
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(25));
            assert_eq!(l.ler(1, true, curto), None);
        }
        assert_eq!(l.recomecos, 1);
        // a abandonada volta e sai; a próxima troca pode acontecer
        tx.send(()).unwrap();
        for _ in 0..500 {
            if !l.abandonada_viva() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!l.abandonada_viva());
        std::thread::sleep(Duration::from_millis(25));
        assert_eq!(l.ler(1, true, curto), None);
        assert_eq!(l.recomecos, 2);
        drop(tx);
    }

    #[test]
    fn leitor_responde_dentro_do_limite() {
        let mut l = LeitorConversa::com(|| |hwnd: isize, _: bool| Some(format!("janela {hwnd}")));
        assert_eq!(l.ler(7, true, Duration::from_secs(5)), Some("janela 7".into()));
        assert_eq!(l.ler(8, true, Duration::from_secs(5)), Some("janela 8".into()));
    }

    #[test]
    fn leitor_lento_conta_como_nao_reconhecida_e_nao_acumula_pedidos() {
        let (tx, rx) = mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));
        let mut l = LeitorConversa::com(move || {
            let rx = Arc::clone(&rx);
            move |_: isize, _: bool| {
                // segura a consulta até o teste soltar
                let _ = rx.lock().unwrap().recv();
                Some("tarde".to_string())
            }
        });
        assert_eq!(l.ler(1, true, Duration::from_millis(20)), None, "passou do limite");
        assert_eq!(l.ler(1, true, Duration::from_millis(20)), None, "ainda ocupada: nem pergunta");
        assert_eq!(l.atrasadas, 1, "só a que passou do limite conta");
        tx.send(()).unwrap();
        // a resposta atrasada é descartada; a próxima consulta responde na hora
        tx.send(()).unwrap();
        let mut r = None;
        for _ in 0..500 {
            r = l.ler(1, true, Duration::from_secs(5));
            if r.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(r, Some("tarde".into()));
    }
}
