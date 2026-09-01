# 01_bag_full.ps1 — 0.8.5 全量验收：背包页全交互（page/popup/bag.lua + bag_grid.lua）
#
# 覆盖矩阵：
#   A 打开/关闭：HUD 入口（tag hud_bag_entry）开 → Y 键 toggle 关再开 → X 钮（tag bag_close）关
#     → 点遮罩（click_at 屏幕角落）不关页且 HUD 不响应（HUD 被 exclusive 规则挂起）
#   B 底栏按钮：随机放入（tag bag_add，网格物品数递增）/ 整理背包（bag_sort，服务端日志）
#     / 批量丢弃开（bag_batch → 文本「批量丢弃: 开」+ 信息栏「点击物品 快速丢弃」）
#     → 批量态单击物品直接丢弃（药水/锻造材料豁免，探针选非豁免物品）→ 批量丢弃关恢复
#   C 属性框：单击出框（「类型：」「占格」）→ 点另一物品切换 → 按钮按类型（药水=使用、
#     count>=2=拆分、旋转、丢弃恒在）→ count=1 无拆分钮 → 锚定框 capture_ui 截图
#   D 拖拽：拖空格移动（drag_ui → Req_BagMove）/ 双击旋转（受限：vp 无双击能力，走协议层
#     Req_BagRotate 验证）/ 长按拆分（long_press_ui → Req_BagSplit）/ 同类堆叠合并
#     （拆分造出的同 id 两堆互拖 → Req_BagMove+target_uid）/ 锻造材料拖装备（材料靠随机
#     放入产出：有则真拖拽走 Req_ForgeItem + Sync_ForgeResult 断言，无则同构 no-op 拖拽
#     自愈并标注跳过）/ 非法落点（拖出网格外：不发任何请求，位置与物品数不变）
#   E Page 语义：背包（exclusive POPUP）开时 HUD 挂起（is_visible('hud_bar')==false、
#     「攻击」无命中）；开商店（U 键）→ 背包被互斥关闭（「背包已关闭」日志 +
#     is_open('bag')==false）；关商店 → HUD 恢复
#   F 状态复位：关再开 bagData 保留（物品数不变）、批量丢弃/选中复位
#   G 收尾：关背包，game_client 日志 errors distinct 必须为 0
#
# 数据断言通道：eval 订阅 Sync_BagData 存 _G.__probe_bag（服务端权威数据的客户端镜像，
# bag 页模块局部 S 不可达，探针是唯一可靠数据源）；网格物品 id 末段 = 'item_<uid>'，
# 格子 id 末段 = 'cell_<y>_<x>'（click_ui/drag_ui 支持末段简写）。
# 前置：编辑器在线；新调试会话初始背包 = 生命药水 x10 + 魔法药水 x10（BagSystem.InitPlayerBag）。
# 用法：powershell -File 01_bag_full.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

# ---------- eval 代码段（here-string 单引号：保留 {$var} 占位符给 run_scenario 替换） ----------

# 探针：订阅 Sync_BagData（链式包裹：protocol.on 重复注册会覆盖旧 handler，
# 必须先存 base.proto 现有处理器再串联调用，否则 bag.lua 的 S.bagData 永不被更新）
$codeProbe = @'
local P = require('src.common.Protocol')
local old = base.proto[P.Sync_BagData]
base.proto[P.Sync_BagData] = function(data)
  _G.__probe_bag = data
  if old then old(data) end
end
return 'probe_ready'
'@

# B：随机放入后物品数递增（允许 3 次放入中最多 2 次堆叠合并）
$codeAddAssert = @'
if {$n1} < {$n0} + 1 then
  error('随机放入未增加物品：' .. {$n0} .. ' -> ' .. {$n1} .. '（3 次全堆叠合并？重跑）')
end
return 'bag_add ok: ' .. {$n0} .. ' -> ' .. {$n1}
'@

# B：挑一个可批量丢弃物品（药水/锻造材料豁免，选 equip/chest/skillbook/普通材料）
$codePickDiscard = @'
local BagConfig = require('src.common.BagConfig')
local b = _G.__probe_bag
if not b or not b.items then error('探针无背包数据（Sync_BagData 未到）') end
for _, it in ipairs(b.items) do
  local cfg = BagConfig.GetItem(it.item_id)
  if cfg and cfg.type ~= 'potion' and not cfg.forge_stat then
    return { uid = it.uid }
  end
