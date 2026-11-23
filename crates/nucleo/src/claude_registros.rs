// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Leitura do transcript das sessões do Claude Code: o nome (o `customTitle` dado com `/rename` ou
//! no app; sem ele, o `aiTitle` que o Claude Code do terminal gera, 0.6.1) e a conta (o
//! `accountUuid` da linha `session-owner`, que sai daqui só como resumo, 0.6.1). Nenhum texto de
//! prompt ou de resposta é guardado: só as linhas desses três tipos são lidas.

use std::path::Path;

use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom};

/// O fim do arquivo lido primeiro (o nome mais novo fica no fim).
const CAUDA: u64 = 256 * 1024;
/// O começo do arquivo lido para achar a conta (a linha `session-owner` vem no início).
const CABECA: u64 = 256 * 1024;

#[derive(Deserialize)]
struct Linha {
    #[serde(rename = "type")]
    tipo: Option<String>,
    #[serde(rename = "customTitle")]
    titulo: Option<String>,
    #[serde(rename = "aiTitle")]
    titulo_ia: Option<String>,
    #[serde(rename = "accountUuid")]
    conta: Option<String>,
}

/// O que um transcript diz da sessão.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RegistroDaSessao {
    /// O último `customTitle` e, sem nenhum, o último `aiTitle`.
    pub titulo: Option<String>,
    /// O resumo (`fila::resumo_da_conta`) do `accountUuid` da linha `session-owner`; nunca o id.
    pub conta: Option<String>,
}

/// O que um pedaço do transcript tem: o último `custom-title`, o último `ai-title` e a primeira
/// conta (já resumida).
#[derive(Default)]
struct Achados {
    custom: Option<String>,
    ia: Option<String>,
    conta: Option<String>,
}

fn achar(texto: &str) -> Achados {
    let mut a = Achados::default();
    let nao_vazio = |t: Option<String>| t.filter(|t| !t.trim().is_empty());
    for l in texto.lines() {
        // só as linhas dos três tipos são abertas como JSON (as outras nem são lidas por inteiro)
        if !(l.contains("\"custom-title\"") || l.contains("\"ai-title\"") || l.contains("\"session-owner\"")) {
            continue;
        }
        let Ok(x) = serde_json::from_str::<Linha>(l) else { continue };
        match x.tipo.as_deref() {
            Some("custom-title") => {
                if let Some(t) = nao_vazio(x.titulo) {
                    a.custom = Some(t);
                }
            }
            Some("ai-title") => {
                if let Some(t) = nao_vazio(x.titulo_ia) {
                    a.ia = Some(t);
                }
            }
            Some("session-owner") if a.conta.is_none() => a.conta = fila::resumo_da_conta(x.conta.as_deref()),
            _ => {}
        }
    }
    a
}

/// O texto de `bytes` sem a primeira linha, que pode estar cortada ao meio (`cortado`), nem a
/// última, quando o pedaço acaba antes do fim do arquivo (`ate_o_fim` falso).
fn linhas_inteiras(bytes: &[u8], cortado: bool, ate_o_fim: bool) -> String {
    let texto = String::from_utf8_lossy(bytes);
    let mut s: &str = &texto;
    if cortado {
        s = s.split_once('\n').map_or("", |(_, resto)| resto);
    }
    if !ate_o_fim {
        s = s.rsplit_once('\n').map_or("", |(antes, _)| antes);
    }
    s.to_string()
}

/// O nome e a conta da sessão. `quer_conta`: a sessão ainda não tem conta (a do hook vence; sem
/// ela, a do transcript). Lê o fim do arquivo; sem `customTitle` lá, o arquivo inteiro (o
/// `customTitle` antigo vence o `aiTitle` novo); com nome e sem a conta pedida, o começo.
pub fn registro_da_sessao(transcript: &Path, quer_conta: bool) -> RegistroDaSessao {
    ler(transcript, quer_conta).unwrap_or_default()
}

