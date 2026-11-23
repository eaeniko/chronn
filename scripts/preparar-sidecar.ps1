# Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
# Os hooks que vão com o app: o do Windows (o sidecar `work-timer-hook`) e, desde a rodada 28, o
# do Linux (`work-timer-hook-linux`, um recurso do app), que o "Claude Code no WSL" copia para a
# distro. O do Linux é estático (musl) e sai pelo `rust-lld`: sem o alvo instalado, a build para.
$ErrorActionPreference = 'Stop'
$app = Split-Path -Parent $PSScriptRoot
$linux = 'x86_64-unknown-linux-musl'
Push-Location $app
try {
    cargo build -p work-timer-hook --release
    if ($LASTEXITCODE -ne 0) { throw 'falha ao compilar work-timer-hook' }
    $destino = Join-Path $app 'src-tauri\binaries'
    New-Item -ItemType Directory -Force $destino | Out-Null
    Copy-Item (Join-Path $app 'target\release\work-timer-hook.exe') (Join-Path $destino 'work-timer-hook-x86_64-pc-windows-msvc.exe') -Force

    # o hook para Linux (rodada 28)
    $alvos = rustup target list --installed
    if ($LASTEXITCODE -ne 0) { throw 'falha ao listar os alvos do rustup' }
    if (-not ($alvos -contains $linux)) {
        throw "falta o alvo do hook para Linux: rode ``rustup target add $linux`` e a build de novo"
    }
    $env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = 'rust-lld'
    try {
        cargo build -p work-timer-hook --release --target $linux
        if ($LASTEXITCODE -ne 0) { throw 'falha ao compilar o work-timer-hook para Linux' }
    } finally {
        Remove-Item Env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER -ErrorAction SilentlyContinue
    }
    Copy-Item (Join-Path $app "target\$linux\release\work-timer-hook") (Join-Path $destino 'work-timer-hook-linux') -Force
} finally { Pop-Location }
