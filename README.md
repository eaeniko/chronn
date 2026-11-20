# Chronn

**English** · [Português](README.pt-BR.md)

Timers for your working time, on Windows. Press play yourself, or let Chronn count on its own
while a connected app works in a folder linked to the timer (today: Claude Code).

- **Local:** everything runs on your computer. Chronn uses no AI, collects no data and sends
  nothing anywhere.
- **You decide** which apps to connect, which folders count and in which timer.
- **For your personal tracking**, not for company monitoring.

## What it does

- Groups of timers with ▶ / ⏸ / ■ and activities (Work, Meeting, Study...).
- A small dock, always in view, showing what is running.
- Pauses by itself when you are away from the computer.
- With Claude Code connected (hooks), each conversation counts in its folder's timer; a new
  conversation asks which timer to use.
- Reset without deleting: the day's history stays.
- A 1-minute tour, per-screen help and first-time tips.

The interface is in Portuguese for now; English is coming in a next version.

## Install

Download `Chronn_<version>_x64-setup.exe` from [Releases](../../releases). The installer is not
code-signed yet: Windows shows "Windows protected your PC"; click **More info → Run anyway**.

Data lives in `%USERPROFILE%\.worktimer\`. Uninstalling removes the Claude Code hooks (backing up
`settings.json` first) and keeps your data.

## Build

Requires Rust (stable), Node.js and the Tauri 2 CLI.

    powershell -ExecutionPolicy Bypass -File scripts/preparar-sidecar.ps1
    cargo test --workspace
    cd ui && npm test
    powershell -File scripts/build.ps1

## License

[CC BY-NC-SA 4.0](LICENSE), with a permission for donations and sponsorships. Use, study and adapt
it for non-commercial purposes, always crediting the author and keeping the same license. For
commercial use, contact the author.

Created by [@eaeniko](https://eaeniko.okin.club) (Nikolai Cinotti).
