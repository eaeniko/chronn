// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O fim do transcript de uma sessão do Claude Code (rodada 13): a interrupção (o stop na
//! conversa, que não chama o hook `Stop`) e as tarefas em segundo plano ainda rodando.
//!
//! A leitura é **para a frente** (revisão da rodada 13): cada sessão guarda em memória até onde já
//! leu (`LeitorDoTranscript::offset`) e as tarefas pendentes, com o instante do lançamento. Na
//! primeira vez, começa dos últimos `PRIMEIRA_LEITURA` bytes; depois, lê só o que foi
//! acrescentado, e só quando o tamanho muda. A linha incompleta no fim fica para a próxima
//! leitura; se o arquivo encolher, recomeça. Nada do texto sai daqui: só o instante da
//! interrupção e os ids das tarefas (marcadores do próprio Claude Code), e só em memória.
//!
//! Os marcadores:
//! - **interrupção:** uma mensagem do usuário (`type: "user"`) cujo texto começa com
//!   `[Request interrupted by user` (`[Request interrupted by user]` ou `[Request interrupted by
//!   user for tool use]`), com o `timestamp` da linha;
//! - **tarefa lançada:** o resultado de uma ferramenta (numa linha `type: "user"`) que começa com
//!   `Command running in background with ID: <id>` (comando de shell) ou com `Async agent
//!   launched successfully`, com o `agentId: <id>` (subagente);
//! - **tarefa terminada:** uma `<task-notification>` com `<task-id>…</task-id>` igual ao id, num
//!   dos três formatos do Claude Code: `type: "queue-operation"` com `operation: "enqueue"` e o
//!   `content`; `type: "user"` com o `message.content` em texto; ou `attachment.type:
//!   "queued_command"` com o `prompt`. Em qualquer outra linha (`assistant`, `api-request-blob`…)
//!   o mesmo texto não vale.
//!
//! Cada tarefa vale por `TAREFA_MAX_MS` a partir do lançamento; depois é esquecida de vez (um
//! servidor esquecido rodando não marca "segundo plano" em todo fim de turno).

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde_json::Value;

use crate::tempo::{Ms, MIN};

/// Na primeira leitura de uma sessão, de onde começar: os últimos 2 MB.
pub const PRIMEIRA_LEITURA: u64 = 2 * 1024 * 1024;
/// Quanto uma tarefa em segundo plano vale a partir do lançamento.
pub const TAREFA_MAX_MS: Ms = 2 * 60 * MIN;

const INTERRUPCAO: &str = "[Request interrupted by user";
const COMANDO_EM_SEGUNDO_PLANO: &str = "Command running in background with ID:";
const SUBAGENTE: &str = "Async agent launched successfully";
const NOTIFICACAO: &str = "<task-notification>";

/// O que já foi lido do transcript de uma sessão (só em memória).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LeitorDoTranscript {
    /// Até onde já leu (o começo da próxima linha); `None` = ainda não leu.
    pub offset: Option<u64>,
    /// As tarefas lançadas e ainda sem a notificação de fim: (id, instante do lançamento).
    pub pendentes: Vec<(String, Ms)>,
}

/// Os textos de uma mensagem: `content` como texto, ou os `text` dos itens dela.
fn textos(conteudo: &Value) -> Vec<&str> {
    match conteudo {
        Value::String(s) => vec![s.as_str()],
        Value::Array(v) => v.iter().filter_map(|i| i.get("text").and_then(Value::as_str)).collect(),
        _ => vec![],
    }
}

/// O id logo depois de `marca` no texto (letras, números, `-` e `_`, pulando `:`, `"`, espaço).
fn id_depois(texto: &str, marca: &str) -> Option<String> {
    let resto = &texto[texto.find(marca)? + marca.len()..];
    let resto = resto.trim_start_matches([':', '"', ' ', '\\', '=']);
    let id: String = resto.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    (!id.is_empty()).then_some(id)
}

/// O texto da notificação de fim de tarefa, se a linha é uma, num dos três formatos.
fn notificacao(v: &Value) -> Option<&str> {
    fn s(x: Option<&Value>) -> Option<&str> {
        x.and_then(Value::as_str)
    }
    let texto = match s(v.get("type")) {
        Some("queue-operation") if s(v.get("operation")) == Some("enqueue") => s(v.get("content")),
        Some("user") => s(v.get("message").and_then(|m| m.get("content"))),
        _ => v.get("attachment").filter(|a| s(a.get("type")) == Some("queued_command")).and_then(|a| s(a.get("prompt"))),
    }?;
    texto.starts_with(NOTIFICACAO).then_some(texto)
}

