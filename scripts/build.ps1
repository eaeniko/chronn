$ErrorActionPreference = 'Stop'
$app = Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot 'preparar-sidecar.ps1')
Push-Location $app
try {
    cargo tauri build
    if ($LASTEXITCODE -ne 0) { throw 'falha no cargo tauri build' }
    # o instalador sai com o nome interno ("Work Timer_<versão>_x64-setup.exe": o productName, que
    # segura a pasta de instalação); a cópia ao lado leva o nome de exibição (nucleo::APP_NOME).
    # Os dois são o mesmo instalador, com o work-timer.exe e o work-timer-hook.exe.
    $lib = Get-Content (Join-Path $app 'crates\nucleo\src\lib.rs') -Raw -Encoding UTF8
    $nome = [regex]::Match($lib, 'pub const APP_NOME: &str = "([^"]+)";').Groups[1].Value
    if (-not $nome) { throw 'APP_NOME ausente em crates\nucleo\src\lib.rs' }
    $gerado = Get-ChildItem (Join-Path $app 'target\release\bundle\nsis\Work Timer_*_x64-setup.exe') |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $copia = Join-Path $gerado.DirectoryName ($gerado.Name -replace '^Work Timer_', "${nome}_")
    Copy-Item $gerado.FullName $copia -Force
    Write-Output $copia
} finally { Pop-Location }
