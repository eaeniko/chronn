// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O Claude Code no WSL (rodada 28). O Linux do usuário pode estar endurecido (sem interop e sem o
//! `C:` em `/mnt`), então o hook do Windows não roda lá. No lugar, cada distro ligada tem o hook
//! para Linux (`~/.worktimer/bin/work-timer-hook`), que grava a fila na pasta do usuário de lá
//! (`~/.worktimer/fila/eventos.jsonl`), e o Chronn lê essa fila pelo Windows, por
//! `\\wsl.localhost\<distro>\…`. Nada no Linux roda programas do Windows.
//!
//! Tudo o que fala com o WSL passa pelo `trait Wsl`: a implementação real (o registro e o
//! `wsl.exe`) fica no app; os testes usam o `WslFalso`, com uma pasta temporária no lugar da distro.
//! O Chronn nunca liga uma distro parada: abrir `\\wsl.localhost\<distro>` ou rodar `wsl -d` a
//! ligaria. Por isso, tudo o que lê ou grava na distro confere antes que ela está rodando.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fila::EventoHook;
use serde::Serialize;
use serde_json::json;

use crate::hooks_claude::{self, EstadoHooks};
use crate::idioma::tfa;

/// A origem das sessões do Claude Code no terminal do WSL (o `cli` lido da fila de uma distro).
pub const ORIGEM_WSL: &str = "wsl";
/// O nome do recurso do app com o hook para Linux (`bundle.resources` do `tauri.conf.json`).
pub const RECURSO_HOOK_LINUX: &str = "work-timer-hook-linux";

/// O que o Chronn pede ao WSL.
pub trait Wsl {
    /// As distros do registro (`HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss\*\DistributionName`).
    fn distros(&self) -> Vec<String>;
    /// As distros rodando agora (`wsl -l --running -q`); `None`: não deu para saber.
    fn rodando(&self) -> Option<Vec<String>>;
    /// A pasta do usuário na distro (`/home/<u>`). Só para uma distro rodando.
    fn home(&self, distro: &str) -> Option<String>;
    /// `chmod 755 <caminho>` na distro. Só para uma distro rodando.
    fn chmod(&self, distro: &str, caminho: &str) -> Result<(), String>;
    /// Liga a distro parada (0.6.1). Só no clique da pessoa no "Ligar" (`subir_se_parada`); o
    /// Chronn nunca liga uma distro sozinho.
    fn subir(&self, _distro: &str) -> Result<(), String> {
        Err("não dá para ligar a distro".into())
    }
    /// A raiz da distro vista pelo Windows: `\\wsl.localhost\<distro>` (os testes trocam por uma
    /// pasta temporária).
    fn raiz(&self, distro: &str) -> PathBuf {
        PathBuf::from(format!(r"\\wsl.localhost\{distro}"))
    }
}

// ---- as distros rodando agora (revisão I1 e I4) ----

/// Por quanto tempo a lista das distros rodando vale (re-revisão N1; 0.6.1, revisão C1: era 75 s):
/// vencida, conta como "não sei", e nenhuma pasta do WSL é tocada. Curta de propósito: a distro
/// pode parar logo depois da consulta, e ler uma pasta dela a ligaria de novo. O laço do WSL
/// consulta a cada 10 s (a cada 2 s com uma sessão em curso); quem lê o transcript de uma sessão
/// do WSL com a lista vencida fica para um ciclo seguinte.
pub const VALIDADE_RODANDO: std::time::Duration = std::time::Duration::from_secs(3);

/// As distros que o último `rodando()` do WSL de verdade disse que rodam (em minúsculas), com o
/// instante da consulta. Começa vazio: sem saber, nenhuma pasta do WSL é tocada.
static RODANDO: std::sync::RwLock<Option<(std::time::Instant, std::collections::BTreeSet<String>)>> = std::sync::RwLock::new(None);

/// Guarda as distros rodando agora (o `WslReal` chama a cada consulta; sem resposta, nenhuma).
pub fn definir_rodando(nomes: &[String]) {
    definir_rodando_em(nomes, std::time::Instant::now());
}

/// `definir_rodando` com o instante da consulta.
pub fn definir_rodando_em(nomes: &[String], quando: std::time::Instant) {
    let mut g = RODANDO.write().unwrap_or_else(|e| e.into_inner());
    *g = Some((quando, nomes.iter().map(|n| n.to_lowercase()).collect()));
}

/// A distro de um caminho `\\wsl.localhost\<d>\…` ou `\\wsl$\<d>\…` (em minúsculas); outro
/// caminho, nenhuma.
pub fn distro_do_caminho(caminho: &str) -> Option<String> {
    let n = fila::normaliza(caminho.trim());
    let resto = n.strip_prefix("//wsl.localhost/")?;
    let d = resto.split('/').next().filter(|d| !d.is_empty())?;
    Some(d.to_string())
}

/// Dá para ler este caminho sem ligar uma distro? Fora do WSL, sempre; no WSL, só com a distro
/// na última lista das que rodam. Sem saber, não: "não sei", e o disco não é tocado.
pub fn pode_tocar(caminho: &str) -> bool {
    pode_tocar_em(caminho, std::time::Instant::now())
}

/// `pode_tocar` com uma validade menor que a de sempre (0.6.1): a leitura rápida da fila (a cada
/// 2 s) só vale logo depois de uma consulta, para nunca ler uma distro que parou há pouco.
pub fn pode_tocar_recente(caminho: &str, validade: std::time::Duration) -> bool {
    pode_tocar_recente_em(caminho, validade, std::time::Instant::now())
}

/// `pode_tocar_recente` em `agora`.
pub fn pode_tocar_recente_em(caminho: &str, validade: std::time::Duration, agora: std::time::Instant) -> bool {
    match distro_do_caminho(caminho) {
        None => true,
        Some(d) => RODANDO.read().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|(quando, nomes)| {
            agora.saturating_duration_since(*quando) <= validade.min(VALIDADE_RODANDO) && nomes.contains(&d)
        }),
    }
}

/// `pode_tocar` em `agora`: a lista só vale até `VALIDADE_RODANDO` depois da consulta (sem
/// distro ligada, o laço não pergunta mais, e a lista vence sozinha).
pub fn pode_tocar_em(caminho: &str, agora: std::time::Instant) -> bool {
    match distro_do_caminho(caminho) {
        None => true,
        Some(d) => RODANDO
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|(quando, nomes)| agora.saturating_duration_since(*quando) <= VALIDADE_RODANDO && nomes.contains(&d)),
    }
}

/// As distros internas de outros programas (o Docker Desktop, o Rancher Desktop): ficam fora da
/// seção (revisão M9).
pub fn distro_interna(nome: &str) -> bool {
    let n = nome.to_lowercase();
    n.starts_with("docker-desktop") || n.starts_with("rancher-desktop")
}

/// Um nome de distro que pode ir num caminho e num argumento do `wsl.exe`: letras, números, `.`,
/// `_` e `-`, até 64.
pub fn distro_valida(nome: &str) -> bool {
    !nome.is_empty() && nome.len() <= 64 && nome.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
}

