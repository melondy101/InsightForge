# InsightForge Windows 手动验收脚本（Phase D 黄金路径）
#
# 自动化 E2E 需要 WebView2 + CDP，CI/无界面环境会被探针 BLOCKED。
# 本脚本用于在已签名安装的 Windows 主机上，按清单逐项手动验证核心黄金路径，
# 并先检查运行前提（WebView2 运行时、构建产物）。
#
# 用法：powershell -ExecutionPolicy Bypass -File e2e/manual/verify.ps1

$ErrorActionPreference = 'Continue'

function Check-WebView2 {
    $key = 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A08C11}'
    if (Test-Path $key) {
        Write-Host '[OK] WebView2 runtime detected' -ForegroundColor Green
        return $true
    }
    Write-Host '[MISSING] WebView2 runtime not found — install Microsoft Edge WebView2 Runtime' -ForegroundColor Red
    return $false
}

function Check-Build {
    $exe = Get-ChildItem -Path 'src-tauri\target\release' -Filter 'insightforge.exe' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($exe) {
        Write-Host "[OK] Release build present: $($exe.FullName)" -ForegroundColor Green
        return $true
    }
    Write-Host '[MISSING] release build not found (run: bun run tauri build)' -ForegroundColor Red
    return $false
}

Write-Host '=== InsightForge 手动验收前提检查 ===' -ForegroundColor Cyan
$pre1 = Check-WebView2
$pre2 = Check-Build

if (-not ($pre1 -and $pre2)) {
    Write-Host '前提不满足，跳过清单。' -ForegroundColor Yellow
    exit 1
}

Write-Host ''
Write-Host '=== 黄金路径手动验收清单 ===' -ForegroundColor Cyan
$items = @(
    '1. 首启应用，确认无崩溃且工作区可见',
    '2. 来源：添加知识文件夹 → 观察索引进度 → 完成（Idle）',
    '3. 搜索（中文关键词）→ 出现证据卡',
    '4. 证据卡「钉入草稿」→ 打开草稿 → 引用tab出现该引用',
    '5. 编辑草稿 → 保存 → 用外部编辑器改动磁盘文件 → 再保存 → 出现三选一冲突',
    '6. 选择「保留本地 / 采用外部 / 另存副本」各验证一次，冲突面板消失',
    '7. 删除一个来源 → 确认提示「只清索引不碰文件」→ 草稿/诊断快照仍在',
    '8. 导出 HTML → 确认下载/复制成功',
    '9. 未配置 AI Key 时，AI 命令应不可用但其余功能正常（降级）',
    '10. 重启应用 → 草稿与来源仍存在（状态保持）',
    '11. 设置 → 诊断 → 复制快照 / 导出 → 内容不含 Key/正文/路径'
)

foreach ($it in $items) {
    Write-Host "  [ ] $it"
}

Write-Host ''
Write-Host '逐项验证后在上方 [ ] 打勾。任意一项失败即视为发布阻塞。' -ForegroundColor Yellow
