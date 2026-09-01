# 0.8.5 全量验收 04：组队页全交互·单端可测范围（page/popup/team.lua + TeamSystem.lua 逐字核实）
# 覆盖矩阵（单 PIE 只有一个客户端，双人流程见末尾受限项）：
#   打开：HUD 入口（tag hud_team_entry）→ assert「创建队伍」；X 钮（tag team_close）
#   未组队：说明文本「在线玩家（走近即可邀请）」；可邀请列表空态「暂无其他可邀请玩家」（单端无其他在线玩家）
#   创建队伍（tag team_create）→ 日志「[组队] 创建队伍 #」+ toast「已创建队伍，去邀请队友吧」
#         + 标题变「队伍 1/4」+ 出现「解散队伍」（tag team_dismiss）与「退出队伍」（tag team_leave）
#         + 无「踢出」（自己行不渲染踢出按钮）
#   重复创建：eval 直发 Req_TeamCreate → toast「你已经在队伍里了」（服务端 warn 路径）
#   邀请限制：eval 直发 Req_TeamInvite 邀请自己 / 不存在 uid=999999
#         → 服务端静默 return（fromKey==toKey / 目标不在线），无 toast（负断言「发出组队邀请」不出现）
#   退出队伍（tag team_leave）→ 单人队退出=解散（无 toast 发自己），标题回组队视图「创建队伍」复现
#   再创建 → 解散队伍（tag team_dismiss）→ toast「队伍已解散」+ 日志「解散队伍 #」+ 标题回组队视图
#   非 exclusive 共存：开 team 小窗再 U 开商店 → 商店开且 team 小窗仍可见（「创建队伍」仍命中）
#   team_nearby：单端无附近玩家不渲染按钮（「合并队伍」文本不存在；邀请钮同理）
#   收尾 logs errors=0
# 受限项（双人流程，单端不测，列入报告受限项）：
#   邀请/接受/拒绝/合并/踢出/邀请失效 TTL（TeamSystem.INVITE_TTL=15000ms）——需第二个客户端，
#   相关 toast「已向 X 发出组队邀请」「同意了你的邀请」「拒绝了你的邀请」「队伍已合并！」
#   「你被移出了队伍」「邀请已失效」及 confirm_async 弹窗（组队邀请/队伍合并）均不在本次范围。
# 用法：powershell -File 04_team_full.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '04 组队页全交互（单端可测范围）：打开/空态/创建/重复创建/邀请限制/退出/解散/共存' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        @{ op = 'note'; text = '打开①：HUD 入口（tag hud_team_entry）→ assert「创建队伍」' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_team_entry' }; save_as = 'hud_team' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_team}'; expect = '创建队伍' } },

        @{ op = 'note'; text = '未组队：说明文本 + 可邀请列表空态（单端无其他在线玩家）' },
        @{ op = 'assert_text'; q = '在线玩家（走近即可邀请）'; present = $true },
        @{ op = 'wait_for'; q = '暂无其他可邀请玩家'; timeout_ms = 5000 },

        @{ op = 'note'; text = 'X 钮（tag team_close）关闭 → 重新打开' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'team_close' }; save_as = 'team_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$team_close}' } },
        @{ op = 'wait_for'; q = '创建队伍'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_team_entry' }; save_as = 'hud_team2' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_team2}'; expect = '创建队伍' } },

        @{ op = 'note'; text = '创建队伍（tag team_create）→ toast + 标题「队伍 1/4」+ 解散/退出钮出现 + 无「踢出」+ 日志' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '创建队伍' } },
        @{ op = 'wait_for'; q = '已创建队伍，去邀请队友吧'; timeout_ms = 5000 },
        @{ op = 'wait_for'; q = '队伍 1/4'; timeout_ms = 5000 },
        @{ op = 'assert_text'; q = '解散队伍'; present = $true },
        @{ op = 'assert_text'; q = '退出队伍'; present = $true },
        @{ op = 'assert_text'; q = '踢出'; present = $false },
        @{ op = 'logs'; source = 'game_server'; match = '创建队伍 #' },

        @{ op = 'note'; text = '重复创建：已在队伍视图点不到创建钮，eval 直发 Req_TeamCreate → toast「你已经在队伍里了」' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local protocol = require('libs.common.api.protocol')`nlocal P = require('src.common.Protocol')`nprotocol.send_to_server(P.Req_TeamCreate)`nreturn 'sent'" } },
        @{ op = 'wait_for'; q = '你已经在队伍里了'; timeout_ms = 5000 },

        @{ op = 'note'; text = '邀请限制①：eval 邀请自己 → 服务端 fromKey==toKey 静默 return，无 toast（负断言）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local W = require('src.client.world.WorldState')`nlocal protocol = require('libs.common.api.protocol')`nlocal P = require('src.common.Protocol')`nprotocol.send_to_server(P.Req_TeamInvite, { target_uid = tonumber(W.localPlayerUid) })`nreturn 'sent'" } },
        @{ op = 'wait'; ms = 600 },
        @{ op = 'assert_text'; q = '发出组队邀请'; present = $false },
        @{ op = 'note'; text = '邀请限制②：eval 邀请不存在 uid=999999 → 目标不在线静默 return，无 toast（负断言）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local protocol = require('libs.common.api.protocol')`nlocal P = require('src.common.Protocol')`nprotocol.send_to_server(P.Req_TeamInvite, { target_uid = 999999 })`nreturn 'sent'" } },
        @{ op = 'wait'; ms = 600 },
        @{ op = 'assert_text'; q = '发出组队邀请'; present = $false },
        @{ op = 'assert_text'; q = '队伍 1/4'; present = $true },

        @{ op = 'note'; text = '退出队伍（tag team_leave）：单人队退出=解散，自己无 toast，标题回组队视图' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '退出队伍' } },
        @{ op = 'wait_for'; q = '创建队伍'; timeout_ms = 5000 },
        @{ op = 'assert_text'; q = '解散队伍'; present = $false },
        @{ op = 'logs'; source = 'game_server'; match = '退出队伍' },

        @{ op = 'note'; text = '再创建 → 解散队伍（tag team_dismiss）→ toast「队伍已解散」+ 日志「解散队伍 #」+ 标题回组队视图' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '创建队伍' } },
        @{ op = 'wait_for'; q = '队伍 1/4'; timeout_ms = 5000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '解散队伍' } },
        @{ op = 'wait_for'; q = '队伍已解散'; timeout_ms = 5000 },
        @{ op = 'wait_for'; q = '创建队伍'; timeout_ms = 5000 },
        @{ op = 'logs'; source = 'game_server'; match = '解散队伍 #' },

        @{ op = 'note'; text = '非 exclusive 共存：team 小窗开着，U 开商店 → 商店开且 team 小窗仍可见' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '创建队伍'; present = $true },
        @{ op = 'note'; text = '组队小窗在 exclusive 商店打开后仍可见（非 exclusive 不参与互斥）；U 关商店后小窗仍在' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; present = $false; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '创建队伍'; present = $true },

        @{ op = 'note'; text = 'team_nearby：单端无附近玩家，邀请/合并按钮均不渲染（负断言「合并队伍」；邀请钮文本含玩家名无法定值，同路径不渲染）' },
        @{ op = 'assert_text'; q = '合并队伍'; present = $false },

        @{ op = 'note'; text = '受限项：邀请/接受/拒绝/合并/踢出/邀请失效 TTL 为双人流程，本次单端不测，列入报告受限项' },
        @{ op = 'note'; text = '收尾：关 team 小窗 + 日志 errors 段必须为空' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'team_close' }; save_as = 'team_close2' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$team_close2}' } },
        @{ op = 'wait_for'; q = '创建队伍'; present = $false; timeout_ms = 4000 },
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
