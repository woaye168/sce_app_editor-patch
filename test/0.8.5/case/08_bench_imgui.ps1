# 0.8.5 全量验收 · 用例 08：IMGUI 排障台（imgui_bench）全 15 标签页验收
# 覆盖矩阵：
#   开台   debug_hub「打开 IMGUI 排障台」→ eval 断言 cgui_bench 未开（同档 BENCH 语义）→ 整屏截图
#   15 页  核心API/内置组件/扩展组件/过渡动画/按钮/复选框/输入框/下拉菜单/图标文本/布局/
#          滚动列表/canvas/拖拽/RMGUI模板/网页——逐页 click_at 菜单 + eval M.page 索引断言 + capture
#   交互抽测  核心API core_state 计数 / 按钮「默认」计数 / 复选框翻转 / 滚动列表选「列表项 5」/
#            RMGUI模板 slot 按钮计数 / 拖拽页（受限标注）/ 网页加载等待截图
#   关台   eval 关闭 + is_open==false 断言
# 用法：powershell -File 08_bench_imgui.ps1（编辑器在线即可，脚本自带 start_debug 重置）
# 关键受限（逐页 note 亦标注）：
#   * imgui_bench 内容经 cg.raw 渲染原始 imgui，【不进 dbg 快照】——find_ui/assert_text/tap
#     对台内控件一律找不到，属预期边界（本台探测的正是引擎原生行为）；
#     断言三板斧：eval bgd_api.client.imgui_bench.page == N + capture 截图 + click_at 坐标点击
#   * 菜单坐标按代码布局推算：菜单宽 170 padding 8，标题高 36，项高 36+margin 2
#     → 第 i 项中心 (85, 64+(i-1)*38)，逻辑坐标（与 find_ui rect/capture crop 同系）；
#     逻辑视口高 < 660 时末项「网页」(y=596) 可能出屏——此时该页改 eval M.page=N 切页（见页内 note）
#   * 菜单「关闭调试台」在视口底部、y 随分辨率变化（坐标注入受限）→ 关台走 eval close()
#   * capture 截图文件路径在各 capture 步 result.path（留证清单见末尾 note）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '用例 08：IMGUI 排障台 15 页验收（raw 渲染不进 dbg 快照=预期，eval+capture+click_at 三板斧）' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        # ---------- 开台 ----------
        @{ op = 'note'; text = '开台：HUD「调试」→ 面板「打开 IMGUI 排障台」' },
        @{ op = 'wait_for'; q = '调试'; timeout_ms = 8000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '调试' } },
        @{ op = 'assert_text'; q = '调试台'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '打开 IMGUI 排障台' } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'note'; text = '同档 BENCH 语义：cgui_bench 未被打开（is_open==false）；imgui_bench 已开' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`nif cg.page.is_open('cgui_bench') then error('cgui_bench 意外打开（同档 BENCH 语义破坏）') end`nif not cg.page.is_open('imgui_bench') then error('imgui_bench 未打开') end`nreturn 'cgui_bench==false imgui_bench==true'" } },
        @{ op = 'capture'; max_width = 1280 },

        # ---------- 15 页逐页 ----------
        @{ op = 'note'; text = 'P1 核心API（菜单坐标 85,64）：点 core_state 计数块（400,62）→ 左键计数 +1（截图判读）' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 64 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 1 then error('期望页 1 实际 ' .. tostring(b.page)) end`nreturn 'page=1'" } },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 400; y = 62 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P2 内置组件（85,102）：页面索引断言 + 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 102 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 2 then error('期望页 2 实际 ' .. tostring(b.page)) end`nreturn 'page=2'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P3 扩展组件（85,140）：页面索引断言 + 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 140 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 3 then error('期望页 3 实际 ' .. tostring(b.page)) end`nreturn 'page=3'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P4 过渡动画（85,178）：页面索引断言 + 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 178 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 4 then error('期望页 4 实际 ' .. tostring(b.page)) end`nreturn 'page=4'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P5 按钮（85,216）：点「默认」钮（226,62）→ 点击计数行 默认+1（截图判读）' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 216 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 5 then error('期望页 5 实际 ' .. tostring(b.page)) end`nreturn 'page=5'" } },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 226; y = 62 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P6 复选框（85,254）：点首行 checkbox（196,55）→ color_primary 勾选翻转 true→false（截图判读）' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 254 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 6 then error('期望页 6 实际 ' .. tostring(b.page)) end`nreturn 'page=6'" } },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 196; y = 55 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P7 输入框（85,292）：页面索引断言 + 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 292 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 7 then error('期望页 7 实际 ' .. tostring(b.page)) end`nreturn 'page=7'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P8 下拉菜单（85,330）：页面索引断言 + 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 330 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 8 then error('期望页 8 实际 ' .. tostring(b.page)) end`nreturn 'page=8'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P9 图标文本（85,368）：页面索引断言 + 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 368 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 9 then error('期望页 9 实际 ' .. tostring(b.page)) end`nreturn 'page=9'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P10 布局（85,406）：页面索引断言 + 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 406 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 10 then error('期望页 10 实际 ' .. tostring(b.page)) end`nreturn 'page=10'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P11 滚动列表（85,444）：点「列表项 5」（300,172）→ 选中: 5（截图判读）' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 444 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 11 then error('期望页 11 实际 ' .. tostring(b.page)) end`nreturn 'page=11'" } },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 300; y = 172 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P12 canvas（85,482）：页面索引断言 + 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 482 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 12 then error('期望页 12 实际 ' .. tostring(b.page)) end`nreturn 'page=12'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P13 拖拽（85,520）：受限——raw 控件不进快照，drag_ui 无 id 可用（拖「拖我 1」到「拖我 333」无法注入）；页面索引断言 + 截图覆盖' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 520 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 13 then error('期望页 13 实际 ' .. tostring(b.page)) end`nreturn 'page=13'" } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P14 RMGUI模板（85,558）：点 slot 内 IMGUI 按钮（316,158）→ 卡片 desc 点击 1 次（截图判读）' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 558 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 14 then error('期望页 14 实际 ' .. tostring(b.page)) end`nreturn 'page=14'" } },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 316; y = 158 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'P15 网页（85,596；逻辑视口高 < 660 出屏时改 eval b.page=15）：等 2s 加载 → 截图' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 85; y = 596 } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nif b.page ~= 15 then error('期望页 15 实际 ' .. tostring(b.page) .. '（菜单项出屏则改用 eval 切页重跑）') end`nreturn 'page=15'" } },
        @{ op = 'wait'; ms = 2000 },
        @{ op = 'capture'; max_width = 1280 },

        # ---------- 关台 ----------
        @{ op = 'note'; text = '关台：菜单「关闭调试台」在视口底部、y 随分辨率变（坐标注入受限）→ eval close + is_open==false；顺带关 debug_hub 面板' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local b = bgd_api.client.imgui_bench`nb.close()`nlocal cg = bgd_api.client.cgui`nif cg.page.is_open('imgui_bench') then error('imgui_bench 未关闭') end`ncg.page.close('debug_hub')`nreturn 'closed'" } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = '留证清单：本用例全部 capture 步 result.path（开台整屏 + 15 页 + 关台）；日志 errors 段必须为空' },
        @{ op = 'logs'; source = 'game_client'; tail_lines = 3 }
    )
}

$ndjson = @(
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}',
    (@{ jsonrpc = '2.0'; id = 2; method = 'tools/call'; params = @{ name = 'run_scenario'; arguments = $scenario } } | ConvertTo-Json -Depth 20 -Compress)
) -join "`n"

$out = $ndjson | & $exe mcp 2>&1 | Out-String
$resp = ($out -split "`n" | Where-Object { $_ -match '"id":2' }) -join "`n"
try {
    $j = $resp | ConvertFrom-Json
    $sj = ([string]$j.result.content[0].text) | ConvertFrom-Json
    foreach ($r in $sj.results) {
        $tag = if ($r.ok) { 'OK ' } else { 'ERR' }
        $line = "{0} step {1,2} [{2}]" -f $tag, $r.step, $r.op
        if (-not $r.ok) { $line += ' :: ' + ([string]$r.error) }
        [Console]::WriteLine($line)
    }
    [Console]::WriteLine(("failed_step: {0}    elapsed: {1}ms" -f $sj.failed_step, $sj.elapsed_ms))
    $last = $sj.results[$sj.results.Count - 1]
    if ($last.op -eq 'logs') {
        [Console]::WriteLine(("logs errors distinct: {0}" -f $last.result.logs.game_client.errors.distinct))
    }
} catch {
    [Console]::WriteLine("PARSE FAIL (likely 32KB truncation): $($_.Exception.Message)")
    $blob = $out + "`n" + $_.Exception.Message
    $okCount = ([regex]::Matches($blob, '"ok":\s*true')).Count
    $errMatches = [regex]::Matches($blob, '"ok":\s*false,\s*"error":\s*"([^"]*)"')
    [Console]::WriteLine(("fallback: ok={0} err={1}" -f $okCount, $errMatches.Count))
    foreach ($em in $errMatches) { [Console]::WriteLine('  ERR :: ' + $em.Groups[1].Value) }
    $mf = [regex]::Match($out, '"failed_step":\s*(\S+?)[,\s]')
    if ($mf.Success) { [Console]::WriteLine('failed_step: ' + $mf.Groups[1].Value) }
}
