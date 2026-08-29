# 0.8.2 存量界面逐面验收：游戏真实面板（商店/背包/GM/HUD/2D 场景覆盖件）
# 每面标准：≥1 次 find 文本命中 + ≥1 次操作生效 + ≥1 次 assert_text + 全程 errors=0
# 用法：powershell -File game_panels082.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '0.8.2 游戏面板验收：商店/背包/GM/HUD/场景覆盖件' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        @{ op = 'note'; text = '商店面（cgui ShopUI + hud_shop 场景覆盖件入口）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '商店' } },
        @{ op = 'wait_for'; q = '每周商店'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'weekly' } },
        @{ op = 'assert_text'; q = '每周豪礼'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'monthly' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'note'; text = '键盘 U 关商店（key_down/key_up 能力验收）' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '每周商店'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = '背包面（cgui BagUI；键盘 Y 开）' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '整理背包'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '整理背包' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'assert_text'; q = '背 包'; present = $true },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '整理背包' } },
        @{ op = 'note'; text = '点 X 关闭背包' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = 'BagUI/root/screen/center/center_wrap/center/window/titlebar/close' } },
        @{ op = 'wait_for'; q = '整理背包'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'GM 面：base.ui 旧体系（非 cgui）——find_ui 只定位不可操作（R4 边界确认）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'gm_hud_btn' } },
        @{ op = 'assert_text'; q = 'gm_hud_btn'; present = $true },

        @{ op = 'note'; text = 'HUD/场景覆盖件：hud_shop 商店入口即 2D 场景 cgui 覆盖件（前面已操作生效）' },
        @{ op = 'assert_text'; q = '商店'; present = $true },

        @{ op = 'note'; text = '日志 errors 段必须为空' },
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
