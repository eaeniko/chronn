# Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
# A build de release: o instalador assinado para o atualizador e o latest.json ao lado dele.
# Uso: scripts\build.ps1 [-Changelog <CHANGELOG.md com a entrada da versão>]
param([string]$Changelog = '')
$ErrorActionPreference = 'Stop'
$app = Split-Path -Parent $PSScriptRoot

# O CHANGELOG pedido tem de existir (antes da build, e pelo caminho de quem chamou: a build roda
# dentro de app\)
if ($Changelog) {
    if (-not (Test-Path -LiteralPath $Changelog -PathType Leaf)) { throw "CHANGELOG não encontrado: $Changelog" }
    $Changelog = (Resolve-Path -LiteralPath $Changelog).Path
}

# A chave do atualizador (rodada 25): sem ela, não sai instalador. A chave não tem senha. O Tauri
# lê o arquivo pelo caminho em TAURI_SIGNING_PRIVATE_KEY; o "--ci" faz a senha ausente valer como
# vazia, sem perguntar (no PowerShell, uma variável de ambiente vazia some). As duas variáveis saem
# do ambiente no fim, com ou sem erro: um "cargo tauri build" depois, na mesma janela, não assina.
# Se a chave não for o par da pubkey do tauri.conf.json, o Tauri só avisa no log; quem para a
# build é o latest-json.mjs, que confere o número da chave da assinatura.
$chave = Join-Path $env:USERPROFILE '.chronn-assinatura\chronn.key'
if (-not (Test-Path -LiteralPath $chave -PathType Leaf)) { throw "chave de assinatura ausente: $chave" }
$conf = Get-Content (Join-Path $app 'src-tauri\tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$versao = $conf.version
$inicio = Get-Date
try {
    $env:TAURI_SIGNING_PRIVATE_KEY = $chave
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''

    & (Join-Path $PSScriptRoot 'preparar-sidecar.ps1')
    Push-Location $app
    try {
        cargo tauri build --ci
        if ($LASTEXITCODE -ne 0) { throw 'falha no cargo tauri build' }
        # o instalador sai com o nome interno ("Work Timer_<versão>_x64-setup.exe": o productName,
        # que segura a pasta de instalação); a cópia ao lado leva o nome de exibição
        # (nucleo::APP_NOME). Os dois são o mesmo instalador, com o work-timer.exe e o
        # work-timer-hook.exe. O instalador é o da versão do tauri.conf.json, feito agora.
        $lib = Get-Content (Join-Path $app 'crates\nucleo\src\lib.rs') -Raw -Encoding UTF8
        $nome = [regex]::Match($lib, 'pub const APP_NOME: &str = "([^"]+)";').Groups[1].Value
        if (-not $nome) { throw 'APP_NOME ausente em crates\nucleo\src\lib.rs' }
        $pasta = Join-Path $app 'target\release\bundle\nsis'
        $gerado = Get-Item -LiteralPath (Join-Path $pasta "Work Timer_${versao}_x64-setup.exe") -ErrorAction SilentlyContinue
        if (-not $gerado) { throw "instalador da $versao ausente em $pasta" }
        if ($gerado.LastWriteTime -lt $inicio) { throw "o instalador da $versao não foi feito por esta build: $($gerado.FullName)" }
        $copia = Join-Path $pasta "${nome}_${versao}_x64-setup.exe"
        Copy-Item -LiteralPath $gerado.FullName $copia -Force

        # a assinatura que a build gerou ao lado do instalador (a mesma para a cópia: os bytes
        # são iguais) e o latest.json, que sobe na release junto do instalador
        $sig = "$($gerado.FullName).sig"
        if (-not (Test-Path -LiteralPath $sig)) { throw "assinatura ausente: $sig" }
        if ((Get-Item -LiteralPath $sig).LastWriteTime -lt $inicio) { throw "a assinatura não foi feita por esta build: $sig" }
        Copy-Item -LiteralPath $sig "$copia.sig" -Force
        $latest = Join-Path $pasta 'latest.json'
        $argumentos = @((Join-Path $PSScriptRoot 'latest-json.mjs'), $versao, $sig, $latest)
        if ($Changelog) { $argumentos += $Changelog }
        node @argumentos
        if ($LASTEXITCODE -ne 0) { throw 'falha ao gerar o latest.json' }
        # o latest.json é o desta build e leva a versão do tauri.conf.json
        $gravado = Get-Item -LiteralPath $latest -ErrorAction SilentlyContinue
        if (-not $gravado -or $gravado.LastWriteTime -lt $inicio) { throw "o latest.json não foi gravado agora: $latest" }
        $lido = Get-Content -LiteralPath $latest -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($lido.version -ne $versao) { throw "o latest.json tem a versão $($lido.version), não a $versao" }
        Write-Output $copia
        Write-Output $latest
    } finally { Pop-Location }
} finally {
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}