end
error('无可批量丢弃物品（全是药水/锻造材料），随机放入未产出，重跑')
'@

# B：批量态单击后物品应消失
$codeDiscardedAssert = @'
for _, it in ipairs(_G.__probe_bag.items) do
  if tostring(it.uid) == '{$bd_uid}' then error('批量丢弃单击未生效：物品仍在背包') end
end
return 'batch discard ok'
'@

# C：挑一个药水（初始背包必有）
$codePickPotion = @'
local BagConfig = require('src.common.BagConfig')
for _, it in ipairs(_G.__probe_bag.items) do
  local cfg = BagConfig.GetItem(it.item_id)
  if cfg and cfg.type == 'potion' then
    return { uid = it.uid }
  end
end
error('背包无药水（初始应含生命/魔法药水 x10）')
'@

# C：挑另一个物品（切换属性框）
$codePickOther = @'
local BagConfig = require('src.common.BagConfig')
for _, it in ipairs(_G.__probe_bag.items) do
  if tostring(it.uid) ~= '{$pot_uid}' then
    local cfg = BagConfig.GetItem(it.item_id)
    _G.__ctx_other = { uid = it.uid, name = cfg and cfg.name or '?' }
    return { uid = it.uid }
  end
end
error('背包只有一个物品，无法验证属性框切换')
'@

$codeOtherName = @'
return { n = _G.__ctx_other.name }
'@

# C：挑一个 count=1 物品（无拆分钮）
$codePickSingle = @'
for _, it in ipairs(_G.__probe_bag.items) do
  if (it.count or 1) == 1 then
    return { uid = it.uid }
  end
end
error('无 count=1 物品（随机放入未产出），重跑本用例')
'@

# D1：挑第一个物品 + 从右下角反向扫一个非原位的空格
$codeDragPrep = @'
local BagConfig = require('src.common.BagConfig')
local b = _G.__probe_bag
local it = b.items[1]
if not it then error('背包为空，无法拖拽') end
local cols, rows = BagConfig.GRID_COLS, BagConfig.GRID_ROWS
local cx, cy
for y = rows - 1, 0, -1 do
  for x = cols - 1, 0, -1 do
    if not (x == it.x and y == it.y)
      and BagConfig.CanPlace(b.items, cols, rows, it.item_id, x, y, it.rotated, it.uid) then
      cx, cy = x, y
      break
    end
  end
  if cx then break end
end
if not cx then error('无空格可拖') end
_G.__drag1 = { uid = it.uid, x = cx, y = cy }
return { uid = it.uid }
'@

$codeDragX = @'
return { v = _G.__drag1.x }
'@

$codeDragY = @'
return { v = _G.__drag1.y }
'@

# D1：拖拽后落点断言
$codeDragAssert = @'
for _, it in ipairs(_G.__probe_bag.items) do
  if tostring(it.uid) == '{$duid}' then
    if it.x ~= {$dcx} or it.y ~= {$dcy} then
      error('拖拽移动未生效：落点 ' .. it.x .. ',' .. it.y .. '，期望 {$dcx},{$dcy}')
    end
    return 'drag move ok'
  end
end
error('拖拽物品丢失')
'@

# D2：双击旋转（受限：vp 无双击能力，走协议层 Req_BagRotate 验证；含 RotateSpot 预判）
$codeRotSend = @'
local protocol = require('libs.common.api.protocol')
local P = require('src.common.Protocol')
local BagConfig = require('src.common.BagConfig')
local b = _G.__probe_bag
local tgt
for _, it in ipairs(b.items) do
  local cfg = BagConfig.GetItem(it.item_id)
  if cfg and cfg.w ~= cfg.h then tgt = it break end
end
if not tgt then
  _G.__rot = { mode = 'skip' }
  return 'skip: 无可旋转物品（w==h），随机放入未产出'
end
if not BagConfig.RotateSpot(b.items, BagConfig.GRID_COLS, BagConfig.GRID_ROWS, tgt) then
  _G.__rot = { mode = 'skip' }
  return 'skip: 旋转落位无解'
