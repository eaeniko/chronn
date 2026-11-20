# Chronn

Timers para o seu tempo de trabalho, no Windows. Você dá play à mão, ou o Chronn conta sozinho
enquanto um app conectado trabalha numa pasta ligada ao timer (hoje: Claude Code).

- **Local:** tudo roda no seu computador. O Chronn não usa IA, não coleta dados e não envia nada
  para lugar nenhum.
- **Você decide:** quais apps conectar, quais pastas contam e em qual timer.
- **Para o seu controle pessoal**, não para controle empresarial.

## O que ele faz

- Grupos de timers, com ▶ / ⏸ / ■ e atividades (Trabalho, Reunião, Estudo...).
- Um dock pequeno, sempre à vista, com o que está rodando.
- Pausa sozinho quando você fica longe do computador.
- Com o Claude Code conectado (hooks), a conversa conta no timer da pasta dela; conversa nova
  pergunta em qual timer contar.
- Zerar sem apagar: o histórico do dia fica.
- Tour de 1 minuto, ajuda por tela e dicas na primeira vez.

## Instalar

Baixe o `Chronn_<versão>_x64-setup.exe` em [Releases](../../releases). O instalador ainda não
tem assinatura digital: o Windows mostra "O Windows protegeu o computador"; clique em
**Mais informações → Executar assim mesmo**.

Os dados ficam em `%USERPROFILE%\.worktimer\`. Desinstalar tira os hooks do Claude Code (com uma
cópia do `settings.json` antes) e mantém os dados.

## Compilar

Requer Rust (stable), Node.js e o Tauri 2 CLI.

    powershell -ExecutionPolicy Bypass -File scripts/preparar-sidecar.ps1
    cargo test --workspace
    cd ui && npm test
    powershell -File scripts/build.ps1

## Licença

[CC BY-NC-SA 4.0](LICENSE), com permissão para doações e patrocínios. Use, estude e adapte para
fins não comerciais, sempre com o crédito ao autor e sob a mesma licença. Uso comercial: fale
com o autor.

Criado por [@eaeniko](https://eaeniko.okin.club) (Nikolai Cinotti).
