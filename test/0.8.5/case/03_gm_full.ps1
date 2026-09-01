# 0.8.5 全量验收 03：GM 面板全交互（page/popup/gm.lua + GMSystem.lua 逐字核实）
# 覆盖矩阵：
#   打开：HUD 入口（tag hud_gm_entry）→ 标题「GM 面板（我的 ID: <uid>）」；X 钮（tag gm_close）
#   表单：uid 输入（tag gm_uid_input）/ 数量输入（tag gm_amount_input）/
#         货币单选（tag gm_currency，项「钻石」「金币」，默认选中金币=form.currency 'money'）
#   空输入确认 → warn toast「GM：请输入目标玩家数字 ID」+ 服务端日志无「GM 发放货币」（负断言）
#   数量 0 / 负数 → toast「GM：请输入大于 0 的数量」（同一代码路径 amount<=0）
#   成功：填自己 uid（eval 读 W.localPlayerUid，save_as 串联）+ 数量 100 + 金币
#         → 日志「GM 发放货币：操作者」+ toast「已给玩家」/「金币x100」
#   钻石单选切换后再发放 50 → toast「钻石x50」→ U 开商店验证钱包 445+50=495
#   不在线：uid=999999 → toast「GM：玩家 999999 不在线」
#   表单重置：填一半关掉重开 → 输入框清空（on_open 重置 form，旧值文本消失）
#   收尾 logs errors=0
# 受限项：单选「选中态」无文本断言手段，仅覆盖切换后发放结果（钻石入账即证明切换生效）；
#         负断言（未发放）以 logs match 输出人工核对计数，场景引擎不支持跨步骤计数比较。
# 用法：powershell -File 03_gm_full.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '03 GM 面板全交互：打开/表单/校验/成功发放/钻石/不在线/表单重置' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        @{ op = 'note'; text = '先 eval 读自己 uid（WorldState.localPlayerUid），save_as 供后续表单填写引用' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local W = require('src.client.world.WorldState')`nreturn tostring(W.localPlayerUid)" }; save_as = 'my_uid'; save_field = 'result' },

        @{ op = 'note'; text = '打开①：HUD 入口（tag hud_gm_entry）→ 标题含「GM 面板（我的 ID:」' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_gm_entry' }; save_as = 'hud_gm' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_gm}'; expect = 'GM 面板' } },
        @{ op = 'assert_text'; q = 'GM 面板（我的 ID:'; present = $true },

        @{ op = 'note'; text = 'X 钮（tag gm_close）关闭 → 重新打开' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_close' }; save_as = 'gm_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$gm_close}' } },
        @{ op = 'wait_for'; q = 'GM 面板'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_gm_entry' }; save_as = 'hud_gm2' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_gm2}'; expect = 'GM 面板' } },

        @{ op = 'note'; text = '表单控件存在：uid 输入 / 数量输入 / 货币单选（项「钻石」「金币」，默认选中金币）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_uid_input' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_amount_input' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_currency' } },
        @{ op = 'assert_text'; q = '钻石'; present = $true },
        @{ op = 'assert_text'; q = '金币'; present = $true },

        @{ op = 'note'; text = '空输入确认 → warn toast「GM：请输入目标玩家数字 ID」+ 日志无「GM 发放货币」（负断言，人工核对计数）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确认发放' } },
        @{ op = 'wait_for'; q = 'GM：请输入目标玩家数字 ID'; timeout_ms = 5000 },
        @{ op = 'logs'; source = 'game_server'; match = 'GM 发放货币' },

        @{ op = 'note'; text = '数量 0 → toast「GM：请输入大于 0 的数量」' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_uid_input' }; save_as = 'uid_in' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$uid_in}'; text = '{$my_uid}' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_amount_input' }; save_as = 'amt_in' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$amt_in}'; text = '0' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确认发放' } },
        @{ op = 'wait_for'; q = 'GM：请输入大于 0 的数量'; timeout_ms = 5000 },
        @{ op = 'note'; text = '数量负数（-5）：同一代码路径 amount<=0，toast 文本相同，wait 后目视/断言' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$amt_in}'; text = '-5' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确认发放' } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'assert_text'; q = 'GM：请输入大于 0 的数量'; present = $true },

        @{ op = 'note'; text = '成功：自己 uid + 数量 100 + 金币（默认选中）→ 日志「GM 发放货币：操作者」+ toast「已给玩家」「金币x100」' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$amt_in}'; text = '100' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确认发放' } },
        @{ op = 'wait_for'; q = '已给玩家'; timeout_ms = 5000 },
        @{ op = 'assert_text'; q = '金币x100'; present = $true },
        @{ op = 'logs'; source = 'game_server'; match = 'GM 发放货币：操作者' },

        @{ op = 'note'; text = '钻石单选切换后发放 50 → toast「钻石x50」→ U 开商店验证钱包 445+50=495' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '钻石' } },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$amt_in}'; text = '50' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确认发放' } },
        @{ op = 'wait_for'; q = '钻石x50'; timeout_ms = 5000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; timeout_ms = 4000 },
        @{ op = 'wait_for'; q = '495'; timeout_ms = 6000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = '不在线：重开 GM，uid=999999 → toast「GM：玩家 999999 不在线」' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_gm_entry' }; save_as = 'hud_gm3' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_gm3}'; expect = 'GM 面板' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_uid_input' }; save_as = 'uid_in2' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$uid_in2}'; text = '999999' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_amount_input' }; save_as = 'amt_in2' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$amt_in2}'; text = '100' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确认发放' } },
        @{ op = 'wait_for'; q = 'GM：玩家 999999 不在线'; timeout_ms = 5000 },

        @{ op = 'note'; text = '表单重置：填一半（uid=12345）关掉重开 → 输入框清空（on_open 重置 form）' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$uid_in2}'; text = '12345' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '12345'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_close' }; save_as = 'gm_close2' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$gm_close2}' } },
        @{ op = 'wait_for'; q = 'GM 面板'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_gm_entry' }; save_as = 'hud_gm4' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_gm4}'; expect = 'GM 面板' } },
        @{ op = 'assert_text'; q = '12345'; present = $false },

        @{ op = 'note'; text = '收尾：关 GM 面板 + 日志 errors 段必须为空' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_close' }; save_as = 'gm_close3' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$gm_close3}' } },
        @{ op = 'wait_for'; q = 'GM 面板'; present = $false; timeout_ms = 4000 },
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
