param([string]$OutPrefix = "win")
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class WinEnum {
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lp);
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lp);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr hWnd, StringBuilder sb, int max);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$procIds = (Get-Process insightforge-desktop -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
if (-not $procIds) { Write-Output "NO_PROCESS"; exit 1 }
$found = @()
$cb = [WinEnum+EnumWindowsProc]{
    param($hWnd, $lp)
    $pid2 = 0
    [WinEnum]::GetWindowThreadProcessId($hWnd, [ref]$pid2) | Out-Null
    if ($procIds -contains [int]$pid2 -and [WinEnum]::IsWindowVisible($hWnd)) {
        $sb = New-Object System.Text.StringBuilder 256
        [WinEnum]::GetWindowText($hWnd, $sb, 256) | Out-Null
        $script:found += [PSCustomObject]@{ HWnd = $hWnd; Title = $sb.ToString() }
    }
    return $true
}
[WinEnum]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
$i = 0
foreach ($w in $found) {
    $i++
    $rect = New-Object WinEnum+RECT
    [WinEnum]::GetWindowRect($w.HWnd, [ref]$rect) | Out-Null
    $width = $rect.Right - $rect.Left; $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) { continue }
    $bmp = New-Object System.Drawing.Bitmap $width, $height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [WinEnum]::PrintWindow($w.HWnd, $hdc, 2) | Out-Null   # PW_RENDERFULLCONTENT
    $g.ReleaseHdc($hdc); $g.Dispose()
    $title = if ($w.Title) { $w.Title -replace '[^a-zA-Z0-9\u4e00-\u9fff-]', '_' } else { "untitled" }
    if ($title.Length -gt 30) { $title = $title.Substring(0, 30) }
    $file = "$OutPrefix-$i-$title.png"
    $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Output "CAPTURED $file ${width}x${height} title='$($w.Title)'"
}
