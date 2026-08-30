# Free a TCP port by killing its listener (ticket 08 tooling).
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File free-port-08.ps1 <port>
param(
    [Parameter(Mandatory = $true)][int]$Port
)

$conns = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
foreach ($conn in $conns) {
    $proc = Get-Process -Id $conn.OwningProcess -ErrorAction SilentlyContinue
    Write-Output ('port' + $Port + ' listener pid=' + $conn.OwningProcess + ' name=' + $proc.ProcessName)
    Stop-Process -Id $conn.OwningProcess -Force
}
Start-Sleep -Milliseconds 500
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    Write-Output ('PORT' + $Port + '-STILL-LISTENING')
} else {
    Write-Output ('PORT' + $Port + '-FREE')
}