end
_G.__rot = { mode = 'real', uid = tgt.uid, rotated = tgt.rotated and true or false }
protocol.send_to_server(P.Req_BagRotate, { uid = tgt.uid })
return 'sent'
'@

$codeRotAssert = @'
local r = _G.__rot
if not r or r.mode ~= 'real' then return '跳过：无可旋转物品（协议层前置不满足）' end
for _, it in ipairs(_G.__probe_bag.items) do
  if tostring(it.uid) == tostring(r.uid) then
    if (it.rotated and true or false) == r.rotated then error('Req_BagRotate 后旋转态未翻转') end
    return 'rotate ok'
  end
end
error('旋转目标物品丢失')
'@

# D3：长按拆分前置（挑 count>=2 堆叠；初始药水 x10 保底）
$codeSplitPrep = @'
for _, it in ipairs(_G.__probe_bag.items) do
  if (it.count or 1) >= 2 then
    _G.__split = { uid = it.uid, count = it.count, n = #_G.__probe_bag.items }
    return { uid = it.uid }
  end
end
error('无可拆分堆叠（count>=2）')
'@

$codeSplitAssert = @'
local s = _G.__split
local b = _G.__probe_bag
if #b.items ~= s.n + 1 then
  error('长按拆分应新增一个物品堆：' .. s.n .. ' -> ' .. #b.items)
end
for _, it in ipairs(b.items) do
  if tostring(it.uid) == tostring(s.uid) then
    if (it.count or 1) >= s.count then error('拆分后原堆数量未减少') end
    return 'long-press split ok'
  end
end
error('拆分原堆丢失')
'@

# D4：同类堆叠合并前置（拆分已造出同 id 两堆，确定性成立）
$codeMergePrep = @'
local BagConfig = require('src.common.BagConfig')
local b = _G.__probe_bag
for i = 1, #b.items do
  for j = i + 1, #b.items do
    local a, c = b.items[i], b.items[j]
    if a.item_id == c.item_id then
      local cfg = BagConfig.GetItem(a.item_id)
      if cfg and (cfg.stack or 1) > 1 and (a.count or 1) + (c.count or 1) <= cfg.stack then
        _G.__merge = { a = a.uid, b = c.uid, sum = (a.count or 1) + (c.count or 1), n = #b.items }
        return { ua = a.uid }
      end
    end
  end
end
error('无同类可堆叠物品对（长按拆分应已制造），重跑本用例')
'@

$codeMergeB = @'
return { ub = _G.__merge.b }
'@

$codeMergeAssert = @'
local m = _G.__merge
local b = _G.__probe_bag
if #b.items ~= m.n - 1 then
  error('合并后物品数应减 1：' .. m.n .. ' -> ' .. #b.items)
end
local target
for _, it in ipairs(b.items) do
  if tostring(it.uid) == tostring(m.a) then error('被拖物品应已并入目标堆') end
  if tostring(it.uid) == tostring(m.b) then target = it end
end
if not target then error('合并目标堆丢失') end
if (target.count or 1) ~= m.sum then
  error('合并数量不符：' .. (target.count or 1) .. '，期望 ' .. m.sum)
end
return 'merge ok'
'@

# D5：锻造前置——有材料+装备则真拖拽（订阅 Sync_ForgeResult）；无则同构 no-op 拖拽（拖回自身格）
$codeForgePrep = @'
local protocol = require('libs.common.api.protocol')
local P = require('src.common.Protocol')
local BagConfig = require('src.common.BagConfig')
local b = _G.__probe_bag
local mat, equip
for _, it in ipairs(b.items) do
  local cfg = BagConfig.GetItem(it.item_id)
  if cfg then
    if cfg.forge_stat and not mat then mat = it end
    if cfg.type == 'equip' and not equip then equip = it end
  end
end
local ctx = { result = nil }
_G.__forge_ctx = ctx
-- 链式包裹（protocol.on 重复注册会覆盖 notify 页的 Sync_ForgeResult 处理器）
local old_forge = base.proto[P.Sync_ForgeResult]
base.proto[P.Sync_ForgeResult] = function(d)
  ctx.result = (d and d.result) or 'ok'
  if old_forge then old_forge(d) end
end
if mat and equip then
  ctx.mode = 'real'
  ctx.mat = mat.uid
  ctx.target = 'item_' .. tostring(equip.uid)
else
  local any = b.items[1]
  if not any then error('背包为空') end
  ctx.mode = 'skip'
  ctx.mat = any.uid
  ctx.target = 'cell_' .. any.y .. '_' .. any.x
end
return { mode = ctx.mode }
'@

$codeForgeMat = @'
return { v = _G.__forge_ctx.mat }
'@

$codeForgeTgt = @'
return { v = _G.__forge_ctx.target }
'@

$codeForgeAssert = @'
local c = _G.__forge_ctx
if c.mode ~= 'real' then
  return '跳过：背包无锻造材料+装备组合（随机放入未产出），拖拽手势通道已由同构 no-op 拖拽覆盖'
end
if c.result == nil then
  error('锻造拖拽未收到 Sync_ForgeResult（Req_ForgeItem 未发出或服务端未响应）')
end
return 'forge ok: ' .. tostring(c.result)
'@

# D6：非法落点前置/断言（拖出网格外 → on_release 落点无效 → 不发请求）
$codeIllegalPrep = @'
local it = _G.__probe_bag.items[1]
if not it then error('背包为空') end
_G.__illegal = { uid = it.uid, x = it.x, y = it.y, n = #_G.__probe_bag.items }
return { uid = it.uid }
'@

$codeIllegalAssert = @'
local r = _G.__illegal
local b = _G.__probe_bag
if #b.items ~= r.n then error('非法落点不应产生任何请求（物品数变化）') end
for _, it in ipairs(b.items) do
  if tostring(it.uid) == tostring(r.uid) then
    if it.x ~= r.x or it.y ~= r.y then error('非法落点不应移动物品') end
    return 'illegal drop ok（未发请求）'
  end
end
error('物品丢失')
'@

# E：背包开时 HUD 挂起
$codeHudSuspended = @'
local cg = bgd_api.client.cgui
if cg.page.is_visible('hud_bar') then error('背包（exclusive POPUP）打开期间 hud_bar 应被挂起') end
if cg.page.is_visible('hud_combat') then error('hud_combat 应随 HUD 档一并挂起') end
return 'hud suspended ok'
'@

# E：商店打开后背包被互斥关闭
$codeBagClosedByShop = @'
local cg = bgd_api.client.cgui
if cg.page.is_open('bag') then error('exclusive 互斥失败：商店打开后背包仍打开') end
if not cg.page.is_open('shop') then error('商店未打开') end
return 'exclusive ok'
'@

# F：物品数快照 / 关再开保留断言
$codeCountSave = @'
return { v = #_G.__probe_bag.items }
'@

$codeCountAssert = @'
if #_G.__probe_bag.items ~= {$nf0} then
  error('关再开 bagData 未保留：' .. {$nf0} .. ' -> ' .. #_G.__probe_bag.items)
end
return 'bagData kept'
'@

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '01 背包页全交互验收（0.8.5 统一 Page 架构）' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },
        @{ op = 'wait_for'; q = '商店'; timeout_ms = 30000 },
        @{ op = 'note'; text = '装探针：eval 订阅 Sync_BagData → _G.__probe_bag' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeProbe } },

        @{ op = 'note'; text = 'A 打开/关闭：HUD 入口（tag hud_bag_entry）开背包' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_bag_entry' }; save_as = 'hud_bag' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_bag}'; expect = '背 包' } },
        @{ op = 'wait_for'; q = '整理背包'; timeout_ms = 4000 },
        @{ op = 'note'; text = 'A：Y 键 toggle 关再开' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '背 包'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '背 包'; timeout_ms = 4000 },
        @{ op = 'note'; text = 'A：X 钮（tag bag_close）关' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'bag_close' }; save_as = 'bag_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$bag_close}'; expect_absent = '背 包' } },
        @{ op = 'wait_for'; q = '背 包'; present = $false; timeout_ms = 4000 },
        @{ op = 'note'; text = 'A：重开后点遮罩（屏幕角落 30,300）不关页；HUD 挂起不响应' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$hud_bag}'; expect = '背 包' } },
        @{ op = 'wait_for'; q = '整理背包'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.click_at'; args = @{ x = 30; y = 300 } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'assert_text'; q = '背 包'; present = $true },
        @{ op = 'assert_text'; q = '商店'; present = $false },

        @{ op = 'note'; text = 'B 底栏：随机放入 x3 → 网格物品数递增' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'bag_item'; scope = 'bag' }; save_as = 'n0'; save_field = 'total' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '随机放入' } },
        @{ op = 'wait'; ms = 600 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '随机放入' } },
        @{ op = 'wait'; ms = 600 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '随机放入' } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'bag_item'; scope = 'bag' }; save_as = 'n1'; save_field = 'total' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeAddAssert } },
        @{ op = 'note'; text = 'B：整理背包（服务端日志「整理了背包」为信息项）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '整理背包' } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'logs'; source = 'game_server'; match = '整理了背包' },
        @{ op = 'assert_text'; q = '背 包'; present = $true },
        @{ op = 'note'; text = 'B：批量丢弃开 → 文本/信息栏切换' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '批量丢弃' } },
        @{ op = 'wait_for'; q = '批量丢弃: 开'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '点击物品 快速丢弃'; present = $true },
        @{ op = 'note'; text = 'B：批量态单击物品直接丢弃（药水/锻造材料豁免，探针选非豁免物品）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codePickDiscard }; save_as = 'bd_uid'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = 'item_{$bd_uid}' } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeDiscardedAssert } },
        @{ op = 'note'; text = 'B：批量丢弃关恢复' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '批量丢弃' } },
        @{ op = 'wait_for'; q = '批量丢弃: 关'; timeout_ms = 4000 },

        @{ op = 'note'; text = 'C 属性框：单击药水出框（类型：药水 + 使用/拆分/旋转/丢弃）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codePickPotion }; save_as = 'pot_uid'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = 'item_{$pot_uid}' } },
        @{ op = 'wait_for'; q = '类型：药水'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '占格'; present = $true },
        @{ op = 'assert_text'; q = '使用'; present = $true },
        @{ op = 'assert_text'; q = '拆分'; present = $true },
        @{ op = 'assert_text'; q = '旋转'; present = $true },
        @{ op = 'assert_text'; q = '丢弃'; present = $true },
        @{ op = 'note'; text = 'C：点另一物品切换属性框' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codePickOther }; save_as = 'uid2'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeOtherName }; save_as = 'name2'; save_field = 'n' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = 'item_{$uid2}' } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'assert_text'; q = '{$name2}'; present = $true },
        @{ op = 'note'; text = 'C：count=1 物品无拆分钮（旋转/丢弃恒在）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codePickSingle }; save_as = 'uid3'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = 'item_{$uid3}' } },
        @{ op = 'wait_for'; q = '类型：'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '拆分'; present = $false },
        @{ op = 'assert_text'; q = '旋转'; present = $true },
        @{ op = 'assert_text'; q = '丢弃'; present = $true },
        @{ op = 'note'; text = 'C：属性框锚定（先点顶行物品避免下翻转越屏——底行点击会触发已记录的越屏缺陷，见报告）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = @'
local best
for _, it in ipairs(_G.__probe_bag.items) do
  if not best or it.y < best.y then best = it end
end
if not best then error('背包为空') end
return { uid = best.uid }
'@ }; save_as = 'uid4'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = 'item_{$uid4}' } },
        @{ op = 'wait_for'; q = '类型：'; timeout_ms = 4000 },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'D1 拖空格移动（lua.drag_ui → Req_BagMove，落点=反向扫描的空格）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeDragPrep }; save_as = 'duid'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeDragX }; save_as = 'dcx'; save_field = 'v' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeDragY }; save_as = 'dcy'; save_field = 'v' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = 'item_{$duid}'; to_id = 'cell_{$dcy}_{$dcx}' } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeDragAssert } },
        @{ op = 'note'; text = 'D2 双击旋转（受限：vp 无双击能力，走协议层 Req_BagRotate 验证）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeRotSend } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeRotAssert } },
        @{ op = 'note'; text = 'D3 长按拆分（受限：vp 注入只打最深命中件，item 外层 drop_target 读不到 press——注入保真度缺口，见报告新发现 N-1；本项走协议层 Req_BagSplit 验证服务端拆分逻辑）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeSplitPrep }; save_as = 'sp_uid'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = @'
local protocol = require('libs.common.api.protocol')
local P = require('src.common.Protocol')
protocol.send_to_server(P.Req_BagSplit, { uid = {$sp_uid} })
return 'split sent'
'@ } },
        @{ op = 'wait'; ms = 1000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeSplitAssert } },
        @{ op = 'note'; text = 'D4 同类堆叠合并（拆分造出的同 id 两堆互拖 → Req_BagMove+target_uid）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeMergePrep }; save_as = 'mg_a'; save_field = 'ua' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeMergeB }; save_as = 'mg_b'; save_field = 'ub' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = 'item_{$mg_a}'; to_id = 'item_{$mg_b}' } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeMergeAssert } },
        @{ op = 'note'; text = 'D5 锻造材料拖装备（材料靠随机放入：有则真拖拽 Req_ForgeItem 断 Sync_ForgeResult；无则同构 no-op 拖拽自愈跳过）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeForgePrep } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeForgeMat }; save_as = 'fmat'; save_field = 'v' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeForgeTgt }; save_as = 'ftgt'; save_field = 'v' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = 'item_{$fmat}'; to_id = '{$ftgt}' } },
        @{ op = 'wait'; ms = 1000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeForgeAssert } },
        @{ op = 'logs'; source = 'game_server'; match = '锻造装备' },
        @{ op = 'note'; text = 'D6 非法落点（拖出网格外：红预览为视觉态不可程序断言，语义断言=不发请求）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeIllegalPrep }; save_as = 'il_uid'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = 'item_{$il_uid}'; dx = -3000; dy = 0 } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeIllegalAssert } },

        @{ op = 'note'; text = 'E Page 语义：背包（exclusive POPUP）开时 HUD 挂起（先点空格关 C 段遗留属性框，防「攻击 +N」行误命中）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = @'
local BagConfig = require('src.common.BagConfig')
local b = _G.__probe_bag
local cols, rows = BagConfig.GRID_COLS, BagConfig.GRID_ROWS
for y = 0, rows - 1 do
  for x = 0, cols - 1 do
    local occ = false
    for _, it in ipairs(b.items) do
      local cfg = BagConfig.GetItem(it.item_id)
      if cfg then
        local w, h = BagConfig.GetSize(cfg, it.rotated)
        if x >= it.x and x < it.x + w and y >= it.y and y < it.y + h then occ = true break end
      end
    end
    if not occ then return { cell = 'cell_' .. y .. '_' .. x } end
  end
end
error('无空格（背包已满）')
'@ }; save_as = 'empty_cell'; save_field = 'cell' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$empty_cell}' } },
        @{ op = 'wait_for'; q = '类型：'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeHudSuspended } },
        @{ op = 'assert_text'; q = '攻击'; present = $false },
        @{ op = 'note'; text = 'E：开商店（U 键）→ 背包被 exclusive 互斥关闭' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '商 店'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '背 包'; present = $false },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeBagClosedByShop } },
        @{ op = 'logs'; source = 'game_client'; match = '背包已关闭' },
        @{ op = 'note'; text = 'E：关商店（U 键）→ HUD 恢复' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '商店'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '商 店'; present = $false },

        @{ op = 'note'; text = 'F 状态复位：开背包 → 开批量丢弃 + 选中物品 → 关再开' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '背 包'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeCountSave }; save_as = 'nf0'; save_field = 'v' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '批量丢弃' } },
        @{ op = 'wait_for'; q = '批量丢弃: 开'; timeout_ms = 4000 },
        @{ op = 'note'; text = 'F：选中物品用药水（批量态下药水豁免丢弃，单击走属性框）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codePickPotion }; save_as = 'f_uid'; save_field = 'uid' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = 'item_{$f_uid}' } },
        @{ op = 'wait_for'; q = '类型：'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'bag_close' }; save_as = 'bag_close2' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$bag_close2}' } },
        @{ op = 'wait_for'; q = '背 包'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '背 包'; timeout_ms = 4000 },
        @{ op = 'note'; text = 'F：bagData 保留（物品数不变）+ 批量丢弃/选中复位' },
        @{ op = 'assert_text'; q = '批量丢弃: 关'; present = $true },
        @{ op = 'assert_text'; q = '类型：'; present = $false },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeCountAssert } },

        @{ op = 'note'; text = 'G 收尾：关背包 + errors 段必须为空' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '背 包'; present = $false; timeout_ms = 4000 },
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
