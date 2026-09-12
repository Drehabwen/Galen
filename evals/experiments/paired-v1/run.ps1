[CmdletBinding()]
param(
    [ValidateSet("smoke", "formal")]
    [string]$Profile = "smoke",
    [string]$ConfigPath = (Join-Path $PSScriptRoot "experiment.json"),
    [string]$EvalExe = (Join-Path $PSScriptRoot "..\..\..\rust\target\debug\eval.exe"),
    [string]$CasesDir = (Join-Path $PSScriptRoot "..\..\cases\paired"),
    [string]$OutputRoot = (Join-Path $PSScriptRoot "..\..\runs\paired-v1"),
    [string]$RunId = "",
    [string]$Model = "",
    [ValidateSet("medical", "none")]
    [string]$Persona = "medical",
    [switch]$Resume,
    [switch]$SkipFunctional,
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-FullPath([string]$PathValue) {
    return [System.IO.Path]::GetFullPath($PathValue)
}

function Get-LineCount([string]$PathValue) {
    if (-not (Test-Path -LiteralPath $PathValue -PathType Leaf)) {
        return 0
    }
    return @(Get-Content -LiteralPath $PathValue).Count
}

function Invoke-EvalRun(
    [string]$CaseId,
    [string]$Variant,
    [int]$Repeat,
    [string]$OutputPath
) {
    if ($Resume -and (Get-LineCount $OutputPath) -ge $Repeat) {
        Write-Host "SKIP $CaseId/$Variant：已有 $Repeat 条记录"
        return
    }
    if (Test-Path -LiteralPath $OutputPath) {
        throw "运行文件已存在但不完整：$OutputPath。请换 RunId，或删除该不完整运行后重试。"
    }
    $arguments = @(
        "run", "--case", $CaseId,
        "--cases", $script:CasesDirFull,
        "--variant", $Variant,
        "--repeat", $Repeat,
        "--output", $OutputPath
    )
    if ($Persona -eq "none") {
        $arguments += @("--persona", "none")
    }
    if ($Model) {
        $arguments += @("--model", $Model)
    }
    Write-Host "RUN  $CaseId/$Variant × $Repeat"
    & $script:EvalExeFull @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "案例运行失败：$CaseId/$Variant，exit=$LASTEXITCODE"
    }
}

function Merge-Jsonl([string[]]$Sources, [string]$Destination) {
    $allLines = foreach ($source in $Sources) {
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "缺少运行记录：$source"
        }
        Get-Content -LiteralPath $source
    }
    [System.IO.File]::WriteAllLines($Destination, [string[]]$allLines)
}

function Read-Jsonl([string]$PathValue) {
    return @(
        Get-Content -LiteralPath $PathValue |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
            ForEach-Object { $_ | ConvertFrom-Json }
    )
}

function Get-Quantile([double[]]$Values, [double]$Probability) {
    if ($Values.Count -eq 0) { return 0.0 }
    $sorted = @($Values | Sort-Object)
    $position = [Math]::Max(0.0, [Math]::Min(1.0, $Probability)) * ($sorted.Count - 1)
    $lower = [Math]::Floor($position)
    $upper = [Math]::Ceiling($position)
    $weight = $position - $lower
    return ([double]$sorted[$lower] * (1.0 - $weight)) + ([double]$sorted[$upper] * $weight)
}

function Get-WilsonLower([int]$Successes, [int]$Total) {
    if ($Total -le 0) { return 0.0 }
    $z = 1.959963984540054
    $rate = $Successes / [double]$Total
    $z2 = $z * $z
    $center = $rate + ($z2 / (2.0 * $Total))
    $spread = $z * [Math]::Sqrt((($rate * (1.0 - $rate)) / $Total) + ($z2 / (4.0 * $Total * $Total)))
    return ($center - $spread) / (1.0 + ($z2 / $Total))
}