fn ler(transcript: &Path, quer_conta: bool) -> Option<RegistroDaSessao> {
    let mut f = std::fs::File::open(transcript).ok()?;
    let tamanho = f.metadata().ok()?.len();
    let fim = |a: Achados| RegistroDaSessao { titulo: a.custom.or(a.ia), conta: a.conta };
    if tamanho > CAUDA {
        f.seek(SeekFrom::Start(tamanho - CAUDA)).ok()?;
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).ok()?;
        let cauda = achar(&linhas_inteiras(&bytes, true, true));
        if cauda.custom.is_some() {
            if !quer_conta || cauda.conta.is_some() {
                return Some(fim(cauda));
            }
            // a conta vem no começo
            f.seek(SeekFrom::Start(0)).ok()?;
            let mut bytes = Vec::new();
            (&mut f).take(CABECA).read_to_end(&mut bytes).ok()?;
            let cabeca = achar(&linhas_inteiras(&bytes, false, (bytes.len() as u64) >= tamanho));
            return Some(RegistroDaSessao { titulo: cauda.custom, conta: cabeca.conta });
        }
        f.seek(SeekFrom::Start(0)).ok()?;
    }
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;
    Some(fim(achar(&String::from_utf8_lossy(&bytes))))
}

/// O `oauthAccount` do `~/.claude.json` (só os campos do rótulo).
#[derive(Deserialize)]
struct ClaudeJson {
    #[serde(rename = "oauthAccount")]
    conta: Option<OauthAccount>,
}

#[derive(Deserialize)]
struct OauthAccount {
    #[serde(rename = "accountUuid")]
    id: Option<String>,
    #[serde(rename = "displayName")]
    nome: Option<String>,
    #[serde(rename = "emailAddress")]
    email: Option<String>,
    #[serde(rename = "organizationType")]
    tipo: Option<String>,
    #[serde(rename = "billingType")]
    cobranca: Option<String>,
}

/// O e-mail mascarado: as duas primeiras letras, "•••" e o domínio ("ni•••@exemplo.com").
pub fn email_mascarado(email: &str) -> Option<String> {
    let (nome, dominio) = email.trim().split_once('@')?;
    if nome.is_empty() || dominio.is_empty() {
        return None;
    }
    // as duas primeiras letras; num nome curto, menos (revisão M5: nunca o nome inteiro)
    let n = nome.chars().count();
    let inicio: String = nome.chars().take(if n > 2 { 2 } else { n - 1 }).collect();
    Some(format!("{inicio}•••@{dominio}"))
}

/// O plano pelo `organizationType` ("claude_pro" → "Pro"); sem ele, pelo `billingType` só quando
/// diz algo (o "stripe_subscription" não diz qual plano).
pub fn nome_do_plano(tipo: Option<&str>, cobranca: Option<&str>) -> Option<String> {
    let bonito = |s: &str| {
        let s = s.trim().trim_start_matches("claude_").replace('_', " ");
        let mut c = s.chars();
        c.next().map(|p| p.to_uppercase().chain(c).collect::<String>())
    };
    match tipo.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => bonito(t),
        None => cobranca.map(str::trim).filter(|c| !c.is_empty() && !c.contains("subscription")).and_then(bonito),
    }
}

/// O rótulo da conta do `~/.claude.json` (do Windows ou de uma distro; 0.6.1): o resumo do
/// `accountUuid` (para achar a conta já vista, sem nunca guardar o id) e "Nome · ni•••@dominio ·
/// Pro". Sem `oauthAccount` (ou sem id), nada.
pub fn rotulo_da_conta(claude_json: &Path) -> Option<(String, String)> {
    let bytes = std::fs::read(claude_json).ok()?;
    let c = serde_json::from_slice::<ClaudeJson>(&bytes).ok()?.conta?;
    let resumo = fila::resumo_da_conta(c.id.as_deref())?;
    let partes: Vec<String> = [
        c.nome.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
        c.email.as_deref().and_then(email_mascarado),
        nome_do_plano(c.tipo.as_deref(), c.cobranca.as_deref()),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!partes.is_empty()).then(|| (resumo, partes.join(" · ")))
}

