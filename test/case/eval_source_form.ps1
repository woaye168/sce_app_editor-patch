# eval 源码形态直通验收：require 四象限 + res 三类型字面量 + 盖戳缺失响亮失败
# 机制：bgd_sce_tools 构建盖戳 path_rules.lua → 框架 entrance 加载 _G.bgd_path_rules
#       → dbg_bus eval 收敛翻译（MCP 与直连桥 HTTP 双通道同权，HTTP 侧见注释末行）
# 用法：powershell -File eval_source_form.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = 'eval 源码形态直通：require 四象限 + res 字面量 + 响亮失败' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 5000 },

        @{ op = 'note'; text = '盖戳已加载：_G.bgd_path_rules 存在且 map 正确' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local r = _G.bgd_path_rules`nreturn { has = r ~= nil, map = r and r.map }" } },

        @{ op = 'note'; text = 'require 四象限：libs/src × client/common 源码形态直写' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local io_api = require('libs.client.api.io')`nlocal json = require('libs.common.api.json')`nlocal DemoLog = require('src.client.api.DemoLog')`nlocal mu = require('src.common.api.math_util')`nDemoLog.PrintHello()`nreturn { io = io_api ~= nil, json = json.encode_x({ a = 1 }), demolog = DemoLog ~= nil, d2 = mu.DistSq(0, 0, 3, 4) }" } },

        @{ op = 'note'; text = 'res 字面量三类型：image 保留 .png / sound 去 .ogg / sprites 加 @地图包前缀' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "return {`n  img = 'src/res/image/armor_dark.png',`n  snd = 'src/res/sound/airjump_7781.ogg',`n  spr = 'src/res/sprites/tmw_desert_packed.png',`n  lib_img = 'libs/res/image/armor_dark.png'`n}" } },

        @{ op = 'note'; text = 'res 真实渲染：cg.image 用源码形态路径出图' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`ncg.page({ name = 'res_probe', type = cg.PAGE.HUD, render = function()`n  cg.pin({ anchor = 'tl', offset = { 400, 300 } }, function()`n    cg.image({ src = 'src/res/image/armor_dark.png', layout = { width = 96, height = 96 } })`n  end)`nend })`ncg.page.open('res_probe')`nreturn 'ok'" } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'capture'; max_width = 900 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "bgd_api.client.cgui.page.close('res_probe')`nreturn 'closed'" } },

        @{ op = 'note'; text = '盖戳缺失响亮失败（本步故意置 nil，会话结束自动恢复）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "_G.bgd_path_rules = nil`nreturn 'cleared'" } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local mu = require('src.common.api.math_util')`nreturn mu.DistSq(0, 0, 1, 1)" } },
        @{ op = 'logs'; source = 'game_client'; tail_lines = 3 }
    )
    stop_on_error = $false
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
        if ($r.ok -and $r.result) { $line += ' :: ' + ([string]($r.result | ConvertTo-Json -Compress -Depth 6)) }
        if (-not $r.ok) { $line += ' :: ' + ([string]$r.error) }
        [Console]::WriteLine($line)
    }
    [Console]::WriteLine(("failed_step: {0}    elapsed: {1}ms" -f $sj.failed_step, $sj.elapsed_ms))
} catch {
    [Console]::WriteLine("PARSE FAIL: $($_.Exception.Message)")
    [Console]::WriteLine($out)
}
