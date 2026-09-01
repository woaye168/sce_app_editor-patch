# 0.8.5 全量验收 02：商店页全交互（page/popup/shop.lua + ShopConfig + ShopSystem 逐字核实）
# 覆盖矩阵：
#   打开：HUD 入口（tag hud_shop_entry）/ U 键 toggle / X 钮（tag shop_close）/ 遮罩点击不关页
#   数据：钱包栏（tag shop_wallet）数值 2000/445（INIT_MONEY/INIT_GEM）/ 倒计时「刷新: 」前缀
#   页签：特惠/每日/每周/每月全部切换，断言各页签独有礼包名出现、他页签礼包名消失
#   免费领取：特惠礼包 + 每日补给 → 「限购 0/1」→「限购 1/1」、按钮变「已领取」、
#            toast「获得」、服务端日志「购买礼包」、钱包 4000/455
#   付费购买：eval 直发 Req_GMAddCurrency 发 500 钻（uid 读 WorldState.localPlayerUid）→ 955；
#            eval 购精选伙伴礼一（60 钻）→ 895、toast「获得精选伙伴礼一」
#   货币不足：每月至尊礼包（980 钻）→ toast「钻石不足」；每月特惠礼包（5000 金）→ toast「金币不足」；限购不变
#   一键购买：特惠页签（tag shop_buy_all）→ 日志「一键购买页签」+ toast「一键购买成功」、钱包 647；
#            再次（eval 直发）→ toast「没有可一键购买的礼包」
#   红点：免费礼包未领时 capture 目视 HUD 商店钮红点；两个免费礼包领完后 capture 对比（红点消失）
#   exclusive：商店开时 Y 开背包 → 商店关；关背包后 U 开回商店
#   收尾 logs errors=0
# 受限项：付费购买按钮与礼包行无唯一文本定位（同 tag shop_buy 多行复用），付费/不足/二次一键走 eval 直发协议；
#         钱包数值断言基于 start_debug 全新对局（内存数据，无持久化），顺序敏感勿调步序。
# 用法：powershell -File 02_shop_full.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '02 商店页全交互：打开/数据/页签/免费/付费/不足/一键购买/红点/exclusive' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        @{ op = 'note'; text = '打开①：HUD 入口（tag hud_shop_entry）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_shop_entry' }; save_as = 'hud_shop' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_shop}'; expect = '特惠商店' } },

        @{ op = 'note'; text = '数据就绪：钱包 2000/445（ShopConfig.INIT_MONEY/INIT_GEM）+ 倒计时「刷新: 」前缀' },
        @{ op = 'wait_for'; q = '2000'; timeout_ms = 6000 },
        @{ op = 'assert_text'; q = '445'; present = $true },
        @{ op = 'assert_text'; q = '刷新:'; present = $true },

        @{ op = 'note'; text = '四页签全部切换：各页签独有礼包名出现 / 他页签礼包名消失' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '每日商店' } },
        @{ op = 'wait_for'; q = '每日补给'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '特惠礼包'; present = $false },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '每周商店' } },
        @{ op = 'wait_for'; q = '每周豪礼'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '每日补给'; present = $false },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '每月商店' } },
        @{ op = 'wait_for'; q = '每月至尊礼包'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '每周豪礼'; present = $false },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '特惠商店' } },
        @{ op = 'wait_for'; q = '精选伙伴礼一'; timeout_ms = 4000 },

        @{ op = 'note'; text = '红点：免费礼包未领，capture 目视 HUD 商店钮（右上最右）红点存在' },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = '免费领取①（特惠礼包）：限购 0/1→1/1、按钮变「已领取」、toast「获得」、钱包 4000/455' },
        @{ op = 'assert_text'; q = '限购 0/1'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '免费领取' } },
        @{ op = 'wait_for'; q = '获得特惠礼包'; timeout_ms = 5000 },
        @{ op = 'wait_for'; q = '已领取'; timeout_ms = 5000 },
        @{ op = 'assert_text'; q = '限购 1/1'; present = $true },
        @{ op = 'wait_for'; q = '4000'; timeout_ms = 5000 },
        @{ op = 'assert_text'; q = '455'; present = $true },
        @{ op = 'logs'; source = 'game_server'; match = '购买礼包 特惠礼包' },

        @{ op = 'note'; text = '免费领取②（每日补给）：每日页签免费礼包，领完全部免费→红点消失' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '每日商店' } },
        @{ op = 'wait_for'; q = '每日补给'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '免费领取' } },
        @{ op = 'wait_for'; q = '获得每日补给'; timeout_ms = 5000 },
        @{ op = 'assert_text'; q = '限购 1/1'; present = $true },
        @{ op = 'note'; text = '红点对比：全部免费礼包已领，capture 目视 HUD 商店钮红点消失' },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'GM 发钻（eval 直发 Req_GMAddCurrency 给自己，uid 读 WorldState.localPlayerUid）：445+500=955' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local W = require('src.client.world.WorldState')`nlocal protocol = require('libs.common.api.protocol')`nlocal P = require('src.common.Protocol')`nprotocol.send_to_server(P.Req_GMAddCurrency, { target_uid = tonumber(W.localPlayerUid), currency = 'gem', amount = 500 })`nreturn 'sent'" } },
        @{ op = 'wait_for'; q = '钻石x500'; timeout_ms = 5000 },
        @{ op = 'wait_for'; q = '955'; timeout_ms = 5000 },

        @{ op = 'note'; text = '付费购买（eval 购精选伙伴礼一，60 钻）：955-60=895、toast「获得精选伙伴礼一」、日志「购买礼包」' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '特惠商店' } },
        @{ op = 'wait_for'; q = '精选伙伴礼一'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local protocol = require('libs.common.api.protocol')`nlocal P = require('src.common.Protocol')`nprotocol.send_to_server(P.Req_ShopBuy, { pack_id = 2 })`nreturn 'sent'" } },
        @{ op = 'wait_for'; q = '获得精选伙伴礼一'; timeout_ms = 5000 },
        @{ op = 'wait_for'; q = '895'; timeout_ms = 5000 },
        @{ op = 'logs'; source = 'game_server'; match = '购买礼包 精选伙伴礼一' },

        @{ op = 'note'; text = '货币不足：每月至尊礼包 980 钻 > 895 → toast「钻石不足」；每月特惠礼包 5000 金 > 4000 → toast「金币不足」；限购不变' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '每月商店' } },
        @{ op = 'wait_for'; q = '每月至尊礼包'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local protocol = require('libs.common.api.protocol')`nlocal P = require('src.common.Protocol')`nprotocol.send_to_server(P.Req_ShopBuy, { pack_id = 31 })`nreturn 'sent'" } },
        @{ op = 'wait_for'; q = '钻石不足'; timeout_ms = 5000 },
        @{ op = 'assert_text'; q = '限购 0/1'; present = $true },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local protocol = require('libs.common.api.protocol')`nlocal P = require('src.common.Protocol')`nprotocol.send_to_server(P.Req_ShopBuy, { pack_id = 32 })`nreturn 'sent'" } },
        @{ op = 'wait_for'; q = '金币不足'; timeout_ms = 5000 },
        @{ op = 'assert_text'; q = '限购 0/1'; present = $true },

        @{ op = 'note'; text = '一键购买（特惠页签 tag shop_buy_all）：剩余付费礼二/三 折后 99+149=248 钻，895-248=647' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '特惠商店' } },
        @{ op = 'wait_for'; q = '精选伙伴礼三'; timeout_ms = 4000 },
        @{ op = 'wait_for'; q = '一键购买'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '一键购买' } },
        @{ op = 'wait_for'; q = '一键购买成功'; timeout_ms = 5000 },
        @{ op = 'wait_for'; q = '647'; timeout_ms = 5000 },
        @{ op = 'logs'; source = 'game_server'; match = '一键购买页签' },
        @{ op = 'note'; text = '再次一键购买（按钮已随可购列表清空消失，eval 直发）→ toast「没有可一键购买的礼包」' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local protocol = require('libs.common.api.protocol')`nlocal P = require('src.common.Protocol')`nprotocol.send_to_server(P.Req_ShopBuyAll, { tab_id = 'special' })`nreturn 'sent'" } },
        @{ op = 'wait_for'; q = '没有可一键购买的礼包'; timeout_ms = 5000 },

        @{ op = 'note'; text = 'exclusive：商店开时 Y 开背包 → 商店关；关背包后 U 开回商店' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '整理背包'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '特惠商店'; present = $false },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'bag_close' }; save_as = 'bag_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$bag_close}' } },
        @{ op = 'wait_for'; q = '整理背包'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; timeout_ms = 4000 },

        @{ op = 'note'; text = '遮罩点击不关页（click_at 左上角 backdrop 区域，面板居中 980x660 之外）' },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 8; y = 8 } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'assert_text'; q = '特惠商店'; present = $true },

        @{ op = 'note'; text = '打开②：X 钮（tag shop_close）关闭' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'shop_close' }; save_as = 'shop_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$shop_close}' } },
        @{ op = 'wait_for'; q = '特惠商店'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = '打开③：U 键 toggle 开 → 再 U 关' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = '收尾：日志 errors 段必须为空（历史 sprobe 报错为 0.8.5 开发期探针遗留，看 distinct 增量）' },
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
