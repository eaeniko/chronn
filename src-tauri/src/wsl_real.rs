// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O WSL de verdade (rodada 28): as distros do registro e o `wsl.exe`, sempre sem janela
//! (`CREATE_NO_WINDOW`), pelo caminho completo (`%SystemRoot%\System32\wsl.exe`) e com prazo: no
//! fim dele, o processo é morto e a resposta vale "não sei" (revisão I2). Só o `rodando()` e o
//! registro podem ser consultados com a distro parada; a pasta do usuário e o `chmod`
//! (`wsl -d <distro> -e …`) ligariam uma distro parada, e quem chama confere antes que ela roda.
//! Os testes nunca usam isto: usam o `nucleo::wsl::WslFalso`.
use std::time::{Duration, Instant};

use nucleo::wsl::{definir_rodando, distro_interna, distro_valida, Wsl};

/// O registro das distros do WSL do usuário.
const CHAVE_LXSS: &str = r"Software\Microsoft\Windows\CurrentVersion\Lxss";
/// O prazo do `wsl -l --running -q`.
pub const PRAZO_LISTA: Duration = Duration::from_secs(3);
/// O prazo de um comando dentro da distro (`wsl -d <d> -e …`).
pub const PRAZO_COMANDO: Duration = Duration::from_secs(5);
/// O prazo para ligar uma distro parada (0.6.1; a VM do WSL pode levar uns segundos para subir).
pub const PRAZO_SUBIR: Duration = Duration::from_secs(40);

pub struct WslReal;

/// A saída do `wsl.exe`: o `-l` vem em UTF-16 (LE), a não ser com `WSL_UTF8=1`; a do Linux, em
/// UTF-8. Com algum byte `\0`, UTF-16; sem nenhum, UTF-8 (revisão M7).
pub fn texto_da_saida(bytes: &[u8]) -> String {
    if !bytes.contains(&0) {
        return String::from_utf8_lossy(bytes).replace('\u{feff}', "");
    }
    let u: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&u).replace('\u{feff}', "")
}

/// Os nomes que o `wsl -l --running -q` listou, só os que são distros do registro (a mensagem
/// "nenhuma distro rodando" e as linhas vazias ficam de fora).
pub fn nomes_rodando(saida: &str, distros: &[String]) -> Vec<String> {
    saida
        .lines()
        .map(|l| l.trim().trim_matches('\0'))
        .filter(|l| distros.iter().any(|d| d == l))
        .map(str::to_string)
        .collect()
}

/// O `wsl.exe` do sistema (nunca um da pasta do app ou da pasta atual).
fn caminho_do_wsl() -> std::path::PathBuf {
    let sistema = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| r"C:\Windows".into());
    sistema.join("System32").join("wsl.exe")
}

/// Roda o comando e espera no máximo `prazo`; passou, mata o processo e devolve nulo. A saída é
/// lida em threads, para um processo que escreve muito não travar, e entregue por um canal com o
/// tempo que resta do prazo (re-revisão N3: um neto que herdou o pipe e o mantém aberto não prende
/// ninguém; a thread de leitura acaba sozinha quando ele fechar).
pub fn rodar_com_prazo(cmd: &mut std::process::Command, prazo: Duration) -> Option<std::process::Output> {
    use std::io::Read;
    let mut filho = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let ler = |p: Option<Box<dyn Read + Send>>| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut v = vec![];
            if let Some(mut p) = p {
                let _ = p.read_to_end(&mut v);
            }
            let _ = tx.send(v);
        });
        rx
    };
    let saida = ler(filho.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let erro = ler(filho.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let inicio = Instant::now();
    let status = loop {
        match filho.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if inicio.elapsed() < prazo => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = filho.kill();
                let _ = filho.wait();
                return None;
            }
        }
    };
    let resta = || prazo.saturating_sub(inicio.elapsed()).max(Duration::from_millis(50));
    let stdout = saida.recv_timeout(resta()).ok()?;
    let stderr = erro.recv_timeout(resta()).ok()?;
    Some(std::process::Output { status, stdout, stderr })
}

#[cfg(windows)]
fn wsl_exe(args: &[&str], prazo: Duration) -> Option<std::process::Output> {
    use std::os::windows::process::CommandExt;
    const SEM_JANELA: u32 = 0x0800_0000;
    let mut cmd = std::process::Command::new(caminho_do_wsl());
    cmd.args(args).creation_flags(SEM_JANELA);
    rodar_com_prazo(&mut cmd, prazo)
}

#[cfg(not(windows))]
fn wsl_exe(_args: &[&str], _prazo: Duration) -> Option<std::process::Output> {
    let _ = caminho_do_wsl;
    None
}

impl Wsl for WslReal {
    fn distros(&self) -> Vec<String> {
        #[cfg(windows)]
        {
            let Ok(lxss) = windows_registry::CURRENT_USER.open(CHAVE_LXSS) else { return vec![] };
            let Ok(chaves) = lxss.keys() else { return vec![] };
            let mut v: Vec<String> = chaves
                .filter_map(|k| lxss.open(&k).ok()?.get_string("DistributionName").ok())
                .filter(|n| distro_valida(n) && !distro_interna(n))
                .collect();
            v.sort();
            v.dedup();
            v
        }
        #[cfg(not(windows))]
        {
            let _ = (CHAVE_LXSS, distro_interna);
            vec![]
        }
    }

