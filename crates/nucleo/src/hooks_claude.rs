// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Instalador dos hooks do Work Timer no `settings.json` do Claude Code.
//! Todas as funções recebem o caminho explícito; nada aqui conhece o perfil real.

use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const EVENTOS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "Notification",
    "SessionEnd",
    "PostToolUse",
];

/// O que marca um comando como nosso: o nome do executável do hook.
pub const MARCA: &str = "work-timer-hook";
/// As marcas reconhecidas para atualizar e remover.
pub const MARCAS: [&str; 1] = [MARCA];

fn hook_e_nosso(hook: &Value, marcas: &[&str]) -> bool {
    hook["command"].as_str().is_some_and(|c| marcas.iter().any(|m| c.contains(m)))
}

/// Tira os nossos comandos de dentro de cada grupo; o grupo só sai se ficar vazio
/// (um grupo pode ter também o hook de outra pessoa). Devolve se tirou alguma coisa.
fn tirar_os_nossos(lista: &mut Vec<Value>, marcas: &[&str]) -> bool {
    let mut tirou = false;
    lista.retain_mut(|g| {
        let Some(hs) = g.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let antes = hs.len();
        hs.retain(|h| !hook_e_nosso(h, marcas));
        if hs.len() == antes {
            return true;
        }
        tirou = true;
        !hs.is_empty()
    });
    tirou
}

pub fn mesclar(mut settings: Value, comando: &str) -> Value {
    if !settings.is_object() {
        settings = json!({});
    }
    let raiz = settings.as_object_mut().unwrap();
    let hooks = raiz.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks = hooks.as_object_mut().unwrap();
    for evento in EVENTOS {
        let lista = hooks.entry(evento).or_insert_with(|| json!([]));
        if !lista.is_array() {
            *lista = json!([]);
        }
        let lista = lista.as_array_mut().unwrap();
        // o comando antigo (ou de outra pasta) sai: fica só o de agora
        tirar_os_nossos(lista, &MARCAS);
        let mut grupo = json!({"hooks":[{"type":"command","command":comando,"timeout":5}]});
        if evento == "PostToolUse" {
            grupo = json!({"matcher":"*","hooks":[{"type":"command","command":comando,"timeout":5}]});
        }
        lista.push(grupo);
    }
    settings
}

pub fn remover(mut settings: Value, marcas: &[&str]) -> Value {
    let Some(raiz) = settings.as_object_mut() else {
        return settings;
    };
    // Só some o que ficou vazio por nossa causa: lista ou `hooks` que já eram vazios ficam como estavam.
    let mut tirou = false;
    if let Some(hooks) = raiz.get_mut("hooks").and_then(Value::as_object_mut) {
        hooks.retain(|_, lista| {
            let Some(l) = lista.as_array_mut() else {
                return true;
            };
            if !tirar_os_nossos(l, marcas) {
                return true;
            }
            tirou = true;
            !l.is_empty()
        });
    }
    if tirou && raiz.get("hooks").and_then(Value::as_object).is_some_and(|h| h.is_empty()) {
        raiz.remove("hooks");
    }
    settings
}

pub fn previa(antes: &Value, depois: &Value) -> String {
    previa_em(crate::idioma::atual(), antes, depois)
}

