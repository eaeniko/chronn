// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Sessoes do Claude: eventos dos hooks viram trechos `Claude`.
use fila::{pasta_ignorada, EventoHook};

use super::Motor;
use crate::modelo::{EstadoSessao, Ligacao, MotivoFim, Mudanca, Origem, Pergunta, RegraPasta, Sessao, TrechoAberto};
use crate::tempo::{inicio_do_dia, Ms, MIN};

/// O que acontece com os trechos que a sessao ja contou ao liga-la a outro timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mover {
    /// Ficam onde estao; o aberto fecha e reabre no timer novo.
    DaquiParaFrente,
    /// Vem para o timer novo (todos, ou so os do dia de trabalho atual).
    DesdeInicio { so_hoje: bool },
}

/// Por quanto tempo o dock avisa que uma regra de pasta (ou a heranca) ligou uma sessao sozinha.
pub const AVISO_LIGADA_MS: Ms = 15_000;
/// A conversa continuada herda o timer de uma sessao com o mesmo nome e a mesma pasta destes
/// ultimos dias (os mesmos 7 dias das sessoes carregadas ao abrir).
pub const HERANCA_MS: Ms = 7 * 24 * 60 * MIN;

/// Uma sessao que a regra de pasta (`Ligacao::Regra`) ou a heranca (`Ligacao::Herdada`) ligou
/// sozinha em `em`: o dock mostra "Ligada a X pela pasta · Trocar" por `AVISO_LIGADA_MS`.
#[derive(Clone, Debug, PartialEq)]
pub struct LigadaSozinha {
    pub timer_id: i64,
    pub como: Ligacao,
    pub em: Ms,
}

// IMPL-INICIO
/// A pasta comparável (`fila::normaliza`): `/` = `\`, sem maiúsculas, `\\wsl$\` = `\\wsl.localhost\`.
fn normaliza(s: &str) -> String {
    fila::normaliza(s)
}

/// Regra de pasta de prefixo mais longo (sem diferenciar maiusculas, `/` = `\`, respeitando limite de caminho).
pub fn regra_para(regras: &[RegraPasta], cwd: &str) -> Option<i64> {
    regra_da_pasta(regras, cwd).map(|r| r.timer_id)
}

/// A regra que vale para `cwd` (a de prefixo mais longo), inteira.
pub fn regra_da_pasta<'a>(regras: &'a [RegraPasta], cwd: &str) -> Option<&'a RegraPasta> {
    let c = normaliza(cwd);
    regras
        .iter()
        .filter_map(|r| {
            let p = normaliza(&r.pasta);
            let p = p.trim_end_matches('/');
            (!p.is_empty() && (c == p || c.starts_with(&format!("{p}/")))).then_some((p.len(), r))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, r)| r)
}

/// Pasta comparavel: sem diferenciar maiusculas, `/` = `\\`, sem barra no fim.
pub fn chave_de_pasta(p: &str) -> String {
    normaliza(p).trim_end_matches('/').to_string()
}

/// Pasta larga demais para regra: a raiz de um drive (`G:`) ou a pasta do usuario
/// (`C:\\Users\\<nome>`, ou o `%USERPROFILE%`). Uma regra ali ligaria tudo o que esta embaixo.
pub fn pasta_larga(p: &str, perfil: Option<&str>) -> bool {
    let c = chave_de_pasta(p);
    let partes: Vec<&str> = c.split('/').filter(|x| !x.is_empty()).collect();
    let drive = |x: &str| x.len() == 2 && x.ends_with(':');
    match partes.as_slice() {
        [] => true,
        [d] if drive(d) => true,
        [d, "users", _] if drive(d) => true,
        // o WSL (rodada 28): a raiz de uma distro e a pasta de um usuário dela
        ["wsl.localhost"] | ["wsl.localhost", _] | ["wsl.localhost", _, "home" | "root"] => true,
        ["wsl.localhost", _, "home", _] => true,
        _ => perfil.is_some_and(|u| !u.trim().is_empty() && chave_de_pasta(u) == c),
    }
}

/// Raiz do projeto: num worktree do Claude Code (`<projeto>\.claude\worktrees\<nome>`), a pasta
/// antes do `.claude`; fora dele, a propria pasta. Regras de pasta e sugestoes de timer usam a raiz,
/// para todos os worktrees do projeto cairem na mesma regra. Se a raiz for a de um drive ou a pasta
/// do usuario, fica a pasta do worktree.
pub fn raiz_do_projeto(cwd: &str) -> &str {
    raiz_com_perfil(cwd, std::env::var("USERPROFILE").ok().as_deref())
}

fn raiz_com_perfil<'a>(cwd: &'a str, perfil: Option<&str>) -> &'a str {
    // ASCII minusculo e `/` → `\\` mantem as posicoes dos bytes
    let mut busca: String = cwd.to_ascii_lowercase().replace('/', "\\");
    busca.push('\\');
    match busca.find("\\.claude\\worktrees\\") {
        Some(i) if i > 0 => {
            let raiz = cwd[..i].trim_end_matches(['\\', '/']);
            if pasta_larga(raiz, perfil) { cwd } else { raiz }
        }
        _ => cwd,
    }
}

/// As pastas que o "Sempre que for na pasta" da pergunta de sessao nova oferece (rodada 27): da
/// pasta onde a sessao comecou (`inicio`) ate a pasta onde ela esta agora (`agora`), cada nivel
/// entre as duas, de cima para baixo. Cada uma passa por `raiz_do_projeto` (um worktree vira o
/// projeto). Se a de agora nao esta dentro da de inicio, so as duas. Sem pasta (o Chat), nenhuma.
/// Ficam de fora as que nao podem ter regra: a raiz de um drive, a pasta do usuario (`perfil`),
/// as que nunca contam (`ignoradas`) e as que estao dentro de uma pasta `.claude` (um worktree
/// dentro da pasta do usuario, que fica como esta: `…\.claude` e `…\.claude\worktrees` nao sao
/// pastas de projeto).
pub fn pastas_entre(inicio: &str, agora: &str, perfil: Option<&str>, ignoradas: &[String]) -> Vec<String> {
    let limpa = |p: &str| raiz_com_perfil(p.trim(), perfil).trim_end_matches(['\\', '/']).to_string();
    let ini = limpa(inicio);
    if ini.is_empty() {
        return vec![];
    }
    let fim = match limpa(agora) {
        f if f.is_empty() => ini.clone(),
        f => f,
    };
    let mut v = vec![ini.clone()];
    if crate::pastas::ancestral(&ini, &fim) {
        // cada nivel abaixo da de inicio, pelo proprio texto da de agora (as barras e as
        // maiusculas dela)
        let partes = |p: &str| p.split(['\\', '/']).filter(|x| !x.is_empty()).count();
        let n = partes(&ini);
        for (i, c) in fim.char_indices() {
            if (c == '\\' || c == '/') && i > 0 && partes(&fim[..i]) > n {
                v.push(fim[..i].to_string());
            }
        }
        v.push(fim);
    } else if chave_de_pasta(&ini) != chave_de_pasta(&fim) {
        v.push(fim);
    }
    let mut vistas = std::collections::BTreeSet::new();
    v.retain(|p| {
        !pasta_larga(p, perfil) && !pasta_ignorada(p, ignoradas) && !dentro_de_claude(p) && vistas.insert(chave_de_pasta(p))
    });
    v
}

/// A pasta tem um nivel `.claude` (sem diferenciar maiusculas): a propria `.claude` ou algo dentro.
fn dentro_de_claude(pasta: &str) -> bool {
    pasta.split(['\\', '/']).any(|x| x.eq_ignore_ascii_case(".claude"))
}

/// Nome comparavel da sessao: sem diferenciar maiusculas, sem os espacos das pontas (vazio = sem nome).
fn chave_de_nome(nome: Option<&str>) -> Option<String> {
    let n = nome?.trim().to_lowercase();
    (!n.is_empty()).then_some(n)
}

/// Ultimo nome da pasta, para sugerir o nome do timer.
pub fn sugestao_de_pasta(cwd: &str) -> String {
    cwd.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or("").to_string()
}

impl Motor {
    pub(crate) fn abrir_claude(&mut self, sessao_id: &str, timer_id: Option<i64>, inicio: Ms) -> Option<Mudanca> {
        if self.abertos.iter().any(|t| t.origem == Origem::Claude && t.sessao_id.as_deref() == Some(sessao_id)) {
            return None;
        }
        // a atividade padrao do timer (Trabalho, ou Estudo num timer de curso)
        let atividade_id = self.atividade_padrao(timer_id);
        self.abertos.push(TrechoAberto {
            timer_id,
            sessao_id: Some(sessao_id.to_string()),
            origem: Origem::Claude,
            inicio,
            atividade_id,
        });
        Some(Mudanca::Abriu { timer_id, sessao_id: Some(sessao_id.to_string()), origem: Origem::Claude, inicio, atividade_id })
    }

    /// Fecha os abertos da sessao (so os do Claude, ou todos).
    pub(super) fn fechar_sessao(&mut self, sessao_id: &str, fim: Ms, motivo: MotivoFim, so_claude: bool) -> Vec<Mudanca> {
        self.fechar_se(fim, motivo, |t| {
            t.sessao_id.as_deref() == Some(sessao_id) && (!so_claude || t.origem == Origem::Claude)
        })
    }

