param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('codex', 'claude')]
    [string]$Backend,

    [Parameter(Mandatory = $true)]
    [string]$CommandArgsJson,

    [string]$GalenModelsPath = "$env:USERPROFILE\.galen\models.toml"
)

$ErrorActionPreference = 'Stop'
$runtimeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$commandArgs = @((ConvertFrom-Json $CommandArgsJson))
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

$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("galen-deepseek-{0}" -f [guid]::NewGuid())
[IO.Directory]::CreateDirectory($tempRoot) | Out-Null
$exitCode = 1
$originalEnvironment = @{
    USERPROFILE = $env:USERPROFILE
    HOME = $env:HOME
    CODEX_HOME = $env:CODEX_HOME
    CLAUDE_CONFIG_DIR = $env:CLAUDE_CONFIG_DIR
    ANTHROPIC_BASE_URL = $env:ANTHROPIC_BASE_URL
    ANTHROPIC_API_KEY = $env:ANTHROPIC_API_KEY
    ANTHROPIC_AUTH_TOKEN = $env:ANTHROPIC_AUTH_TOKEN
    CODEX_THREAD_ID = $env:CODEX_THREAD_ID
    CODEX_INTERNAL_ORIGINATOR_OVERRIDE = $env:CODEX_INTERNAL_ORIGINATOR_OVERRIDE
    CODEX_PERMISSION_PROFILE = $env:CODEX_PERMISSION_PROFILE
}

try {
    # Keep host-level rules, skills, plugins, sessions, and keychains out of
    # controlled runs. Backend-specific homes are created below this root.
    $env:USERPROFILE = $tempRoot
    $env:HOME = $tempRoot
    Remove-Item Env:CODEX_THREAD_ID -ErrorAction SilentlyContinue
    Remove-Item Env:CODEX_INTERNAL_ORIGINATOR_OVERRIDE -ErrorAction SilentlyContinue
    Remove-Item Env:CODEX_PERMISSION_PROFILE -ErrorAction SilentlyContinue
    if ($Backend -eq 'codex') {
        $codexHome = Join-Path $tempRoot 'codex-home'
        [IO.Directory]::CreateDirectory($codexHome) | Out-Null
        $setupScriptPath = Join-Path $runtimeRoot 'cache\codex-deepseek-setup-en.ps1'
        if (Test-Path -LiteralPath $setupScriptPath -PathType Leaf) {
            $setupScript = [IO.File]::ReadAllText($setupScriptPath)
        } else {
            $setupScript = Invoke-RestMethod -Uri 'https://cdn.deepseek.com/api-docs/codex-deepseek-setup-en.ps1'
        }
        if ($setupScript -notmatch "\`$SCRIPT_VERSION\s*=\s*'1\.4\.0'") {
            throw 'The official DeepSeek Codex setup catalog is not the pinned 1.4.0 version.'
        }
        $catalogMatch = [regex]::Match(
            $setupScript,
            "(?ms)\`$ModelsJson\s*=\s*@'\r?\n(.*?)\r?\n'@"
        )
        if (-not $catalogMatch.Success) {
            throw 'Could not extract the official DeepSeek Codex model catalog.'
        }
        $modelsPath = Join-Path $codexHome 'models.json'
        [IO.File]::WriteAllText($modelsPath, $catalogMatch.Groups[1].Value, [Text.UTF8Encoding]::new($false))
        $escapedKey = $apiKey.Replace('\', '\\').Replace('"', '\"')
        $catalogValue = $modelsPath.Replace('\', '/')
        $config = @"
model = "deepseek-flash"
model_provider = "deepseek"
forced_login_method = "api"
model_reasoning_effort = "high"
web_search = "disabled"
model_catalog_json = "$catalogValue"

[model_providers.deepseek]
name = "deepseek"
base_url = "https://api.deepseek.com/"
wire_api = "responses"
experimental_bearer_token = "$escapedKey"

[features]
skip_host_skill_discovery = true
plugins = false
apps = false
"@
        [IO.File]::WriteAllText((Join-Path $codexHome 'config.toml'), $config, [Text.UTF8Encoding]::new($false))
        $env:CODEX_HOME = $codexHome
        $codexPackage = Join-Path $runtimeRoot 'node_modules\@openai'
        $executable = Get-ChildItem $codexPackage -Recurse -File -Filter 'codex.exe' |
            Select-Object -First 1 -ExpandProperty FullName
    } else {
        $env:CLAUDE_CONFIG_DIR = Join-Path $tempRoot 'claude-config'
        [IO.Directory]::CreateDirectory($env:CLAUDE_CONFIG_DIR) | Out-Null
        $env:ANTHROPIC_BASE_URL = 'https://api.deepseek.com/anthropic'
        $env:ANTHROPIC_API_KEY = $apiKey
        $env:ANTHROPIC_AUTH_TOKEN = $apiKey
        $executable = Join-Path $runtimeRoot 'node_modules\@anthropic-ai\claude-code\bin\claude.exe'
    }

    if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
        throw "Pinned $Backend executable is missing. Run npm ci --prefix evals/external-runtimes."
    }
    # Native CLIs use stderr for warnings and progress. Do not let PowerShell
    # convert a non-fatal stderr line into a terminating ErrorRecord.
    $ErrorActionPreference = 'Continue'
    & $executable @commandArgs
    $exitCode = $LASTEXITCODE
} finally {
    $apiKey = $null
    $modelsText = $null
    $deepseekBlock = $null
    $keyMatch = $null
    foreach ($entry in $originalEnvironment.GetEnumerator()) {
        if ($null -eq $entry.Value) {
            Remove-Item -LiteralPath ("Env:{0}" -f $entry.Key) -ErrorAction SilentlyContinue
        } else {
            Set-Item -LiteralPath ("Env:{0}" -f $entry.Key) -Value $entry.Value
        }
    }
    if (Test-Path -LiteralPath $tempRoot) {
        $resolvedTemp = [IO.Path]::GetFullPath($tempRoot)
        $resolvedSystemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
        if ($resolvedTemp.StartsWith($resolvedSystemTemp, [StringComparison]::OrdinalIgnoreCase) -and
            (Split-Path -Leaf $resolvedTemp).StartsWith('galen-deepseek-')) {
            Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
        }
    }
}

exit $exitCode
