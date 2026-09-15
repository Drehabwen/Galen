[CmdletBinding()]
param(
    [ValidateSet("Replay", "Live")]
    [string]$Mode = "Replay",
    [switch]$Full
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot
$Output = Join-Path $Root "verification-output"
New-Item -ItemType Directory -Force -Path $Output | Out-Null

function Read-Jsonl([string]$Path) {
    @(Get-Content -LiteralPath $Path | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })
}

function Get-Median([double[]]$Values) {
    $ordered = @($Values | Sort-Object)
    if ($ordered.Count % 2) { return $ordered[[int][math]::Floor($ordered.Count / 2)] }
    return ($ordered[$ordered.Count / 2 - 1] + $ordered[$ordered.Count / 2]) / 2
}

if ($Mode -eq "Live") {
    $repo = Resolve-Path (Join-Path $Root "..\..\..\..\..\..\..")
    $runner = Join-Path $repo "evals\experiments\paired-v1\run.ps1"
    if (-not (Test-Path -LiteralPath $runner)) { throw "未找到 Galen 实验运行器：$runner" }
    $profile = if ($Full) { "formal" } else { "smoke" }
    & $runner -Profile $profile -RunId ("judge-live-" + (Get-Date -Format "yyyyMMdd-HHmmss"))
    exit $LASTEXITCODE
}

$galenPath = Join-Path $Root "evidence-data\raw-results\galen-75-runs.jsonl"
$codexPath = Join-Path $Root "evidence-data\raw-results\codex-deepseek-75-runs.jsonl"
$galen = Read-Jsonl $galenPath
$codex = Read-Jsonl $codexPath

if ($galen.Count -ne 75 -or $codex.Count -ne 75) { throw "记录数量不完整：Galen=$($galen.Count), Codex=$($codex.Count)" }

$rows = foreach ($item in @(@{Name="Galen"; Data=$galen}, @{Name="Codex + DeepSeek"; Data=$codex})) {
    $data = @($item.Data)
    $passed = @($data | Where-Object { $_.hard_gates_passed }).Count
    [pscustomobject]@{
        System = $item.Name
        Runs = $data.Count
        Passed = $passed
        PassRate = [math]::Round(100 * $passed / $data.Count, 1)
        MeanToolCalls = [math]::Round(($data | ForEach-Object { [double]$_.tools.calls } | Measure-Object -Average).Average, 2)
        MedianLatencySeconds = [math]::Round((Get-Median @($data | ForEach-Object { [double]$_.latency.total_ms / 1000 })), 1)
    }
}

if ($rows[0].Passed -ne 74 -or $rows[1].Passed -ne 21) { throw "结果与冻结报告不一致，停止生成。" }

$rows | Export-Csv -LiteralPath (Join-Path $Output "judge-summary.csv") -NoTypeInformation -Encoding utf8
$rows | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output "judge-summary.json") -Encoding utf8
$rows | Format-Table -AutoSize
Write-Host "核验通过。输出：$Output" -ForegroundColor Green