    /// A consulta também guarda o resultado para quem lê caminhos do WSL (`nucleo::wsl::pode_tocar`);
    /// sem resposta, nenhuma distro vale como rodando.
    fn rodando(&self) -> Option<Vec<String>> {
        let distros = self.distros();
        if distros.is_empty() {
            definir_rodando(&[]);
            return Some(vec![]);
        }
        let Some(saida) = wsl_exe(&["-l", "--running", "-q"], PRAZO_LISTA) else {
            definir_rodando(&[]);
            return None;
        };
        // sem nenhuma rodando, o wsl.exe sai com erro e uma mensagem: nenhuma
        let v = nomes_rodando(&texto_da_saida(&saida.stdout), &distros);
        definir_rodando(&v);
        Some(v)
    }

    fn home(&self, distro: &str) -> Option<String> {
        if !distro_valida(distro) {
            return None;
        }
        let saida = wsl_exe(&["-d", distro, "--cd", "~", "-e", "sh", "-c", "echo $HOME"], PRAZO_COMANDO)?;
        saida.status.success().then(|| texto_da_saida(&saida.stdout).trim().to_string())
    }

    /// Liga a distro rodando um `true` nela (0.6.1): só no clique da pessoa no "Ligar".
    fn subir(&self, distro: &str) -> Result<(), String> {
        if !distro_valida(distro) {
            return Err("distro inválida".into());
        }
        let saida = wsl_exe(&["-d", distro, "--cd", "~", "-e", "true"], PRAZO_SUBIR).ok_or("o wsl.exe não respondeu a tempo")?;
        if saida.status.success() {
            Ok(())
        } else {
            Err(texto_da_saida(&saida.stderr).trim().chars().take(200).collect())
        }
    }

    fn chmod(&self, distro: &str, caminho: &str) -> Result<(), String> {
        if !distro_valida(distro) {
            return Err("distro inválida".into());
        }
        let saida = wsl_exe(&["-d", distro, "--cd", "~", "-e", "chmod", "755", caminho], PRAZO_COMANDO).ok_or("o wsl.exe não respondeu a tempo")?;
        if saida.status.success() {
            Ok(())
        } else {
            Err(texto_da_saida(&saida.stderr).trim().chars().take(200).collect())
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_saida_do_wsl_em_utf16_ou_utf8_e_so_as_distros_do_registro() {
        let utf16 = |s: &str| s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect::<Vec<u8>>();
        let distros = vec!["Ubuntu".to_string(), "Debian".to_string()];
        assert_eq!(nomes_rodando(&texto_da_saida(&utf16("\u{feff}Ubuntu\r\n\r\n")), &distros), ["Ubuntu"]);
        assert!(nomes_rodando(&texto_da_saida(&utf16("There are no running distributions.\r\n")), &distros).is_empty());
        // com WSL_UTF8=1, a saída vem em UTF-8
        assert_eq!(nomes_rodando(&texto_da_saida(b"Debian\nUbuntu\n"), &distros), ["Debian", "Ubuntu"]);
        assert!(nomes_rodando("Ubuntu-22.04\n", &distros).is_empty(), "só o nome exato");
        assert_eq!(texto_da_saida("/home/usuário\n".as_bytes()), "/home/usuário\n");
    }

    #[test]
    fn o_comando_que_passa_do_prazo_e_morto_e_vale_nao_sei() {
        // um comando do próprio Windows que demora (nunca o wsl.exe)
        let mut lento = std::process::Command::new("cmd");
        lento.args(["/C", "ping", "-n", "6", "127.0.0.1"]);
        let inicio = Instant::now();
        assert!(rodar_com_prazo(&mut lento, Duration::from_millis(300)).is_none());
        assert!(inicio.elapsed() < Duration::from_secs(3), "não esperou o fim: {:?}", inicio.elapsed());
        let mut rapido = std::process::Command::new("cmd");
        rapido.args(["/C", "echo", "oi"]);
        let s = rodar_com_prazo(&mut rapido, Duration::from_secs(5)).unwrap();
        assert!(s.status.success() && String::from_utf8_lossy(&s.stdout).contains("oi"));
        assert!(caminho_do_wsl().ends_with(r"System32\wsl.exe"));
        // re-revisão N3: o processo sai, mas um neto segura o pipe aberto; a leitura não espera ele
        let mut neto = std::process::Command::new("cmd");
        neto.args(["/C", "start", "/b", "ping", "-n", "6", "127.0.0.1", ">nul", "&", "echo", "saiu"]);
        let inicio = Instant::now();
        let _ = rodar_com_prazo(&mut neto, Duration::from_millis(800));
        assert!(inicio.elapsed() < Duration::from_secs(3), "esperou o neto: {:?}", inicio.elapsed());
    }
}
