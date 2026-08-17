# 0.5.0 MCP Gateway 冒烟测试脚本
# 用法：启动编辑器（打开 test_res002）后执行：powershell -File test/case/mcp_smoke.ps1 [-Port 39177]
# 不传 Port 时读 D:\sce_online\logs\bgd_csharp\port
param([int]$Port = 0)

$ErrorActionPreference = 'Continue'
$engineRoot = 'D:\sce_online'
if ($Port -eq 0) {
    $Port = [int](Get-Content "$engineRoot\logs\bgd_csharp\port" -Raw).Trim()
}
$base = "http://127.0.0.1:$Port"
$script:pass = 0
$script:fail = 0
$script:skip = 0

function Check([string]$name, [bool]$cond, [string]$detail = '') {
    if ($cond) { $script:pass++; Write-Host "[PASS] $name" -ForegroundColor Green }
    else { $script:fail++; Write-Host "[FAIL] $name  $detail" -ForegroundColor Red }
}
function Skip([string]$name, [string]$why) { $script:skip++; Write-Host "[SKIP] $name  ($why)" -ForegroundColor Yellow }

function RpcRaw([string]$method, $params) {
    $body = @{ id = 1; method = $method; params = $params } | ConvertTo-Json -Depth 12 -Compress
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($body)
    Invoke-WebRequest -Uri "$base/rpc" -Method Post -ContentType 'application/json; charset=utf-8' -Body $bytes -TimeoutSec 130
}
function Rpc([string]$method, $params = $null) {
    $r = RpcRaw $method $params
    ($r.Content | ConvertFrom-Json)
}
function InvokeCap([string]$id, $capArgs = $null, [int]$timeoutMs = 30000) {
    $p = @{ id = $id }
    if ($null -ne $capArgs) { $p.args = $capArgs }
    $p.timeout_ms = $timeoutMs
    Rpc 'invoke_capability' $p
}

Write-Host "== target: $base ==" -ForegroundColor Cyan

# ---- M5-1 server_info ----
try {
    $si = Rpc 'server_info'
    Check 'M5-1 server_info.catalog_count>0' ($si.result.catalog_count -gt 0) ($si | ConvertTo-Json -Compress)
    Check 'M5-1 server_info.catalog_drifted=false' ($si.result.catalog_drifted -eq $false)
    Check 'M5-1 server_info.port match' ($si.result.port -eq $Port) "actual=$($si.result.port)"
} catch { Check 'M5-1 server_info' $false $_.Exception.Message }

# ---- M2-1 tools/list ----
try {
    $init = @{ id = 1; method = 'initialize'; params = @{ protocolVersion = '2025-03-26'; capabilities = @{}; clientInfo = @{ name = 'smoke'; version = '0' } } } | ConvertTo-Json -Depth 8 -Compress
    Invoke-WebRequest -Uri "$base/mcp" -Method Post -ContentType 'application/json' -Body ([System.Text.Encoding]::UTF8.GetBytes($init)) -TimeoutSec 10 | Out-Null
    $tlRaw = RpcRaw 'tools/list' @{}  # /rpc 分发
    $tl = $tlRaw.Content | ConvertFrom-Json
    # 走 /mcp 的 tools/list 才是真实载荷
    $mcpBody = @{ jsonrpc = '2.0'; id = 2; method = 'tools/list' } | ConvertTo-Json -Compress
    $mcpResp = Invoke-WebRequest -Uri "$base/mcp" -Method Post -ContentType 'application/json' -Body ([System.Text.Encoding]::UTF8.GetBytes($mcpBody)) -TimeoutSec 10
    $size = [System.Text.Encoding]::UTF8.GetByteCount($mcpResp.Content)
    $tools = ($mcpResp.Content | ConvertFrom-Json).result.tools
    Check 'M2-1 tools/list 恰好10个元工具' ($tools.Count -eq 10) "count=$($tools.Count)"
    Check "M2-1/M6-1 tools/list 载荷<=4KB (实际 ${size}B)" ($size -le 4096)
    Check 'M1-1 tools/list 无unicode转义' ($mcpResp.Content -notmatch '\\u[0-9a-fA-F]{4}')
} catch { Check 'M2-1 tools/list' $false $_.Exception.Message }

# ---- M2-2 list_namespaces ----
try {
    $ns = Rpc 'list_namespaces'
    $txt = $ns.result | ConvertTo-Json -Compress
    Check 'M2-2 list_namespaces 含各空间' ($txt -match 'svc' -and $txt -match 'datacore' -and $txt -match 'cmd' -and $txt -match 'lua' -and $txt -match 'sys') $txt
} catch { Check 'M2-2 list_namespaces' $false $_.Exception.Message }

