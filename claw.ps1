param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ClawArgs
)

$ErrorActionPreference = "Stop"

$envNames = @(
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "XAI_API_KEY",
    "XAI_BASE_URL",
    "DASHSCOPE_API_KEY",
    "DASHSCOPE_BASE_URL"
)

$wslEnvEntries = @()
if ($env:WSLENV) {
    $wslEnvEntries += ($env:WSLENV -split ":")
}

foreach ($name in $envNames) {
    if ([Environment]::GetEnvironmentVariable($name, "Process")) {
        $wslEnvEntries += "$name/u"
    }
}

$env:WSLENV = ($wslEnvEntries | Where-Object { $_ } | Select-Object -Unique) -join ":"

$windowsCwd = (Get-Location).Path
$wslInputPath = $windowsCwd -replace "\\", "/"
$wslCwd = (& wsl.exe wslpath -a $wslInputPath).Trim()

if (-not $wslCwd) {
    throw "Failed to convert current directory to a WSL path."
}

$env:CLAW_CWD = $wslCwd
if (-not ($wslEnvEntries -contains "CLAW_CWD/u")) {
    $wslEnvEntries += "CLAW_CWD/u"
    $env:WSLENV = ($wslEnvEntries | Where-Object { $_ } | Select-Object -Unique) -join ":"
}

& wsl.exe --cd $wslCwd /home/wen/.local/bin/claw @ClawArgs
exit $LASTEXITCODE