    pub fn evento(&mut self, ev: &EventoHook) -> Vec<Mudanca> {
        if pasta_ignorada(&ev.cwd, &self.cfg.pastas_ignoradas) {
            return vec![];
        }
        let t = ev.t;
        // `Stop` perdido: quem trabalhava sem evento ha `sessao_parada_ms` fecha no ultimo evento antes
        // de este ser tratado (e antes do corte). Vale para a fila lida de uma vez, sem tique no meio.
        let mut out = self.sessoes_sem_evento(t);
        out.extend(self.corte_do_dia(t));

        if !self.sessoes.contains_key(&ev.sessao) {
            // a regra de um timer escondido não vale (spec 004)
            let timer_id = self.regra_viva(&ev.cwd);
            self.sessoes.insert(
                ev.sessao.clone(),
                Sessao {
                    id: ev.sessao.clone(),
                    cwd: ev.cwd.clone(),
                    nome: None,
                    transcript: None,
                    timer_id,
                    estado: EstadoSessao::Parada,
                    ultimo_evento: t,
                    terminou_em: None,
                    criada_em: t,
                    ligacao: timer_id.map(|_| Ligacao::Regra),
                    desligada_em: None,
                    origem: ev.origem.clone().filter(|o| !o.trim().is_empty()),
                    conta: fila::conta_valida(ev.conta.as_deref()),
                },
            );
        } else if t < self.sessoes[&ev.sessao].ultimo_evento {
            return out;
        }
        // ligada a um timer escondido (spec 004; o `esconder` já soltou, isto é a defesa): fica sem
        // timer, e a próxima atividade pergunta
        if self.escondido(self.sessoes[&ev.sessao].timer_id) {
            out.extend(self.soltar_do_escondido(&ev.sessao, t));
        }

        // A conversa que ia passar a ser lida (troca suave) teve evento: a leitura dela, se valer,
        // comeca depois dele (nada sobreposto ao trecho do Claude).
        if let Some(tr) = self.troca.as_mut().filter(|tr| tr.alvo.as_deref() == Some(ev.sessao.as_str())) {
            tr.inicio = tr.inicio.max(t);
        }
        let mut s = self.sessoes[&ev.sessao].clone();
        let timer = s.timer_id;
        s.ultimo_evento = t;
        if ev.transcript.is_some() {
            s.transcript = ev.transcript.clone();
        }
        // a primeira origem não vazia fica (rodada 27)
        if s.origem.is_none() {
            s.origem = ev.origem.clone().filter(|o| !o.trim().is_empty());
        }
        // a conta do Claude (rodada 28): a primeira fica na sessão; a do app do Claude passa às
        // conversas do Chat novas; toda conta vista vai para `contas`
        if let Some(conta) = fila::conta_valida(ev.conta.as_deref()) {
            if s.conta.is_none() {
                s.conta = Some(conta.clone());
            }
            if ev.origem.as_deref() == Some(super::ORIGEM_APP_DO_CLAUDE) {
                self.conta_do_app = Some(conta.clone());
            }
            out.push(Mudanca::Conta { id: conta, origem: ev.origem.clone().filter(|o| !o.trim().is_empty()), visto_em: t });
        }
        // a pasta de agora (o `cwd` vem a cada evento), só em memória: o `Sessao.cwd` gravado segue
        // sendo onde a sessão começou
        if !ev.cwd.trim().is_empty() {
            self.pasta_agora.insert(ev.sessao.clone(), ev.cwd.clone());
        }
        let trabalhando = s.estado == EstadoSessao::Trabalhando;
        match ev.evento.as_str() {
            // Mensagem de antes do pause ou do ■ (a fila foi lida depois do clique): o clique vale.
            // A sessao trabalha, mas segue bloqueada e o timer nao liga.
            "UserPromptSubmit" if self.bloqueadas.get(&ev.sessao).is_some_and(|em| t < *em) => {
                s.estado = EstadoSessao::Trabalhando;
            }
            "UserPromptSubmit" => {
                s.estado = EstadoSessao::Trabalhando;
                self.bloqueadas.remove(&ev.sessao);
                // Escrever na conversa tambem libera a leitura pausada dela e desfaz o encerrar do timer.
                self.leitura_bloqueada.remove(&ev.sessao);
                out.extend(self.desencerrar(timer));
                // Escrever na sessao que voce lia encerra a leitura; em outra sessao, nao.
                out.extend(self.fechar_leitura_de(&ev.sessao, t, MotivoFim::NovaMensagem));
                out.extend(self.abrir_claude(&ev.sessao, timer, t));
                // um turno novo: o ▶ armado do timer não para mais (spec 009)
                if let Some(t) = timer {
                    self.play_armado.remove(&t);
                }
            }
            "PostToolUse" => {
                // Pausada ou com o timer encerrado: so a proxima mensagem religa.
                if !trabalhando && !self.bloqueadas.contains_key(&ev.sessao) && !self.encerrado(timer) {
                    s.estado = EstadoSessao::Trabalhando;
                    // Retomou sem novo prompt (ex.: permissao respondida): a leitura acaba aqui.
                    out.extend(self.fechar_leitura_de(&ev.sessao, t, MotivoFim::NovaMensagem));
                    out.extend(self.abrir_claude(&ev.sessao, timer, t));
                    // o turno continua: o ▶ armado pelo `Notification` não para mais (spec 009)
                    if let Some(timer) = timer {
                        self.play_armado.remove(&timer);
                    }
                }
            }
            "Stop" | "Notification" => {
                if trabalhando {
                    s.estado = EstadoSessao::EsperandoVoce;
                    s.terminou_em = Some(t);
                    out.extend(self.fechar_sessao(&ev.sessao, t, MotivoFim::Terminou, true));
                    // o ▶ que para com o Claude (spec 009)
                    out.extend(self.play_no_fim_do_turno(&ev.sessao, t));
                }
            }
            "SessionEnd" => {
                s.estado = EstadoSessao::Parada;
                out.extend(self.fechar_sessao(&ev.sessao, t, MotivoFim::SessaoParou, false));
                // spec 002: depois do `/exit`, o tempo naquela janela não é mais desta sessão
                self.soltar_janela(&ev.sessao);
            }
            // SessionStart so atualiza ultimo_evento/transcript: nao muda o estado.
            _ => {}
        }
        // Uma vez, quando a sessao comeca a contar tempo: sem timer (e sem decisao sua), a
        // pergunta; ligada pela regra de pasta, o aviso no dock, no instante do evento (o evento
        // antigo, da fila lida de uma vez, ja nasce vencido).
        if matches!(ev.evento.as_str(), "UserPromptSubmit" | "PostToolUse") && self.perguntadas.insert(ev.sessao.clone()) {
            match (s.timer_id, s.ligacao) {
                // sem timer: a pergunta. Também a desligada à mão (o ✎ ou o X da sessão), que volta a
                // perguntar na próxima atividade (rodada 15); o "Não contar" já está em `perguntadas`.
                (None, None | Some(Ligacao::Mao)) => out.push(self.pergunta_sessao_nova(&ev.sessao, &ev.cwd)),
                // a conversa nova numa pasta com regra conta no timer dela e pergunta se está certo
                // (rodada 15); com "Usar a regra direto", só o aviso
                (Some(timer_id), Some(Ligacao::Regra)) if self.cfg.confirmar_conversa_nova => {
                    out.push(self.nova_pergunta(Pergunta::ConversaNova { sessao_id: ev.sessao.clone(), cwd: ev.cwd.clone(), timer_id }));
                }
                (Some(timer_id), Some(como @ (Ligacao::Regra | Ligacao::Herdada))) => {
                    self.ligadas_sozinhas.insert(ev.sessao.clone(), LigadaSozinha { timer_id, como, em: t });
                }
                _ => {}
            }
        }
        self.sessoes.insert(ev.sessao.clone(), s.clone());
        // a mensagem no terminal liga a sessão à janela em que você escreveu (rodada 28)
        if ev.evento == "UserPromptSubmit" {
            self.pedir_ligacao_da_janela(&ev.sessao, t);
        }
        out.push(Mudanca::Sessao(s));
        out
    }

    /// Você apertou o stop na conversa em `em` (rodada 13; o app achou a interrupção no fim do
    /// transcript, porque o hook `Stop` não vem nessa hora). Vale só para a sessão trabalhando e
    /// com `em` depois do último evento dela: o trecho do Claude fecha em `em` (motivo
    /// `interrompida`) e a sessão passa a esperar você, como depois de um `Stop`. Um evento novo
    /// depois disso reabre o trecho normalmente.
    pub fn interromper(&mut self, sessao_id: &str, em: Ms) -> Vec<Mudanca> {
        // como num evento: antes, a regra dos 10 min sem evento e o corte do dia no instante
        // (revisão da rodada 13)
        let mut out = self.sessoes_sem_evento(em);
        out.extend(self.corte_do_dia(em));
        let Some(s) = self.sessoes.get(sessao_id) else { return out };
        if s.estado != EstadoSessao::Trabalhando || em <= s.ultimo_evento {
            return out;
        }
        out.extend(self.fechar_sessao(sessao_id, em, MotivoFim::Interrompida, true));
        let s = self.sessoes.get_mut(sessao_id).expect("sessao existe");
        s.estado = EstadoSessao::EsperandoVoce;
        s.terminou_em = Some(em);
        s.ultimo_evento = em;
        out.push(Mudanca::Sessao(s.clone()));
        // o ▶ que para com o Claude (spec 009)
        out.extend(self.play_no_fim_do_turno(sessao_id, em));
        out
    }