/// Só o nome (o `customTitle`; sem ele, o `aiTitle`).
pub fn titulo_da_sessao(transcript: &Path) -> Option<String> {
    registro_da_sessao(transcript, false).titulo
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;

    #[test]
    fn titulo_vem_do_ultimo_custom_title() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        fs::write(
            &p,
            concat!(
                "{\"type\":\"user\",\"message\":\"oi\"}\n",
                "{\"type\":\"custom-title\",\"customTitle\":\"Primeiro\",\"sessionId\":\"a\"}\n",
                "{\"type\":\"assistant\",\"message\":\"ola\"}\n",
                "{\"type\":\"custom-title\",\"customTitle\":\"Spec 018: segundo\",\"sessionId\":\"a\"}\n",
                "{\"type\":\"user\",\"message\":\"fim\"}\n",
            ),
        )
        .unwrap();
        assert_eq!(titulo_da_sessao(&p).as_deref(), Some("Spec 018: segundo"));
    }

    #[test]
    fn sem_titulo_retorna_none() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        fs::write(&p, "{\"type\":\"user\",\"message\":\"oi\"}\n").unwrap();
        assert_eq!(titulo_da_sessao(&p), None);
        assert_eq!(titulo_da_sessao(&d.path().join("nao-existe.jsonl")), None);
        assert_eq!(registro_da_sessao(&d.path().join("nao-existe.jsonl"), true), RegistroDaSessao::default());
    }

    #[test]
    fn titulo_antigo_fora_da_cauda_e_achado_no_arquivo_inteiro() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        let mut s = String::from("{\"type\":\"custom-title\",\"customTitle\":\"Antigo\"}\n");
        let linha = format!("{{\"type\":\"user\",\"message\":\"{}\"}}\n", "x".repeat(1000));
        for _ in 0..400 {
            s.push_str(&linha);
        }
        fs::write(&p, s).unwrap();
        assert_eq!(titulo_da_sessao(&p).as_deref(), Some("Antigo"));
    }

    // ---- 0.6.1: o `aiTitle` do terminal e a conta pelo `session-owner` ----

    #[test]
    fn sem_custom_title_vale_o_ultimo_ai_title() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        fs::write(
            &p,
            concat!(
                "{\"type\":\"ai-title\",\"aiTitle\":\"Primeiro\",\"sessionId\":\"a\"}\n",
                "{\"type\":\"user\",\"message\":\"oi\"}\n",
                "{\"type\":\"ai-title\",\"aiTitle\":\"Arrumar o login\",\"sessionId\":\"a\"}\n",
                "{\"type\":\"ai-title\",\"aiTitle\":\"   \",\"sessionId\":\"a\"}\n",
            ),
        )
        .unwrap();
        assert_eq!(titulo_da_sessao(&p).as_deref(), Some("Arrumar o login"));
    }

    #[test]
    fn o_custom_title_vence_o_ai_title_mesmo_mais_antigo() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        fs::write(
            &p,
            concat!(
                "{\"type\":\"custom-title\",\"customTitle\":\"Meu nome\"}\n",
                "{\"type\":\"ai-title\",\"aiTitle\":\"Gerado\"}\n",
            ),
        )
        .unwrap();
        assert_eq!(titulo_da_sessao(&p).as_deref(), Some("Meu nome"));
        // fora da cauda: o arquivo inteiro é lido, e o antigo ainda vence
        let mut s = String::from("{\"type\":\"custom-title\",\"customTitle\":\"Antigo\"}\n");
        let linha = format!("{{\"type\":\"user\",\"message\":\"{}\"}}\n", "x".repeat(1000));
        for _ in 0..400 {
            s.push_str(&linha);
        }
        s.push_str("{\"type\":\"ai-title\",\"aiTitle\":\"Novo gerado\"}\n");
        fs::write(&p, s).unwrap();
        assert_eq!(titulo_da_sessao(&p).as_deref(), Some("Antigo"));
    }

    #[test]
    fn a_conta_sai_do_session_owner_so_como_resumo() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        let uuid = "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0";
        fs::write(
            &p,
            format!(
                "{{\"type\":\"session-owner\",\"accountUuid\":\"{uuid}\",\"organizationUuid\":\"x\",\"sessionId\":\"a\"}}\n\
                 {{\"type\":\"ai-title\",\"aiTitle\":\"Tarefa\"}}\n"
            ),
        )
        .unwrap();
        let r = registro_da_sessao(&p, true);
        assert_eq!(r.titulo.as_deref(), Some("Tarefa"));
        assert_eq!(r.conta, fila::resumo_da_conta(Some(uuid)));
        assert!(!r.conta.as_deref().unwrap().contains("0f1e"), "nunca o id");
    }

    #[test]
    fn o_rotulo_da_conta_tem_nome_email_mascarado_e_plano_e_nunca_o_id() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(".claude.json");
        let uuid = "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0";
        fs::write(
            &p,
            format!(
                "{{\"numStartups\":3,\"projects\":{{}},\"oauthAccount\":{{\"accountUuid\":\"{uuid}\",\"emailAddress\":\"ana.souza@exemplo.com\",\
                 \"organizationUuid\":\"x\",\"displayName\":\"Ana\",\"organizationType\":\"claude_max\",\"billingType\":\"stripe_subscription\",\
                 \"organizationName\":\"Org da Ana\"}}}}"
            ),
        )
        .unwrap();
        let (resumo, rotulo) = rotulo_da_conta(&p).unwrap();
        assert_eq!(Some(resumo), fila::resumo_da_conta(Some(uuid)));
        assert_eq!(rotulo, "Ana · an•••@exemplo.com · Max");
        assert!(!rotulo.contains("souza") && !rotulo.contains("0f1e"));
        // sem a conta (deslogado) ou sem o arquivo: nada
        fs::write(&p, "{\"numStartups\":3}").unwrap();
        assert_eq!(rotulo_da_conta(&p), None);
        assert_eq!(rotulo_da_conta(&d.path().join("nao-existe.json")), None);
        assert_eq!(nome_do_plano(Some("claude_pro"), None).as_deref(), Some("Pro"));
        assert_eq!(nome_do_plano(Some("claude_team"), Some("x")).as_deref(), Some("Team"));
        assert_eq!(nome_do_plano(None, Some("stripe_subscription")), None);
        assert_eq!(email_mascarado("a@b.c").as_deref(), Some("•••@b.c"));
        assert_eq!(email_mascarado("ab@b.c").as_deref(), Some("a•••@b.c"));
        assert_eq!(email_mascarado("abc@b.c").as_deref(), Some("ab•••@b.c"));
        assert_eq!(email_mascarado("sem-arroba"), None);
    }

    #[test]
    fn a_conta_no_comeco_de_um_transcript_grande_com_o_nome_no_fim() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        let uuid = "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0";
        let mut s = format!("{{\"type\":\"session-owner\",\"accountUuid\":\"{uuid}\"}}\n");
        let linha = format!("{{\"type\":\"user\",\"message\":\"{}\"}}\n", "x".repeat(1000));
        for _ in 0..600 {
            s.push_str(&linha);
        }
        s.push_str("{\"type\":\"custom-title\",\"customTitle\":\"No fim\"}\n");
        fs::write(&p, s).unwrap();
        let r = registro_da_sessao(&p, true);
        assert_eq!((r.titulo.as_deref(), r.conta), (Some("No fim"), fila::resumo_da_conta(Some(uuid))));
        // sem pedir a conta, o começo nem é lido
        assert_eq!(registro_da_sessao(&p, false).conta, None);
    }
}