/// A prévia no idioma `i`: "Antes:" e "Depois:" ("Before:" e "After:") com os blocos de hooks.
pub fn previa_em(i: crate::idioma::Idioma, antes: &Value, depois: &Value) -> String {
    use crate::idioma::t;
    let bloco = |v: &Value| match v.get("hooks") {
        Some(h) => serde_json::to_string_pretty(h).unwrap_or_default(),
        None => t(i, "hooks.sem").to_string(),
    };
    format!("{}:
{}

{}:
{}", t(i, "hooks.antes"), bloco(antes), t(i, "hooks.depois"), bloco(depois))
}

/// A prévia do `settings.json` em `caminho` com os hooks de `comando`, sem gravar nada (o arquivo
/// que falta ou está vazio vale como `{}`).
pub fn previa_do_arquivo(caminho: &Path, comando: &str) -> Result<String, String> {
    let antes = match std::fs::read_to_string(caminho) {
        Ok(t) if t.trim().is_empty() => json!({}),
        Ok(t) => serde_json::from_str(&t).map_err(|e| crate::idioma::tfa("erro.json", &[&e.to_string()]))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(crate::idioma::tfa("erro.ler", &[&e.to_string()])),
    };
    let depois = mesclar(antes.clone(), comando);
    Ok(previa(&antes, &depois))
}

fn ler(caminho: &Path) -> Result<Option<(String, Value)>, String> {
    if !caminho.exists() {
        return Ok(None);
    }
    let texto = std::fs::read_to_string(caminho).map_err(|e| crate::idioma::tfa("erro.ler", &[&e.to_string()]))?;
    let valor: Value = if texto.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&texto).map_err(|e| crate::idioma::tfa("erro.json", &[&e.to_string()]))?
    };
    if !valor.is_object() {
        return Err(crate::idioma::ta("erro.json_raiz").into());
    }
    Ok(Some((texto, valor)))
}

fn gravar(caminho: &Path, valor: &Value) -> Result<(), String> {
    let mut texto = serde_json::to_string_pretty(valor).map_err(|e| e.to_string())?;
    texto.push('\n');
    if let Some(pai) = caminho.parent() {
        std::fs::create_dir_all(pai).map_err(|e| crate::idioma::tfa("erro.criar_pasta", &[&e.to_string()]))?;
    }
    let mut tmp = caminho.as_os_str().to_owned();
    tmp.push(".worktimer-tmp");
    let tmp = PathBuf::from(tmp);
    let escrever = || -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(texto.as_bytes())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, caminho)
    };
    escrever().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        crate::idioma::tfa("erro.gravar", &[&e.to_string()])
    })
}

fn copia_de_seguranca(caminho: &Path, texto: &str) -> Result<PathBuf, String> {
    let carimbo = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let mut nome = caminho.as_os_str().to_owned();
    nome.push(format!(".worktimer-backup-{carimbo}"));
    let base = nome.to_string_lossy().into_owned();
    for n in 0.. {
        let destino = if n == 0 {
            PathBuf::from(&base)
        } else {
            PathBuf::from(format!("{base}-{n}"))
        };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&destino) {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(texto.as_bytes())
                    .map_err(|e| crate::idioma::tfa("erro.copia_seguranca", &[&e.to_string()]))?;
                return Ok(destino);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(crate::idioma::tfa("erro.copia_seguranca", &[&e.to_string()])),
        }
    }
    unreachable!()
}

pub fn instalar(caminho: &Path, comando: &str) -> Result<Option<PathBuf>, String> {
    match ler(caminho)? {
        None => {
            gravar(caminho, &mesclar(json!({}), comando))?;
            Ok(None)
        }
        Some((texto, valor)) => {
            let backup = copia_de_seguranca(caminho, &texto)?;
            gravar(caminho, &mesclar(valor, comando))?;
            Ok(Some(backup))
        }
    }
}

pub fn desinstalar(caminho: &Path) -> Result<Option<PathBuf>, String> {
    match ler(caminho)? {
        None => Ok(None),
        Some((texto, valor)) => {
            let depois = remover(valor.clone(), &MARCAS);
            // nada nosso no arquivo: não regrava nem faz cópia
            if depois == valor {
                return Ok(None);
            }
            let backup = copia_de_seguranca(caminho, &texto)?;
            gravar(caminho, &depois)?;
            Ok(Some(backup))
        }
    }
}

/// O que o `work-timer.exe --remover-hooks` fez (o desinstalador, rodada 16).
#[derive(Debug, PartialEq, Eq)]
pub struct RemocaoAoDesinstalar {
    /// A cópia de segurança do `settings.json`, se ele mudou; o erro, se não deu para tirar.
    pub hooks: Result<Option<PathBuf>, String>,
    /// O `work-timer-hook.exe` foi apagado (ou já não estava lá).
    pub exe_apagado: bool,
}

