# Start tauri dev detached from the shell session (ticket 08 Golden Path).
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File start-tauri-dev-08.ps1 <worktree> <outLog> <errLog>
param(
    [Parameter(Mandatory = $true)][string]$Worktree,
    [Parameter(Mandatory = $true)][string]$OutLog,
    [Parameter(Mandatory = $true)][string]$ErrLog
)

$env:Path = 'C:\Users\20145\scoop\persist\rustup\.cargo\bin;' + $env:Path
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9222'

Start-Process -FilePath 'bun' `
    -ArgumentList 'run', 'tauri', 'dev' `
    -WorkingDirectory $Worktree `
    -WindowStyle Hidden `
    -RedirectStandardOutput $OutLog `
    -RedirectStandardError $ErrLog

Write-Output 'TAURI_DEV_STARTED'
