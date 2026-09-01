# 06_notify_framework.ps1 — 0.8.5 全量验收：notify 页（游戏）+ 框架页 toast/dialog/guide 触发验证
# 覆盖矩阵（签名照抄 libs/client/cgui/notify.lua：cg.toast/dialog_async/confirm_async/guide_mask/guide_hide/guide_active）：
#   A toast（框架 NOTIFY 页 page/toast.lua）：投递命中 → 同文 2 秒合并窗口连投 3 次合并「验收合并 ×3」→
#     warn 级 → 等自然消失（duration 默认 2.5s，present=false）→ 连投 7 条不同文本 eval 断言队列长度==5
#     （TOAST_MAX=5 丢最旧，队首应为「验收队列3」；队列读 require("libs.client.cgui.notify").toast_queue()）
#   B dialog（框架 SYSTEM 页 page/dialog.lua）：confirm_async 弹「验收确认框」→ 标题/确定/取消命中 →
#     点确定 on_result=true / 点取消=false / 点遮罩(click_at 10,300)=false（mask_value）→
#     close_on_mask=false 时点遮罩不关 → 队列模式连弹 2 个只显第一个、关后第二个出现 →
#     栈模式 stack=true 两个同屏 → dialog_async 句柄 close 传值走 on_result
#   C guide（框架 GUIDE 页 page/guide.lua）：guide_mask(rect+text+on_close) → 文本命中 + guide_active()=true +
#     capture（暗罩+高亮洞）→ click_at 遮罩 → active=false + on_close 回调标志=true → 无 rect 整屏罩 →
#     guide_hide 立即消失
#   D notify 页（游戏 page/notify/notify.lua）：ShowWarning 中央警告 3 秒自然消失；
#     eval 直写 WorldState ghostActive/ghostTimer + speedBoostActive/speedBoostTimer → BUFF 条
#     「穿墙 」「加速 」文本出现（带空格避技能钮同名文本），随后清字段；
#     eval 写 isDead/respawnTimer → 死亡提示「已死亡，」前缀，随后恢复；FlashMPBar → MPFlashColor 非 nil + crop 蓝条
#   E BENCH 共存语义：tap「调试」→「打开 CGUI 调试台」→ toast 仍可见（NOTIFY 档不被挂起）；
#     guide/dialog 被挂起不可见但 guide_active()/投递回调正常（0.8.5 已认可语义，note 标注非 bug）→
#     关调试台后挂起期投递的弹窗恢复可见且回调正常 → 关调试台面板
#   F 收尾日志 errors 段
# 用法：powershell -File 06_notify_framework.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '06：notify 页 + 框架页（toast/dialog/guide）触发验证（0.8.5 全量验收）' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },
        @{ op = 'wait_for'; q = '攻击'; timeout_ms = 15000 },

        @{ op = 'note'; text = 'A1 toast 投递：cg.toast 命中' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'bgd_api.client.cgui.toast("验收toastA")
return "posted"' } },
        @{ op = 'wait_for'; q = '验收toastA'; timeout_ms = 4000 },

        @{ op = 'note'; text = 'A2 同文 2 秒合并窗口连投 3 次 → 合并文本「验收合并 ×3」（队列计数断言）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
cg.toast("验收合并")
cg.toast("验收合并")
cg.toast("验收合并")
local q = require("libs.client.cgui.notify").toast_queue()
local last = q[#q]
if not last or last.text ~= "验收合并" or last.count ~= 3 then
    error("toast 合并失败：count=" .. tostring(last and last.count))
end
return "merged x3"' } },
        @{ op = 'wait_for'; q = '验收合并 ×3'; timeout_ms = 4000 },

        @{ op = 'note'; text = 'A3 warn 级 toast（警示配色）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'bgd_api.client.cgui.toast("验收warn提示", { kind = "warn" })
return "warn posted"' } },
        @{ op = 'wait_for'; q = '验收warn提示'; timeout_ms = 4000 },

        @{ op = 'note'; text = 'A4 自然消失（duration 默认 2.5s）' },
        @{ op = 'wait_for'; q = '验收toastA'; present = $false; timeout_ms = 8000 },
        @{ op = 'wait_for'; q = '验收合并'; present = $false; timeout_ms = 8000 },
        @{ op = 'wait_for'; q = '验收warn提示'; present = $false; timeout_ms = 8000 },

        @{ op = 'note'; text = 'A5 连投 7 条不同文本 → 队列上限 TOAST_MAX=5 丢最旧（队首=验收队列3）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
for i = 1, 7 do cg.toast("验收队列" .. i) end
local q = require("libs.client.cgui.notify").toast_queue()
if #q ~= 5 then error("toast 队列上限失效，长度=" .. #q) end
if q[1].text ~= "验收队列3" then error("应丢最旧两条，队首=" .. tostring(q[1].text)) end
return "queue=5 head=验收队列3"' } },
        @{ op = 'wait_for'; q = '验收队列7'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '验收队列1'; present = $false },
        @{ op = 'wait_for'; q = '验收队列'; present = $false; timeout_ms = 8000 },

        @{ op = 'note'; text = 'B1 confirm_async：标题/确定/取消命中 → 点确定 on_result=true' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
_G.__t_dlg1 = nil
cg.confirm_async({ title = "验收确认框", text = "验收：第一轮点确定" }, function(ok) _G.__t_dlg1 = ok end)
return "pushed"' } },
        @{ op = 'wait_for'; q = '验收确认框'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '确定'; present = $true },
        @{ op = 'assert_text'; q = '取消'; present = $true },
        @{ op = 'capture_ui'; q = '验收确认框' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确定' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if _G.__t_dlg1 ~= true then error("确认框点确定回调应为 true，实际=" .. tostring(_G.__t_dlg1)) end
return "ok=true"' } },

        @{ op = 'note'; text = 'B2 再弹点取消 → on_result=false' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
_G.__t_dlg2 = nil
cg.confirm_async({ title = "验收确认框", text = "验收：第二轮点取消" }, function(ok) _G.__t_dlg2 = ok end)
return "pushed"' } },
        @{ op = 'wait_for'; q = '验收确认框'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '取消' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if _G.__t_dlg2 ~= false then error("确认框点取消回调应为 false，实际=" .. tostring(_G.__t_dlg2)) end
return "ok=false"' } },

        @{ op = 'note'; text = 'B3 再弹点遮罩（click_at 左上角空白）→ mask_value=false → on_result=false' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
_G.__t_dlg3 = nil
cg.confirm_async({ title = "验收确认框", text = "验收：第三轮点遮罩" }, function(ok) _G.__t_dlg3 = ok end)
return "pushed"' } },
        @{ op = 'wait_for'; q = '验收确认框'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 10; y = 300 } },
        @{ op = 'wait_for'; q = '验收确认框'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if _G.__t_dlg3 ~= false then error("点遮罩回调应为 false，实际=" .. tostring(_G.__t_dlg3)) end
return "mask=false"' } },

        @{ op = 'note'; text = 'B4 close_on_mask=false：点遮罩不关，点按钮才关' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
_G.__t_dlg4 = nil
cg.dialog_async({
    title = "验收禁遮罩",
    text = "验收：close_on_mask=false，点遮罩不应关闭",
    close_on_mask = false,
    buttons = { { text = "知道了", kind = "primary", value = "ok" } },
    on_result = function(v) _G.__t_dlg4 = v end,
})
return "pushed"' } },
        @{ op = 'wait_for'; q = '验收禁遮罩'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 10; y = 300 } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'assert_text'; q = '验收禁遮罩'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '知道了' } },
        @{ op = 'wait_for'; q = '验收禁遮罩'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if _G.__t_dlg4 ~= "ok" then error("禁遮罩弹窗按钮回调异常: " .. tostring(_G.__t_dlg4)) end
return "ok"' } },

        @{ op = 'note'; text = 'B5 队列模式（默认）：连弹 2 个只显第一个，关后第二个自动补位' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
cg.dialog_async({ title = "验收排队1", text = "队列模式第 1 个", buttons = { { text = "关排队1", value = 1 } } })
cg.dialog_async({ title = "验收排队2", text = "队列模式第 2 个", buttons = { { text = "关排队2", value = 2 } } })
return "pushed x2"' } },
        @{ op = 'wait_for'; q = '验收排队1'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '验收排队2'; present = $false },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '关排队1' } },
        @{ op = 'wait_for'; q = '验收排队2'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '验收排队1'; present = $false },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '关排队2' } },
        @{ op = 'wait_for'; q = '验收排队2'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'B6 栈模式（stack=true）：两个同屏叠放，逐层关闭' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
cg.dialog_async({ title = "验收叠放1", text = "栈模式第 1 层", stack = true, buttons = { { text = "关叠放1", value = 1 } } })
cg.dialog_async({ title = "验收叠放2", text = "栈模式第 2 层", stack = true, buttons = { { text = "关叠放2", value = 2 } } })
return "pushed stack x2"' } },
        @{ op = 'wait_for'; q = '验收叠放1'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '验收叠放2'; present = $true },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '关叠放1' } },
        @{ op = 'wait_for'; q = '验收叠放1'; present = $false; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '验收叠放2'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '关叠放2' } },
        @{ op = 'wait_for'; q = '验收叠放2'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'B7 dialog_async 句柄：close 传值走 on_result' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
_G.__t_dlg5 = nil
_G.__t_dh = cg.dialog_async({
    title = "验收句柄",
    text = "验收：编程 close 传值",
    buttons = { { text = "无用钮", value = 0 } },
    on_result = function(v) _G.__t_dlg5 = v end,
})
return "handle saved"' } },
        @{ op = 'wait_for'; q = '验收句柄'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = '_G.__t_dh.close("验收传值")
return "closed with value"' } },
        @{ op = 'wait_for'; q = '验收句柄'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if _G.__t_dlg5 ~= "验收传值" then error("句柄 close 传值未走 on_result: " .. tostring(_G.__t_dlg5)) end
return "value ok"' } },

        @{ op = 'note'; text = 'C1 guide：guide_mask(rect+text+on_close) → active → capture 暗罩+高亮洞 → 点遮罩关闭+回调' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
_G.__t_guide_closed = false
cg.guide_mask({ rect = { 100, 100, 200, 120 }, text = "验收引导文本", on_close = function() _G.__t_guide_closed = true end })
if not cg.guide_active() then error("guide_active 应为 true") end
return "guide on"' } },
        @{ op = 'wait_for'; q = '验收引导文本'; timeout_ms = 4000 },
        @{ op = 'capture'; max_width = 1280; crop = @{ x = 60; y = 60; w = 700; h = 320 } },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 10; y = 600 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
if cg.guide_active() then error("点遮罩后引导未关闭") end
if not _G.__t_guide_closed then error("guide on_close 回调未触发") end
return "guide closed + callback"' } },

        @{ op = 'note'; text = 'C2 无 rect = 无洞整屏罩；guide_hide 立即消失' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
cg.guide_mask({ text = "验收整屏引导" })
if not cg.guide_active() then error("guide_active 应为 true") end
return "full mask on"' } },
        @{ op = 'wait_for'; q = '验收整屏引导'; timeout_ms = 4000 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'bgd_api.client.cgui.guide_hide()
if bgd_api.client.cgui.guide_active() then error("guide_hide 后仍 active") end
return "hidden"' } },
        @{ op = 'wait_for'; q = '验收整屏引导'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'D1 notify 页中央警告：ShowWarning → 3 秒自然消失' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'require("src.client.page.notify.notify").ShowWarning("没有药水可以使用")
return "warn shown"' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '没有药水可以使用'; present = $true },
        @{ op = 'wait_for'; q = '没有药水可以使用'; present = $false; timeout_ms = 6000 },

        @{ op = 'note'; text = 'D2 穿墙/加速 BUFF 条：eval 直写 WorldState ghost/speedBoost 字段（断言后清字段）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
W.ghostActive = true
W.ghostTimer = 2.5
W.ghostDuration = 3
W.speedBoostActive = true
W.speedBoostTimer = 4.5
W.speedBoostDuration = 5
return "buff on"' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '穿墙 '; present = $true },
        @{ op = 'assert_text'; q = '加速 '; present = $true },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
W.ghostActive = false
W.ghostTimer = 0
W.speedBoostActive = false
W.speedBoostTimer = 0
return "buff off"' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '穿墙 '; present = $false },
        @{ op = 'assert_text'; q = '加速 '; present = $false },

        @{ op = 'note'; text = 'D3 死亡提示：eval 写 isDead/respawnTimer → 「已死亡，」前缀 → 立即恢复' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
W.isDead = true
W.respawnTimer = 3
return "dead on"' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '已死亡，'; present = $true },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
W.isDead = false
W.respawnTimer = 0
return "dead off"' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '已死亡，'; present = $false },

        @{ op = 'note'; text = 'D4 MP 蓝条闪烁：FlashMPBar → MPFlashColor 非 nil + crop 蓝条' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local N = require("src.client.page.notify.notify")
N.FlashMPBar()
if not N.MPFlashColor() then error("MPFlashColor 应非 nil（闪烁期）") end
return "mp flashing"' } },
        @{ op = 'capture'; max_width = 1280; crop = @{ x = 24; y = 24; w = 320; h = 80 } },

        @{ op = 'note'; text = 'E BENCH 共存：开调试台 → toast 仍可见（NOTIFY 不挂起）；guide/dialog 挂起不可见但状态/回调正常（已认可语义，非 bug）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '调试' } },
        @{ op = 'wait_for'; q = '打开 CGUI 调试台'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '打开 CGUI 调试台' } },
        @{ op = 'wait'; ms = 1000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if not bgd_api.client.cgui_bench.open_flag then error("CGUI 调试台未打开") end
return "bench open"' } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'bgd_api.client.cgui.toast("验收BENCH共存toast")
return "toast posted under bench"' } },
        @{ op = 'wait_for'; q = '验收BENCH共存toast'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
cg.guide_mask({ text = "验收BENCH下引导" })
if not cg.guide_active() then error("guide_active 应为 true（挂起≠状态丢失）") end
return "guide active under bench"' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '验收BENCH下引导'; present = $false },
        @{ op = 'note'; text = 'guide 挂起不可见但 guide_active()=true——GUIDE 档低于 BENCH 的当然结果（0.8.5 评审已认可，非 bug）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local cg = bgd_api.client.cgui
_G.__t_bench_dlg = nil
cg.confirm_async({ title = "验收BENCH下弹窗", text = "挂起期间投递，关台后应弹出" }, function(ok) _G.__t_bench_dlg = ok end)
return "dialog queued under bench"' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '验收BENCH下弹窗'; present = $false },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'bgd_api.client.cgui.guide_hide()
if bgd_api.client.cgui.guide_active() then error("guide_hide 未生效") end
return "guide hidden"' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '关闭 CGUI 调试台' } },
        @{ op = 'wait'; ms = 1000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if bgd_api.client.cgui_bench.open_flag then error("CGUI 调试台未关闭") end
return "bench closed"' } },
        @{ op = 'wait_for'; q = '验收BENCH下弹窗'; timeout_ms = 5000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确定' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if _G.__t_bench_dlg ~= true then error("挂起期投递的弹窗回调异常: " .. tostring(_G.__t_bench_dlg)) end
return "queued dialog ok"' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '调试' } },
        @{ op = 'wait_for'; q = '调试台'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'F 收尾：日志 errors 段必须为空（历史 sprobe 报错为开发期探针遗留，看 distinct 增量）' },
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