/// Tira os nossos hooks do `settings.json` (com a cópia de segurança de sempre) e, se eles saíram
/// (ou não havia nenhum), apaga a cópia do hook (`hook_exe`). Se não deu para tirar (por exemplo,
/// o `settings.json` está inválido), o exe fica: o Claude Code seguiria chamando o hook, e sem o
/// exe ele mostraria erro em toda mensagem.
pub fn remover_ao_desinstalar(settings: &Path, hook_exe: &Path) -> RemocaoAoDesinstalar {
    let hooks = desinstalar(settings);
    let exe_apagado = hooks.is_ok()
        && match std::fs::remove_file(hook_exe) {
            Ok(()) => true,
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        };
    RemocaoAoDesinstalar { hooks, exe_apagado }
}

/// Como estão os hooks no `settings.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoHooks {
    /// Nenhum hook nosso.
    Desligado,
    /// Há hooks nossos, mas não exatamente os de agora (o comando da versão anterior, de outra
    /// pasta ou faltando evento): a pessoa decide se atualiza.
    Antigo,
    /// Os seis eventos chamam `comando`, e nenhum hook nosso chama outra coisa.
    Ligado,
}

pub fn estado(caminho: &Path, comando: &str) -> EstadoHooks {
    let Ok(Some((_, v))) = ler(caminho) else {
        return EstadoHooks::Desligado;
    };
    let nossos: Vec<&str> = v["hooks"]
        .as_object()
        .into_iter()
        .flat_map(|h| h.values())
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|g| g["hooks"].as_array())
        .flatten()
        .filter(|h| hook_e_nosso(h, &MARCAS))
        .filter_map(|h| h["command"].as_str())
        .collect();
    if nossos.is_empty() {
        return EstadoHooks::Desligado;
    }
    let todos_os_eventos = EVENTOS.iter().all(|e| {
        v["hooks"][e].as_array().is_some_and(|l| {
            l.iter().any(|g| g["hooks"].as_array().is_some_and(|hs| hs.iter().any(|h| h["command"] == comando)))
        })
    });
    if todos_os_eventos && nossos.iter().all(|c| *c == comando) {
        EstadoHooks::Ligado
    } else {
        EstadoHooks::Antigo
    }
}

/// Os hooks de agora estão todos lá?
pub fn instalado(caminho: &Path, comando: &str) -> bool {
    estado(caminho, comando) == EstadoHooks::Ligado
}