function Get-MetricRows([object[]]$Records, [string]$Suite, [string]$Variant) {
    $rows = @()
    foreach ($group in ($Records | Group-Object case_id | Sort-Object Name)) {
        $items = @($group.Group)
        $successes = @($items | Where-Object { $_.hard_gates_passed }).Count
        $quality = ($items | Measure-Object -Property quality_score -Average).Average
        $ttfr = @($items | ForEach-Object { if ($null -ne $_.latency.ttfr_ms) { [double]$_.latency.ttfr_ms } })
        $total = @($items | ForEach-Object { [double]$_.latency.total_ms })
        $tokens = @($items | ForEach-Object {
            [double]$_.usage.input + [double]$_.usage.output +
            [double]$_.usage.cache_create + [double]$_.usage.cache_read
        })
        $modelRequests = @($items | ForEach-Object { [double]$_.model_requests })
        $compactions = @($items | ForEach-Object { [double]$_.context.compactions })
        $toolCalls = @($items | ForEach-Object { [double]$_.tools.calls })
        $toolErrors = ($items | ForEach-Object { [int]$_.tools.errors } | Measure-Object -Sum).Sum
        $maxRepeat = ($items | ForEach-Object { [int]$_.tools.max_repeat } | Measure-Object -Maximum).Maximum
        $requiredFacts = ($items | ForEach-Object { [int]$_.context.required_facts } | Measure-Object -Sum).Sum
        $retainedFacts = ($items | ForEach-Object { [int]$_.context.retained_facts } | Measure-Object -Sum).Sum
        $staleFailures = @(
            $items.assertions |
                Where-Object {
                    -not $_.pass -and (
                        $_.name.StartsWith("forbidden_response_pattern:") -or
                        $_.name.StartsWith("forbidden_artifact_pattern:")
                    )
                }
        ).Count
        $coverageValues = @()
        foreach ($item in $items) {
            $coverage = $item.context.summary_field_coverage
            if ($null -ne $coverage -and $coverage.Count -eq 2 -and [double]$coverage[1] -gt 0) {
                $coverageValues += [double]$coverage[0] / [double]$coverage[1]
            }
        }
        $artifactRequired = ($items | ForEach-Object { [int]$_.artifacts.required } | Measure-Object -Sum).Sum
        $artifactPreviewable = ($items | ForEach-Object { [int]$_.artifacts.previewable } | Measure-Object -Sum).Sum
        $rows += [pscustomobject]@{
            suite = $Suite
            variant = $Variant
            case_id = $group.Name
            runs = $items.Count
            successes = $successes
            success_rate = [Math]::Round($successes / [double]$items.Count, 6)
            wilson_lower_95 = [Math]::Round((Get-WilsonLower $successes $items.Count), 6)
            quality_mean = [Math]::Round([double]$quality, 6)
            fact_recall = if ($requiredFacts -gt 0) { [Math]::Round($retainedFacts / [double]$requiredFacts, 6) } else { 1.0 }
            stale_scope_failures = $staleFailures
            summary_field_coverage = if ($coverageValues.Count -gt 0) { [Math]::Round(($coverageValues | Measure-Object -Average).Average, 6) } else { $null }
            artifact_preview_rate = if ($artifactRequired -gt 0) { [Math]::Round($artifactPreviewable / [double]$artifactRequired, 6) } else { 1.0 }
            ttfr_p50_ms = [Math]::Round((Get-Quantile $ttfr 0.50), 0)
            ttfr_p90_ms = [Math]::Round((Get-Quantile $ttfr 0.90), 0)
            total_p50_ms = [Math]::Round((Get-Quantile $total 0.50), 0)
            token_mean = [Math]::Round(($tokens | Measure-Object -Average).Average, 0)
            model_requests_mean = [Math]::Round(($modelRequests | Measure-Object -Average).Average, 3)
            compactions_mean = [Math]::Round(($compactions | Measure-Object -Average).Average, 3)
            tool_calls_mean = [Math]::Round(($toolCalls | Measure-Object -Average).Average, 3)
            tool_errors = $toolErrors
            max_repeat = $maxRepeat
        }
    }
    return $rows
}

function Add-MetricTable([System.Text.StringBuilder]$Builder, [object[]]$Rows, [string]$Title) {
    [void]$Builder.AppendLine("## $Title")
    [void]$Builder.AppendLine()
    [void]$Builder.AppendLine("| Case | Variant | 通过 | Lower95 | 质量 | 事实召回 | 旧范围失败 | 摘要覆盖 | 压缩次数 | Token | TTFR P50 | 工具调用 |")
    [void]$Builder.AppendLine("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    foreach ($row in $Rows) {
        $coverageText = if ($null -eq $row.summary_field_coverage) { "N/A" } else { "{0:P0}" -f $row.summary_field_coverage }
        [void]$Builder.AppendLine((
            "| {0} | {1} | {2}/{3} | {4:P1} | {5:N3} | {6:P0} | {7} | {8} | {9:N1} | {10:N0} | {11:N0} ms | {12:N1} |" -f
            $row.case_id, $row.variant, $row.successes, $row.runs, $row.wilson_lower_95,
            $row.quality_mean, $row.fact_recall, $row.stale_scope_failures, $coverageText,
            $row.compactions_mean, $row.token_mean, $row.ttfr_p50_ms, $row.tool_calls_mean
        ))
    }
    [void]$Builder.AppendLine()
}