/// Uma pasta do usuário da distro que vale: absoluta, sem `..`, sem `\` nem caracteres de controle.
pub fn home_valida(home: &str) -> Option<String> {
    let h = home.trim().trim_end_matches('/');
    let ok = h.starts_with('/')
        && h.len() > 1
        && !h.split('/').any(|p| p == ".." || p == ".")
        && !h.chars().any(|c| c.is_control() || c == '\\' || c == '"');
    ok.then(|| h.to_string())
}

/// Os caminhos Linux do Chronn e do Claude Code na distro, a partir da pasta do usuário.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PastasDaDistro {
    pub home: String,
}

impl PastasDaDistro {
    pub fn fila(&self) -> String {
        format!("{}/.worktimer/fila/eventos.jsonl", self.home)
    }
    pub fn hook(&self) -> String {
        format!("{}/.worktimer/bin/{}", self.home, fila::caminhos::HOOK_NOME)
    }
    pub fn hook_json(&self) -> String {
        format!("{}/.worktimer/hook.json", self.home)
    }
    pub fn claude(&self) -> String {
        format!("{}/.claude", self.home)
    }
    pub fn settings(&self) -> String {
        format!("{}/.claude/settings.json", self.home)
    }
}

/// O `~/.claude.json` da distro (o rótulo da conta; 0.6.1) a partir do caminho da fila dela pelo
/// Windows (`<home>\.worktimer\fila\eventos.jsonl`).
pub fn claude_json_da_fila(fila: &Path) -> Option<PathBuf> {
    let home = fila.parent()?.parent()?.parent()?;
    Some(home.join(".claude.json"))
}

/// O caminho Linux absoluto `linux` visto pelo Windows, dentro de `raiz` (`\\wsl.localhost\<d>`).
pub fn no_windows(raiz: &Path, linux: &str) -> PathBuf {
    let mut p = raiz.to_path_buf();
    for parte in linux.split('/').filter(|x| !x.is_empty()) {
        p.push(parte);
    }
    p
}

/// O comando dos hooks no `settings.json` da distro: o caminho Linux absoluto do hook, entre aspas.
pub fn comando_linux(pastas: &PastasDaDistro) -> String {
    format!("\"{}\"", pastas.hook())
}

/// O `cwd` de um evento da distro, como o Windows o vê: `/home/u/proj` vira
/// `\\wsl.localhost\<distro>\home\u\proj`; `/mnt/<letra>/…` (num WSL sem endurecimento) vira
/// `<LETRA>:\…`. Um `cwd` que não começa com `/` fica como está.
pub fn traduzir_cwd(distro: &str, cwd: &str) -> String {
    if !cwd.starts_with('/') {
        return cwd.to_string();
    }
    let partes: Vec<&str> = cwd.split('/').filter(|x| !x.is_empty()).collect();
    if let ["mnt", letra, resto @ ..] = partes.as_slice() {
        if letra.len() == 1 && letra.chars().all(|c| c.is_ascii_alphabetic()) {
            let mut s = format!("{}:\\", letra.to_ascii_uppercase());
            s.push_str(&resto.join("\\"));
            return s;
        }
    }
    let mut s = format!(r"\\wsl.localhost\{distro}");
    for p in partes {
        s.push('\\');
        s.push_str(p);
    }
    s
}

/// As pastas ignoradas que estão na distro, no caminho Linux: `\\wsl.localhost\<d>\…` (ou
/// `\\wsl$\<d>\…`) vira `/…`. As outras ficam de fora (o hook de lá não as veria).
pub fn ignoradas_da_distro(distro: &str, ignoradas: &[String]) -> Vec<String> {
    ignoradas
        .iter()
        .filter_map(|i| {
            // por partes do caminho, do jeito que foi escrito (as maiúsculas do Linux importam):
            // `\\`, o host (`wsl.localhost` ou `wsl$`), a distro e o resto (revisão M6)
            let i = i.trim();
            if !(i.starts_with(r"\\") || i.starts_with("//")) {
                return None;
            }
            let partes: Vec<&str> = i.split(['\\', '/']).filter(|x| !x.is_empty()).collect();
            let [host, d, resto @ ..] = partes.as_slice() else { return None };
            let host_ok = host.eq_ignore_ascii_case("wsl.localhost") || host.eq_ignore_ascii_case("wsl$");
            (host_ok && d.eq_ignore_ascii_case(distro)).then(|| format!("/{}", resto.join("/")))
        })
        .collect()
}

/// O evento lido da fila da distro, pronto para o motor: o `cwd` e o `transcript` (revisão I4)
/// traduzidos para o Windows e a origem `cli` como `wsl` (o terminal do WSL). A distro não vai no
/// evento. Quem lê o transcript confere antes que a distro roda (`pode_tocar`).
pub fn evento_da_distro(distro: &str, mut ev: EventoHook) -> EventoHook {
    ev.cwd = traduzir_cwd(distro, &ev.cwd);
    ev.transcript = ev.transcript.map(|t| traduzir_cwd(distro, &t));
    if ev.origem.as_deref() == Some("cli") {
        ev.origem = Some(ORIGEM_WSL.to_string());
    }
    ev
}

/// O estado de uma distro na tela ("Claude Code no WSL").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoDistro {
    /// Ligada (o hook de agora).
    Ligada,
    /// Rodando, sem a pasta `~/.claude`: o Claude Code não está lá.
    SemClaude,
    /// Parada: o Chronn não olha dentro dela.
    Parada,
    /// Ligada, mas o hook de lá é de outra versão (a cópia nova vai no próximo ciclo).
    Desatualizado,
    /// Rodando, com o Claude Code, sem os nossos hooks.
    Desligada,
    /// Ligada na config, mas os hooks saíram do `settings.json` de lá (0.6.1: outro programa
    /// regravou o arquivo): os eventos não chegam até ligar de novo.
    SemHooks,
}

/// Uma linha da seção "Claude Code no WSL".
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DistroDto {
    pub nome: String,
    pub estado: EstadoDistro,
    /// Está na config (`wsl_distros`): o botão é o "Desligar", mesmo parada.
    pub ligada: bool,
}

/// O que o Chronn sabe de cada distro nesta execução: a pasta do usuário, pedida uma vez (o
/// `wsl -e sh -c 'echo $HOME'` leva ~100 ms), e as distros cujo hook teve o `chmod` confirmado
/// (revisão I3: sem a confirmação, a tela mostra "hook desatualizado" e o ciclo tenta de novo).
#[derive(Clone, Debug, Default)]
pub struct Homes {
    pastas: BTreeMap<String, PastasDaDistro>,
    executavel: std::collections::BTreeSet<String>,
    /// As distros cujo último `chmod` (ou a cópia) falhou: "hook desatualizado" até dar certo.
    /// Antes da primeira conferência, nenhuma: a ligada aparece "ligada" (re-revisão N7).
    falhou: std::collections::BTreeSet<String>,
}

impl Homes {
    /// As pastas da distro (rodando: quem chama confere).
    pub fn de(&mut self, w: &dyn Wsl, distro: &str) -> Option<PastasDaDistro> {
        if let Some(p) = self.pastas.get(distro) {
            return Some(p.clone());
        }
        let home = home_valida(&w.home(distro)?)?;
        let p = PastasDaDistro { home };
        self.pastas.insert(distro.to_string(), p.clone());
        Some(p)
    }