    /// Sessoes trabalhando sem evento ha `sessao_parada_ms` viram `Parada`; o trecho fecha no ultimo evento.
    pub(crate) fn sessoes_sem_evento(&mut self, agora: Ms) -> Vec<Mudanca> {
        let limite = self.cfg.sessao_parada_ms;
        let paradas: Vec<String> = self
            .sessoes
            .values()
            .filter(|s| s.estado == EstadoSessao::Trabalhando && agora - s.ultimo_evento >= limite)
            .map(|s| s.id.clone())
            .collect();
        let mut out = vec![];
        for id in paradas {
            let fim = self.sessoes[&id].ultimo_evento;
            out.extend(self.fechar_sessao(&id, fim, MotivoFim::SessaoParou, false));
            let s = self.sessoes.get_mut(&id).expect("sessao existe");
            s.estado = EstadoSessao::Parada;
            out.push(Mudanca::Sessao(s.clone()));
        }
        out
    }

    /// Tira a pergunta da conversa nova da sessao (ela foi ligada ou desligada a mao).
    pub(crate) fn tirar_conversa_nova(&mut self, sessao_id: &str) -> Vec<Mudanca> {
        let mut out = vec![];
        self.perguntas.retain(|(id, p)| {
            let dela = matches!(p, Pergunta::ConversaNova { sessao_id: s, .. } if s == sessao_id);
            if dela {
                out.push(Mudanca::PerguntaResolvida(*id));
            }
            !dela
        });
        out
    }

    /// A pergunta "Em qual timer contar?" da sessão sem timer, na pasta `cwd`.
    pub(super) fn pergunta_sessao_nova(&mut self, sessao_id: &str, cwd: &str) -> Mudanca {
        let raiz = raiz_do_projeto(cwd);
        // a pasta-mae e conferida aqui, uma vez (o disco so nas subpastas diretas); sem pasta (o
        // Chat), nada
        // só as regras que valem (spec 004: a de um escondido fica inerte)
        let pasta_mae = !raiz.trim().is_empty() && crate::pastas::pasta_mae(raiz, &self.regras_vivas());
        self.nova_pergunta(Pergunta::SessaoNova {
            sessao_id: sessao_id.to_string(),
            cwd: cwd.to_string(),
            sugestao: sugestao_de_pasta(raiz),
            pasta_mae,
        })
    }

    /// Liga a sessao ao timer a mao (pela pergunta, pelo "Ligar", ao mover): vence a regra de
    /// pasta e passa para a conversa continuada. "Desde o inicio" move todos os trechos dela,
    /// menos os de antes de um "Desligar" (rodada 15: esses ficam onde estavam).
    pub fn ligar_sessao(&mut self, sessao_id: &str, timer_id: i64, desde_inicio: bool, agora: Ms) -> Vec<Mudanca> {
        let mover = if desde_inicio { Mover::DesdeInicio { so_hoje: false } } else { Mover::DaquiParaFrente };
        self.ligar_com(sessao_id, timer_id, mover, agora, Ligacao::Mao)
    }

    /// Como `ligar_sessao` "desde o inicio", mas so com os trechos do dia de trabalho atual (e os
    /// de depois de um "Desligar"): a resposta da conversa nova e o "Mover" do ✎ (rodada 15). Os
    /// dias anteriores nunca mudam de timer por esses caminhos (podem ja ter sido lancados).
    pub fn ligar_sessao_hoje(&mut self, sessao_id: &str, timer_id: i64, agora: Ms) -> Vec<Mudanca> {
        self.ligar_com(sessao_id, timer_id, Mover::DesdeInicio { so_hoje: true }, agora, Ligacao::Mao)
    }

    fn ligar_com(&mut self, sessao_id: &str, timer_id: i64, mover: Mover, agora: Ms, como: Ligacao) -> Vec<Mudanca> {
        // nunca a um escondido (spec 004)
        if self.escondido(Some(timer_id)) {
            return vec![];
        }
        let mut out = self.corte_do_dia(agora);
        let dia_ini = inicio_do_dia(self.dia, self.cfg.corte_min, self.cfg.fuso_min);
        let Some(s) = self.sessoes.get_mut(sessao_id) else {
            return out;
        };
        s.timer_id = Some(timer_id);
        s.ligacao = Some(como);
        let s = s.clone();
        // de onde os trechos mudam de timer: o mais recente entre o "Desligar" e, so hoje, o
        // inicio do dia; sem nenhum dos dois, todos
        let desde = match mover {
            Mover::DesdeInicio { so_hoje } => Some([s.desligada_em, so_hoje.then_some(dia_ini)].into_iter().flatten().max()),
            Mover::DaquiParaFrente => None,
        };
        // ligada a mao: o aviso de ligada sozinha, se havia, sai, e a pergunta da conversa nova
        // tambem (ela foi para onde voce quis)
        if como == Ligacao::Mao {
            self.ligadas_sozinhas.remove(sessao_id);
            out.extend(self.tirar_conversa_nova(sessao_id));
        }
        // os trechos da sessao ficam com a atividade padrao do timer novo
        let atividade_id = self.atividade_padrao(Some(timer_id));
        if let Some(limite) = desde {
            // os abertos sao todos do dia corrente e de depois do "Desligar" (ele fecha o aberto)
            for t in self.abertos.iter_mut().filter(|t| t.sessao_id.as_deref() == Some(sessao_id)) {
                t.timer_id = Some(timer_id);
                t.atividade_id = atividade_id;
            }
            out.push(match limite {
                Some(desde) => Mudanca::ReatribuirDesde { sessao_id: sessao_id.to_string(), timer_id, desde },
                None => Mudanca::Reatribuir { sessao_id: sessao_id.to_string(), timer_id: Some(timer_id) },
            });
        } else {
            let reabrir: Vec<TrechoAberto> =
                self.abertos.iter().filter(|t| t.sessao_id.as_deref() == Some(sessao_id)).cloned().collect();
            out.extend(self.fechar_sessao(sessao_id, agora, MotivoFim::Desligada, false));
            for t in reabrir {
                out.push(Mudanca::Abriu {
                    timer_id: Some(timer_id),
                    sessao_id: t.sessao_id.clone(),
                    origem: t.origem,
                    inicio: agora,
                    atividade_id,
                });
                self.abertos.push(TrechoAberto { timer_id: Some(timer_id), inicio: agora, atividade_id, ..t });
            }
        }
        // Ligar a um timer encerrado uma sessao que esta contando: ele volta a rodar, ao normal.
        if self.abertos.iter().any(|t| t.timer_id == Some(timer_id)) {
            out.extend(self.desencerrar(Some(timer_id)));
        }
        out.push(Mudanca::Sessao(s));
        out
    }

    /// Desliga a sessao (decisao sua: a heranca nao a liga de novo).
    pub fn desligar_sessao(&mut self, sessao_id: &str, agora: Ms) -> Vec<Mudanca> {
        let mut out = self.corte_do_dia(agora);
        let Some(s) = self.sessoes.get_mut(sessao_id) else {
            return out;
        };
        s.timer_id = None;
        s.ligacao = Some(Ligacao::Mao);
        // o tempo de antes fica onde esta, mesmo ligando de novo "desde o inicio" (rodada 15)
        s.desligada_em = Some(agora);
        let s = s.clone();
        self.ligadas_sozinhas.remove(sessao_id);
        // volta a perguntar na proxima atividade (rodada 15)
        self.perguntadas.remove(sessao_id);
        out.extend(self.tirar_conversa_nova(sessao_id));
        out.extend(self.fechar_sessao(sessao_id, agora, MotivoFim::Desligada, false));
        out.push(Mudanca::Sessao(s));
        out
    }

    /// O timer que a sessao herda, se herda. Ela ainda nao foi mexida a mao (sem ligacao, ou
    /// ligada pela regra de pasta) e tem nome; outra sessao dos ultimos `HERANCA_MS`, criada antes
    /// dela, com o mesmo nome (sem diferenciar maiusculas, sem os espacos das pontas) e a mesma
    /// pasta, foi ligada a mao (ou herdou de uma ligada a mao) a um timer que ainda vale
    /// (`vale(timer)`: nao arquivado). Com varias, a de evento mais recente.
    pub fn heranca(&self, sessao_id: &str, agora: Ms, vale: impl Fn(i64) -> bool) -> Option<i64> {
        let s = self.sessoes.get(sessao_id)?;
        // mexida a mao, herdada ou deduzida (de antes da versao 4, sem saber se foi a mao): fica
        if matches!(s.ligacao, Some(Ligacao::Mao | Ligacao::Herdada | Ligacao::Deduzida)) {
            return None;
        }
        let nome = chave_de_nome(s.nome.as_deref())?;
        let pasta = chave_de_pasta(&s.cwd);
        // sem pasta (o Chat): cada conversa e uma so, sem heranca
        if pasta.is_empty() {
            return None;
        }
        self.sessoes
            .values()
            .filter(|o| {
                o.id != s.id
                    && matches!(o.ligacao, Some(Ligacao::Mao | Ligacao::Herdada))
                    && o.criada_em <= s.criada_em
                    && o.ultimo_evento >= agora - HERANCA_MS
                    && o.timer_id.is_some_and(&vale)
                    && !self.escondido(o.timer_id)
                    && chave_de_nome(o.nome.as_deref()).as_deref() == Some(nome.as_str())
                    && chave_de_pasta(&o.cwd) == pasta
            })
            .max_by(|a, b| a.ultimo_evento.cmp(&b.ultimo_evento).then(a.id.cmp(&b.id)))
            .and_then(|o| o.timer_id)
    }