# ---- M2-3 旧方法已删 ----
try {
    $old = Rpc 'list_tool_categories'
    Check 'M2-3 list_tool_categories -> UNKNOWN_METHOD' ($old.error.code -eq 'UNKNOWN_METHOD') ($old | ConvertTo-Json -Compress)
} catch { Check 'M2-3 旧方法' $false $_.Exception.Message }

# ---- M3-1 search（含别名） ----
try {
    $s1 = Rpc 'search_capabilities' @{ query = '文件存在' }
    $hits1 = $s1.result | ConvertTo-Json -Compress -Depth 6
    Check 'M3-1 search 中文描述命中' (($hits1 -match 'svc\.FileSystem\.FileExists') -eq $true) $hits1
    $s2 = Rpc 'search_capabilities' @{ query = 'file_exists' }
    $hits2 = $s2.result | ConvertTo-Json -Compress -Depth 6
    Check 'M3-1 search 别名命中' (($hits2 -match 'svc\.FileSystem\.FileExists') -eq $true) $hits2
} catch { Check 'M3-1 search' $false $_.Exception.Message }

# ---- M3-2 参数自愈 ----
try {
    $r = InvokeCap 'svc.FileSystem.FileExists'
    $errTxt = $r | ConvertTo-Json -Depth 10 -Compress
    Check 'M3-2 缺参 PARAM_INVALID' ($r.error.code -eq 'PARAM_INVALID') $errTxt
    Check 'M3-2 内嵌 compact schema' ($null -ne $r.error.schema -and $null -ne $r.error.schema.properties) $errTxt
} catch { Check 'M3-2 参数自愈' $false $_.Exception.Message }

# ---- M3-3 FileExists 正调 ----
try {
    $r = InvokeCap 'svc.FileSystem.FileExists' @{ fileName = "$engineRoot\logs\bgd_csharp\port" }
    $txt = $r | ConvertTo-Json -Depth 10 -Compress
    Check 'M3-3 FileExists=true' ($txt -match 'true') $txt
} catch { Check 'M3-3 FileExists' $false $_.Exception.Message }

# ---- M3-4 ScanDir 正调（托管签名无默认参数，全量传参） ----
try {
    $r = InvokeCap 'svc.FileSystem.ScanDir' @{ pathName = "$engineRoot\logs\bgd_csharp"; filter = '*.*'; flags = 0; recursive = $false }
    $txt = $r | ConvertTo-Json -Depth 10 -Compress
    Check 'M3-4 ScanDir 正调返回列表' ($null -eq $r.error) $txt
} catch { Check 'M3-4 ScanDir' $false $_.Exception.Message }

# ---- M3-5 未准入服务 ----
try {
    $r = InvokeCap 'svc.EditorSettingsManager.GetAutoSaveStatus'
    $txt = $r | ConvertTo-Json -Depth 10 -Compress
    Check 'M3-5 未准入服务被拒' ($null -ne $r.error) $txt
} catch { Check 'M3-5 未准入' $false $_.Exception.Message }

# ---- M3-6 黑名单 danger ----
try {
    $r = InvokeCap 'svc.FileSystem.SystemCommand' @{ cmd = 'echo hi' }
    $txt = $r | ConvertTo-Json -Depth 10 -Compress
    Check 'M3-6 黑名单拒绝' ($null -ne $r.error) $txt
} catch { Check 'M3-6 黑名单' $false $_.Exception.Message }

# ---- D-1 get_status ----
try {
    $gs = Rpc 'get_status'
    $txt = $gs | ConvertTo-Json -Depth 10 -Compress
    Check 'D-1 get_status.map_path 非空' (-not [string]::IsNullOrEmpty($gs.result.map_path)) $txt
} catch { Check 'D-1 get_status' $false $_.Exception.Message }

