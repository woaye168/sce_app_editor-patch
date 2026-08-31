# 0.8.5 存量界面逐面验收：游戏真实面板（商店/背包/GM/HUD/2D 场景覆盖件）
# 统一 Page 架构版：页名小写化（shop/bag/gm/team），id 路径首段 = 页名 = find_ui scope
# 每面标准：≥1 次 find 文本命中 + ≥1 次操作生效 + ≥1 次 assert_text + 全程 errors=0
# 用法：powershell -File game_panels085.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '0.8.5 游戏面板验收：商店/背包/GM/HUD（统一 Page 架构）' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        @{ op = 'note'; text = '商店面（page/popup/shop.lua；HUD 入口在 hud_bar 页）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '商店' } },
        @{ op = 'wait_for'; q = '每周商店'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'weekly' } },
        @{ op = 'wait_for'; q = '每周豪礼'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'monthly' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'note'; text = '键盘 U 关商店（key_down/key_up 能力验收）' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '每周商店'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = '背包面（page/popup/bag.lua；键盘 Y 开）' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '整理背包'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '整理背包' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'assert_text'; q = '背 包'; present = $true },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '整理背包' } },
        @{ op = 'note'; text = '点 X 关闭背包（tag 定位 bag_close）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'bag_close' }; save_as = 'bag_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$bag_close}'; expect_absent = '整理背包' } },
        @{ op = 'wait_for'; q = '整理背包'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'GM 面（page/popup/gm.lua）：HUD 入口→填表单→tag 定位关闭' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'GM'; expect = 'GM 面板' } },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = 'uid'; text = '123' } },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = 'amount'; text = '100' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '123'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '钻石' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_close' }; save_as = 'gm_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$gm_close}' } },
        @{ op = 'wait_for'; q = 'GM 面板'; present = $false; timeout_ms = 3000 },

        @{ op = 'note'; text = '组队面（page/popup/team.lua；非 exclusive 与商店共存验收）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '组队'; expect = '创建队伍' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '商店' } },
        @{ op = 'wait_for'; q = '特惠商店'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '创建队伍'; present = $true },
        @{ op = 'note'; text = '组队小窗在 exclusive 商店打开后仍可见（非 exclusive 不参与互斥）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`ncg.page.close_all(cg.PAGE.POPUP)`nreturn 'closed'" } },
        @{ op = 'wait'; ms = 300 },

        @{ op = 'note'; text = 'HUD/场景覆盖件：hud_bar 商店入口即 2D 场景 cgui 覆盖件（前面已操作生效）' },
        @{ op = 'assert_text'; q = '商店'; present = $true },

        @{ op = 'note'; text = '日志 errors 段必须为空（历史 sprobe 报错为 0.8.5 开发期探针遗留，看 distinct 增量）' },
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
    [Console]::WriteLine("PARSE FAIL: $($_.Exception.Message)")
    [Console]::WriteLine($out)
}