    /// O `chmod` do hook da distro foi confirmado nesta execução?
    pub fn executavel(&self, distro: &str) -> bool {
        self.executavel.contains(distro)
    }

    /// A última cópia ou o último `chmod` do hook da distro falhou?
    pub fn falhou(&self, distro: &str) -> bool {
        self.falhou.contains(distro)
    }
}

/// Os bytes do hook da distro são os do recurso do app?
fn hook_igual(raiz: &Path, pastas: &PastasDaDistro, recurso: &Path) -> bool {
    use std::collections::HashMap;
    use std::sync::Mutex;
    type Assinatura = (u64, Option<std::time::SystemTime>);
    // 0.6.1 (revisão M3): com os Ajustes abertos, a seção do WSL é lida a cada 10 s; os ~440 KB do
    // hook de cada distro só são comparados de novo quando a data ou o tamanho de um dos dois mudam
    static VISTOS: Mutex<Option<HashMap<PathBuf, (Assinatura, Assinatura, bool)>>> = Mutex::new(None);
    let hook = no_windows(raiz, &pastas.hook());
    let assinatura = |p: &Path| std::fs::metadata(p).ok().map(|m| (m.len(), m.modified().ok()));
    let (Some(a), Some(b)) = (assinatura(&hook), assinatura(recurso)) else { return false };
    if a.0 != b.0 {
        return false;
    }
    let mut g = VISTOS.lock().unwrap_or_else(|e| e.into_inner());
    let vistos = g.get_or_insert_with(HashMap::new);
    if let Some((x, y, igual)) = vistos.get(&hook) {
        if *x == a && *y == b {
            return *igual;
        }
    }
    let igual = match (std::fs::read(&hook), std::fs::read(recurso)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    };
    vistos.insert(hook, (a, b, igual));
    igual
}

/// As distros do registro com o estado de cada uma (as internas do Docker e do Rancher ficam de
/// fora). Só as rodando são olhadas por dentro. Sem saber quais rodam (o `wsl.exe` não respondeu),
/// erro: a tela diz que não deu para ler.
pub fn estado_das_distros(w: &dyn Wsl, homes: &mut Homes, ligadas: &[String], recurso: Option<&Path>) -> Result<Vec<DistroDto>, String> {
    let distros: Vec<String> = w.distros().into_iter().filter(|d| distro_valida(d) && !distro_interna(d)).collect();
    if distros.is_empty() {
        return Ok(vec![]);
    }
    let rodando = w.rodando().ok_or_else(|| crate::idioma::ta("erro.wsl_nao_respondeu").to_string())?;
    Ok(distros
        .into_iter()
        .map(|nome| {
            let ligada = ligadas.iter().any(|l| l == &nome);
            let estado = if !rodando.contains(&nome) {
                EstadoDistro::Parada
            } else {
                match homes.de(w, &nome) {
                    None => EstadoDistro::SemClaude,
                    Some(p) => {
                        let raiz = w.raiz(&nome);
                        if !no_windows(&raiz, &p.claude()).is_dir() {
                            EstadoDistro::SemClaude
                        } else if !ligada {
                            EstadoDistro::Desligada
                        } else if recurso.is_some_and(|r| !hook_igual(&raiz, &p, r)) || homes.falhou(&nome) {
                            EstadoDistro::Desatualizado
                        } else if estado_dos_hooks(w, &nome, &p) != EstadoHooks::Ligado {
                            EstadoDistro::SemHooks
                        } else {
                            EstadoDistro::Ligada
                        }
                    }
                }
            };
            DistroDto { nome, estado, ligada }
        })
        .collect())
}

/// O "Ligar" numa distro parada (0.6.1): a pessoa clicou, então o Chronn liga a distro (o
/// `wsl.exe` do Windows; o interop do Linux não entra) e confere que ela passou a rodar. Devolve
/// se precisou ligar. Uma distro que não é do registro: erro, sem ligar nada.
pub fn subir_se_parada(w: &dyn Wsl, distro: &str) -> Result<bool, String> {
    if !distro_valida(distro) || !w.distros().iter().any(|d| d == distro) {
        return Err(tfa("erro.wsl_desconhecida", &[distro]));
    }
    if w.rodando().unwrap_or_default().iter().any(|d| d == distro) {
        return Ok(false);
    }
    w.subir(distro)?;
    if !w.rodando().unwrap_or_default().iter().any(|d| d == distro) {
        return Err(tfa("erro.wsl_nao_subiu", &[distro]));
    }
    Ok(true)
}

/// As pastas da distro para ligar: ela existe, está rodando e tem o `~/.claude`.
pub fn preparar(w: &dyn Wsl, homes: &mut Homes, distro: &str) -> Result<PastasDaDistro, String> {
    if !distro_valida(distro) || !w.distros().iter().any(|d| d == distro) {
        return Err(tfa("erro.wsl_desconhecida", &[distro]));
    }
    if !w.rodando().unwrap_or_default().iter().any(|d| d == distro) {
        return Err(tfa("erro.wsl_parada", &[distro]));
    }
    let p = homes.de(w, distro).ok_or_else(|| tfa("erro.wsl_sem_home", &[distro]))?;
    if !no_windows(&w.raiz(distro), &p.claude()).is_dir() {
        return Err(tfa("erro.wsl_sem_claude", &[distro]));
    }
    Ok(p)
}

/// A prévia exata do `settings.json` da distro (a de hoje, `hooks_claude::previa`), sem gravar
/// nada, e o caminho Linux dele.
pub fn previa(w: &dyn Wsl, homes: &mut Homes, distro: &str) -> Result<(String, String), String> {
    let p = preparar(w, homes, distro)?;
    let settings = no_windows(&w.raiz(distro), &p.settings());
    Ok((hooks_claude::previa_do_arquivo(&settings, &comando_linux(&p))?, p.settings()))
}

/// Grava o `hook.json` da distro com as pastas ignoradas que estão nela (no caminho Linux), se
/// mudou. Devolve se gravou.
pub fn gravar_hook_json(w: &dyn Wsl, distro: &str, pastas: &PastasDaDistro, ignoradas: &[String]) -> Result<bool, String> {
    let caminho = no_windows(&w.raiz(distro), &pastas.hook_json());
    let mut j = serde_json::to_vec_pretty(&json!({ "pastas_ignoradas": ignoradas_da_distro(distro, ignoradas) })).map_err(|e| e.to_string())?;
    j.push(b'\n');
    if std::fs::read(&caminho).is_ok_and(|atual| atual == j) {
        return Ok(false);
    }
    if let Some(pai) = caminho.parent() {
        std::fs::create_dir_all(pai).map_err(|e| tfa("erro.criar_pasta", &[&e.to_string()]))?;
    }
    std::fs::write(&caminho, j).map_err(|e| tfa("erro.gravar", &[&e.to_string()]))?;
    Ok(true)
}

