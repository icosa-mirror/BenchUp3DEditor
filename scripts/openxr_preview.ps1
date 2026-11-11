# Launch an already-built interactive preview from the clean bundled example.
# Run through host PowerShell 7 from the repository root. This never builds.
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Binary,
    [string]$PreferencesFrom,
    [string]$LoaderDirectory,
    [switch]$PreserveViewMode
)

$ErrorActionPreference = 'Stop'
$repository = (Get-Location).Path
if (-not (Test-Path -LiteralPath 'OPENXR_PLAN.md')) {
    throw 'Run this script from the BenchUp repository root.'
}
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
if ([IO.Path]::GetFileName($binaryPath) -ne 'openxr_editor.exe') {
    throw 'Pass the already-built openxr_editor.exe to -Binary.'
}
$running = @(Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'openxr_editor.exe' })
if ($running.Count) {
    $running | Select-Object ProcessId, CreationDate, ExecutablePath | Format-List | Out-Host
    throw 'An XR preview is already running. Close it normally before starting another.'
}
if (-not $LoaderDirectory) {
    $steam = (Get-ItemProperty 'HKCU:\Software\Valve\Steam' -ErrorAction SilentlyContinue).SteamPath
    if ($steam) { $LoaderDirectory = Join-Path $steam 'steamapps/common/SteamVR/bin/win64' }
}
if (-not $LoaderDirectory -or -not (Test-Path -LiteralPath (Join-Path $LoaderDirectory 'openxr_loader.dll'))) {
    throw 'Pass -LoaderDirectory with the installed SteamVR win64 loader directory.'
}

$sessions = Join-Path $repository '.local/openxr/preview-sessions'
$lastSessionFile = Join-Path $sessions 'last-session.txt'
if (-not $PreferencesFrom -and (Test-Path -LiteralPath $lastSessionFile)) {
    $previous = (Get-Content -LiteralPath $lastSessionFile -Raw).Trim()
    $prefix = [IO.Path]::GetFullPath($sessions) + [IO.Path]::DirectorySeparatorChar
    if (-not [IO.Path]::GetFullPath($previous).StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The previous preview directory is outside the preview-sessions directory.'
    }
    $candidate = Join-Path $previous 'preferences.ron'
    if (Test-Path -LiteralPath $candidate) { $PreferencesFrom = $candidate }
}
if (-not $PreferencesFrom) {
    # One-time migration of the existing headset/readability controls, not its scene.
    $candidate = Join-Path $repository '.local/openxr/parity-data/preferences.ron'
    if (Test-Path -LiteralPath $candidate) { $PreferencesFrom = $candidate }
}
if ($PreferencesFrom) { $PreferencesFrom = (Resolve-Path -LiteralPath $PreferencesFrom).Path }

$stamp = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff')
$suffix = [Guid]::NewGuid().ToString('N').Substring(0, 8)
$sessionDirectory = Join-Path $sessions "$stamp-$suffix"
New-Item -ItemType Directory -Path $sessionDirectory -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $repository 'assets/examples/Example.bup') -Destination (Join-Path $sessionDirectory 'model.json')
if ($PreferencesFrom) {
    Copy-Item -LiteralPath $PreferencesFrom -Destination (Join-Path $sessionDirectory 'preferences.ron')
    if (-not $PreserveViewMode) {
        # This is the immersive test launcher. Carry readability/preferences,
        # but do not silently restore an older panel-only test session.
        $preferencesPath = Join-Path $sessionDirectory 'preferences.ron'
        $preferencesText = Get-Content -LiteralPath $preferencesPath -Raw
        $preferencesText = [regex]::Replace($preferencesText, '\bimmersive_scene:false\b', 'immersive_scene:true')
        Set-Content -LiteralPath $preferencesPath -Value $preferencesText -Encoding utf8NoBOM -NoNewline
    }
}
Write-Output $(if ($PreserveViewMode) { '[BenchUpXR] Restoring the saved view mode.' } else { '[BenchUpXR] Starting in immersive 3D mode.' })

$variables = @('PATH', 'BENCHUP_DATA_DIR', 'BENCHUP_XR_VALIDATION', 'VK_INSTANCE_LAYERS')
$saved = @{}
foreach ($name in $variables) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    $env:PATH = "$LoaderDirectory;$env:PATH"
    $env:BENCHUP_DATA_DIR = $sessionDirectory
    $env:BENCHUP_XR_VALIDATION = $null
    $env:VK_INSTANCE_LAYERS = $null
    $preview = Start-Process -FilePath $binaryPath -WorkingDirectory $repository -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $sessionDirectory 'stdout.log') `
        -RedirectStandardError (Join-Path $sessionDirectory 'stderr.log')
    Set-Content -LiteralPath $lastSessionFile -Value $sessionDirectory -Encoding utf8
    $file = Get-Item -LiteralPath $binaryPath
    [ordered]@{
        process_id = $preview.Id
        started_utc = [DateTime]::UtcNow.ToString('o')
        executable = $binaryPath
        executable_modified_utc = $file.LastWriteTimeUtc.ToString('o')
        data_directory = $sessionDirectory
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $sessionDirectory 'launch.json') -Encoding utf8
    Write-Output "[BenchUpXR] Preview PID: $($preview.Id)"
    Write-Output "[BenchUpXR] Executable: $binaryPath"
    Write-Output "[BenchUpXR] Executable built: $($file.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))"
    Write-Output "[BenchUpXR] Clean example workspace: $sessionDirectory"
    Write-Output '[BenchUpXR] No deadline. Close the BenchUp VR window to finish.'
}
finally {
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