$script:EvalExeFull = Resolve-FullPath $EvalExe
$script:CasesDirFull = Resolve-FullPath $CasesDir
$configFull = Resolve-FullPath $ConfigPath
$outputRootFull = Resolve-FullPath $OutputRoot

if (-not (Test-Path -LiteralPath $script:EvalExeFull -PathType Leaf)) {
    throw "找不到 eval 可执行文件：$script:EvalExeFull。请先构建 cargo build -p galen --bin eval。"
}
if (-not (Test-Path -LiteralPath $script:CasesDirFull -PathType Container)) {
    throw "找不到任务卡目录：$script:CasesDirFull"
}
$config = Get-Content -LiteralPath $configFull -Raw | ConvertFrom-Json

& $script:EvalExeFull validate --cases $script:CasesDirFull
if ($LASTEXITCODE -ne 0) { throw "任务卡校验失败" }
if ($ValidateOnly) {
    Write-Host "VALID：$($config.experiment_id)"
    return
}

$profileConfig = $config.profiles.$Profile
$contextRepeat = [int]$profileConfig.context_repeat
$functionalRepeat = [int]$profileConfig.functional_repeat
if (-not $RunId) {
    $repoRoot = Resolve-FullPath (Join-Path $PSScriptRoot "..\..\..")
    $shortCommit = (& git -C $repoRoot rev-parse --short HEAD 2>$null)
    if (-not $shortCommit) { $shortCommit = "unknown" }
    $RunId = "{0}-{1}-{2}" -f (Get-Date -Format "yyyyMMdd-HHmmss"), $Profile, $shortCommit
}
$runDirectory = Join-Path $outputRootFull $RunId
if ((Test-Path -LiteralPath $runDirectory) -and -not $Resume) {
    throw "运行目录已存在：$runDirectory。请指定新的 RunId，或使用 -Resume。"
}
[System.IO.Directory]::CreateDirectory($runDirectory) | Out-Null
[System.IO.Directory]::CreateDirectory((Join-Path $runDirectory "raw")) | Out-Null

$baselineFiles = @()
$candidateFiles = @()
foreach ($caseId in $config.context.cases) {
    $baselinePath = Join-Path $runDirectory "raw\$caseId-baseline.jsonl"
    $candidatePath = Join-Path $runDirectory "raw\$caseId-candidate.jsonl"
    Invoke-EvalRun $caseId $config.context.baseline_variant $contextRepeat $baselinePath
    Invoke-EvalRun $caseId $config.context.candidate_variant $contextRepeat $candidatePath
    $baselineFiles += $baselinePath
    $candidateFiles += $candidatePath
}

$baselineCombined = Join-Path $runDirectory "context-baseline.jsonl"
$candidateCombined = Join-Path $runDirectory "context-candidate.jsonl"
Merge-Jsonl $baselineFiles $baselineCombined
Merge-Jsonl $candidateFiles $candidateCombined

$comparisonJson = Join-Path $runDirectory "context-comparison.json"
$comparisonError = Join-Path $runDirectory "context-comparison.stderr.txt"
$comparisonOutput = & $script:EvalExeFull compare --baseline $baselineCombined --candidate $candidateCombined --ignore-config 2>$comparisonError
$comparisonExitCode = $LASTEXITCODE
[System.IO.File]::WriteAllText($comparisonJson, (($comparisonOutput -join [Environment]::NewLine) + [Environment]::NewLine))

$functionalCombined = Join-Path $runDirectory "functional-current.jsonl"
$functionalFiles = @()
if (-not $SkipFunctional) {
    foreach ($caseId in $config.functional.cases) {
        $functionalPath = Join-Path $runDirectory "raw\$caseId-current.jsonl"
        Invoke-EvalRun $caseId $config.functional.variant $functionalRepeat $functionalPath
        $functionalFiles += $functionalPath
    }
    Merge-Jsonl $functionalFiles $functionalCombined
}