/// O recurso é um executável do Linux (começa com `\x7fELF`)? Um arquivo de mentira (um marcador
/// para os testes, um arquivo trocado) nunca vai para a distro (revisão M2).
pub fn e_elf(recurso: &Path) -> bool {
    use std::io::Read;
    let mut cabeca = [0u8; 4];
    std::fs::File::open(recurso).and_then(|mut f| f.read_exact(&mut cabeca)).is_ok() && cabeca == *b"\x7fELF"
}

/// Copia o hook para Linux (`recurso`) para a distro, se faltar ou mudou, e o torna executável
/// (com `sempre_chmod`, mesmo sem cópia). Só com o `chmod` feito a distro fica com o hook
/// confirmado (`Homes::executavel`); o que falhou é refeito na próxima vez. Devolve se copiou.
pub fn copiar_hook(w: &dyn Wsl, homes: &mut Homes, distro: &str, pastas: &PastasDaDistro, recurso: &Path, sempre_chmod: bool) -> Result<bool, String> {
    if !recurso.is_file() {
        return Err(tfa("erro.wsl_sem_hook", &[&recurso.display().to_string()]));
    }
    if !e_elf(recurso) {
        return Err(tfa("erro.wsl_hook_invalido", &[&recurso.display().to_string()]));
    }
    let destino = no_windows(&w.raiz(distro), &pastas.hook());
    let copiou = match crate::dados::atualizar_hook(recurso, &destino) {
        Ok(c) => c,
        Err(e) => {
            homes.executavel.remove(distro);
            homes.falhou.insert(distro.to_string());
            return Err(e);
        }
    };
    if copiou || sempre_chmod {
        homes.executavel.remove(distro);
        if let Err(e) = w.chmod(distro, &pastas.hook()) {
            homes.falhou.insert(distro.to_string());
            return Err(tfa("erro.wsl_chmod", &[distro, &e]));
        }
        homes.executavel.insert(distro.to_string());
        homes.falhou.remove(distro);
    }
    Ok(copiou)
}

/// "Ligar" o Claude Code da distro (com a prévia já vista):
/// 1. a distro existe, está rodando e tem o `~/.claude`;
/// 2. o hook para Linux vai para `~/.worktimer/bin/work-timer-hook`, com `chmod 755`;
/// 3. o `hook.json` de lá recebe as pastas ignoradas que estão nela;
/// 4. os hooks entram no `settings.json` de lá (com a cópia de segurança, como no Windows; os
///    finais de linha LF, e os hooks de outros programas ficam).
///
/// Devolve o caminho Linux da cópia de segurança ("" se não havia arquivo). Quem chama guarda a
/// distro na config (`wsl_distros`).
pub fn ligar(w: &dyn Wsl, homes: &mut Homes, distro: &str, recurso: &Path, ignoradas: &[String]) -> Result<String, String> {
    let p = preparar(w, homes, distro)?;
    copiar_hook(w, homes, distro, &p, recurso, true)?;
    gravar_hook_json(w, distro, &p, ignoradas)?;
    let raiz = w.raiz(distro);
    let backup = hooks_claude::instalar(&no_windows(&raiz, &p.settings()), &comando_linux(&p))?;
    Ok(backup.map(|b| linux_de(&raiz, &b)).unwrap_or_default())
}

/// "Desligar" o Claude Code da distro: tira só os nossos hooks do `settings.json` de lá (o
/// `remover` de hoje). Parada: erro (abrir a pasta dela a ligaria). Uma distro que saiu do
/// registro só sai da config (quem chama tira).
pub fn desligar(w: &dyn Wsl, homes: &mut Homes, distro: &str) -> Result<(), String> {
    if !w.distros().iter().any(|d| d == distro) {
        return Ok(());
    }
    if !w.rodando().unwrap_or_default().iter().any(|d| d == distro) {
        return Err(tfa("erro.wsl_parada_desligar", &[distro]));
    }
    let p = homes.de(w, distro).ok_or_else(|| tfa("erro.wsl_sem_home", &[distro]))?;
    hooks_claude::desinstalar(&no_windows(&w.raiz(distro), &p.settings()))?;
    homes.executavel.remove(distro);
    Ok(())
}

/// O estado dos hooks no `settings.json` da distro (rodando: quem chama confere).
pub fn estado_dos_hooks(w: &dyn Wsl, distro: &str, pastas: &PastasDaDistro) -> EstadoHooks {
    hooks_claude::estado(&no_windows(&w.raiz(distro), &pastas.settings()), &comando_linux(pastas))
}

/// O ciclo de 30 s do laço (rodada 28): das distros ligadas (`ligadas`, a config), as que estão
/// rodando têm a fila lida neste ciclo. A primeira vez que cada uma é vista rodando (e de novo
/// depois de parar, ou quando `conferidas` é esvaziado), o hook de lá é conferido contra o do
/// app (a cópia nova, se mudou) e recebe o `chmod` (revisão I3: o que falhou é refeito no ciclo
/// seguinte), e o `hook.json` é regravado se as ignoradas mudaram (revisão M5), sem mexer no
/// `settings.json`. Sem nenhuma distro ligada, o WSL nem é consultado. Uma distro parada nunca é
/// tocada. Devolve as filas (a distro e o caminho pelo Windows) e os erros, para o log.
pub fn ciclo_das_distros(
    w: &dyn Wsl,
    homes: &mut Homes,
    ligadas: &[String],
    ignoradas: &[String],
    recurso: Option<&Path>,
    conferidas: &mut std::collections::BTreeSet<String>,
) -> (Vec<(String, PathBuf)>, Vec<String>) {
    let mut erros = vec![];
    if ligadas.is_empty() {
        conferidas.clear();
        return (vec![], erros);
    }
    let Some(rodando) = w.rodando() else {
        return (vec![], vec!["não deu para saber quais distros do WSL rodam".into()]);
    };
    // a que parou é conferida de novo quando voltar
    conferidas.retain(|d| rodando.contains(d));
    let mut filas = vec![];
    for d in ligadas.iter().filter(|d| distro_valida(d) && rodando.contains(d)) {
        let Some(p) = homes.de(w, d) else {
            erros.push(tfa("erro.wsl_sem_home", &[d]));
            continue;
        };
        if !conferidas.contains(d) {
            let mut ok = true;
            if let Some(r) = recurso {
                if let Err(e) = copiar_hook(w, homes, d, &p, r, true) {
                    erros.push(e);
                    ok = false;
                }
            }
            if let Err(e) = gravar_hook_json(w, d, &p, ignoradas) {
                erros.push(e);
                ok = false;
            }
            if ok {
                conferidas.insert(d.clone());
            }
        }
        filas.push((d.clone(), no_windows(&w.raiz(d), &p.fila())));
    }
    (filas, erros)
}

/// Regrava o `hook.json` das distros ligadas e rodando (as pastas ignoradas mudaram).
pub fn regravar_hook_json(w: &dyn Wsl, homes: &mut Homes, ligadas: &[String], ignoradas: &[String]) -> Vec<String> {
    if ligadas.is_empty() {
        return vec![];
    }
    let rodando = w.rodando().unwrap_or_default();
    let mut erros = vec![];
    for d in ligadas.iter().filter(|d| distro_valida(d) && rodando.contains(d)) {
        match homes.de(w, d) {
            Some(p) => {
                if let Err(e) = gravar_hook_json(w, d, &p, ignoradas) {
                    erros.push(e);
                }
            }
            None => erros.push(tfa("erro.wsl_sem_home", &[d])),
        }
    }
    erros
}

