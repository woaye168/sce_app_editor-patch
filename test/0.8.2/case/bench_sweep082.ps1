# CGUI 调试台全功能扫描 0.8.2 版：run_scenario 新语法（tap/pick/save_as/wait_for/assert_text）
# 对比 0.8.0 bench_sweep.ps1（150 行 ps1 逐步往返）→ 本场景一次调用跑完。
# 用法：powershell -File bench_sweep082.ps1  （前提：编辑器在线；脚本自带 start_debug 重置状态）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8  # 管道进 exe stdin 的编码（中文必须 UTF-8）
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '0.8.2 bench_sweep：重启调试拿干净 UI 态' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        @{ op = 'note'; text = '开调试台（hub 入口 → CGUI 调试台）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'debug_hub_entry' } },
        @{ op = 'wait_for'; q = '打开 CGUI 调试台'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '打开 CGUI 调试台' } },
        @{ op = 'wait_for'; q = '关闭调试台'; timeout_ms = 3000 },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P1 内置组件：输入框注入 + 开关/复选' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = 'bi_color'; text = '#FF8800FF' } },
        @{ op = 'wait'; ms = 200 },
        @{ op = 'assert_text'; q = '#FF8800FF'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'bi_clip' } },
        @{ op = 'wait'; ms = 200 },

        @{ op = 'note'; text = 'P2 扩展组件：下拉选 button → 弹窗开/关（点遮罩）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '扩展组件' } },
        @{ op = 'wait_for'; q = 'wc_type'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.pick'; args = @{ q = 'wc_type'; item = 'popup' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'wc_popup_btn' } },
        @{ op = 'wait_for'; q = '点击遮罩或按钮关闭'; timeout_ms = 3000 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 150; y = 1300 } },
        @{ op = 'wait_for'; q = '点击遮罩或按钮关闭'; present = $false; timeout_ms = 3000 },

        @{ op = 'note'; text = 'P3 游戏件：slider 设值 / 摇杆按住松开 / 长按 / hover tooltip / 滚动 / 拖放排序' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '游戏件' } },
        @{ op = 'wait_for'; q = 'kit_joy'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.set_value'; args = @{ id = 'kit_segs_sl'; value = 3 } },
        @{ op = 'assert_text'; q = '3 / 10'; present = $true },
        @{ op = 'invoke'; id = 'lua.press_ui'; args = @{ id = 'kit_joy'; x = 1; y = 0 } },
        @{ op = 'wait_for'; q = '按住中'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.release_ui'; args = @{ id = 'kit_joy' } },
        @{ op = 'wait_for'; q = '已松开'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.long_press_ui'; args = @{ id = 'kit_gesture_btn'; hold_ms = 900 } },
        @{ op = 'wait'; ms = 1400 },
        @{ op = 'invoke'; id = 'lua.hover_ui'; args = @{ id = 'kit_tip_cell1' } },
        @{ op = 'wait_for'; q = '精铁剑'; timeout_ms = 3000 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.scroll_ui'; args = @{ id = 'kit_pscroll'; delta_y = 120 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '当前偏移：120'; present = $true },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = 'row5/drag'; to_id = 'row1/drag' } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'assert_text'; q = 'on_move(row5, row1)'; present = $true },

        @{ op = 'note'; text = 'P4-P7 页面巡回（每页 find 命中 + 一次操作）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '样式编辑' } },
        @{ op = 'wait_for'; q = '新增字段'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = 'newf_path'; text = 'style_demo' } },
        @{ op = 'wait'; ms = 200 },
        @{ op = 'assert_text'; q = 'style_demo'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '布局演练' } },
        @{ op = 'wait_for'; q = 'lay_pad'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.set_value'; args = @{ id = 'lay_pad'; value = 24 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '动画演练' } },
        @{ op = 'wait_for'; q = 'an_play_size'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'an_play_size' } },
        @{ op = 'wait'; ms = 1000 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'an_play_size' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '诊断' } },
        @{ op = 'wait_for'; q = '校准标记'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'diag_marker_toggle' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'diag_marker_toggle' } },

        @{ op = 'note'; text = '关闭调试台 + 日志 errors 段必须为空' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '关闭调试台' } },
        @{ op = 'wait_for'; q = '关闭调试台'; present = $false; timeout_ms = 3000 },
        @{ op = 'logs'; source = 'game_client'; tail_lines = 3 }
    )
}

$ndjson = @(
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}',
    (@{ jsonrpc = '2.0'; id = 2; method = 'tools/call'; params = @{ name = 'run_scenario'; arguments = $scenario } } | ConvertTo-Json -Depth 20 -Compress)
) -join "`n"

$out = $ndjson | & $exe mcp 2>&1 | Out-String
# 只留 tools/call 响应行
$resp = ($out -split "`n" | Where-Object { $_ -match '"id":2' }) -join "`n"
try {
    $j = $resp | ConvertFrom-Json
    $text = [string]$j.result.content[0].text
    $sj = $text | ConvertFrom-Json
    foreach ($r in $sj.results) {
        $tag = if ($r.ok) { 'OK ' } else { 'ERR' }
        $line = "{0} step {1,2} [{2}]" -f $tag, $r.step, $r.op
        if (-not $r.ok) { $line += ' :: ' + ([string]$r.error) }
        [Console]::WriteLine($line)
    }
    [Console]::WriteLine(("failed_step: {0}    elapsed: {1}ms" -f $sj.failed_step, $sj.elapsed_ms))
    # 末尾 logs 步的 errors 段检查
    $last = $sj.results[$sj.results.Count - 1]
    if ($last.op -eq 'logs') {
        $errs = $last.result.logs.game_client.errors
        [Console]::WriteLine(("logs errors distinct: {0}" -f $errs.distinct))
    }
} catch {
    [Console]::WriteLine("PARSE FAIL: $($_.Exception.Message)")
    [Console]::WriteLine($out)
}
