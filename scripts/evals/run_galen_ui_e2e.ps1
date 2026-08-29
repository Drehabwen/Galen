param([string]$BaseUrl = "http://127.0.0.1:1420/?e2e=1")

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$appRoot = Join-Path $repoRoot "rust\crates\galen"
$artifactRoot = Join-Path $appRoot "output\playwright"
$journeyPath = Join-Path $PSScriptRoot "galen_ui_journey.js"
$sessionName = "galen-e2e"
New-Item -ItemType Directory -Force -Path $artifactRoot | Out-Null

if (Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue) {
    throw "Port 1420 is already in use; stop the existing Galen dev server first"
}

function Invoke-PlaywrightCli {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
    & npx.cmd --yes --package @playwright/cli playwright-cli "-s=$sessionName" @Arguments
    if ($LASTEXITCODE -ne 0) { throw "playwright-cli failed: $($Arguments -join ' ')" }
}

$vite = Start-Process -FilePath "npm.cmd" -ArgumentList @("run", "dev", "--", "--host", "127.0.0.1") -WorkingDirectory $appRoot -WindowStyle Hidden -PassThru
$viteServerPid = $null
try {
    $ready = $false
    foreach ($attempt in 1..40) {
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $BaseUrl -TimeoutSec 1 | Out-Null
            $ready = $true
            break
        } catch { Start-Sleep -Milliseconds 500 }
    }
    if (-not $ready) { throw "Galen Vite server did not become ready" }
    $viteServerPid = (Get-NetTCPConnection -LocalPort 1420 -State Listen).OwningProcess

    Invoke-PlaywrightCli open $BaseUrl
    Invoke-PlaywrightCli tracing-start
    Invoke-PlaywrightCli run-code --filename $journeyPath
    Invoke-PlaywrightCli console error
    Invoke-PlaywrightCli tracing-stop
} finally {
    try { Invoke-PlaywrightCli close } catch { }
    if ($viteServerPid) { Stop-Process -Id $viteServerPid -Force -ErrorAction SilentlyContinue }
    if (-not $vite.HasExited) { Stop-Process -Id $vite.Id }
}
