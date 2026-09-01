# 0.8.5 全量验收 · 用例 10：静态审查问题的动态复核（探针式）
# 覆盖矩阵（能复现的构造场景复现并打标记；标记写 _G._iss + 游戏日志 [REVIEW] 前缀，
# 不 error 让脚本跑完汇总；探针页名前缀 probe_，结束统一 close 清理）：
#   ISSUE-1  队列×挂起卡死：DIALOG A 显示 + B 排队 + exclusive POPUP X(suspend={DIALOG}) 挂起 A
#            → close A（pump_queue 出队 B 时 X 仍持有 DIALOG，B 走「已在挂起态仅刷参」分支，
#            open=true 但 suspended/queued 标志均不成立）→ close X → B 永不补位
#   ISSUE-2  排队页 close 钩子失衡：B 排队中直接 close → 从未 page_opened 却发 page_closed
#   ISSUE-3  close_all 补位脉冲：close_all(DIALOG) 迭代中 A 先关 → pump 补位 B(on_open)
#            → 同一 close_all 再关 B(on_close)（注：pairs 迭代序不定，先关 B 则退化为 ISSUE-2 形态）
#   ISSUE-6  slider on_commit 失效：交互块整体被 cb.on_change 守卫（control.lua:514），
#            只传 on_commit 不传 on_change 时 set_value 后 on_commit 不触发
#   ISSUE-7  countdown 无 key 槽位共享：remember 槽位 'cd_done@auto'（feedback.lua:153），
#            同容器两个无 key countdown 仅第一个 on_done 触发
#   ISSUE-9  pscroll 高度覆盖：vp_ov.layout.height 被 view_h（缺省 320）强制回写
#            （pscroll.lua:89），layout.height=500 静默失效
#   ISSUE-11/12/13/14/15/16/17/18/19/20  静态事实（注释/死代码/行数/约定）——
#            静态已证实，见审查报告，本脚本不做动态步骤
# 用法：powershell -File 10_review_issues.ps1（编辑器在线即可，脚本自带 start_debug 重置）
# 注意：探针注册走 cg.page({...})，type 必填 cg.PAGE.* 枚举（def 白名单见 page.lua）；
#   事件订阅 bgd_api.common.event_bus.on('cgui.page_opened'/'cgui.page_closed')；
#   同一调试会话内重复跑本脚本事件监听器会累积（start_debug 重置 VM 后干净）。
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '用例 10：静态审查问题动态复核（探针式）；ISSUE-11~20 为静态事实已证实，见审查报告' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        # ---------- ISSUE-1 队列×挂起卡死 ----------
        @{ op = 'note'; text = 'ISSUE-1：DIALOG A 显示+B 排队，exclusive POPUP X(suspend=DIALOG) 挂起 A；close A 触发 pump 出队 B（X 仍持有 DIALOG）；close X；期望 B 显示，若 open=true 但不 visible 即永久卡死' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`n_G._iss = _G._iss or {}`nlocal D = cg.PAGE.DIALOG`ncg.page({ name = 'probeA', type = D, render = function() cg.text({ text = '探针A' }) end })`ncg.page({ name = 'probeB', type = D, render = function() cg.text({ text = '探针B' }) end })`ncg.page({ name = 'probeX', type = cg.PAGE.POPUP, exclusive = true, suspend = { D }, render = function() cg.text({ text = '探针X' }) end })`ncg.page.open('probeA')`ncg.page.open('probeB')`ncg.page.open('probeX')`ncg.page.close('probeA')`ncg.page.close('probeX')`nlocal o, v, qd = cg.page.is_open('probeB'), cg.page.is_visible('probeB'), cg.page.is_queued('probeB')`nlocal msg`nif o and not v then`n  msg = 'ISSUE-1 REPRODUCED: probeB 永久卡死(open=true visible=false queued=' .. tostring(qd) .. '；已出 FIFO，restore_suspended 只认 suspended 标志，永不补位)'`nelse`n  msg = 'ISSUE-1 未复现: probeB open=' .. tostring(o) .. ' visible=' .. tostring(v)`nend`ncg.page.close('probeB')`n_G._iss['ISSUE-1'] = msg`nlog.info('[REVIEW] ' .. msg)`nreturn msg" } },

        # ---------- ISSUE-2 排队页 close 钩子失衡 ----------
        @{ op = 'note'; text = 'ISSUE-2：DIALOG C 显示+D 排队；订阅 cgui.page_opened/page_closed 计数；close D（排队中）→ 期望 opened==0 且 closed==1' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`nlocal eb = bgd_api.common.event_bus`n_G._iss = _G._iss or {}`n_G._ev2 = { opened = 0, closed = 0 }`neb.on('cgui.page_opened', function(n) if n == 'probeD' then _G._ev2.opened = _G._ev2.opened + 1 end end)`neb.on('cgui.page_closed', function(n) if n == 'probeD' then _G._ev2.closed = _G._ev2.closed + 1 end end)`ncg.page({ name = 'probeC', type = cg.PAGE.DIALOG, render = function() cg.text({ text = '探针C' }) end })`ncg.page({ name = 'probeD', type = cg.PAGE.DIALOG, render = function() cg.text({ text = '探针D' }) end })`ncg.page.open('probeC')`ncg.page.open('probeD')`ncg.page.close('probeD')`nlocal o, c = _G._ev2.opened, _G._ev2.closed`nlocal msg`nif o == 0 and c == 1 then`n  msg = 'ISSUE-2 REPRODUCED: 排队页从未 page_opened 却收到 page_closed(opened=0 closed=1)，钩子/事件失衡'`nelse`n  msg = 'ISSUE-2 未复现: opened=' .. o .. ' closed=' .. c`nend`ncg.page.close('probeC')`n_G._iss['ISSUE-2'] = msg`nlog.info('[REVIEW] ' .. msg)`nreturn msg" } },

        # ---------- ISSUE-3 close_all 补位脉冲 ----------
        @{ op = 'note'; text = 'ISSUE-3：DIALOG E 显示+F 排队；close_all(DIALOG)（此刻无业务 DIALOG 打开）；F 在过程中被 on_open 一次再 on_close 一次即复现（pairs 迭代序不定：先关 F 则退化为 ISSUE-2 形态）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`nlocal eb = bgd_api.common.event_bus`n_G._iss = _G._iss or {}`n_G._ev3 = { opened = 0, closed = 0 }`neb.on('cgui.page_opened', function(n) if n == 'probeF' then _G._ev3.opened = _G._ev3.opened + 1 end end)`neb.on('cgui.page_closed', function(n) if n == 'probeF' then _G._ev3.closed = _G._ev3.closed + 1 end end)`ncg.page({ name = 'probeE', type = cg.PAGE.DIALOG, render = function() cg.text({ text = '探针E' }) end })`ncg.page({ name = 'probeF', type = cg.PAGE.DIALOG, render = function() cg.text({ text = '探针F' }) end })`ncg.page.open('probeE')`ncg.page.open('probeF')`ncg.page.close_all(cg.PAGE.DIALOG)`nlocal o, c = _G._ev3.opened, _G._ev3.closed`nlocal msg`nif o == 1 and c == 1 then`n  msg = 'ISSUE-3 REPRODUCED: close_all 过程中排队页被补位脉冲(on_open 一次→on_close 一次)'`nelseif o == 0 and c == 1 then`n  msg = 'ISSUE-3 未复现: close_all 迭代序先关排队页（退化为 ISSUE-2 钩子失衡形态）'`nelse`n  msg = 'ISSUE-3 结果异常: opened=' .. o .. ' closed=' .. c`nend`n_G._iss['ISSUE-3'] = msg`nlog.info('[REVIEW] ' .. msg)`nreturn msg" } },

        # ---------- ISSUE-6 slider on_commit 失效 ----------
        @{ op = 'note'; text = 'ISSUE-6：探针页 probe_sl 含 slider（只传 on_commit 不传 on_change）→ set_value 0.9 → 检查 _G._sl 是否仍为 nil' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`n_G._sl = nil`ncg.page({ name = 'probe_sl', type = cg.PAGE.POPUP, render = function()`n  cg.text({ text = '探针滑块（只传 on_commit 不传 on_change）' })`n  cg.slider({ key = 'sl', value = 0.5, min = 0, max = 1, layout = { width = 240 }, on_commit = function(v) _G._sl = v end })`nend })`ncg.page.open('probe_sl')`nreturn 'probe_sl opened'" } },
        @{ op = 'wait'; ms = 600 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ scope = 'probe_sl'; q = 'probe_sl/sl' }; save_as = 'slid' },
        @{ op = 'invoke'; id = 'lua.set_value'; args = @{ id = '{$slid}'; value = 0.9 } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`n_G._iss = _G._iss or {}`nlocal msg`nif _G._sl == nil then`n  msg = 'ISSUE-6 REPRODUCED: slider 只传 on_commit 时交互块被 cb.on_change 守卫整体跳过，set_value 0.9 后 on_commit 未触发(_G._sl=nil)'`nelse`n  msg = 'ISSUE-6 未复现: on_commit 已触发 _G._sl=' .. tostring(_G._sl)`nend`ncg.page.close('probe_sl')`n_G._iss['ISSUE-6'] = msg`nlog.info('[REVIEW] ' .. msg)`nreturn msg" } },

        # ---------- ISSUE-7 countdown 槽位共享 ----------
        @{ op = 'note'; text = 'ISSUE-7：探针页 probe_cd 同容器放两个无 key countdown（1.5 秒，on_done 各置 _G._cd1/_G._cd2）→ 等 2.5s → 若仅第一个置位即复现（cd_done@auto 槽位共享）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`n_G._cd1 = false`n_G._cd2 = false`nlocal t0 = os.clock()`ncg.page({ name = 'probe_cd', type = cg.PAGE.POPUP, render = function()`n  local s = math.max(1.5 - (os.clock() - t0), 0)`n  cg.countdown({ seconds = s, on_done = function() _G._cd1 = true end })`n  cg.countdown({ seconds = s, on_done = function() _G._cd2 = true end })`nend })`ncg.page.open('probe_cd')`nreturn 'probe_cd opened'" } },
        @{ op = 'wait'; ms = 2500 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`n_G._iss = _G._iss or {}`nlocal msg`nif _G._cd1 and not _G._cd2 then`n  msg = 'ISSUE-7 REPRODUCED: 同容器两个无 key countdown 共享 remember 槽位(cd_done@auto)，仅第一个 on_done 触发'`nelseif _G._cd1 and _G._cd2 then`n  msg = 'ISSUE-7 未复现: 两个 on_done 均触发'`nelse`n  msg = 'ISSUE-7 结果异常: cd1=' .. tostring(_G._cd1) .. ' cd2=' .. tostring(_G._cd2)`nend`ncg.page.close('probe_cd')`n_G._iss['ISSUE-7'] = msg`nlog.info('[REVIEW] ' .. msg)`nreturn msg" } },

        # ---------- ISSUE-9 pscroll 高度覆盖 ----------
        @{ op = 'note'; text = 'ISSUE-9：探针页 probe_h9 渲染 cg.pscroll({ key=psc9, layout={ height=500 } }, 长内容) → 快照取视口 rect 高 → ~=500 即复现（被缺省 320 静默覆盖）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`ncg.page({ name = 'probe_h9', type = cg.PAGE.POPUP, render = function()`n  cg.pscroll({ key = 'psc9', layout = { height = 500 } }, function()`n    for i = 1, 60 do cg.text({ key = 'h9_' .. i, text = '高度探针行 ' .. i }) end`n  end)`nend })`ncg.page.open('probe_h9')`nreturn 'probe_h9 opened'" } },
        @{ op = 'wait'; ms = 600 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local core = require('libs.client.cgui.core')`nlocal cg = bgd_api.client.cgui`n_G._iss = _G._iss or {}`nlocal h = nil`nfor id, e in pairs(core.dbg.snapshot) do`n  if id:match('psc9$') and e.rect then h = e.rect.h break end`nend`nlocal msg`nif not h then`n  msg = 'ISSUE-9 探针异常: 快照未找到 psc9 视口 rect'`nelseif math.abs(h - 500) > 1 then`n  msg = 'ISSUE-9 REPRODUCED: pscroll 传 layout.height=500 被缺省 height=320 静默覆盖（实测视口高=' .. tostring(h) .. '）'`nelse`n  msg = 'ISSUE-9 未复现: 视口高=500 生效'`nend`ncg.page.close('probe_h9')`n_G._iss['ISSUE-9'] = msg`nlog.info('[REVIEW] ' .. msg)`nreturn msg" } },

        # ---------- 汇总 + 清理 ----------
        @{ op = 'note'; text = '汇总各 ISSUE 标记并清理全部 probe_ 探针页（ISSUE-11~20 静态已证实，见审查报告）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`nlocal names = { 'probeA', 'probeB', 'probeC', 'probeD', 'probeE', 'probeF', 'probeX', 'probe_sl', 'probe_cd', 'probe_h9' }`nfor _, n in ipairs(names) do cg.page.close(n) end`nlocal order = { 'ISSUE-1', 'ISSUE-2', 'ISSUE-3', 'ISSUE-6', 'ISSUE-7', 'ISSUE-9' }`nlocal parts = {}`nfor _, k in ipairs(order) do parts[#parts + 1] = tostring(_G._iss and _G._iss[k] or (k .. '=未执行')) end`nlocal msg = '复核汇总 | ' .. table.concat(parts, ' | ')`nlog.info('[REVIEW] ' .. msg)`nreturn msg" } },

        @{ op = 'note'; text = '标记明细见各 eval 步 result 与游戏日志 [REVIEW] 前缀；日志 errors 段必须为空' },
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
