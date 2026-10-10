# Fail if README or specification drift from clap flags or HUD labels.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$main = Get-Content (Join-Path $root "src\main.rs") -Raw
$ui = Get-Content (Join-Path $root "src\ui.rs") -Raw
$readme = Get-Content (Join-Path $root "README.md") -Raw
$spec = Get-Content (Join-Path $root "specification.md") -Raw

$fields = [regex]::Matches($main, '(?m)^\s+(\w+):\s+(?:Option<)?(?:PathBuf|String|bool|u32)') |
    ForEach-Object { $_.Groups[1].Value } |
    Select-Object -Unique
$missing = @()
foreach ($field in $fields) {
    $flag = "--" + ($field -creplace '_', '-')
    if ($readme -notmatch [regex]::Escape($flag) -or $spec -notmatch [regex]::Escape($flag)) {
        $missing += $flag
    }
}
$hud = @("PLUGIN", "SCOPE", "STATS", "PROMPT", "GRAPH", "LANES", "TOOLS", "Skill roster")
foreach ($label in $hud) {
    if ($ui -notmatch [regex]::Escape($label)) {
        $missing += "ui:$label"
    }
    if ($readme -notmatch [regex]::Escape($label) -or $spec -notmatch [regex]::Escape($label)) {
        $missing += "docs:$label"
    }
}
if ($missing.Count -gt 0) {
    Write-Error ("docs drift: " + ($missing -join ", "))
}
Write-Host "docs flags and HUD labels match"
Write-Host ("clap fields seen: " + ($fields -join ", "))