$baselineRecords = Read-Jsonl $baselineCombined
$candidateRecords = Read-Jsonl $candidateCombined
$functionalRecords = if (Test-Path -LiteralPath $functionalCombined) { Read-Jsonl $functionalCombined } else { @() }
$baselineRows = Get-MetricRows $baselineRecords "paired-context" "baseline:$($config.context.baseline_variant)"
$candidateRows = Get-MetricRows $candidateRecords "paired-context" "candidate:$($config.context.candidate_variant)"
$functionalRows = if ($functionalRecords.Count -gt 0) { Get-MetricRows $functionalRecords "paired-functional" "current:$($config.functional.variant)" } else { @() }
$metricRows = @($baselineRows) + @($candidateRows) + @($functionalRows)
$metricsCsv = Join-Path $runDirectory "metrics.csv"
$metricRows | Export-Csv -LiteralPath $metricsCsv -NoTypeInformation -Encoding UTF8

$runRows = @()
$failureRows = @()
$recordSets = @(
    [pscustomobject]@{ suite = "paired-context"; variant = "baseline:$($config.context.baseline_variant)"; records = $baselineRecords },
    [pscustomobject]@{ suite = "paired-context"; variant = "candidate:$($config.context.candidate_variant)"; records = $candidateRecords },
    [pscustomobject]@{ suite = "paired-functional"; variant = "current:$($config.functional.variant)"; records = $functionalRecords }
)
foreach ($set in $recordSets) {
    foreach ($record in @($set.records)) {
        $requiredFacts = [int]$record.context.required_facts
        $coverage = $record.context.summary_field_coverage
        $coverageValue = if ($null -ne $coverage -and $coverage.Count -eq 2 -and [double]$coverage[1] -gt 0) {
            [double]$coverage[0] / [double]$coverage[1]
        } else { $null }
        $runRows += [pscustomobject]@{
            suite = $set.suite
            variant = $set.variant
            run_id = $record.run_id
            case_id = $record.case_id
            run_index = $record.run_index
            hard_gates_passed = $record.hard_gates_passed
            quality_score = $record.quality_score
            fact_recall = if ($requiredFacts -gt 0) { [double]$record.context.retained_facts / $requiredFacts } else { 1.0 }
            summary_field_coverage = $coverageValue
            compactions = $record.context.compactions
            tokens_total = [double]$record.usage.input + [double]$record.usage.output + [double]$record.usage.cache_create + [double]$record.usage.cache_read
            model_requests = $record.model_requests
            tool_calls = $record.tools.calls
            tool_errors = $record.tools.errors
            max_repeat = $record.tools.max_repeat
            ttfr_ms = $record.latency.ttfr_ms
            total_ms = $record.latency.total_ms
            artifact_valid = $record.artifacts.valid
            artifact_previewable = $record.artifacts.previewable
        }
        foreach ($assertion in @($record.assertions | Where-Object { -not $_.pass })) {
            $failureRows += [pscustomobject]@{
                suite = $set.suite
                variant = $set.variant
                run_id = $record.run_id
                case_id = $record.case_id
                run_index = $record.run_index
                assertion = $assertion.name
                hard_gate = $assertion.hard_gate
                detail = $assertion.detail
            }
        }
    }
}
$runsCsv = Join-Path $runDirectory "runs.csv"
$failuresCsv = Join-Path $runDirectory "failures.csv"
$runRows | Export-Csv -LiteralPath $runsCsv -NoTypeInformation -Encoding UTF8
if ($failureRows.Count -gt 0) {
    $failureRows | Export-Csv -LiteralPath $failuresCsv -NoTypeInformation -Encoding UTF8
} else {
    [System.IO.File]::WriteAllText(
        $failuresCsv,
        '"suite","variant","run_id","case_id","run_index","assertion","hard_gate","detail"' + [Environment]::NewLine
    )
}

$comparison = $null
try { $comparison = Get-Content -LiteralPath $comparisonJson -Raw | ConvertFrom-Json } catch {}
$decision = if ($null -ne $comparison) { [string]$comparison.decision } elseif ($comparisonExitCode -eq 0) { "accept" } else { "unavailable" }
$candidateCompactionRuns = @($candidateRecords | Where-Object { [int]$_.context.compactions -gt 0 }).Count

