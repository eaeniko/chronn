# Chronn

[English](README.md) · **Português**

Timers para o seu tempo de trabalho, no Windows. Você dá play à mão, ou o Chronn conta sozinho
enquanto um app conectado trabalha numa pasta ligada ao timer (hoje: Claude Code).

> **Alpha.** O Chronn está no começo: espere arestas e faça backup dos dados antes de atualizar.
> Sugestões e problemas são bem-vindos em [Issues](../../issues).

- **Local:** tudo roda no seu computador. O Chronn não usa IA e não coleta dados. A única conexão
  com a internet é a busca de atualizações: uma vez por dia, ao abrir, ele pergunta ao GitHub se
  há versão nova, sem enviar nada sobre você. Dá para desligar em Ajustes → Sobre.
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
- Português (Brasil) e inglês, seguindo o Windows ou escolhido nos Ajustes.
- Atualiza por dentro do app: um ícone aparece quando sai versão nova, e um clique baixa, confere
  a assinatura, instala e reabre.

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

O `build.ps1` assina o instalador para o atualizador e espera uma chave privada em
`%USERPROFILE%\.chronn-assinatura\chronn.key`. Para a sua própria build, crie um par de chaves com
`cargo tauri signer generate`, guarde a privada nesse lugar e ponha a sua chave pública em
`plugins.updater.pubkey` (`src-tauri/tauri.conf.json`).

## Licença

[CC BY-NC-SA 4.0](LICENSE), com permissão para doações e patrocínios. Use, estude e adapte para
fins não comerciais, sempre com o crédito ao autor e sob a mesma licença. Uso comercial: fale
com o autor.

Criado por [@eaeniko](https://eaeniko.okin.club) (Nikolai Cinotti).
