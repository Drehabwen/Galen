param(
    [Parameter(Mandatory = $true)]
    [string]$CasesDir,

    [Parameter(Mandatory = $true)]
    [string]$Output,

    [Parameter(Mandatory = $true)]
    [string]$RunTag,

    [string]$GalenModelsPath = "$env:USERPROFILE\.galen\models.toml"
)

$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$evalExecutable = Join-Path $repoRoot 'rust\target\release\eval.exe'
$casesPath = [IO.Path]::GetFullPath((Join-Path $repoRoot $CasesDir))
$outputPath = [IO.Path]::GetFullPath((Join-Path $repoRoot $Output))
$apiKey = [Environment]::GetEnvironmentVariable('DEEPSEEK_API_KEY', 'Process')

if ([string]::IsNullOrWhiteSpace($apiKey)) {
    if (-not (Test-Path -LiteralPath $GalenModelsPath -PathType Leaf)) {
        throw 'DEEPSEEK_API_KEY is unset and the Galen model config is unavailable.'
    }
    $modelsText = [IO.File]::ReadAllText($GalenModelsPath)
    $deepseekBlock = [regex]::Match(
        $modelsText,
        '(?ms)^\[models\.deepseek\]\s*(.*?)(?=^\[|\z)'
    ).Groups[1].Value
    $keyMatch = [regex]::Match($deepseekBlock, '(?m)^api_key\s*=\s*"([^"]+)"')
    if (-not $keyMatch.Success) {
        throw 'The Galen DeepSeek model entry does not contain an API credential.'
    }
    $apiKey = $keyMatch.Groups[1].Value
}

if (-not (Test-Path -LiteralPath $evalExecutable -PathType Leaf)) {
    throw "Galen evaluator executable is missing: $evalExecutable"
}
if (-not (Test-Path -LiteralPath $casesPath -PathType Container)) {
    throw "Galen cases directory is missing: $casesPath"
}
if (Test-Path -LiteralPath $outputPath) {
    throw "Refusing to overwrite Galen output: $outputPath"
}

$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("galen-flash-eval-{0}" -f [guid]::NewGuid())
[IO.Directory]::CreateDirectory($tempRoot) | Out-Null
$originalTag = $env:GALEN_EVAL_RUN_TAG
$exitCode = 1

try {
    $escapedKey = $apiKey.Replace('\', '\\').Replace('"', '\"')
    $config = @"
[router]
default = "deepseek-flash"
fast = "deepseek-flash"
analysis = "deepseek-flash"

[models.deepseek-flash]
provider = "openai_compat"
api_key = "$escapedKey"
model_id = "deepseek-flash"
base_url = "https://api.deepseek.com/v1"
"@
    [IO.File]::WriteAllText(
        (Join-Path $tempRoot 'models.toml'),
        $config,
        [Text.UTF8Encoding]::new($false)
    )
    $env:GALEN_EVAL_RUN_TAG = $RunTag
    Push-Location $tempRoot
    try {
        & $evalExecutable run `
            --case SABCLINTOX01 `
            --cases $casesPath `
            --model deepseek-flash `
            --persona none `
            --variant none `
            --repeat 1 `
            --output $outputPath
        $exitCode = $LASTEXITCODE
    } finally {
        Pop-Location
    }
} finally {
    $apiKey = $null
    $modelsText = $null
    $deepseekBlock = $null
    $keyMatch = $null
    if ($null -eq $originalTag) {
        Remove-Item Env:GALEN_EVAL_RUN_TAG -ErrorAction SilentlyContinue
    } else {
        $env:GALEN_EVAL_RUN_TAG = $originalTag
    }
    if (Test-Path -LiteralPath $tempRoot) {
        $resolvedTemp = [IO.Path]::GetFullPath($tempRoot)
        $resolvedSystemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
        if ($resolvedTemp.StartsWith($resolvedSystemTemp, [StringComparison]::OrdinalIgnoreCase) -and
            (Split-Path -Leaf $resolvedTemp).StartsWith('galen-flash-eval-')) {
            Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
        }
    }
}

exit $exitCode