$builder = [System.Text.StringBuilder]::new()
[void]$builder.AppendLine("# Galen 配对实验报告")
[void]$builder.AppendLine()
[void]$builder.AppendLine("> Run ID: ``$RunId`` · Profile: ``$Profile`` · 原始 JSONL 为事实源。")
[void]$builder.AppendLine()
[void]$builder.AppendLine("## 判定")
[void]$builder.AppendLine()
[void]$builder.AppendLine("- 上下文候选决策：**$decision**")
[void]$builder.AppendLine("- 上下文运行：baseline $($baselineRecords.Count) 条；candidate $($candidateRecords.Count) 条。")
[void]$builder.AppendLine("- 候选压缩实际启用：$candidateCompactionRuns/$($candidateRecords.Count) 条。")
[void]$builder.AppendLine("- 工具/数据运行：$($functionalRecords.Count) 条；当前仅为功能基线，尚未伪装成策略配对。")
if ($null -ne $comparison -and $comparison.reasons.Count -gt 0) {
    foreach ($reason in $comparison.reasons) { [void]$builder.AppendLine("- $reason") }
}
[void]$builder.AppendLine()
Add-MetricTable $builder (@($baselineRows) + @($candidateRows)) "上下文配对指标"
if ($functionalRows.Count -gt 0) { Add-MetricTable $builder $functionalRows "工具与数据功能指标" }
[void]$builder.AppendLine("## 失败热点")
[void]$builder.AppendLine()
if ($failureRows.Count -eq 0) {
    [void]$builder.AppendLine("无失败断言。")
} else {
    [void]$builder.AppendLine("| 次数 | 断言 |")
    [void]$builder.AppendLine("|---:|---|")
    foreach ($failureGroup in ($failureRows | Group-Object assertion | Sort-Object Count -Descending | Select-Object -First 12)) {
        $assertionText = $failureGroup.Name.Replace("|", "\|")
        [void]$builder.AppendLine("| $($failureGroup.Count) | ``$assertionText`` |")
    }
}
[void]$builder.AppendLine()
[void]$builder.AppendLine("## 产物")
[void]$builder.AppendLine()
[void]$builder.AppendLine("- ``context-baseline.jsonl`` / ``context-candidate.jsonl``：逐次原始记录")
[void]$builder.AppendLine("- ``context-comparison.json``：评测器正式比较结果")
[void]$builder.AppendLine("- ``metrics.csv``：按案例聚合指标")
[void]$builder.AppendLine("- ``runs.csv``：逐次运行宽表，可直接进入统计软件")
[void]$builder.AppendLine("- ``failures.csv``：失败断言清单，用于确定下一刀")
[void]$builder.AppendLine("- ``manifest.json``：运行环境、参数和文件哈希")
[void]$builder.AppendLine("- ``raw/``：每个案例的独立记录，支持断点核验")
[System.IO.File]::WriteAllText((Join-Path $runDirectory "report.md"), $builder.ToString())

$repoRootForManifest = Resolve-FullPath (Join-Path $PSScriptRoot "..\..\..")
$commit = (& git -C $repoRootForManifest rev-parse HEAD 2>$null)
$dirty = [bool]((& git -C $repoRootForManifest status --porcelain --untracked-files=no 2>$null) | Select-Object -First 1)
$manifest = [ordered]@{
    schema_version = 1
    experiment_id = $config.experiment_id
    run_id = $RunId
    profile = $Profile
    started_or_completed_at = (Get-Date).ToString("o")
    git_commit = if ($commit) { [string]$commit } else { "unknown" }
    git_dirty = $dirty
    model = if ($Model) { $Model } else { "configured-default" }
    persona = $Persona
    context_repeat = $contextRepeat
    functional_repeat = if ($SkipFunctional) { 0 } else { $functionalRepeat }
    cases_dir = $script:CasesDirFull
    eval_exe = $script:EvalExeFull
    decision = $decision
    comparison_exit_code = $comparisonExitCode
    files = [ordered]@{
        context_baseline = (Get-FileHash -LiteralPath $baselineCombined -Algorithm SHA256).Hash
        context_candidate = (Get-FileHash -LiteralPath $candidateCombined -Algorithm SHA256).Hash
        functional_current = if (Test-Path -LiteralPath $functionalCombined) { (Get-FileHash -LiteralPath $functionalCombined -Algorithm SHA256).Hash } else { $null }
        metrics_csv = (Get-FileHash -LiteralPath $metricsCsv -Algorithm SHA256).Hash
        runs_csv = (Get-FileHash -LiteralPath $runsCsv -Algorithm SHA256).Hash
        failures_csv = (Get-FileHash -LiteralPath $failuresCsv -Algorithm SHA256).Hash
    }
}
[System.IO.File]::WriteAllText(
    (Join-Path $runDirectory "manifest.json"),
    (($manifest | ConvertTo-Json -Depth 8) + [Environment]::NewLine)
)

Write-Host "DONE $runDirectory"
Write-Host "REPORT $(Join-Path $runDirectory 'report.md')"
Write-Host "DECISION $decision (compare exit=$comparisonExitCode)"
