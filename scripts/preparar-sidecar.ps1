$ErrorActionPreference = 'Stop'
$app = Split-Path -Parent $PSScriptRoot
Push-Location $app
try {
    cargo build -p work-timer-hook --release
    if ($LASTEXITCODE -ne 0) { throw 'falha ao compilar work-timer-hook' }
    $destino = Join-Path $app 'src-tauri\binaries'
    New-Item -ItemType Directory -Force $destino | Out-Null
    Copy-Item (Join-Path $app 'target\release\work-timer-hook.exe') (Join-Path $destino 'work-timer-hook-x86_64-pc-windows-msvc.exe') -Force
} finally { Pop-Location }
