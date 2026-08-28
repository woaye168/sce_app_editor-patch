# CGUI 调试台全功能扫描（0.8.0 验收）：桥 HTTP 直连 invoke + exe CLI capture/logs
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"
$script:rpcId = 0
$script:fails = @()

function Invoke-Cap([string]$id, $capArgs) {
    $script:rpcId++
    $body = @{ jsonrpc = '2.0'; id = $script:rpcId; method = 'tools/call';
        params = @{ name = 'invoke_capability'; arguments = @{ id = $id; args = $capArgs } } } | ConvertTo-Json -Depth 8 -Compress
    try {
        $r = Invoke-RestMethod -Uri 'http://127.0.0.1:39177/mcp' -Method Post -ContentType 'application/json; charset=utf-8' -Body ([System.Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 15
        $txt = [string]$r.result.content[0].text
        $flat = ($txt -replace '\s+', ' ')
        $isErr = $r.result.isError -eq $true -or $flat -match '"ok": false' -or $flat -match '"error"'
        if ($isErr) { $script:fails += "$id :: $($flat.Substring(0, [Math]::Min(200, $flat.Length)))" }
        # 日志走 Console 直写，不占函数输出流（输出流只放返回值 $txt）；
        # 必须先 -f 拼好再 WriteLine——直接 WriteLine(fmt, a, b, c) 会绑错重载（WriteLine(string,object)）
        $tag = if ($isErr) { 'ERR' } else { 'OK ' }
        $logline = "{0} {1} :: {2}" -f ($tag, $id, $flat.Substring(0, [Math]::Min(160, $flat.Length)))
        [Console]::WriteLine($logline)
        return $txt
    }
    catch { $script:fails += "$id :: EXC $($_.Exception.Message)"; [Console]::WriteLine("EXC $id :: $($_.Exception.Message)"); return $null }
}

function Find-First([string]$q) {
    $txt = Invoke-Cap 'lua.find_ui' @{ q = $q }
    if (-not $txt) { return $null }
    try {
        $j = $txt | ConvertFrom-Json
        if ($j.items -and $j.items.Count -gt 0) { return [string]$j.items[0].id }
    } catch {}
    return $null
}

function Click([string]$cid) { Invoke-Cap 'lua.click_ui' @{ id = $cid } | Out-Null }
function Click-Found([string]$q) { $t = Find-First $q; if ($t) { Click $t } else { $script:fails += "find $q :: no hit" } }
# 找「可直接点的」：优先 clickable 本体，其次 clickable_ancestor（文本命中叶子时点父按钮）
function Find-Clickable([string]$q) {
    $txt = Invoke-Cap 'lua.find_ui' @{ q = $q }
    if (-not $txt) { return $null }
    try {
        $j = $txt | ConvertFrom-Json
        foreach ($it in $j.items) {
            if ($it.clickable -eq $true) { return [string]$it.id }
        }
        foreach ($it in $j.items) {
            if ($it.clickable_ancestor) { return [string]$it.clickable_ancestor }
        }
    } catch {}
    return $null
}
function Click-FC([string]$q) { $t = Find-Clickable $q; if ($t) { Click $t } else { $script:fails += "findc $q :: no hit" } }
# 下拉框：find 到的是组件根，展开必须点 /anchor 子控件
function Open-Dropdown([string]$q) { $t = Find-First $q; if ($t) { Click ($t + "/anchor"); W 300 } else { $script:fails += "find dd $q :: no hit" } }
function W([int]$ms) { Start-Sleep -Milliseconds $ms }
function Shot([string]$tag) {
    $o = & $exe capture --project $proj 2>&1 | Out-String
    try { $p = ($o | ConvertFrom-Json).path } catch { $p = $o }
    Write-Output "[shot] $tag => $p"
}

Write-Output "===== open bench ====="
$benchRoot = Find-First "cgui_bench_root"
if (-not $benchRoot) {
    $hub = Find-First "debug_hub_panel"
    if (-not $hub) { Click "cgui_debug_hub/__root/hub_entry_pin/debug_hub_entry"; W 500 }
    Click "cgui_debug_hub/__root/panel@1/debug_hub_panel/cgui_bench"; W 800
}

Write-Output "===== P2 widgets: button/dropdown/window/popup ====="
Click "cgui_bench/cgui_bench_root/menu/menu_2"; W 400
Click "cgui_bench/cgui_bench_root/detail/panel@1/wc_type/anchor"; W 300
Click "cgui_bench/overlay/dropdown_list/1"; W 400
Click-Found "wc_button"
Click "cgui_bench/cgui_bench_root/detail/panel@1/wc_type/anchor"; W 300
Click "cgui_bench/overlay/dropdown_list/8"; W 400
Open-Dropdown "wc_dropdown"
Click "cgui_bench/overlay/dropdown_list/2"; W 300
Click "cgui_bench/cgui_bench_root/detail/panel@1/wc_type/anchor"; W 300
Click "cgui_bench/overlay/dropdown_list/13"; W 400
Click-Found "wc_win_btn"; W 500
Shot "P2-window-open"
Click-Found "wc_win_btn"; W 400
Click "cgui_bench/cgui_bench_root/detail/panel@1/wc_type/anchor"; W 300
Click "cgui_bench/overlay/dropdown_list/15"; W 400
Click-Found "wc_popup_btn"; W 500
Shot "P2-popup-open"
Find-First "wc_popup" | Out-Null
Click-FC "关闭"; W 400

Write-Output "===== P3 kit ====="
Click "cgui_bench/cgui_bench_root/menu/menu_3"; W 500
$t = Find-First "kit_vol"; if ($t) { Invoke-Cap 'lua.set_value' @{ id = $t; value = 80 } | Out-Null }
Open-Dropdown "kit_quality"
Click "cgui_bench/overlay/dropdown_list/3"; W 300
$t = Find-First "kit_joy"; if ($t) { Invoke-Cap 'lua.press_ui' @{ id = $t } | Out-Null; W 800; Invoke-Cap 'lua.release_ui' @{ id = $t } | Out-Null }
Click-Found "kit_dlg_now"; W 600
Shot "P3-confirm-open"
Click-FC "确定"; W 400

Write-Output "===== P4 style ====="
Click "cgui_bench/cgui_bench_root/menu/menu_4"; W 500
Click-Found "comp_canvas"; W 400
$t = Find-First "n_canvas_layout.width"; if ($t) { Invoke-Cap 'lua.set_value' @{ id = $t; value = 300 } | Out-Null }
$t = Find-First "newf_path"; if ($t) { Invoke-Cap 'lua.input_text' @{ id = $t; text = "style_demo" } | Out-Null }
Click-FC "newf_kind"; W 300
Click "cgui_bench/overlay/dropdown_list/2"; W 300
Shot "P4-style"

Write-Output "===== P5 layout ====="
Click "cgui_bench/cgui_bench_root/menu/menu_5"; W 500
$t = Find-First "lay_pad"; if ($t) { Invoke-Cap 'lua.set_value' @{ id = $t; value = 24 } | Out-Null }
Click-Found "lay_row"; W 300
Click "cgui_bench/overlay/dropdown_list/3"; W 300
Click-Found "lay_col"; W 300
Click "cgui_bench/overlay/dropdown_list/3"; W 300
Shot "P5-layout"

Write-Output "===== P6 anim ====="
Click "cgui_bench/cgui_bench_root/menu/menu_6"; W 500
Click-Found "an_play_size"; W 1200
Shot "P6-anim-playing"
Click-Found "an_play_size"; W 400
Open-Dropdown "an_ease"
Click "cgui_bench/overlay/dropdown_list/3"; W 300

Write-Output "===== P7 diag ====="
Click "cgui_bench/cgui_bench_root/menu/menu_7"; W 500
Click-Found "diag_marker_toggle"
Click-Found "diag_mouse_probe"
W 400; Shot "P7-diag-on"
Click-Found "diag_marker_toggle"
Click-Found "diag_mouse_probe"

Write-Output "===== close bench ====="
Click "cgui_bench/cgui_bench_root/menu/menu_close"; W 600
Shot "closed"

Write-Output "===== logs errors ====="
& $exe logs game_client 3 --project $proj 2>&1 | Out-String | Write-Output

Write-Output "===== summary ====="
Write-Output ("fails: " + $script:fails.Count)
$script:fails | ForEach-Object { Write-Output ("  FAIL " + $_) }