    /// Conversa continuada (o Claude troca o id da sessao ao retomar ou compactar): a sessao herda
    /// o timer da anterior (`heranca`); a ligacao a mao vence a regra de pasta. Os trechos dela
    /// mudam de timer so a partir do inicio do dia de trabalho corrente: os dias anteriores nunca
    /// mudam sozinhos (podem ja ter sido lancados). Sem trecho hoje, so o timer da sessao muda (as
    /// proximas horas). A pergunta "Sessao nova…" dela, se havia, sai. `avisar`: o dock mostra
    /// "Ligada a X pela conversa anterior" (nao ao abrir o app).
    pub fn herdar(&mut self, sessao_id: &str, agora: Ms, avisar: bool, vale: impl Fn(i64) -> bool) -> Vec<Mudanca> {
        let Some(timer_id) = self.heranca(sessao_id, agora, vale) else {
            return vec![];
        };
        let Some(s) = self.sessoes.get_mut(sessao_id) else {
            return vec![];
        };
        if s.timer_id == Some(timer_id) {
            // a regra ja dava o mesmo timer: so a origem muda (e o aviso da regra, se havia, sai)
            s.ligacao = Some(Ligacao::Herdada);
            let s = s.clone();
            self.ligadas_sozinhas.remove(sessao_id);
            // a herdada nao pergunta "Certo?" (rodada 15)
            let mut out = vec![Mudanca::Sessao(s)];
            out.extend(self.tirar_conversa_nova(sessao_id));
            return out;
        }
        let mut out = self.corte_do_dia(agora);
        let desde = inicio_do_dia(self.dia, self.cfg.corte_min, self.cfg.fuso_min);
        let Some(s) = self.sessoes.get_mut(sessao_id) else {
            return out;
        };
        s.timer_id = Some(timer_id);
        s.ligacao = Some(Ligacao::Herdada);
        let s = s.clone();
        // os abertos sao todos do dia corrente (o corte do dia os divide); ficam com a atividade
        // padrao do timer novo, como no banco
        let atividade_id = self.atividade_padrao(Some(timer_id));
        for t in self.abertos.iter_mut().filter(|t| t.sessao_id.as_deref() == Some(sessao_id)) {
            t.timer_id = Some(timer_id);
            t.atividade_id = atividade_id;
        }
        out.push(Mudanca::ReatribuirDesde { sessao_id: sessao_id.to_string(), timer_id, desde });
        if self.abertos.iter().any(|t| t.timer_id == Some(timer_id)) {
            out.extend(self.desencerrar(Some(timer_id)));
        }
        out.push(Mudanca::Sessao(s));
        if let Some(pos) = self
            .perguntas
            .iter()
            .position(|(_, p)| matches!(p, Pergunta::SessaoNova { sessao_id: id, .. } if id == sessao_id))
        {
            let (id, _) = self.perguntas.remove(pos);
            out.push(Mudanca::PerguntaResolvida(id));
        }
        out.extend(self.tirar_conversa_nova(sessao_id));
        if avisar {
            self.ligadas_sozinhas
                .insert(sessao_id.to_string(), LigadaSozinha { timer_id, como: Ligacao::Herdada, em: agora });
        } else {
            self.ligadas_sozinhas.remove(sessao_id);
        }
        out
    }

    /// A pasta onde a sessao esta agora (o `cwd` do ultimo evento; sem evento com pasta, onde ela
    /// comecou). `cwd`: a pasta de inicio, se a sessao ja saiu do motor (a da pergunta).
    pub fn pasta_de_agora<'a>(&'a self, sessao_id: &str, cwd: &'a str) -> &'a str {
        let inicio = self.sessoes.get(sessao_id).map_or(cwd, |s| s.cwd.as_str());
        self.pasta_agora.get(sessao_id).map_or(inicio, String::as_str)
    }

    /// As pastas do "Sempre que for na pasta" da sessao (`pastas_entre`), da de inicio a de agora.
    /// `cwd`: a pasta de inicio, se a sessao ja saiu do motor (a da pergunta). O Chat: nenhuma.
    pub fn pastas_da_sessao(&self, sessao_id: &str, cwd: &str) -> Vec<String> {
        if super::e_sessao_do_chat(sessao_id) {
            return vec![];
        }
        let inicio = self.sessoes.get(sessao_id).map_or(cwd, |s| s.cwd.as_str());
        let perfil = std::env::var("USERPROFILE").ok();
        pastas_entre(inicio, self.pasta_de_agora(sessao_id, cwd), perfil.as_deref(), &self.cfg.pastas_ignoradas)
    }

