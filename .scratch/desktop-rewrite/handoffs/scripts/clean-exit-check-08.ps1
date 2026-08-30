# Clean-exit verification without the CDP debug env (ticket 08, per 01/02/05 convention).
# Starts tauri dev WITHOUT WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, waits for the app,
# closes the main window, and measures how long the app process takes to exit.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File clean-exit-check-08.ps1 <worktree> <outLog> <errLog>
param(
    [Parameter(Mandatory = $true)][string]$Worktree,
    [Parameter(Mandatory = $true)][string]$OutLog,
    [Parameter(Mandatory = $true)][string]$ErrLog
)

$env:Path = 'C:\Users\20145\scoop\persist\rustup\.cargo\bin;' + $env:Path
Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue

# Stop any leftover dev session artifacts first (keep the mock server alive by port).
Get-Process insightforge-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
foreach ($port in 1420, 9222) {
    Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue |
        ForEach-Object { Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue }
}
Get-CimInstance Win32_Process -Filter "Name = 'bun.exe'" |
    Where-Object { $_.CommandLine -like '*tauri*dev*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 2

Start-Process -FilePath 'bun' `
    -ArgumentList 'run', 'tauri', 'dev' `
    -WorkingDirectory $Worktree `
    -WindowStyle Hidden `
    -RedirectStandardOutput $OutLog `
    -RedirectStandardError $ErrLog

# Wait for the app process to appear.
$deadline = (Get-Date).AddSeconds(120)
$app = $null
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $app = Get-Process insightforge-desktop -ErrorAction SilentlyContinue
    if ($app) { break }
}
if (-not $app) {
    Write-Output 'APP-NOT-STARTED'
    exit 1
}
Write-Output ('APP-STARTED pid=' + $app.Id)
Start-Sleep -Seconds 4

# Close the main window and measure exit time.
$startedAt = Get-Date
$app.CloseMainWindow() | Out-Null
$exited = $false
while ((Get-Date) -lt $startedAt.AddSeconds(5)) {
    Start-Sleep -Milliseconds 100
    if (-not (Get-Process insightforge-desktop -ErrorAction SilentlyContinue)) {
        $exited = $true
        break
    }
}
$elapsedMs = [math]::Round(((Get-Date) - $startedAt).TotalMilliseconds)
if ($exited) {
    Write-Output ('APP-EXITED-AFTER-' + $elapsedMs + '-MS')
} else {
    Write-Output 'APP-STILL-RUNNING-AFTER-5S'
}

# Wait for the tauri dev bun wrapper to exit as well (dev session ends cleanly).
$devDeadline = (Get-Date).AddSeconds(15)
while ((Get-Date) -lt $devDeadline) {
    $dev = Get-CimInstance Win32_Process -Filter "Name = 'bun.exe'" |
        Where-Object { $_.CommandLine -like '*tauri*dev*' }
    if (-not $dev) { break }
    Start-Sleep -Milliseconds 500
}
if ($dev) {
    Write-Output 'TAURI-DEV-WRAPPER-STILL-RUNNING'
} else {
    Write-Output 'TAURI-DEV-WRAPPER-EXITED'
}