/// O id entre `<task-id>` e `</task-id>`.
fn id_da_notificacao(texto: &str) -> Option<&str> {
    let ini = texto.find("<task-id>")? + "<task-id>".len();
    let fim = texto[ini..].find("</task-id>")? + ini;
    Some(texto[ini..fim].trim())
}

impl LeitorDoTranscript {
    /// Linhas inteiras novas. Devolve o instante da última interrupção nelas. `agora`: o
    /// lançamento sem `timestamp` conta deste instante.
    pub fn processar(&mut self, texto: &str, agora: Ms) -> Option<Ms> {
        let mut interrupcao = None;
        for l in texto.lines() {
            if !l.contains(INTERRUPCAO) && !l.contains(COMANDO_EM_SEGUNDO_PLANO) && !l.contains(SUBAGENTE) && !l.contains(NOTIFICACAO) {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
            let em = v
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.timestamp_millis());
            // a tarefa terminou: só nos três formatos da notificação, com o id igual
            if let Some(id) = notificacao(&v).and_then(id_da_notificacao) {
                self.pendentes.retain(|(p, _)| p != id);
                continue;
            }
            if v.get("type").and_then(Value::as_str) != Some("user") {
                continue;
            }
            let Some(conteudo) = v.get("message").and_then(|m| m.get("content")) else { continue };
            // o stop: o texto da própria mensagem do usuário
            if em.is_some() && textos(conteudo).iter().any(|t| t.starts_with(INTERRUPCAO)) {
                interrupcao = em;
            }
            // as tarefas: o resultado de uma ferramenta
            if let Value::Array(itens) = conteudo {
                for i in itens.iter().filter(|i| i.get("type").and_then(Value::as_str) == Some("tool_result")) {
                    for t in i.get("content").map(textos).unwrap_or_default() {
                        let id = if t.starts_with(COMANDO_EM_SEGUNDO_PLANO) {
                            id_depois(t, COMANDO_EM_SEGUNDO_PLANO)
                        } else if t.starts_with(SUBAGENTE) {
                            id_depois(t, "agentId")
                        } else {
                            None
                        };
                        if let Some(id) = id.filter(|id| !self.pendentes.iter().any(|(p, _)| p == id)) {
                            self.pendentes.push((id, em.unwrap_or(agora)));
                        }
                    }
                }
            }
        }
        interrupcao
    }

    /// Esquece de vez as tarefas lançadas há `TAREFA_MAX_MS` ou mais.
    pub fn expirar(&mut self, agora: Ms) {
        self.pendentes.retain(|(_, em)| agora < em + TAREFA_MAX_MS);
    }

    /// Até quando a sessão fica em "segundo plano": o vencimento da tarefa pendente que vence por
    /// último (`None` sem nenhuma valendo em `agora`).
    pub fn segundo_plano_ate(&self, agora: Ms) -> Option<Ms> {
        self.pendentes.iter().map(|(_, em)| em + TAREFA_MAX_MS).filter(|v| *v > agora).max()
    }

