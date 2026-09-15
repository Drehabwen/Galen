$ErrorActionPreference = "Stop"

$packageRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $packageRoot "..\..\..\..\..\..\.." )).Path

Write-Host "[1/3] cargo check --workspace"
Push-Location (Join-Path $repoRoot "rust")
cargo check --workspace

Write-Host "[2/3] cargo test -p galen --lib"
cargo test -p galen --lib
Pop-Location

Write-Host "[3/3] npx tsc --noEmit"
Push-Location (Join-Path $repoRoot "rust\crates\galen")
npx tsc --noEmit
Pop-Location

Write-Host "基础检查全部通过。配对任务包请按 paired-benchmark\README.md 单独运行。"
