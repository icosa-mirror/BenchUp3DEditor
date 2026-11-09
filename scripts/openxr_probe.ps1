# Run from the repository root in a host PowerShell terminal. Environment changes
# apply only to this invocation; no runtime selection or system installation.
[CmdletBinding()]
param(
    [ValidateSet('Diagnostics', 'Panel')]
    [string]$Probe = 'Diagnostics',
    [string]$LoaderDirectory,
    [string]$ValidationDirectory
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath 'OPENXR_PLAN.md')) {
    throw 'Run this script from the BenchUp repository root.'
}
if (-not $LoaderDirectory) {
    $steam = (Get-ItemProperty 'HKCU:\Software\Valve\Steam' -ErrorAction SilentlyContinue).SteamPath
    if ($steam) {
        $LoaderDirectory = Join-Path $steam 'steamapps/common/SteamVR/bin/win64'
    }
}
if ($LoaderDirectory -and -not (Test-Path -LiteralPath (Join-Path $LoaderDirectory 'openxr_loader.dll'))) {
    throw 'Loader directory does not contain openxr_loader.dll. Pass -LoaderDirectory with the installed SteamVR win64 directory.'
}

$variables = @('PATH', 'VK_LAYER_PATH', 'BENCHUP_XR_VALIDATION', 'VK_LAYER_SETTINGS_PATH')
$saved = @{}
foreach ($name in $variables) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    if ($LoaderDirectory) {
        $env:PATH = "$LoaderDirectory;$env:PATH"
        Write-Output "[BenchUpXR] Loader directory: $LoaderDirectory"
    }
    if ($ValidationDirectory) {
        $ValidationDirectory = (Resolve-Path -LiteralPath $ValidationDirectory).Path
        if (-not (Test-Path -LiteralPath (Join-Path $ValidationDirectory 'VkLayer_khronos_validation.json'))) {
            throw 'Validation directory must contain VkLayer_khronos_validation.json and its matching DLL.'
        }
        $settings = Join-Path (Get-Location).Path '.local/openxr/settings'
        New-Item -ItemType Directory -Force -Path $settings | Out-Null
        @'
khronos_validation.debug_action = VK_DBG_LAYER_ACTION_LOG_MSG
khronos_validation.log_filename = stdout
khronos_validation.report_flags = error,warn,info
khronos_validation.validate_sync = true
'@ | Set-Content -LiteralPath (Join-Path $settings 'vk_layer_settings.txt')
        $env:VK_LAYER_PATH = $ValidationDirectory
        $env:BENCHUP_XR_VALIDATION = '1'
        $env:VK_LAYER_SETTINGS_PATH = $settings
        Write-Output "[BenchUpXR] Vulkan validation requested (including synchronization): $ValidationDirectory"
    }
    $example = if ($Probe -eq 'Panel') { 'openxr_panel' } else { 'openxr_diagnostics' }
    & cargo run -p benchup-app --locked --features openxr --example $example
    $result = $LASTEXITCODE
}
finally {
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
exit $result