    /// Lê o que foi acrescentado ao transcript desde a última vez (na primeira, os últimos
    /// `PRIMEIRA_LEITURA` bytes; com o arquivo menor que o já lido, recomeça). Devolve `None` se
    /// nada mudou (ou o arquivo não abre) e, se leu, o instante da última interrupção nas linhas
    /// novas.
    pub fn ler(&mut self, transcript: &Path, agora: Ms) -> Option<Option<Ms>> {
        let mut f = std::fs::File::open(transcript).ok()?;
        let tamanho = f.metadata().ok()?.len();
        match self.offset {
            Some(o) if o == tamanho => return None,
            // encolheu (outro arquivo no lugar): recomeça
            Some(o) if o > tamanho => *self = LeitorDoTranscript::default(),
            _ => {}
        }
        let (inicio, no_meio) = match self.offset {
            Some(o) => (o, false),
            None => {
                let i = tamanho.saturating_sub(PRIMEIRA_LEITURA);
                (i, i > 0)
            }
        };
        f.seek(SeekFrom::Start(inicio)).ok()?;
        let mut bytes = Vec::new();
        f.take(tamanho - inicio).read_to_end(&mut bytes).ok()?;
        // começando no meio de uma linha, ela é pulada
        let pular = if no_meio { bytes.iter().position(|b| *b == b'\n').map_or(bytes.len(), |p| p + 1) } else { 0 };
        // a linha incompleta no fim fica para a próxima leitura
        let ate = bytes.iter().rposition(|b| *b == b'\n').map_or(pular, |p| (p + 1).max(pular));
        self.offset = Some(inicio + ate as u64);
        let texto = String::from_utf8_lossy(&bytes[pular..ate]);
        Some(self.processar(&texto, agora))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::tempo::ms_local;

    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms { ms_local(2026, 11, 19, hh, mm, F) }
    fn ts(em: Ms) -> String {
        chrono::DateTime::from_timestamp_millis(em).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }
    /// Uma mensagem do usuário com o texto `t` (como item de texto) no instante dado.
    fn usuario(t: &str, em: Ms) -> String {
        serde_json::json!({ "type": "user", "message": { "role": "user", "content": [{ "type": "text", "text": t }] }, "timestamp": ts(em) })
            .to_string()
            + "\n"
    }
    fn resultado(t: &str, em: Ms) -> String {
        serde_json::json!({ "type": "user", "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "toolu_1", "content": t }] }, "timestamp": ts(em) })
            .to_string()
            + "\n"
    }
    fn assistente(t: &str) -> String {
        serde_json::json!({ "type": "assistant", "message": { "role": "assistant", "content": [{ "type": "text", "text": t }] }, "timestamp": ts(h(9, 0)) })
            .to_string()
            + "\n"
    }
    fn notificacao_fila(id: &str) -> String {
        serde_json::json!({ "type": "queue-operation", "operation": "enqueue", "content": format!("<task-notification>\n<task-id>{id}</task-id>\n<status>completed</status>\n</task-notification>") })
            .to_string()
            + "\n"
    }
    fn notificacao_usuario(id: &str) -> String {
        serde_json::json!({ "type": "user", "message": { "role": "user", "content": format!("<task-notification>\n<task-id>{id}</task-id>\n</task-notification>") } })
            .to_string()
            + "\n"
    }
    fn notificacao_anexo(id: &str) -> String {
        serde_json::json!({ "type": "attachment", "attachment": { "type": "queued_command", "prompt": format!("<task-notification><task-id>{id}</task-id></task-notification>") } })
            .to_string()
            + "\n"
    }
    fn ids(l: &LeitorDoTranscript) -> Vec<&str> {
        l.pendentes.iter().map(|(id, _)| id.as_str()).collect()
    }

    #[test]
    fn os_dois_textos_de_interrupcao_dao_o_instante_da_ultima() {
        let mut l = LeitorDoTranscript::default();
        let t = assistente("vou ler o arquivo")
            + &usuario("[Request interrupted by user]", h(8, 53))
            + &usuario("[Request interrupted by user for tool use]", h(8, 58) + 500);
        assert_eq!(l.processar(&t, h(9, 0)), Some(h(8, 58) + 500));
        // o texto em forma de string também vale
        let s = serde_json::json!({ "type": "user", "message": { "content": "[Request interrupted by user]" }, "timestamp": ts(h(8, 53)) }).to_string();
        assert_eq!(l.processar(&s, h(9, 0)), Some(h(8, 53)));
        // dentro de um resultado de ferramenta (um arquivo lido) ou na fala do assistente, não
        let t = resultado("x [Request interrupted by user] y", h(9, 0)) + &assistente("[Request interrupted by user]");
        assert_eq!(l.processar(&t, h(9, 0)), None);
    }

    #[test]
    fn tarefas_lancadas_e_terminadas_nos_tres_formatos_com_o_id_exato() {
        let mut l = LeitorDoTranscript::default();
        let t = resultado("Command running in background with ID: bw54hpb9u. Output is being written to: C:\\x\\bw54hpb9u.output", h(10, 0))
            + &resultado("Async agent launched successfully.\nagentId: a1b2c3 (internal ID - do not mention to user)", h(10, 1))
            + &resultado("Command running in background with ID: srv1", h(10, 2))
            // um arquivo lido que só cita o marcador não é tarefa
            + &resultado("o texto \"Command running in background with ID: zzz\" aparece num teste", h(10, 3));
        l.processar(&t, h(10, 5));
        assert_eq!(ids(&l), ["bw54hpb9u", "a1b2c3", "srv1"]);
        assert_eq!(l.pendentes[0].1, h(10, 0), "o instante do lançamento");
        // o texto da notificação numa linha do assistente ou num blob do pedido à API não termina
        let falsas = assistente("<task-notification><task-id>bw54hpb9u</task-id></task-notification>")
            + &serde_json::json!({ "type": "api-request-blob", "content": "<task-notification><task-id>bw54hpb9u</task-id></task-notification>" }).to_string()
            + "\n"
            // nem um id que só começa igual
            + &notificacao_fila("bw54hpb9uXX");
        l.processar(&falsas, h(10, 6));
        assert_eq!(ids(&l), ["bw54hpb9u", "a1b2c3", "srv1"]);
        l.processar(&notificacao_fila("bw54hpb9u"), h(10, 7));
        assert_eq!(ids(&l), ["a1b2c3", "srv1"]);
        l.processar(&notificacao_usuario("a1b2c3"), h(10, 8));
        assert_eq!(ids(&l), ["srv1"]);
        l.processar(&notificacao_anexo("srv1"), h(10, 9));
        assert!(l.pendentes.is_empty());
    }

    #[test]
    fn tarefa_vale_2_h_do_lancamento_e_depois_e_esquecida() {
        let mut l = LeitorDoTranscript::default();
        l.processar(&(resultado("Command running in background with ID: srv", h(10, 0)) + &resultado("Command running in background with ID: b2", h(11, 0))), h(11, 0));
        assert_eq!(l.segundo_plano_ate(h(11, 30)), Some(h(13, 0)));
        l.expirar(h(12, 0));
        assert_eq!(ids(&l), ["b2"], "o servidor de 10:00 venceu");
        assert_eq!(l.segundo_plano_ate(h(12, 59)), Some(h(13, 0)));
        assert_eq!(l.segundo_plano_ate(h(13, 0)), None);
        l.expirar(h(13, 0));
        assert!(l.pendentes.is_empty());
    }

    #[test]
    fn le_para_a_frente_so_o_acrescentado_e_so_quando_muda() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        let mut l = LeitorDoTranscript::default();
        // o lançamento, e depois mais de 64 KB de linhas (a cauda fixa da primeira versão o perdia)
        let mut s = resultado("Command running in background with ID: dev1", h(10, 0));
        for _ in 0..80 {
            s.push_str(&resultado(&"x".repeat(1000), h(10, 1)));
        }
        std::fs::write(&p, &s).unwrap();
        assert_eq!(l.ler(&p, h(10, 3)), Some(None));
        assert_eq!(ids(&l), ["dev1"]);
        assert_eq!(l.offset, Some(s.len() as u64));
        // sem mudança: não lê
        assert_eq!(l.ler(&p, h(10, 4)), None);
        // a linha incompleta no fim fica para a próxima
        let linha = usuario("[Request interrupted by user]", h(10, 5));
        let (a, b) = linha.split_at(30);
        s.push_str(a);
        std::fs::write(&p, &s).unwrap();
        assert_eq!(l.ler(&p, h(10, 5)), Some(None));
        assert_eq!(l.offset, Some((s.len() - a.len()) as u64), "o offset para no começo da linha incompleta");
        s.push_str(b);
        std::fs::write(&p, &s).unwrap();
        assert_eq!(l.ler(&p, h(10, 6)), Some(Some(h(10, 5))), "inteira agora");
        // a interrupção já lida não volta
        s.push_str(&assistente("oi"));
        std::fs::write(&p, &s).unwrap();
        assert_eq!(l.ler(&p, h(10, 7)), Some(None));
        // encolheu: recomeça
        let novo = notificacao_fila("outro") + &resultado("Command running in background with ID: dev2", h(11, 0));
        std::fs::write(&p, &novo).unwrap();
        assert_eq!(l.ler(&p, h(11, 1)), Some(None));
        assert_eq!(ids(&l), ["dev2"]);
        assert_eq!(l.ler(&d.path().join("nao.jsonl"), h(11, 2)), None);
    }

    #[test]
    fn a_primeira_leitura_comeca_dos_ultimos_2_mb_pulando_a_linha_cortada() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("grande.jsonl");
        let mut s = resultado("Command running in background with ID: velho", h(8, 0));
        let linha = assistente(&"y".repeat(2000));
        while s.len() < (PRIMEIRA_LEITURA as usize) + 10_000 {
            s.push_str(&linha);
        }
        s.push_str(&resultado("Command running in background with ID: novo", h(10, 0)));
        std::fs::write(&p, &s).unwrap();
        let mut l = LeitorDoTranscript::default();
        assert_eq!(l.ler(&p, h(10, 1)), Some(None));
        assert_eq!(ids(&l), ["novo"], "o lançamento de antes dos 2 MB não é visto");
        assert_eq!(l.offset, Some(s.len() as u64));
        // a linha cortada no começo do pedaço: a interrupção começa antes do limite dos 2 MB e
        // termina depois dele; ela é pulada, e não lida pela metade
        let int = usuario("[Request interrupted by user]", h(9, 0));
        let depois = PRIMEIRA_LEITURA as usize - int.len() / 2;
        let vazia = assistente("").len();
        let s2 = linha.clone() + &int + &assistente(&"y".repeat(depois - vazia));
        assert_eq!(s2.len() - PRIMEIRA_LEITURA as usize, linha.len() + int.len() / 2);
        let p2 = d.path().join("cortado.jsonl");
        std::fs::write(&p2, &s2).unwrap();
        let mut l = LeitorDoTranscript::default();
        let r = l.ler(&p2, h(10, 0));
        assert_eq!(r, Some(None), "a linha cortada não é lida pela metade");
    }
}