    /// Avisos de ligada sozinha que valem em `agora`: a sessao segue no timer do aviso, com a
    /// mesma origem (movida a mao, o aviso sai).
    pub fn avisos_de_ligacao(&self, agora: Ms) -> Vec<(&Sessao, &LigadaSozinha)> {
        self.ligadas_sozinhas
            .iter()
            .filter(|(_, l)| l.em <= agora && agora < l.em + AVISO_LIGADA_MS)
            .filter_map(|(id, l)| {
                let s = self.sessoes.get(id)?;
                (s.timer_id == Some(l.timer_id) && s.ligacao == Some(l.como)).then_some((s, l))
            })
            .collect()
    }
}
// IMPL-FIM

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::{ConfigApp, ConfigMotor};
    use crate::tempo::ms_local;

    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms { ms_local(2026, 11, 17, hh, mm, F) }
    fn motor() -> Motor {
        let app = ConfigApp { pastas_ignoradas: vec!["G:\\pessoal".into()], ..ConfigApp::default() };
        let mut m = Motor::novo(ConfigMotor::de(&app, F), h(8, 0));
        m.regras_pasta = regras();
        m
    }
    fn regras() -> Vec<RegraPasta> {
        vec![
            RegraPasta { id: 1, pasta: "G:\\Empresa".into(), timer_id: 1 },
            RegraPasta { id: 2, pasta: "G:\\Empresa\\Clientes\\Cliente D".into(), timer_id: 7 },
        ]
    }
    fn ev(evento: &str, sessao: &str, cwd: &str, t: Ms) -> EventoHook {
        EventoHook { t, evento: evento.into(), sessao: sessao.into(), cwd: cwd.into(), transcript: None, notificacao: None, origem: None, conta: None }
    }
    fn trechos(v: &[Mudanca]) -> Vec<Mudanca> {
        v.iter().filter(|m| matches!(m, Mudanca::Abriu { .. } | Mudanca::Fechou { .. })).cloned().collect()
    }
    fn abriu(timer: Option<i64>, t: Ms) -> Mudanca {
        Mudanca::Abriu { timer_id: timer, sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: t, atividade_id: 1 }
    }
    fn fechou(timer: Option<i64>, t: Ms, motivo: MotivoFim) -> Mudanca {
        Mudanca::Fechou { timer_id: timer, sessao_id: Some("s1".into()), origem: Origem::Claude, fim: t, motivo }
    }
    const CLIENTE_D: &str = "G:\\Empresa\\Clientes\\Cliente D\\sub";

    #[test] fn prompt_abre_e_stop_fecha_trecho_claude() {
        let mut m = motor();
        assert_eq!(trechos(&m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)))), vec![abriu(Some(7), h(9, 0))]);
        assert_eq!(trechos(&m.evento(&ev("Stop", "s1", CLIENTE_D, h(9, 8)))), vec![fechou(Some(7), h(9, 8), MotivoFim::Terminou)]);
        let s = &m.sessoes["s1"];
        assert_eq!(s.estado, EstadoSessao::EsperandoVoce);
        assert_eq!(s.terminou_em, Some(h(9, 8)));
    }
    #[test] fn a_sessao_guarda_a_primeira_origem_e_a_pasta_de_agora_so_em_memoria() {
        let mut m = motor();
        let com = |evento: &str, cwd: &str, t: Ms, origem: Option<&str>| EventoHook { origem: origem.map(str::to_string), ..ev(evento, "s1", cwd, t) };
        m.evento(&com("SessionStart", "G:\\Empresa\\app", h(9, 0), None));
        assert_eq!(m.sessoes["s1"].origem, None);
        m.evento(&com("UserPromptSubmit", "G:\\Empresa\\app", h(9, 1), Some("")));
        assert_eq!(m.sessoes["s1"].origem, None, "vazia não conta");
        m.evento(&com("PostToolUse", "G:\\Empresa\\app\\web", h(9, 2), Some("claude-desktop")));
        m.evento(&com("Stop", "G:\\Empresa\\app\\web\\src", h(9, 3), Some("cli")));
        let s = &m.sessoes["s1"];
        assert_eq!(s.origem.as_deref(), Some("claude-desktop"), "a primeira não vazia fica");
        assert_eq!(s.cwd, "G:\\Empresa\\app", "o cwd gravado é onde ela começou");
        assert_eq!(m.pasta_agora["s1"], "G:\\Empresa\\app\\web\\src");
        // o evento do Chat traz a origem "chat"
        m.sinais_do_chat(
            &super::super::SinaisDoChat {
                aba_do_chat: true,
                aberta: Some(super::super::ChatAberto { id: "0a1b".into(), titulo: "Receita".into(), parando: true }),
                rodando: vec![],
                barra: true,
                repetidos: vec![],
            },
            h(9, 4),
        );
        assert_eq!(m.sessoes["chat:0a1b"].origem.as_deref(), Some("chat"));
        assert!(!m.pasta_agora.contains_key("chat:0a1b"), "o Chat não tem pasta");
    }
    #[test] fn pastas_entre_vao_da_de_inicio_ate_a_de_agora() {
        let p = |ini: &str, agora: &str| pastas_entre(ini, agora, Some("C:\\Users\\voce"), &["G:\\pessoal".into()]);
        assert_eq!(
            p("D:\\Projetos", "D:\\Projetos\\cliente\\app"),
            ["D:\\Projetos", "D:\\Projetos\\cliente", "D:\\Projetos\\cliente\\app"]
        );
        // a mesma pasta (com outra escrita): uma só
        assert_eq!(p("D:\\Projetos\\app", "d:/projetos/app/"), ["D:\\Projetos\\app"]);
        assert_eq!(p("D:\\Projetos\\app", ""), ["D:\\Projetos\\app"]);
        // fora da de início: só as duas
        assert_eq!(p("D:\\Projetos\\app", "E:\\Outra\\coisa"), ["D:\\Projetos\\app", "E:\\Outra\\coisa"]);
        assert_eq!(p("D:\\Projetos\\app\\web", "D:\\Projetos\\app"), ["D:\\Projetos\\app\\web", "D:\\Projetos\\app"]);
        // um worktree vira o projeto
        assert_eq!(p("D:\\Projetos", "D:\\Projetos\\app\\.claude\\worktrees\\feat-1"), ["D:\\Projetos", "D:\\Projetos\\app"]);
        assert_eq!(p("D:\\Projetos\\app", "D:\\Projetos\\app\\.claude\\worktrees\\feat-1"), ["D:\\Projetos\\app"]);
        // as barras de quem veio
        assert_eq!(p("D:/Projetos", "D:/Projetos/app"), ["D:/Projetos", "D:/Projetos/app"]);
        // sem regra possível: o drive, a pasta do usuário e a que nunca conta ficam de fora
        assert_eq!(p("D:\\", "D:\\Projetos\\app"), ["D:\\Projetos", "D:\\Projetos\\app"]);
        assert_eq!(p("C:\\Users\\voce", "C:\\Users\\voce\\app"), ["C:\\Users\\voce\\app"]);
        assert_eq!(p("G:\\", "G:\\pessoal\\notas"), Vec::<String>::new());
        // revisão M7: um worktree dentro da pasta do usuário fica como está (a raiz é larga), e
        // os níveis da `.claude` não entram
        assert_eq!(p("C:\\Users\\voce", "C:\\Users\\voce\\.claude\\worktrees\\x"), Vec::<String>::new());
        assert_eq!(p("C:\\Users\\voce\\app", "C:\\Users\\voce\\app\\.Claude\\outra"), ["C:\\Users\\voce\\app"]);
        assert_eq!(p("D:\\Projetos\\.claude", "D:\\Projetos\\.claude"), Vec::<String>::new());
        // sem pasta (o Chat): nenhuma
        assert!(p("", "").is_empty());
        assert!(p("  ", "D:\\x").is_empty());
    }
    #[test] fn pastas_da_sessao_usam_o_ultimo_cwd_e_o_chat_nao_tem() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", "G:\\Empresa", h(9, 0)));
        assert_eq!(m.pastas_da_sessao("s1", ""), ["G:\\Empresa"]);
        m.evento(&ev("PostToolUse", "s1", "G:\\Empresa\\Clientes\\Cliente D", h(9, 1)));
        assert_eq!(m.pasta_de_agora("s1", ""), "G:\\Empresa\\Clientes\\Cliente D");
        assert_eq!(m.pastas_da_sessao("s1", ""), ["G:\\Empresa", "G:\\Empresa\\Clientes", "G:\\Empresa\\Clientes\\Cliente D"]);
        // a sessão que saiu do motor: pela pasta da pergunta
        assert_eq!(m.pastas_da_sessao("sumiu", "G:\\Empresa\\x"), ["G:\\Empresa\\x"]);
        assert!(m.pastas_da_sessao("chat:0a1b", "").is_empty());
    }
    #[test] fn raiz_do_projeto_tira_o_worktree() {
        assert_eq!(
            raiz_do_projeto("G:\\Empresa\\app-web\\.claude\\worktrees\\feat-029-pagina-do-projeto-57b711"),
            "G:\\Empresa\\app-web"
        );
        assert_eq!(raiz_do_projeto("G:/Empresa/Flow/.claude/worktrees/x/sub"), "G:/Empresa/Flow");
        assert_eq!(raiz_do_projeto("G:\\Flow\\.Claude\\Worktrees\\x"), "G:\\Flow");
        assert_eq!(raiz_do_projeto("G:\\Flow\\.claude\\worktrees"), "G:\\Flow");
        // fora de worktree, ou so parecido: a propria pasta
        for igual in ["G:\\Flow", "G:\\Flow\\.claude", "G:\\Flow\\.claude\\worktreesx\\a", "D:\\Ação\\x", ""] {
            assert_eq!(raiz_do_projeto(igual), igual);
        }
        assert_eq!(raiz_do_projeto("D:\\Ação\\.claude\\worktrees\\é"), "D:\\Ação");
    }
    #[test] fn raiz_nunca_e_o_drive_nem_a_pasta_do_usuario() {
        let perfil = Some("C:\\Users\\eu");
        for fica in [
            "G:\\.claude\\worktrees\\x",
            "G:/.claude/worktrees/x",
            "C:\\Users\\eu\\.claude\\worktrees\\x",
            "D:\\Users\\outra\\.claude\\worktrees\\x",
        ] {
            assert_eq!(raiz_com_perfil(fica, perfil), fica);
        }
        // perfil fora de `Users` (ex.: pasta movida)
        assert_eq!(raiz_com_perfil("E:\\Perfis\\eu\\.claude\\worktrees\\x", Some("e:/perfis/eu/")), "E:\\Perfis\\eu\\.claude\\worktrees\\x");
        // um projeto dentro da pasta do usuario continua indo para a raiz do projeto
        assert_eq!(raiz_com_perfil("C:\\Users\\eu\\proj\\.claude\\worktrees\\x", perfil), "C:\\Users\\eu\\proj");
        assert_eq!(raiz_com_perfil("G:\\Projeto\\.claude\\worktrees\\x", None), "G:\\Projeto");
    }
    #[test] fn sessao_num_worktree_sugere_o_timer_pela_raiz() {
        let mut m = motor();
        let v = m.evento(&ev("UserPromptSubmit", "s1", "D:\\Projetos\\Flow\\.claude\\worktrees\\feat-1", h(9, 0)));
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::SessaoNova { sugestao, .. }) if sugestao == "Flow")));
        // regra na raiz vale para os worktrees
        let r = vec![RegraPasta { id: 1, pasta: "D:\\Projetos\\Flow".into(), timer_id: 3 }];
        assert_eq!(regra_para(&r, "D:\\Projetos\\Flow\\.claude\\worktrees\\feat-2"), Some(3));
    }
    #[test] fn regra_de_pasta_mais_longa_vence() {
        let r = regras();
        assert_eq!(regra_para(&r, "g:/empresa/clientes/cliente d/x"), Some(7));
        assert_eq!(regra_para(&r, "G:\\Empresa\\Flow"), Some(1));
        assert_eq!(regra_para(&r, "D:\\outro"), None);
        assert_eq!(regra_para(&r, "G:\\Empresa2"), None);
    }
    #[test] fn sessao_sem_regra_pergunta_uma_vez_e_conta_sem_timer() {
        let mut m = motor();
        let v = m.evento(&ev("UserPromptSubmit", "s1", "D:\\Projetos\\Novo", h(9, 0)));
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::SessaoNova { sugestao, .. }) if sugestao == "Novo")));
        assert_eq!(trechos(&v), vec![abriu(None, h(9, 0))]);
        let v = m.evento(&ev("PostToolUse", "s1", "D:\\Projetos\\Novo", h(9, 1)));
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Pergunta(..))));
    }
    #[test] fn session_start_nao_marca_esperando_voce() {
        let mut m = motor();
        let v = m.evento(&ev("SessionStart", "s1", CLIENTE_D, h(9, 0)));
        assert_eq!(trechos(&v), vec![]);
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::Parada);
        assert_eq!(m.sessoes["s1"].terminou_em, None);
        m.evento(&ev("SessionStart", "s1", CLIENTE_D, h(9, 1)));
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::Parada);
    }
    #[test] fn sessao_sem_regra_so_pergunta_quando_trabalha() {
        let mut m = motor();
        let v = m.evento(&ev("SessionStart", "s1", "D:\\Projetos\\Novo", h(9, 0)));
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Pergunta(..))));
        let v = m.evento(&ev("UserPromptSubmit", "s1", "D:\\Projetos\\Novo", h(9, 1)));
        assert_eq!(v.iter().filter(|x| matches!(x, Mudanca::Pergunta(..))).count(), 1);
        let v = m.evento(&ev("UserPromptSubmit", "s1", "D:\\Projetos\\Novo", h(9, 2)));
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Pergunta(..))));
    }
    #[test] fn pasta_ignorada_nao_cria_sessao() {
        let mut m = motor();
        assert_eq!(m.evento(&ev("UserPromptSubmit", "s1", "G:\\pessoal\\x", h(9, 0))), vec![]);
        assert!(m.sessoes.is_empty());
    }
    #[test] fn dez_minutos_sem_evento_vira_parada() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)));
        m.evento(&ev("PostToolUse", "s1", CLIENTE_D, h(9, 4)));
        let v = m.tique(h(9, 15));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(9, 4), MotivoFim::SessaoParou)]);
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::Parada);
    }
    #[test] fn interrupcao_fecha_o_trecho_no_instante_e_um_evento_novo_reabre() {
        let mut m = motor();
        let mut mud = m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(8, 53)));
        mud.extend(m.evento(&ev("PostToolUse", "s1", CLIENTE_D, h(8, 54))));
        // interrupção antiga (antes do último evento) não vale
        assert!(m.interromper("s1", h(8, 54)).is_empty());
        assert!(m.interromper("s1", h(8, 53)).is_empty());
        let v = m.interromper("s1", h(8, 55));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(8, 55), MotivoFim::Interrompida)]);
        let s = &m.sessoes["s1"];
        assert_eq!((s.estado, s.terminou_em), (EstadoSessao::EsperandoVoce, Some(h(8, 55))));
        assert!(m.abertos.is_empty());
        // de novo: a sessão já não trabalha
        assert!(m.interromper("s1", h(8, 56)).is_empty());
        // a mensagem seguinte reabre o trecho
        let v = m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(8, 58)));
        assert_eq!(trechos(&v), vec![abriu(Some(7), h(8, 58))]);
        assert_eq!(MotivoFim::do_banco("interrompida"), MotivoFim::Interrompida);
        // sessão que não existe: nada
        assert!(m.interromper("x", h(9, 0)).is_empty());
    }

    #[test] fn segundo_plano_so_esperando_voce_e_ate_a_tarefa_vencer() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)));
        m.segundo_plano.insert("s1".into(), h(11, 0));
        assert!(!m.em_segundo_plano(&m.sessoes["s1"], h(9, 1)), "trabalhando não é segundo plano");
        m.evento(&ev("Stop", "s1", CLIENTE_D, h(9, 5)));
        assert!(m.em_segundo_plano(&m.sessoes["s1"], h(10, 59)));
        assert!(!m.em_segundo_plano(&m.sessoes["s1"], h(11, 0)), "a tarefa venceu (2 h do lançamento)");
        // o fim de turno não apaga a marca: quem a refaz é o app, pelo transcript
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 10)));
        m.evento(&ev("Stop", "s1", CLIENTE_D, h(9, 12)));
        assert!(m.em_segundo_plano(&m.sessoes["s1"], h(9, 13)));
    }

    #[test] fn interrupcao_logo_depois_da_meia_noite_divide_o_trecho_no_corte() {
        let mut m = motor();
        let ontem = |hh, mm| ms_local(2026, 11, 17, hh, mm, F);
        let hoje = |hh, mm| ms_local(2026, 11, 18, hh, mm, F);
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, ontem(23, 30)));
        m.evento(&ev("PostToolUse", "s1", CLIENTE_D, ontem(23, 59)));
        let v = m.interromper("s1", hoje(0, 0) + 1_000);
        assert_eq!(
            trechos(&v),
            vec![fechou(Some(7), hoje(0, 0), MotivoFim::CorteDoDia), abriu(Some(7), hoje(0, 0)), fechou(Some(7), hoje(0, 0) + 1_000, MotivoFim::Interrompida)]
        );
        // 10 min sem evento antes do stop: vale a regra de sempre (fecha no último evento)
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)));
        let v = m.interromper("s1", h(9, 20));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(9, 0), MotivoFim::SessaoParou)]);
    }

    #[test] fn stop_duplicado_e_evento_atrasado_sao_ignorados() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)));
        m.evento(&ev("Stop", "s1", CLIENTE_D, h(9, 8)));
        assert_eq!(trechos(&m.evento(&ev("Stop", "s1", CLIENTE_D, h(9, 8)))), vec![]);
        assert_eq!(trechos(&m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 5)))), vec![]);
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::EsperandoVoce);
    }
    #[test] fn notification_espera_voce_e_posttooluse_retoma() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)));
        assert_eq!(trechos(&m.evento(&ev("Notification", "s1", CLIENTE_D, h(9, 2)))), vec![fechou(Some(7), h(9, 2), MotivoFim::Terminou)]);
        assert_eq!(trechos(&m.evento(&ev("PostToolUse", "s1", CLIENTE_D, h(9, 5)))), vec![abriu(Some(7), h(9, 5))]);
    }
    #[test] fn pausar_bloqueia_ate_proxima_mensagem() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)));
        assert_eq!(trechos(&m.pausar(7, h(9, 10))), vec![fechou(Some(7), h(9, 10), MotivoFim::Pausa)]);
        assert_eq!(trechos(&m.evento(&ev("PostToolUse", "s1", CLIENTE_D, h(9, 12)))), vec![]);
        assert_eq!(trechos(&m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 20)))), vec![abriu(Some(7), h(9, 20))]);
    }
    #[test] fn ligar_desde_inicio_reatribui() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", "D:\\x", h(9, 0)));
        let v = m.ligar_sessao("s1", 3, true, h(9, 5));
        assert!(v.contains(&Mudanca::Reatribuir { sessao_id: "s1".into(), timer_id: Some(3) }));
        assert_eq!(m.abertos[0].timer_id, Some(3));
        assert_eq!(m.sessoes["s1"].timer_id, Some(3));
    }
    #[test] fn ligar_daqui_pra_frente_fecha_e_reabre() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", "D:\\x", h(9, 0)));
        let v = m.ligar_sessao("s1", 3, false, h(9, 5));
        assert_eq!(trechos(&v), vec![fechou(None, h(9, 5), MotivoFim::Desligada), abriu(Some(3), h(9, 5))]);
        assert_eq!(m.abertos[0].timer_id, Some(3));
    }
    #[test] fn desligar_fecha_e_zera_timer() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)));
        let v = m.desligar_sessao("s1", h(9, 5));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(9, 5), MotivoFim::Desligada)]);
        assert_eq!(m.sessoes["s1"].timer_id, None);
    }
    #[test] fn responder_ligar_a_liga_desde_o_inicio() {
        use crate::modelo::Resposta;
        let mut m = motor();
        let v = m.evento(&ev("UserPromptSubmit", "s1", "D:\\x", h(9, 0)));
        let id = v.iter().find_map(|x| if let Mudanca::Pergunta(i, _) = x { Some(*i) } else { None }).unwrap();
        let r = m.responder(id, crate::modelo::Resposta::LigarA { timer_id: 3, criar_regra: true }, h(9, 2));
        assert!(r.contains(&Mudanca::Reatribuir { sessao_id: "s1".into(), timer_id: Some(3) }));
        assert!(r.contains(&Mudanca::PerguntaResolvida(id)));
        assert!(m.perguntas.is_empty());
        let v = m.evento(&ev("UserPromptSubmit", "s2", "D:\\y", h(9, 3)));
        let id = v.iter().find_map(|x| if let Mudanca::Pergunta(i, _) = x { Some(*i) } else { None }).unwrap();
        let v = m.responder(id, Resposta::NaoContar, h(9, 4));
        assert_eq!(v.last(), Some(&Mudanca::PerguntaResolvida(id)));
        assert_eq!(m.sessoes["s2"].timer_id, None);
        assert_eq!(m.sessoes["s2"].ligacao, Some(Ligacao::Mao), "nao contar e decisao sua");
    }
    #[test] fn corte_do_dia_divide_trecho_claude() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(23, 50)));
        m.evento(&ev("PostToolUse", "s1", CLIENTE_D, h(23, 58))); // sessao viva: so 3 min sem evento no tique
        let corte = ms_local(2026, 11, 18, 0, 0, F);
        let v = m.tique(corte + 60_000);
        assert_eq!(trechos(&v), vec![fechou(Some(7), corte, MotivoFim::CorteDoDia), abriu(Some(7), corte)]);
    }
    #[test] fn session_end_fecha_tudo() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 0)));
        let v = m.evento(&ev("SessionEnd", "s1", CLIENTE_D, h(9, 5)));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(9, 5), MotivoFim::SessaoParou)]);
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::Parada);
        assert!(m.abertos.is_empty());
    }
    // I1: fila lida de uma vez (app fechado, PC dormindo), sem tique entre os eventos.
    #[test] fn stop_perdido_fecha_no_ultimo_evento_mesmo_sem_tique() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(10, 0)));
        let v = m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(15, 0)));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(10, 0), MotivoFim::SessaoParou), abriu(Some(7), h(15, 0))]);
        let v = m.evento(&ev("Stop", "s1", CLIENTE_D, h(15, 8)));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(15, 8), MotivoFim::Terminou)]);
    }
    #[test] fn posttooluse_depois_de_horas_nao_conta_o_buraco() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(10, 0)));
        m.evento(&ev("PostToolUse", "s1", CLIENTE_D, h(10, 5)));
        let v = m.evento(&ev("PostToolUse", "s1", CLIENTE_D, h(15, 0)));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(10, 5), MotivoFim::SessaoParou), abriu(Some(7), h(15, 0))]);
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::Trabalhando);
    }
    #[test] fn evento_de_outra_sessao_tambem_fecha_a_que_ficou_sem_evento() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(10, 0)));
        let v = m.evento(&ev("UserPromptSubmit", "s2", CLIENTE_D, h(10, 30)));
        assert!(trechos(&v).contains(&fechou(Some(7), h(10, 0), MotivoFim::SessaoParou)));
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::Parada);
        // o Stop atrasado da s1 nao reabre nem fecha nada
        assert_eq!(trechos(&m.evento(&ev("Stop", "s1", CLIENTE_D, h(10, 31)))), vec![]);
    }
    #[test] fn stop_perdido_antes_da_meia_noite_nao_passa_para_o_dia_novo() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(23, 0)));
        let t = ms_local(2026, 11, 18, 8, 0, F);
        let v = m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, t));
        assert_eq!(trechos(&v), vec![fechou(Some(7), h(23, 0), MotivoFim::SessaoParou), abriu(Some(7), t)]);
    }
    // ---- conversa continuada (rodada 7) ----

    const FLOW: &str = "G:\\Empresa\\app-web";
    /// Regra da pasta do Flow (timer 1) e a sessao "5ee991" la, ligada sozinha a Flow e depois
    /// movida a mao para Cliente B (timer 9).
    fn caso_do_cliente_b() -> Motor {
        let mut m = motor();
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: FLOW.into(), timer_id: 1 }];
        m.evento(&ev("UserPromptSubmit", "5ee991", FLOW, h(10, 0)));
        assert_eq!(m.sessoes["5ee991"].ligacao, Some(Ligacao::Regra));
        m.sessoes.get_mut("5ee991").unwrap().nome = Some("Projeto: avaliação da demo do Cliente B".into());
        m.ligar_sessao("5ee991", 9, true, h(10, 48));
        assert_eq!(m.sessoes["5ee991"].ligacao, Some(Ligacao::Mao));
        m.evento(&ev("Stop", "5ee991", FLOW, h(11, 0)));
        m
    }
    fn nomear(m: &mut Motor, id: &str, nome: &str) {
        m.sessoes.get_mut(id).unwrap().nome = Some(nome.into());
    }
    #[test] fn conversa_continuada_herda_o_timer_ligado_a_mao_desde_o_inicio() {
        let mut m = caso_do_cliente_b();
        // 11:35: a conversa continua numa sessao nova; a regra a liga a Flow
        let v = m.evento(&ev("UserPromptSubmit", "536b82", FLOW, h(11, 35)));
        assert_eq!(trechos(&v)[0], Mudanca::Abriu { timer_id: Some(1), sessao_id: Some("536b82".into()), origem: Origem::Claude, inicio: h(11, 35), atividade_id: 1 });
        // o titulo chega depois (lido do transcript), com outras maiusculas e espacos
        nomear(&mut m, "536b82", "  projeto: Avaliação da demo do Cliente B ");
        let v = m.herdar("536b82", h(11, 36), true, |_| true);
        // desde o inicio do dia de trabalho (os dias anteriores nunca mudam sozinhos)
        assert!(v.contains(&Mudanca::ReatribuirDesde { sessao_id: "536b82".into(), timer_id: 9, desde: h(0, 0) }));
        let s = &m.sessoes["536b82"];
        assert_eq!((s.timer_id, s.ligacao), (Some(9), Some(Ligacao::Herdada)));
        assert_eq!(m.abertos[0].timer_id, Some(9));
        // a herdada nao pergunta "Certo?" (a pergunta da regra saiu)
        assert!(!m.perguntas.iter().any(|(_, p)| matches!(p, Pergunta::ConversaNova { sessao_id, .. } if sessao_id == "536b82")));
        // o aviso diz que foi pela conversa anterior
        let a = m.avisos_de_ligacao(h(11, 36));
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].1.timer_id, a[0].1.como), (9, Ligacao::Herdada));
        // a proxima continuacao herda da herdada
        m.evento(&ev("UserPromptSubmit", "c3", FLOW, h(14, 0)));
        nomear(&mut m, "c3", "Projeto: avaliação da demo do Cliente B");
        m.herdar("c3", h(14, 1), true, |_| true);
        assert_eq!(m.sessoes["c3"].timer_id, Some(9));
    }
    #[test] fn heranca_so_com_mesmo_nome_mesma_pasta_e_sem_mexer_a_mao() {
        let mut m = caso_do_cliente_b();
        // outra pasta (um worktree e outra pasta, mesmo com a regra na raiz)
        m.evento(&ev("UserPromptSubmit", "wt", &format!("{FLOW}\\.claude\\worktrees\\x"), h(11, 30)));
        nomear(&mut m, "wt", "Projeto: avaliação da demo do Cliente B");
        assert_eq!(m.heranca("wt", h(11, 31), |_| true), None);
        // outro nome
        m.evento(&ev("UserPromptSubmit", "outra", FLOW, h(11, 30)));
        nomear(&mut m, "outra", "Projeto: outra coisa");
        assert_eq!(m.heranca("outra", h(11, 31), |_| true), None);
        // sem nome ainda
        m.evento(&ev("UserPromptSubmit", "sem", FLOW, h(11, 30)));
        assert_eq!(m.heranca("sem", h(11, 31), |_| true), None);
        // ja mexida a mao: nada muda
        m.evento(&ev("UserPromptSubmit", "mexida", FLOW, h(11, 30)));
        m.ligar_sessao("mexida", 1, false, h(11, 31));
        nomear(&mut m, "mexida", "Projeto: avaliação da demo do Cliente B");
        assert!(m.herdar("mexida", h(11, 32), true, |_| true).is_empty());
        assert_eq!(m.sessoes["mexida"].timer_id, Some(1));
        // timer da anterior arquivado: nao herda
        m.evento(&ev("UserPromptSubmit", "arq", FLOW, h(11, 29)));
        nomear(&mut m, "arq", "Projeto: avaliação da demo do Cliente B");
        assert_eq!(m.heranca("arq", h(11, 31), |t| t != 9), None);
        // deduzida (de antes da versao 4, sem prova de movimento a mao): nao passa adiante nem herda
        m.sessoes.get_mut("5ee991").unwrap().ligacao = Some(Ligacao::Deduzida);
        assert_eq!(m.heranca("arq", h(11, 31), |_| true), None);
        m.sessoes.get_mut("arq").unwrap().ligacao = Some(Ligacao::Deduzida);
        m.sessoes.get_mut("5ee991").unwrap().ligacao = Some(Ligacao::Mao);
        assert_eq!(m.heranca("arq", h(11, 31), |_| true), None);
        m.sessoes.get_mut("arq").unwrap().ligacao = Some(Ligacao::Regra);
        // a anterior de mais de 7 dias: nao herda
        let depois = h(11, 0) + HERANCA_MS + MIN;
        assert_eq!(m.heranca("arq", depois, |_| true), None);
        assert_eq!(m.heranca("arq", h(11, 31), |_| true), Some(9));
    }
    #[test] fn ligada_sozinha_pela_regra_nao_passa_adiante() {
        let mut m = motor();
        let regras = vec![RegraPasta { id: 1, pasta: FLOW.into(), timer_id: 1 }];
        m.regras_pasta = regras;
        m.evento(&ev("UserPromptSubmit", "a", FLOW, h(9, 0)));
        nomear(&mut m, "a", "Mesmo nome");
        m.evento(&ev("UserPromptSubmit", "b", FLOW, h(10, 0)));
        nomear(&mut m, "b", "Mesmo nome");
        assert_eq!(m.heranca("b", h(10, 1), |_| true), None, "a anterior so foi ligada pela regra");
    }
    #[test] fn heranca_resolve_a_pergunta_da_sessao_sem_regra() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "a", "D:\\Novo", h(9, 0)));
        let id = m.perguntas[0].0;
        m.responder(id, crate::modelo::Resposta::LigarA { timer_id: 3, criar_regra: false }, h(9, 1));
        nomear(&mut m, "a", "Conversa");
        m.evento(&ev("UserPromptSubmit", "b", "d:/novo", h(10, 0)));
        let id = m.perguntas[0].0;
        nomear(&mut m, "b", "conversa");
        let v = m.herdar("b", h(10, 1), true, |_| true);
        assert!(v.contains(&Mudanca::PerguntaResolvida(id)));
        assert!(m.perguntas.is_empty());
        assert_eq!(m.sessoes["b"].timer_id, Some(3));
    }
    #[test] fn nao_contar_e_desligar_sao_decisoes_suas() {
        let mut m = caso_do_cliente_b();
        m.evento(&ev("UserPromptSubmit", "x", "D:\\Novo", h(9, 0)));
        let id = m.perguntas[0].0;
        let v = m.responder(id, crate::modelo::Resposta::NaoContar, h(9, 1));
        assert!(v.iter().any(|x| matches!(x, Mudanca::Sessao(s) if s.ligacao == Some(Ligacao::Mao))));
        // desligada a mao: a heranca nao a religa
        m.evento(&ev("UserPromptSubmit", "y", FLOW, h(12, 0)));
        m.desligar_sessao("y", h(12, 1));
        nomear(&mut m, "y", "Projeto: avaliação da demo do Cliente B");
        assert_eq!(m.heranca("y", h(12, 2), |_| true), None);
    }
    #[test] fn regra_avisa_uma_vez_quando_a_sessao_comeca_a_contar() {
        let mut m = motor();
        // "Usar a regra direto": o aviso da rodada 7, sem a pergunta
        m.cfg.confirmar_conversa_nova = false;
        m.evento(&ev("SessionStart", "s1", CLIENTE_D, h(9, 0)));
        assert!(m.avisos_de_ligacao(h(9, 0)).is_empty(), "so abriu: ainda nao conta");
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 1)));
        let a = m.avisos_de_ligacao(h(9, 1) + 5_000);
        assert_eq!((a.len(), a[0].1.timer_id, a[0].1.como), (1, 7, Ligacao::Regra));
        assert!(m.avisos_de_ligacao(h(9, 1) + AVISO_LIGADA_MS).is_empty(), "vence em 15 s");
        // movida a mao: o aviso sai
        m.evento(&ev("UserPromptSubmit", "s2", CLIENTE_D, h(9, 2)));
        m.ligar_sessao("s2", 3, false, h(9, 2) + 1_000);
        assert!(m.avisos_de_ligacao(h(9, 2) + 2_000).is_empty());
        // evento antigo (a fila lida depois): o aviso ja nasce vencido
        m.evento(&ev("UserPromptSubmit", "s3", CLIENTE_D, h(9, 3)));
        assert!(m.avisos_de_ligacao(h(9, 30)).is_empty());
        m.tique(h(9, 30));
        assert!(m.ligadas_sozinhas.is_empty());
    }
    fn conversas_novas(m: &Motor) -> Vec<(u64, String, i64)> {
        m.perguntas
            .iter()
            .filter_map(|(id, p)| match p {
                Pergunta::ConversaNova { sessao_id, timer_id, .. } => Some((*id, sessao_id.clone(), *timer_id)),
                _ => None,
            })
            .collect()
    }
    #[test] fn conversa_nova_numa_pasta_com_regra_conta_e_pergunta() {
        let mut m = motor();
        m.evento(&ev("SessionStart", "s1", CLIENTE_D, h(9, 0)));
        assert!(conversas_novas(&m).is_empty(), "so abriu: ainda nao pergunta");
        let v = m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 1)));
        // ja conta no timer da regra e pergunta (sem o aviso de ligada sozinha)
        assert_eq!(trechos(&v)[0], Mudanca::Abriu { timer_id: Some(7), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(9, 1), atividade_id: 1 });
        let p = conversas_novas(&m);
        assert_eq!((p.len(), p[0].1.as_str(), p[0].2), (1, "s1", 7));
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(..))));
        assert!(m.avisos_de_ligacao(h(9, 1)).is_empty());
        // a sessao ja com timer nao pergunta de novo
        m.evento(&ev("Stop", "s1", CLIENTE_D, h(9, 5)));
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 6)));
        assert_eq!(conversas_novas(&m).len(), 1);
        // pasta sem regra: a pergunta de sessao nova, nao esta
        m.evento(&ev("UserPromptSubmit", "s2", "D:\\Outro", h(9, 7)));
        assert_eq!(conversas_novas(&m).len(), 1);
        // pasta ignorada: nada
        m.evento(&ev("UserPromptSubmit", "s3", "G:\\pessoal\\x", h(9, 8)));
        assert_eq!(conversas_novas(&m).len(), 1);
    }
    #[test] fn conversa_nova_certo_fica_e_sem_resposta_tambem() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 1)));
        let id = conversas_novas(&m)[0].0;
        let v = m.responder(id, crate::modelo::Resposta::Certo, h(9, 2));
        assert!(v.contains(&Mudanca::PerguntaResolvida(id)));
        assert!(conversas_novas(&m).is_empty());
        let s = &m.sessoes["s1"];
        assert_eq!((s.timer_id, s.ligacao), (Some(7), Some(Ligacao::Regra)));
        // sem resposta: continua no timer da regra (a pergunta so fica la)
        m.evento(&ev("UserPromptSubmit", "s2", CLIENTE_D, h(9, 3)));
        m.tique(h(12, 0));
        assert_eq!(m.sessoes["s2"].timer_id, Some(7));
    }
    #[test] fn conversa_nova_outro_timer_move_desde_o_inicio() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 1)));
        let id = conversas_novas(&m)[0].0;
        let v = m.responder(id, crate::modelo::Resposta::LigarA { timer_id: 3, criar_regra: false }, h(9, 20));
        // so os trechos do dia de trabalho atual (revisao da rodada 15)
        assert!(v.contains(&Mudanca::ReatribuirDesde { sessao_id: "s1".into(), timer_id: 3, desde: h(0, 0) }));
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Reatribuir { .. })));
        assert!(v.contains(&Mudanca::PerguntaResolvida(id)));
        let s = &m.sessoes["s1"];
        assert_eq!((s.timer_id, s.ligacao), (Some(3), Some(Ligacao::Mao)));
        assert_eq!(m.abertos[0].timer_id, Some(3));
        assert!(conversas_novas(&m).is_empty());
        // ligar pelo "Outro timer…" (ligar_sessao direto) tambem tira a pergunta
        m.evento(&ev("UserPromptSubmit", "s2", CLIENTE_D, h(9, 30)));
        assert_eq!(conversas_novas(&m).len(), 1);
        let v = m.ligar_sessao("s2", 3, true, h(9, 31));
        assert!(v.iter().any(|x| matches!(x, Mudanca::PerguntaResolvida(_))));
        assert!(conversas_novas(&m).is_empty());
    }
    #[test] fn conversa_nova_usar_a_regra_direto_nao_pergunta() {
        let mut m = motor();
        m.cfg.confirmar_conversa_nova = false;
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 1)));
        assert!(conversas_novas(&m).is_empty());
        assert_eq!(m.sessoes["s1"].timer_id, Some(7));
        assert_eq!(m.avisos_de_ligacao(h(9, 1)).len(), 1, "o aviso da rodada 7");
    }
    #[test] fn desligada_e_religada_desde_o_inicio_move_so_o_que_veio_depois() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 1)));
        m.desligar_sessao("s1", h(9, 30));
        assert_eq!(m.sessoes["s1"].desligada_em, Some(h(9, 30)));
        // o "Ligar" de sempre, "desde o inicio": so de depois do desligar
        let v = m.ligar_sessao("s1", 3, true, h(10, 0));
        assert!(v.contains(&Mudanca::ReatribuirDesde { sessao_id: "s1".into(), timer_id: 3, desde: h(9, 30) }));
        // o "so hoje" depois de um desligar de ontem: o inicio de hoje
        m.sessoes.get_mut("s1").unwrap().desligada_em = Some(h(0, 0) - 60 * MIN);
        let v = m.ligar_sessao_hoje("s1", 4, h(10, 5));
        assert!(v.contains(&Mudanca::ReatribuirDesde { sessao_id: "s1".into(), timer_id: 4, desde: h(0, 0) }));
        // sem desligar, o "Ligar" de sempre leva tudo
        m.evento(&ev("UserPromptSubmit", "s2", CLIENTE_D, h(10, 10)));
        let v = m.ligar_sessao("s2", 3, true, h(10, 11));
        assert!(v.contains(&Mudanca::Reatribuir { sessao_id: "s2".into(), timer_id: Some(3) }));
    }
    #[test] fn desligada_a_mao_volta_a_perguntar() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 1)));
        m.evento(&ev("Stop", "s1", CLIENTE_D, h(9, 5)));
        m.desligar_sessao("s1", h(9, 6));
        assert!(conversas_novas(&m).is_empty(), "a pergunta da conversa nova sai");
        assert_eq!(m.sessoes["s1"].timer_id, None);
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(9, 10)));
        assert!(m.perguntas.iter().any(|(_, p)| matches!(p, Pergunta::SessaoNova { sessao_id, .. } if sessao_id == "s1")));
        assert_eq!(m.sessoes["s1"].timer_id, None, "nao volta para a regra");
    }
    #[test] fn corte_do_dia_para_quem_esperava_voce() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", CLIENTE_D, h(15, 0)));
        m.evento(&ev("Stop", "s1", CLIENTE_D, h(15, 5)));
        m.evento(&ev("UserPromptSubmit", "s2", CLIENTE_D, h(23, 50)));
        m.evento(&ev("Stop", "s2", CLIENTE_D, h(23, 55)));
        let v = m.tique(ms_local(2026, 11, 18, 0, 1, F));
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::Parada);
        assert!(v.iter().any(|x| matches!(x, Mudanca::Sessao(s) if s.id == "s1" && s.estado == EstadoSessao::Parada)));
        // terminou ha 5 min: ainda espera voce depois da virada
        assert_eq!(m.sessoes["s2"].estado, EstadoSessao::EsperandoVoce);
    }

    #[test] fn regra_de_pasta_num_caminho_do_wsl_com_os_dois_nomes() {
        let mut m = motor();
        m.regras_pasta = vec![RegraPasta { id: 9, pasta: r"\\wsl.localhost\Ubuntu\home\u\proj".into(), timer_id: 4 }];
        assert_eq!(regra_para(&m.regras_pasta, r"\\wsl.localhost\Ubuntu\home\u\proj\api"), Some(4));
        assert_eq!(regra_para(&m.regras_pasta, r"\\wsl$\ubuntu\home\u\proj"), Some(4), "o wsl$ é o mesmo lugar");
        assert_eq!(regra_para(&m.regras_pasta, r"\\wsl.localhost\Debian\home\u\proj"), None, "outra distro");
        assert_eq!(trechos(&m.evento(&ev("UserPromptSubmit", "s1", r"\\wsl$\Ubuntu\home\u\proj\api", h(9, 0)))), vec![abriu(Some(4), h(9, 0))]);
        // a raiz da distro e a pasta do usuário dela não podem ter regra
        for larga in [r"\\wsl.localhost\Ubuntu", r"\\wsl$\Ubuntu\home\u", r"\\wsl.localhost\Ubuntu\home", r"\\wsl.localhost\Ubuntu\root"] {
            assert!(pasta_larga(larga, None), "{larga}");
        }
        assert!(!pasta_larga(r"\\wsl.localhost\Ubuntu\home\u\proj", None));
    }
}