/// Comando do hook: o caminho do executável com barras normais, entre aspas.
pub fn comando_do_hook(exe: &Path) -> String {
    let caminho = exe.to_string_lossy().replace('\\', "/");
    format!("\"{caminho}\"")
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;

    const CMD: &str = "\"C:/u/.worktimer/bin/work-timer-hook.exe\"";
    const ANTIGO: &str = "\"D:/versao-velha/work-timer-hook.exe\"";

    fn base() -> Value {
        json!({"switchModelsOnFlag":false,"hooks":{"Stop":[{"hooks":[{"type":"command","command":"outro.exe"}]}]}})
    }

    fn backups(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().contains(".worktimer-backup-"))
            .collect()
    }

    /// Uma pasta como a do usuário: `.claude\settings.json` e `.worktimer\bin\work-timer-hook.exe`.
    fn pasta_do_usuario(settings: Option<&str>) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let s = d.path().join(".claude").join("settings.json");
        let exe = d.path().join(".worktimer").join("bin").join("work-timer-hook.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, b"MZ").unwrap();
        if let Some(t) = settings {
            fs::create_dir_all(s.parent().unwrap()).unwrap();
            fs::write(&s, t).unwrap();
        }
        (d, s, exe)
    }

    #[test]
    fn remover_ao_desinstalar_tira_os_nossos_e_deixa_os_dos_outros() {
        let com = serde_json::to_string_pretty(&mesclar(base(), CMD)).unwrap();
        let (_d, s, exe) = pasta_do_usuario(Some(&com));
        let r = remover_ao_desinstalar(&s, &exe);
        assert!(matches!(r.hooks, Ok(Some(_))), "com a cópia de segurança: {:?}", r.hooks);
        assert!(r.exe_apagado && !exe.exists());
        let depois: Value = serde_json::from_str(&fs::read_to_string(&s).unwrap()).unwrap();
        assert_eq!(depois["hooks"]["Stop"][0]["hooks"][0]["command"], "outro.exe", "o dos outros fica");
        assert_eq!(depois["switchModelsOnFlag"], json!(false));
        assert!(!fs::read_to_string(&s).unwrap().contains("work-timer-hook"));
        assert_eq!(backups(s.parent().unwrap()).len(), 1);
    }

    #[test]
    fn remover_ao_desinstalar_sem_settings_apaga_o_exe() {
        let (_d, s, exe) = pasta_do_usuario(None);
        let r = remover_ao_desinstalar(&s, &exe);
        assert_eq!(r, RemocaoAoDesinstalar { hooks: Ok(None), exe_apagado: true });
        assert!(!exe.exists() && !s.exists(), "nada é criado");
        // o exe que já não estava lá conta como apagado
        assert!(remover_ao_desinstalar(&s, &exe).exe_apagado);
    }

    #[test]
    fn remover_ao_desinstalar_com_settings_invalido_deixa_o_exe() {
        let (_d, s, exe) = pasta_do_usuario(Some("{ isto não é json"));
        let r = remover_ao_desinstalar(&s, &exe);
        assert!(r.hooks.is_err());
        assert!(!r.exe_apagado && exe.exists(), "o Claude Code não pode ficar sem o hook que chama");
        assert_eq!(fs::read_to_string(&s).unwrap(), "{ isto não é json", "nada mudou");
        assert!(backups(s.parent().unwrap()).is_empty());
    }

    #[test]
    fn remover_ao_desinstalar_sem_hooks_nossos_nao_regrava() {
        let so_dos_outros = serde_json::to_string_pretty(&base()).unwrap();
        let (_d, s, exe) = pasta_do_usuario(Some(&so_dos_outros));
        let r = remover_ao_desinstalar(&s, &exe);
        assert_eq!(r, RemocaoAoDesinstalar { hooks: Ok(None), exe_apagado: true });
        assert_eq!(fs::read_to_string(&s).unwrap(), so_dos_outros, "nem regrava nem faz cópia");
        assert!(backups(s.parent().unwrap()).is_empty());
    }

    #[test]
    fn mesclar_adiciona_seis_eventos_e_preserva_existentes() {
        let r = mesclar(base(), CMD);
        assert_eq!(r["switchModelsOnFlag"], json!(false));
        assert_eq!(r["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(r["hooks"]["Stop"][0]["hooks"][0]["command"], "outro.exe");
        for e in EVENTOS {
            assert!(r["hooks"][e].is_array(), "falta {e}");
        }
        assert_eq!(r["hooks"]["PostToolUse"][0]["matcher"], "*");
        assert_eq!(r["hooks"]["Stop"][1]["hooks"][0]["timeout"], 5);
    }

    #[test]
    fn mesclar_e_idempotente() {
        let um = mesclar(base(), CMD);
        assert_eq!(mesclar(um.clone(), CMD), um);
    }

    #[test]
    fn remover_tira_so_os_nossos() {
        let r = remover(mesclar(base(), CMD), &MARCAS);
        assert_eq!(r["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(r["hooks"]["Stop"][0]["hooks"][0]["command"], "outro.exe");
        let hooks = r["hooks"].as_object().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(r["switchModelsOnFlag"], json!(false));
    }

    #[test]
    fn remover_preserva_hook_de_outro_no_mesmo_grupo() {
        let s = json!({"hooks":{
            "Stop":[{"hooks":[
                {"type":"command","command":"outro.exe"},
                {"type":"command","command":"\"C:/x/work-timer-hook.exe\"","timeout":5}
            ]}],
            "PostToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"\"C:/x/work-timer-hook.exe\""}]}],
            "PreToolUse":[]
        }});
        let r = remover(s.clone(), &MARCAS);
        assert_eq!(
            r,
            json!({"hooks":{
                "Stop":[{"hooks":[{"type":"command","command":"outro.exe"}]}],
                "PreToolUse":[]
            }})
        );
        // instalar por cima também não leva o hook do outro
        let r = mesclar(s, CMD);
        assert_eq!(r["hooks"]["Stop"][0]["hooks"], json!([{"type":"command","command":"outro.exe"}]));
        assert_eq!(r["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(r["hooks"]["PreToolUse"], json!([]));
    }

    #[test]
    fn remover_sem_nada_nosso_nao_muda_nada() {
        for s in [
            json!({"hooks":{}}),
            json!({"hooks":{"Stop":[]}, "x": 1}),
            json!({"hooks":{"Stop":[{"hooks":[]}]}}),
            base(),
        ] {
            assert_eq!(remover(s.clone(), &MARCAS), s);
        }
    }

    #[test]
    fn desinstalar_sem_nada_instalado_nao_regrava_nem_copia() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        // formatação própria de quem escreveu o arquivo
        let original = "{\"hooks\":{\"Stop\":[]},   \"x\":1}";
        fs::write(&p, original).unwrap();
        assert_eq!(desinstalar(&p).unwrap(), None);
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
        assert!(backups(d.path()).is_empty());
        assert_eq!(desinstalar(&d.path().join("nao-existe.json")).unwrap(), None);
    }

    #[test]
    fn remover_apaga_hooks_vazio() {
        let r = remover(mesclar(json!({}), CMD), &MARCAS);
        assert!(r.get("hooks").is_none());
    }

    #[test]
    fn instalar_nao_toca_arquivo_invalido() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        fs::write(&p, "{ quebrado").unwrap();
        assert!(instalar(&p, CMD).is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), "{ quebrado");
        assert!(backups(d.path()).is_empty());
    }

    #[test]
    fn instalar_faz_copia_de_seguranca() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        let original = serde_json::to_string_pretty(&base()).unwrap();
        fs::write(&p, &original).unwrap();
        assert!(!instalado(&p, CMD));
        let b = instalar(&p, CMD).unwrap().expect("backup");
        assert_eq!(fs::read_to_string(&b).unwrap(), original);
        assert!(instalado(&p, CMD));
        let nome = b.file_name().unwrap().to_string_lossy().into_owned();
        assert!(nome.starts_with("settings.json.worktimer-backup-"));
        desinstalar(&p).unwrap();
        assert!(!instalado(&p, CMD));
    }

    #[test]
    fn instalar_cria_arquivo_se_nao_existe() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        assert_eq!(instalar(&p, CMD).unwrap(), None);
        assert!(instalado(&p, CMD));
        assert!(backups(d.path()).is_empty());
    }

    #[test]
    fn previa_mostra_antes_e_depois() {
        let a = base();
        let s = previa(&a, &mesclar(a.clone(), CMD));
        assert!(s.contains("outro.exe") && s.contains("work-timer-hook"));
    }

    #[test]
    fn previa_no_idioma_escolhido() {
        use crate::idioma::Idioma;
        let a = json!({});
        let pt = previa_em(Idioma::Pt, &a, &mesclar(a.clone(), CMD));
        assert!(pt.starts_with("Antes:\n(sem hooks)\n\nDepois:\n"), "{pt}");
        let en = previa_em(Idioma::En, &a, &mesclar(a.clone(), CMD));
        assert!(en.starts_with("Before:\n(no hooks)\n\nAfter:\n"), "{en}");
        assert!(!en.contains("Antes") && !en.contains("Depois"));
    }

    #[test]
    fn comando_com_barras_normais_e_espaco() {
        assert_eq!(
            comando_do_hook(Path::new("C:\\Users\\Empresa\\.worktimer\\bin\\work-timer-hook.exe")),
            "\"C:/Users/Empresa/.worktimer/bin/work-timer-hook.exe\""
        );
    }

    #[test]
    fn copias_no_mesmo_segundo_nao_se_sobrescrevem() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        let original = serde_json::to_string_pretty(&base()).unwrap();
        fs::write(&p, &original).unwrap();
        let b1 = instalar(&p, CMD).unwrap().unwrap();
        let b2 = instalar(&p, CMD).unwrap().unwrap();
        let b3 = desinstalar(&p).unwrap().unwrap();
        assert!(b1 != b2 && b2 != b3 && b1 != b3);
        assert_eq!(fs::read_to_string(&b1).unwrap(), original);
        assert_eq!(backups(d.path()).len(), 3);
    }

    #[test]
    fn gravacao_atomica_nao_deixa_temporario() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        instalar(&p, CMD).unwrap();
        assert!(!d.path().join("settings.json.worktimer-tmp").exists());
        serde_json::from_str::<Value>(&fs::read_to_string(&p).unwrap()).unwrap();
    }

    #[test]
    fn comando_entre_aspas() {
        let c = comando_do_hook(Path::new("C:/u/.worktimer/bin/work-timer-hook.exe"));
        assert!(c.starts_with('"') && c.ends_with("work-timer-hook.exe\""));
    }

    #[test]
    fn estado_ligado_antigo_ou_desligado() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        assert_eq!(estado(&p, CMD), EstadoHooks::Desligado, "sem arquivo");
        fs::write(&p, serde_json::to_string(&base()).unwrap()).unwrap();
        assert_eq!(estado(&p, CMD), EstadoHooks::Desligado, "só o hook de outro");
        // hooks de uma versão anterior (outra pasta): antigo, e o app nunca regrava sozinho
        let velho = mesclar(base(), ANTIGO);
        fs::write(&p, serde_json::to_string(&velho).unwrap()).unwrap();
        assert_eq!(estado(&p, CMD), EstadoHooks::Antigo);
        assert_eq!(serde_json::from_str::<Value>(&fs::read_to_string(&p).unwrap()).unwrap(), velho);
        // atualizar troca o antigo pelo novo e mantém o de outro
        instalar(&p, CMD).unwrap().expect("backup");
        assert_eq!(estado(&p, CMD), EstadoHooks::Ligado);
        let v: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert!(!v.to_string().contains("versao-velha"));
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], "outro.exe");
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2);
        // o novo de outra pasta também conta como antigo
        assert_eq!(estado(&p, "\"D:/outra/work-timer-hook.exe\""), EstadoHooks::Antigo);
        // faltando um evento: antigo
        let mut falta = v.clone();
        falta["hooks"].as_object_mut().unwrap().remove("SessionEnd");
        fs::write(&p, serde_json::to_string(&falta).unwrap()).unwrap();
        assert_eq!(estado(&p, CMD), EstadoHooks::Antigo);
    }

    #[test]
    fn remover_reconhece_o_comando_antigo_e_o_novo() {
        let mut s = mesclar(base(), ANTIGO);
        // um evento com os dois (atualização pela metade feita à mão)
        s["hooks"]["Stop"].as_array_mut().unwrap().push(json!({"hooks":[{"type":"command","command":CMD}]}));
        let r = remover(s, &MARCAS);
        assert_eq!(r, base());
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        fs::write(&p, serde_json::to_string(&mesclar(base(), ANTIGO)).unwrap()).unwrap();
        assert!(desinstalar(&p).unwrap().is_some());
        assert_eq!(estado(&p, CMD), EstadoHooks::Desligado);
    }
}