# ---- M4 datacore ----
# R2 实证结论：commit 不可撤销 → 写类 danger 级、默认不自动提交。先验证默认拒绝，再临时放行
$cfgPath = "$engineRoot\logs\bgd_csharp\config.json"
$cfgOrig = if (Test-Path $cfgPath) { Get-Content $cfgPath -Raw } else { '{"mcp_port":39177}' }
try {
    $denied = InvokeCap 'datacore.write' @{ link = '$$.map_config.dflt.root'; path = @('opened_slots'); value = @() }
    Check 'M4-0 写类默认 danger 拒绝' ($denied.error.code -eq 'DANGER_DENIED') ($denied | ConvertTo-Json -Depth 8 -Compress)
    [System.IO.File]::WriteAllText($cfgPath, '{"mcp_port":39177,"danger_allow":["datacore.*"]}')
try {
    $r = InvokeCap 'datacore.list_entries'
    $txt = $r | ConvertTo-Json -Depth 10 -Compress
    Check 'M4-1 list_entries' ($null -eq $r.error) $txt
    if ($null -eq $r.error) {
        $r2 = InvokeCap 'datacore.read' @{ link = '$$.map_config.dflt.root' }
        $txt2 = $r2 | ConvertTo-Json -Depth 10 -Compress
        Check 'M4-2 read map_config' ($null -eq $r2.error) $txt2
        # 写回读：opened_slots（官方同款可写字段，list 类型）。数编不允许新建 schema 外字段；Name 等文本字段需 MakeText
        $w = InvokeCap 'datacore.write' @{ link = '$$.map_config.dflt.root'; path = @('Game', 'opened_slots'); value = @(1, 2); auto_commit = $false }
        $txtW = $w | ConvertTo-Json -Depth 10 -Compress
        Check 'M4-3 write(带Game前缀,不commit)' ($txtW -match '"applied":true' -and $txtW -match '"committed":false') $txtW
        $rst = InvokeCap 'datacore.write' @{ link = '$$.map_config.dflt.root'; path = @('opened_slots'); value = @(); auto_commit = $true }
        $txtRst = $rst | ConvertTo-Json -Depth 10 -Compress
        Check 'M4-3 还原(无Game前缀+commit)' ($txtRst -match '"committed":true') $txtRst
        $bw = InvokeCap 'datacore.batch_write' @{ changes = @(
            @{ link = '$$.map_config.dflt.root'; path = @('opened_slots'); value = @(2) },
            @{ link = '$$.map_config.dflt.root'; path = @('opened_slots'); value = @() }
        ); auto_commit = $true }
        $txtBw = $bw | ConvertTo-Json -Depth 10 -Compress
        Check 'M4-4 batch_write' ($txtBw -match '"applied_count":2' -and $txtBw -match '"committed":true') $txtBw
        $bad = InvokeCap 'datacore.batch_write' @{ changes = @(
            @{ link = '$$.map_config.dflt.root'; path = @('opened_slots'); value = @() },
            @{ link = '$$.nonexist.entry.root'; path = @('a'); value = 1 }
        ) }
        $txtBad = $bad | ConvertTo-Json -Depth 10 -Compress
        Check 'M4-5 batch_write 遇错即断' ($txtBad -match '"failed_index":1') $txtBad
    } else { Skip 'M4-2~M4-5' 'list_entries 失败' }
} catch { Check 'M4 datacore' $false $_.Exception.Message }
} finally { [System.IO.File]::WriteAllText($cfgPath, $cfgOrig) }

# ---- M7-1 run_lua 默认拒绝 ----
try {
    $r = InvokeCap 'lua.run_lua' @{ code = 'return 1+1' }
    $txt = $r | ConvertTo-Json -Depth 10 -Compress
    Check 'M7-1 run_lua 默认 danger 拒绝' ($null -ne $r.error) $txt
} catch { Check 'M7-1 run_lua' $false $_.Exception.Message }

# ---- M5-3 danger 拒绝推事件 / /events 可用 ----
try {
    $ev = Invoke-WebRequest -Uri "$base/events?since=0" -Method Get -TimeoutSec 10
    $evj = $ev.Content | ConvertFrom-Json
    $evTxt = $ev.Content
    Check 'M5-3 /events 可用' ($null -ne $evj.events)
    Check 'M5-3 danger_denied 事件' ($evTxt -match 'danger_denied') $evTxt.Substring(0, [Math]::Min(400, $evTxt.Length))
} catch { Check 'M5-3 /events' $false $_.Exception.Message }

# ---- N 异常免疫（类型不符参数零异常，防编辑器「发生异常」模态） ----
try {
    $r1 = Rpc 'invoke_capability' @{ id = 'sys.server_info'; timeout_ms = '5000' }
    Check 'N-1 timeout_ms 字符串宽容' ($null -eq $r1.error) ($r1 | ConvertTo-Json -Compress -Depth 6)
    $r2 = Rpc 'invoke_capability' @{ id = 'sys.server_info'; timeout_ms = @{ x = 1 } }
    Check 'N-2 timeout_ms 非法类型回落默认' ($null -eq $r2.error) ($r2 | ConvertTo-Json -Compress -Depth 6)
    $r3 = Rpc 'search_capabilities' @{ query = '文件'; limit = '3' }
    Check 'N-3 limit 字符串宽容' ($null -eq $r3.error) ($r3 | ConvertTo-Json -Compress -Depth 6)
} catch { Check 'N 异常免疫' $false $_.Exception.Message }

# ---- M5-2 审计日志 ----
$today = Get-Date -Format 'yyyy-MM-dd'
$audit = "$engineRoot\logs\bgd_csharp\audit-$today.log"
Check 'M5-2 audit 日志存在且有条目' ((Test-Path $audit) -and (Get-Content $audit -Raw).Length -gt 0) $audit

Write-Host ""
Write-Host "== PASS=$($script:pass) FAIL=$($script:fail) SKIP=$($script:skip) ==" -ForegroundColor Cyan