/// O caminho Linux de um arquivo da distro visto pelo Windows (`raiz` + o resto).
fn linux_de(raiz: &Path, p: &Path) -> String {
    match p.strip_prefix(raiz) {
        Ok(resto) => format!("/{}", resto.to_string_lossy().replace('\\', "/")),
        Err(_) => p.to_string_lossy().into_owned(),
    }
}

/// Um WSL de mentira para os testes: as distros e quais rodam, e uma pasta no lugar de cada uma
/// (`<raiz>/<distro>`). Guarda os `chmod` pedidos. Nunca toca no WSL de verdade.
#[derive(Debug, Default)]
pub struct WslFalso {
    pub raiz: PathBuf,
    pub distros: Vec<String>,
    pub rodando: Vec<String>,
    pub homes: BTreeMap<String, String>,
    pub chmods: std::sync::Mutex<Vec<(String, String)>>,
    /// Quantas vezes a pasta do usuário foi pedida (o cache evita pedir de novo).
    pub pedidos_home: std::sync::atomic::AtomicUsize,
    /// Os próximos `chmod` que falham (um erro passageiro do `wsl.exe`).
    pub falhas_chmod: std::sync::atomic::AtomicUsize,
    /// O `rodando()` sem resposta (o `wsl.exe` passou do prazo).
    pub sem_resposta: bool,
    /// As distros ligadas pelo `subir` (0.6.1): passam a rodar.
    pub subidas: std::sync::Mutex<Vec<String>>,
    /// O `subir` falha (o `wsl.exe` não ligou a distro).
    pub subir_falha: bool,
}

impl WslFalso {
    fn roda(&self, distro: &str) -> bool {
        self.rodando.iter().any(|d| d == distro) || self.subidas.lock().unwrap().iter().any(|d| d == distro)
    }
}

impl Wsl for WslFalso {
    fn distros(&self) -> Vec<String> {
        self.distros.clone()
    }
    fn rodando(&self) -> Option<Vec<String>> {
        (!self.sem_resposta).then(|| {
            let mut v = self.rodando.clone();
            v.extend(self.subidas.lock().unwrap().iter().cloned());
            v
        })
    }
    fn home(&self, distro: &str) -> Option<String> {
        assert!(self.roda(distro), "o Chronn nunca pergunta a uma distro parada");
        self.pedidos_home.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.homes.get(distro).cloned()
    }
    fn chmod(&self, distro: &str, caminho: &str) -> Result<(), String> {
        assert!(self.roda(distro), "o Chronn nunca roda nada numa distro parada");
        self.chmods.lock().unwrap().push((distro.to_string(), caminho.to_string()));
        let falhas = &self.falhas_chmod;
        if falhas.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            falhas.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            return Err("erro passageiro".into());
        }
        Ok(())
    }
    fn raiz(&self, distro: &str) -> PathBuf {
        self.raiz.join(distro)
    }
    fn subir(&self, distro: &str) -> Result<(), String> {
        if self.subir_falha {
            return Err("o wsl.exe não respondeu".into());
        }
        self.subidas.lock().unwrap().push(distro.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;
    use std::sync::atomic::Ordering;

    /// Duas distros: Ubuntu rodando (com o Claude Code e um settings com os hooks de outro
    /// programa, com LF) e Debian parada.
    fn wsl() -> (tempfile::TempDir, WslFalso) {
        let d = tempfile::tempdir().unwrap();
        let w = WslFalso {
            raiz: d.path().to_path_buf(),
            distros: vec!["Ubuntu".into(), "Debian".into()],
            rodando: vec!["Ubuntu".into()],
            homes: [("Ubuntu".to_string(), "/home/u\n".to_string())].into(),
            ..Default::default()
        };
        let claude = d.path().join("Ubuntu").join("home").join("u").join(".claude");
        fs::create_dir_all(&claude).unwrap();
        let outro = "{\n  \"hooks\": {\n    \"SessionStart\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"outro-programa start\"}]}],\n    \"UserPromptSubmit\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"outro-programa prompt\"}]}],\n    \"PreToolUse\": [{\"matcher\": \"*\", \"hooks\": [{\"type\": \"command\", \"command\": \"outro-programa pre\"}]}],\n    \"PostToolUse\": [{\"matcher\": \"*\", \"hooks\": [{\"type\": \"command\", \"command\": \"outro-programa post\"}]}],\n    \"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"outro-programa stop\"}]}]\n  }\n}\n";
        fs::write(claude.join("settings.json"), outro).unwrap();
        (d, w)
    }
    /// O recurso do app: um "ELF" de mentira com `nome` depois da marca.
    fn recurso(d: &Path, nome: &str) -> PathBuf {
        let r = d.join("recurso-work-timer-hook-linux");
        let mut b = b"\x7fELF ".to_vec();
        b.extend_from_slice(nome.as_bytes());
        fs::write(&r, b).unwrap();
        r
    }
    fn hook(d: &Path) -> Vec<u8> {
        fs::read(d.join("Ubuntu/home/u/.worktimer/bin/work-timer-hook")).unwrap()
    }

    #[test]
    fn os_caminhos_da_distro_e_a_traducao_do_cwd() {
        let p = PastasDaDistro { home: "/home/u".into() };
        assert_eq!(p.fila(), "/home/u/.worktimer/fila/eventos.jsonl");
        assert_eq!(p.hook(), "/home/u/.worktimer/bin/work-timer-hook");
        assert_eq!(p.hook_json(), "/home/u/.worktimer/hook.json");
        assert_eq!(p.settings(), "/home/u/.claude/settings.json");
        assert_eq!(comando_linux(&p), "\"/home/u/.worktimer/bin/work-timer-hook\"");
        assert_eq!(no_windows(Path::new(r"\\wsl.localhost\Ubuntu"), &p.fila()), PathBuf::from(r"\\wsl.localhost\Ubuntu\home\u\.worktimer\fila\eventos.jsonl"));
        assert_eq!(traduzir_cwd("Ubuntu", "/home/u/proj"), r"\\wsl.localhost\Ubuntu\home\u\proj");
        assert_eq!(traduzir_cwd("Ubuntu", "/mnt/c/Users/x"), r"C:\Users\x");
        assert_eq!(traduzir_cwd("Ubuntu", "/mnt/d"), r"D:\");
        assert_eq!(traduzir_cwd("Ubuntu", "/mnt/wsl/x"), r"\\wsl.localhost\Ubuntu\mnt\wsl\x", "não é uma letra");
        assert_eq!(traduzir_cwd("Ubuntu", "/"), r"\\wsl.localhost\Ubuntu");
        assert_eq!(traduzir_cwd("Ubuntu", r"C:\x"), r"C:\x", "já é do Windows");
        assert_eq!(home_valida("/home/u\n").as_deref(), Some("/home/u"));
        for ruim in ["", "home/u", "/", "/home/../etc", "/home/u\"x", "/home/a\\b"] {
            assert_eq!(home_valida(ruim), None, "{ruim:?}");
        }
        assert!(distro_valida("Ubuntu-22.04") && distro_valida("Debian"));
        for ruim in ["", "a b", "a\\b", "a/b", "x\"y", &"a".repeat(65)] {
            assert!(!distro_valida(ruim), "{ruim:?}");
        }
        // revisão M9: as distros internas do Docker e do Rancher
        for interna in ["docker-desktop", "docker-desktop-data", "rancher-desktop", "rancher-desktop-data"] {
            assert!(distro_interna(interna), "{interna}");
        }
        assert!(!distro_interna("Ubuntu") && !distro_interna("docker"));
    }

    #[test]
    fn o_evento_da_distro_traduz_o_cwd_o_transcript_e_a_origem() {
        let ev = EventoHook {
            t: 1,
            evento: "Stop".into(),
            sessao: "s".into(),
            cwd: "/home/u/p".into(),
            transcript: Some("/home/u/.claude/projects/-home-u-p/s.jsonl".into()),
            notificacao: None,
            origem: Some("cli".into()),
            conta: None,
        };
        let e = evento_da_distro("Ubuntu", ev.clone());
        assert_eq!((e.cwd.as_str(), e.origem.as_deref()), (r"\\wsl.localhost\Ubuntu\home\u\p", Some("wsl")));
        // revisão I4: o transcript também
        assert_eq!(e.transcript.as_deref(), Some(r"\\wsl.localhost\Ubuntu\home\u\.claude\projects\-home-u-p\s.jsonl"));
        let vscode = evento_da_distro("Ubuntu", EventoHook { origem: Some("claude-vscode".into()), transcript: None, ..ev });
        assert_eq!((vscode.origem.as_deref(), vscode.transcript), (Some("claude-vscode"), None), "o VS Code continua VS Code");
    }

    #[test]
    fn so_toca_um_caminho_do_wsl_com_a_distro_rodando() {
        // um nome que só este teste usa (o conjunto é do processo)
        assert_eq!(distro_do_caminho(r"\\wsl$\TesteRodando\home\u").as_deref(), Some("testerodando"));
        assert_eq!(distro_do_caminho(r"\\wsl.localhost\Ubuntu").as_deref(), Some("ubuntu"));
        assert_eq!(distro_do_caminho(r"C:\x"), None);
        assert_eq!(distro_do_caminho(r"\\servidor\x"), None);
        assert!(pode_tocar(r"C:\x") && pode_tocar(r"\\servidor\pasta"), "fora do WSL, sempre");
        assert!(!pode_tocar(r"\\wsl.localhost\TesteRodando\home\u"), "sem saber: não");
        let t0 = std::time::Instant::now();
        definir_rodando_em(&["TesteRodando".into()], t0);
        assert!(pode_tocar(r"\\wsl$\testerodando\home\u"));
        assert!(!pode_tocar(r"\\wsl.localhost\OutraDistroDoTeste\home\u"));
        // re-revisão N1: a lista vence (sem distro ligada, ninguém pergunta mais)
        assert!(pode_tocar_em(r"\\wsl$\testerodando\home\u", t0 + VALIDADE_RODANDO));
        // 0.6.1: a leitura rápida só logo depois da consulta
        let dez = std::time::Duration::from_secs(2);
        assert!(pode_tocar_recente_em(r"\\wsl$\testerodando\home\u", dez, t0 + dez));
        assert!(!pode_tocar_recente_em(r"\\wsl$\testerodando\home\u", dez, t0 + dez + std::time::Duration::from_secs(1)));
        assert!(pode_tocar_recente_em(r"C:\x", dez, t0 + VALIDADE_RODANDO * 2), "fora do WSL, sempre");
        assert_eq!(
            claude_json_da_fila(Path::new(r"\\wsl.localhost\Ubuntu\home\u\.worktimer\fila\eventos.jsonl")),
            Some(PathBuf::from(r"\\wsl.localhost\Ubuntu\home\u\.claude.json"))
        );
        assert!(!pode_tocar_em(r"\\wsl$\testerodando\home\u", t0 + VALIDADE_RODANDO + std::time::Duration::from_secs(1)));
        definir_rodando(&[]);
        assert!(!pode_tocar(r"\\wsl.localhost\TesteRodando\home\u"));
    }

    #[test]
    fn as_ignoradas_da_distro_viram_caminhos_linux_e_as_outras_ficam_de_fora() {
        let ign: Vec<String> = [
            r"\\wsl.localhost\Ubuntu\home\u\Pessoal",
            r"\\wsl$\ubuntu\home\u\notas\",
            r"\\wsl.localhost\Debian\home\u\x",
            r"G:\pessoal",
            r"\\wsl.localhost\Ubuntu2\x",
            // revisão M6: letras cuja minúscula muda de tamanho não deslocam o corte
            r"\\WSL.LOCALHOST\UBUNTU\home\u\İzmir\Ωhm",
            r"\\wsl.localhost\Ubuntu",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(ignoradas_da_distro("Ubuntu", &ign), ["/home/u/Pessoal", "/home/u/notas", "/home/u/İzmir/Ωhm", "/"]);
        assert_eq!(ignoradas_da_distro("Debian", &ign), ["/home/u/x"]);
        assert!(ignoradas_da_distro("Alpine", &ign).is_empty());
        assert!(ignoradas_da_distro("İ", &["\\\\wsl.localhost\\İ\\x".to_string()]).len() <= 1, "sem pânico");
    }

    #[test]
    fn ligar_copia_o_hook_com_chmod_grava_o_hook_json_e_mescla_mantendo_os_hooks_do_outro() {
        let (d, w) = wsl();
        let r = recurso(d.path(), "hook");
        let mut homes = Homes::default();
        let (previa, settings) = previa(&w, &mut homes, "Ubuntu").unwrap();
        assert_eq!(settings, "/home/u/.claude/settings.json");
        assert!(previa.contains("outro-programa pre") && previa.contains("/home/u/.worktimer/bin/work-timer-hook"), "{previa}");
        let antes = fs::read_to_string(d.path().join("Ubuntu/home/u/.claude/settings.json")).unwrap();
        let ign = vec![r"\\wsl.localhost\Ubuntu\home\u\pessoal".to_string(), r"G:\x".to_string()];
        let backup = ligar(&w, &mut homes, "Ubuntu", &r, &ign).unwrap();
        assert!(backup.starts_with("/home/u/.claude/settings.json.worktimer-backup-"), "{backup}");
        assert_eq!(fs::read_to_string(no_windows(&w.raiz("Ubuntu"), &backup)).unwrap(), antes, "a cópia é o de antes");
        // o hook, executável
        assert_eq!(hook(d.path()), b"\x7fELF hook");
        assert_eq!(*w.chmods.lock().unwrap(), [("Ubuntu".to_string(), "/home/u/.worktimer/bin/work-timer-hook".to_string())]);
        assert!(homes.executavel("Ubuntu"));
        // o hook.json com as ignoradas da distro
        let hj: serde_json::Value = serde_json::from_slice(&fs::read(d.path().join("Ubuntu/home/u/.worktimer/hook.json")).unwrap()).unwrap();
        assert_eq!(hj, json!({"pastas_ignoradas": ["/home/u/pessoal"]}));
        // o settings: os do outro programa ficam, os nossos entram, com LF
        let texto = fs::read_to_string(d.path().join("Ubuntu/home/u/.claude/settings.json")).unwrap();
        assert!(!texto.contains('\r'), "finais de linha LF");
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        for (evento, cmd) in [("SessionStart", "outro-programa start"), ("UserPromptSubmit", "outro-programa prompt"), ("PreToolUse", "outro-programa pre"), ("PostToolUse", "outro-programa post"), ("Stop", "outro-programa stop")] {
            assert_eq!(v["hooks"][evento][0]["hooks"][0]["command"], cmd, "{evento}");
        }
        assert_eq!(estado_dos_hooks(&w, "Ubuntu", &PastasDaDistro { home: "/home/u".into() }), EstadoHooks::Ligado);
        assert!(texto.contains("\"\\\"/home/u/.worktimer/bin/work-timer-hook\\\"\""), "o caminho Linux entre aspas");
        assert_eq!(w.pedidos_home.load(Ordering::SeqCst), 1, "a pasta do usuário é pedida uma vez");
        // os estados
        let e = estado_das_distros(&w, &mut homes, &["Ubuntu".into()], Some(&r)).unwrap();
        assert_eq!(e, vec![
            DistroDto { nome: "Ubuntu".into(), estado: EstadoDistro::Ligada, ligada: true },
            DistroDto { nome: "Debian".into(), estado: EstadoDistro::Parada, ligada: false },
        ]);
        // um hook novo no app: desatualizado, e a cópia leva o chmod sem mexer no settings
        let r2 = recurso(d.path(), "hook novo");
        assert_eq!(estado_das_distros(&w, &mut homes, &["Ubuntu".into()], Some(&r2)).unwrap()[0].estado, EstadoDistro::Desatualizado);
        let p = homes.de(&w, "Ubuntu").unwrap();
        assert!(copiar_hook(&w, &mut homes, "Ubuntu", &p, &r2, false).unwrap());
        assert!(!copiar_hook(&w, &mut homes, "Ubuntu", &p, &r2, false).unwrap(), "igual: nada");
        assert_eq!(w.chmods.lock().unwrap().len(), 2);
        assert_eq!(fs::read_to_string(d.path().join("Ubuntu/home/u/.claude/settings.json")).unwrap(), texto, "o settings não muda");
        // 0.6.1: outro programa regravou o settings.json sem os nossos hooks: "sem hooks", e o
        // "Ligar" de novo devolve os hooks
        let settings = d.path().join("Ubuntu/home/u/.claude/settings.json");
        fs::write(&settings, "{\"hooks\": {}}\n").unwrap();
        assert_eq!(estado_das_distros(&w, &mut homes, &["Ubuntu".into()], Some(&r2)).unwrap()[0].estado, EstadoDistro::SemHooks);
        ligar(&w, &mut homes, "Ubuntu", &r2, &[]).unwrap();
        assert_eq!(estado_das_distros(&w, &mut homes, &["Ubuntu".into()], Some(&r2)).unwrap()[0].estado, EstadoDistro::Ligada);
        fs::write(&settings, &texto).unwrap();
        // desligar: só os nossos saem
        desligar(&w, &mut homes, "Ubuntu").unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(d.path().join("Ubuntu/home/u/.claude/settings.json")).unwrap()).unwrap();
        assert!(!v.to_string().contains("work-timer-hook"));
        assert_eq!(v["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "outro-programa pre");
        assert_eq!(estado_das_distros(&w, &mut homes, &[], Some(&r2)).unwrap()[0].estado, EstadoDistro::Desligada);
    }

    #[test]
    fn o_ciclo_le_so_as_ligadas_e_rodando_e_confere_o_hook_e_o_hook_json_uma_vez() {
        let (d, mut w) = wsl();
        let r = recurso(d.path(), "v1");
        let mut homes = Homes::default();
        let mut conferidas = std::collections::BTreeSet::new();
        // sem nada ligado: o WSL nem é consultado
        assert_eq!(ciclo_das_distros(&w, &mut homes, &[], &[], Some(&r), &mut conferidas), (vec![], vec![]));
        assert_eq!(w.pedidos_home.load(Ordering::SeqCst), 0);
        ligar(&w, &mut homes, "Ubuntu", &r, &[]).unwrap();
        let settings = fs::read_to_string(d.path().join("Ubuntu/home/u/.claude/settings.json")).unwrap();
        let ligadas = vec!["Ubuntu".to_string(), "Debian".to_string()];
        // um hook novo no app: a cópia vai uma vez, com o chmod, e o settings não muda
        let r2 = recurso(d.path(), "v2");
        let (filas, erros) = ciclo_das_distros(&w, &mut homes, &ligadas, &[], Some(&r2), &mut conferidas);
        assert!(erros.is_empty(), "{erros:?}");
        assert_eq!(filas, vec![("Ubuntu".to_string(), d.path().join("Ubuntu/home/u/.worktimer/fila/eventos.jsonl"))], "a Debian parada fica de fora");
        assert_eq!(hook(d.path()), b"\x7fELF v2");
        assert_eq!(w.chmods.lock().unwrap().len(), 2, "o do ligar e o da conferência");
        ciclo_das_distros(&w, &mut homes, &ligadas, &[], Some(&r2), &mut conferidas);
        assert_eq!(w.chmods.lock().unwrap().len(), 2, "conferido: nada de novo");
        assert_eq!(fs::read_to_string(d.path().join("Ubuntu/home/u/.claude/settings.json")).unwrap(), settings);
        assert!(!d.path().join("Debian").exists());
        // a Ubuntu parou: nada é lido; e as ignoradas mudaram com ela parada
        w.rodando = vec![];
        let ign = vec![r"\\wsl.localhost\Ubuntu\home\u\y".to_string()];
        assert!(ciclo_das_distros(&w, &mut homes, &ligadas, &ign, Some(&r2), &mut conferidas).0.is_empty());
        assert!(regravar_hook_json(&w, &mut homes, &ligadas, &ign).is_empty());
        assert!(conferidas.is_empty());
        // revisão M5: voltou, é conferida de novo e o hook.json recebe as ignoradas de agora
        w.rodando = vec!["Ubuntu".into()];
        ciclo_das_distros(&w, &mut homes, &ligadas, &ign, Some(&r2), &mut conferidas);
        let hj: serde_json::Value = serde_json::from_slice(&fs::read(d.path().join("Ubuntu/home/u/.worktimer/hook.json")).unwrap()).unwrap();
        assert_eq!(hj["pastas_ignoradas"], json!(["/home/u/y"]));
        // e a regravação pela tela também, só com ela rodando
        assert!(regravar_hook_json(&w, &mut homes, &ligadas, &[]).is_empty());
        let hj: serde_json::Value = serde_json::from_slice(&fs::read(d.path().join("Ubuntu/home/u/.worktimer/hook.json")).unwrap()).unwrap();
        assert_eq!(hj["pastas_ignoradas"], json!([]));
    }

    #[test]
    fn o_chmod_que_falhou_e_refeito_e_a_tela_mostra_desatualizado_ate_la() {
        let (d, w) = wsl();
        let r = recurso(d.path(), "v1");
        let mut homes = Homes::default();
        let mut conferidas = std::collections::BTreeSet::new();
        ligar(&w, &mut homes, "Ubuntu", &r, &[]).unwrap();
        let ligadas = vec!["Ubuntu".to_string()];
        // a versão nova: a cópia vai, o chmod falha uma vez
        let r2 = recurso(d.path(), "v2");
        w.falhas_chmod.store(1, Ordering::SeqCst);
        let (filas, erros) = ciclo_das_distros(&w, &mut homes, &ligadas, &[], Some(&r2), &mut conferidas);
        assert_eq!((filas.len(), erros.len()), (1, 1), "a fila segue lida; o erro vai para o log");
        assert_eq!(hook(d.path()), b"\x7fELF v2", "os bytes já são os novos");
        assert!(!homes.executavel("Ubuntu"));
        assert_eq!(estado_das_distros(&w, &mut homes, &ligadas, Some(&r2)).unwrap()[0].estado, EstadoDistro::Desatualizado, "não ligada de verdade");
        // o ciclo seguinte pede o chmod de novo, mesmo sem cópia
        let antes = w.chmods.lock().unwrap().len();
        let (_, erros) = ciclo_das_distros(&w, &mut homes, &ligadas, &[], Some(&r2), &mut conferidas);
        assert!(erros.is_empty(), "{erros:?}");
        assert_eq!(w.chmods.lock().unwrap().len(), antes + 1);
        assert!(homes.executavel("Ubuntu"));
        assert_eq!(estado_das_distros(&w, &mut homes, &ligadas, Some(&r2)).unwrap()[0].estado, EstadoDistro::Ligada);
        // re-revisão N7: ao abrir o app, antes da primeira conferência, "ligada" (os bytes iguais);
        // numa build sem o recurso, também, e não "desatualizado" para sempre
        let mut novo = Homes::default();
        assert_eq!(estado_das_distros(&w, &mut novo, &ligadas, Some(&r2)).unwrap()[0].estado, EstadoDistro::Ligada);
        assert_eq!(estado_das_distros(&w, &mut novo, &ligadas, None).unwrap()[0].estado, EstadoDistro::Ligada);
    }

    #[test]
    fn um_recurso_que_nao_e_elf_nunca_vai_para_a_distro() {
        let (d, w) = wsl();
        let falso = d.path().join("marcador");
        fs::write(&falso, b"").unwrap();
        let mut homes = Homes::default();
        let e = ligar(&w, &mut homes, "Ubuntu", &falso, &[]).unwrap_err();
        assert!(e.contains("não é um executável do Linux"), "{e}");
        fs::write(&falso, b"MZ\x90\x00 um exe do Windows").unwrap();
        assert!(ligar(&w, &mut homes, "Ubuntu", &falso, &[]).is_err());
        assert!(!d.path().join("Ubuntu/home/u/.worktimer/bin/work-timer-hook").exists());
        assert!(w.chmods.lock().unwrap().is_empty());
        assert!(e_elf(&recurso(d.path(), "x")));
    }

    /// 0.6.1: o "Ligar" numa distro parada liga a distro só no clique, e só uma do registro.
    #[test]
    fn o_ligar_na_distro_parada_sobe_ela_so_no_clique() {
        let (d, w) = wsl();
        let claude = d.path().join("Debian").join("home").join("u").join(".claude");
        fs::create_dir_all(&claude).unwrap();
        let mut w = WslFalso { homes: [("Ubuntu".to_string(), "/home/u".to_string()), ("Debian".to_string(), "/home/u".to_string())].into(), ..w };
        let mut homes = Homes::default();
        assert!(previa(&w, &mut homes, "Debian").unwrap_err().contains("parada"), "sem o clique, não sobe");
        assert!(w.subidas.lock().unwrap().is_empty());
        assert_eq!(subir_se_parada(&w, "Debian"), Ok(true));
        assert_eq!(*w.subidas.lock().unwrap(), ["Debian"]);
        assert!(previa(&w, &mut homes, "Debian").is_ok());
        assert_eq!(subir_se_parada(&w, "Debian"), Ok(false), "já roda: nada");
        assert_eq!(subir_se_parada(&w, "Ubuntu"), Ok(false));
        assert!(subir_se_parada(&w, "Alpine").unwrap_err().contains("desconhecida"));
        assert_eq!(w.subidas.lock().unwrap().len(), 1);
        // o wsl.exe não ligou: erro, e a prévia não sai
        w.subidas.lock().unwrap().clear();
        w.subir_falha = true;
        assert!(subir_se_parada(&w, "Debian").is_err());
    }

    #[test]
    fn sem_resposta_do_wsl_a_tela_diz_que_nao_deu_para_ler_e_as_internas_ficam_de_fora() {
        let (_d, mut w) = wsl();
        w.distros.push("docker-desktop".into());
        let mut homes = Homes::default();
        let e = estado_das_distros(&w, &mut homes, &[], None).unwrap();
        assert_eq!(e.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>(), ["Ubuntu", "Debian"]);
        w.sem_resposta = true;
        assert!(estado_das_distros(&w, &mut homes, &[], None).is_err());
        w.distros.clear();
        assert_eq!(estado_das_distros(&w, &mut homes, &[], None).unwrap(), vec![], "sem distros: nem pergunta");
    }

    #[test]
    fn uma_distro_parada_ou_sem_claude_code_nunca_e_ligada_nem_aberta() {
        let (d, w) = wsl();
        let r = recurso(d.path(), "hook");
        let mut homes = Homes::default();
        // parada: o falso entra em pânico se o Chronn pedir algo a ela
        let e = ligar(&w, &mut homes, "Debian", &r, &[]).unwrap_err();
        assert!(e.contains("parada"), "{e}");
        assert!(desligar(&w, &mut homes, "Debian").unwrap_err().contains("parada"));
        assert!(!d.path().join("Debian").exists(), "a pasta da distro parada nunca é tocada");
        assert!(ligar(&w, &mut homes, "Alpine", &r, &[]).unwrap_err().contains("desconhecida"));
        assert!(desligar(&w, &mut homes, "Alpine").is_ok(), "fora do registro: só sai da config");
        // rodando sem o ~/.claude
        let w2 = WslFalso { rodando: vec!["Ubuntu".into(), "Debian".into()], homes: [("Debian".to_string(), "/home/d".to_string())].into(), raiz: w.raiz.clone(), distros: w.distros.clone(), ..Default::default() };
        let mut homes = Homes::default();
        assert!(ligar(&w2, &mut homes, "Debian", &r, &[]).unwrap_err().contains("Claude Code"));
        assert_eq!(estado_das_distros(&w2, &mut homes, &[], Some(&r)).unwrap()[1].estado, EstadoDistro::SemClaude);
        // sem o hook para Linux no app
        let mut homes = Homes::default();
        assert!(ligar(&w, &mut homes, "Ubuntu", &d.path().join("nao-existe"), &[]).unwrap_err().contains("hook para Linux"));
        drop(d);
    }
}
