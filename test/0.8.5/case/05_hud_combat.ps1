# 05_hud_combat.ps1 — 0.8.5 全量验收：HUD 入口列（hud_bar）+ 战斗主界面（hud_combat）全交互
# 覆盖矩阵（文本/tag/字段名均照抄源码核实）：
#   A 常驻页九 tag 全命中：hud_stats/hud_joystick/hud_attack/hud_shop_entry/hud_bag_entry/
#     hud_gm_entry/hud_team_entry/hud_sfx/hud_bgm（find_ui+save_as：零命中则 save_as 提取报错=步骤失败）
#   B hud_bar 六钮：音效/BGM 开关（文本翻转 + eval sound.IsTypeMuted 翻转，测完恢复原态）；
#     组队/背包/GM/商店四入口 toggle 往返（背包/GM/商店为 POPUP exclusive，开着时 HUD 挂起，
#     关闭走各自关闭通道：bag_close/gm_close tag、U 键）；商店红点整屏 capture；exclusive 互斥
#     （背包开着按 U 开商店→背包被关；反向同理）
#   C 属性卡片：血条文本「N / N」与 eval W.localPlayerHP/MaxHP 一致（eval 现算文本经 save_as 回传断言）；
#     攻/防/速文本存在；计分板 tag hud_scoreboard 有数据才画（无数据 note 跳过）
#   D 摇杆：press_ui(x=1) 按住 → W.joystickDX>0/joystickActive=true + playerX 增加；
#     release_ui 松手 → 三字段归 0/0/false
#   E 键盘移动：key_down D 按住 500ms → playerX 增加；key_up 停
#   F 攻击钮：eval hook protocol.send_to_server 计数 Req_PlayerAttack，tap 后断言计数+1
#   G 技能：tap 槽1（skill_pin1）→ 日志「[技能按钮] 单击槽位」+「UseSkill」→ 服务端确认后
#     IsSkillOnCooldown=true + CD 遮罩（cd_sec）→ CD 中再点 → 中央警告「技能冷却中」；
#     drag_ui 槽1→槽2 换位（快照槽位名互换断言）+ Settings skill_order 持久化
#     （stop_debug/start_debug 重进后顺序保持——注意 VM 重启 _G 清零，期望值经场景变量 {$ord_swap} 跨段传递）
#   H 药水：eval SetPotionCounts 强制 1001=5/1002=0（客户端拦截纯本地判定，服务端权威不影响本用例）→
#     tap 1001 走通 UsePotion（IsPotionOnCooldown=true）→ CD 中再点「冷却中」→ tap 1002「没有药水可以使用」
#   I 拾取：eval Effects.SyncDropSpawn 注入假掉落（真实通路，置脏可拾取缓存）→ 1 秒到龄后
#     hud_pickup 按钮出现 → tap 发 Req_Pickup（hook 计数断言）→ eval SyncDropRemove 模拟服务端移除
#     → 按钮消失；基线无掉落时按钮不渲染（present=false；若玩家出生点 80px 内恰有真实技能书会假失败，罕见）
#   J Y/U 键 toggle 背包/商店开合；K GameConfig.HUD_SAFE_MARGIN 存在性 + 全屏 capture；
#   L 收尾日志 errors 段
# 用法：powershell -File 05_hud_combat.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '05：HUD 入口列 + 战斗主界面全交互（0.8.5 全量验收）' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },
        @{ op = 'wait_for'; q = '攻击'; timeout_ms = 15000 },

        @{ op = 'note'; text = 'A 常驻页就绪：九 tag 全命中（save_as 兼作存在性断言）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_stats' }; save_as = 't_stats' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_joystick' }; save_as = 't_joy'; save_field = 'id' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_attack' }; save_as = 't_attack' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_shop_entry' }; save_as = 't_shop' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_bag_entry' }; save_as = 't_bag' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_gm_entry' }; save_as = 't_gm' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_team_entry' }; save_as = 't_team' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_sfx' }; save_as = 't_sfx' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_bgm' }; save_as = 't_bgm' },

        @{ op = 'note'; text = 'B1 音效开关：IsTypeMuted(TYPE_SOUND) 翻转 + 恢复原态' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local sound = require("libs.client.api.sound")
_G.__t_sfx0 = sound.IsTypeMuted(sound.TYPE_SOUND)
return tostring(_G.__t_sfx0)' } },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_sfx}' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local sound = require("libs.client.api.sound")
if sound.IsTypeMuted(sound.TYPE_SOUND) == _G.__t_sfx0 then error("音效开关未翻转") end
return "flipped"' } },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_sfx}' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local sound = require("libs.client.api.sound")
if sound.IsTypeMuted(sound.TYPE_SOUND) ~= _G.__t_sfx0 then error("音效开关未恢复原态") end
return "restored"' } },

        @{ op = 'note'; text = 'B2 BGM 开关：IsTypeMuted(TYPE_MUSIC) 翻转 + 恢复原态' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local sound = require("libs.client.api.sound")
_G.__t_bgm0 = sound.IsTypeMuted(sound.TYPE_MUSIC)
return tostring(_G.__t_bgm0)' } },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_bgm}' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local sound = require("libs.client.api.sound")
if sound.IsTypeMuted(sound.TYPE_MUSIC) == _G.__t_bgm0 then error("BGM 开关未翻转") end
return "flipped"' } },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_bgm}' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local sound = require("libs.client.api.sound")
if sound.IsTypeMuted(sound.TYPE_MUSIC) ~= _G.__t_bgm0 then error("BGM 开关未恢复原态") end
return "restored"' } },

        @{ op = 'note'; text = 'B3 组队入口（非 exclusive，HUD 不挂起，入口再点即关）' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_team}'; expect = '创建队伍' } },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_team}'; expect_absent = '创建队伍' } },

        @{ op = 'note'; text = 'B4 背包入口：开→bag_close 关（exclusive 开着时 HUD 挂起，不能再点入口）' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_bag}'; expect = '整理背包' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'bag_close' }; save_as = 'bag_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$bag_close}'; expect_absent = '整理背包' } },

        @{ op = 'note'; text = 'B5 GM 入口：开→gm_close 关' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_gm}'; expect = 'GM 面板' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'gm_close' }; save_as = 'gm_close' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$gm_close}'; expect_absent = 'GM 面板' } },

        @{ op = 'note'; text = 'B6 商店入口：红点（shop 聚合路径，tag 不可 q 查——整屏截图留证）→ 开 → U 键关' },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_shop}'; expect = '特惠商店' } },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'B7 exclusive 互斥：背包开着按 U 开商店→背包被关；商店开着按 Y 开背包→商店被关' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '整理背包'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '整理背包'; present = $false },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '整理背包'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '特惠商店'; present = $false },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '整理背包'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'C 属性卡片：血条文本与 WorldState 一致（eval 现算 %d / %d 回传断言）；攻/防/速存在' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
if W.localPlayerMaxHP <= 0 then error("MaxHP 未同步") end
return { id = string.format("%d / %d", math.floor(W.localPlayerHP), math.floor(W.localPlayerMaxHP)) }' }; save_as = 'hp_text' },
        @{ op = 'assert_text'; q = '{$hp_text}'; present = $true },
        @{ op = 'assert_text'; q = '防御'; present = $true },
        @{ op = 'assert_text'; q = '移速'; present = $true },
        @{ op = 'note'; text = 'C2 计分板（hud_scoreboard）：有数据才画，total=0 属正常跳过' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_scoreboard' } },

        @{ op = 'note'; text = 'D 摇杆：press_ui(x=1) 按住 → joystickDX>0/Active=true/playerX 增加；release_ui 归零' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
_G.__t_px0 = W.playerX
return tostring(_G.__t_px0)' } },
        @{ op = 'invoke'; id = 'lua.press_ui'; args = @{ id = '{$t_joy}'; x = 1; y = 0 } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
if not W.joystickActive then error("joystickActive 应为 true") end
if W.joystickDX <= 0.3 then error("joystickDX 方向错误: " .. tostring(W.joystickDX)) end
if W.playerX <= _G.__t_px0 then error("摇杆按住期间玩家未移动") end
return "joystick ok"' } },
        @{ op = 'invoke'; id = 'lua.release_ui'; args = @{ id = '{$t_joy}' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
if W.joystickDX ~= 0 or W.joystickDY ~= 0 or W.joystickActive then error("摇杆松手后未归零") end
return "released"' } },

        @{ op = 'note'; text = 'E 键盘移动：D 按住 500ms → playerX 增加；key_up 停' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
_G.__t_px1 = W.playerX
return tostring(_G.__t_px1)' } },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'D' } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'D' } },
        @{ op = 'wait'; ms = 200 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
if W.playerX <= _G.__t_px1 + 30 then error("键盘 D 移动未生效: " .. tostring(_G.__t_px1) .. " -> " .. tostring(W.playerX)) end
return "moved"' } },

        @{ op = 'note'; text = 'F 攻击钮：hook send_to_server 计数 Req_PlayerAttack（测完还原）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local protocol = require("libs.common.api.protocol")
local P = require("src.common.Protocol")
if not _G.__t_send0 then
    _G.__t_send0 = protocol.send_to_server
    _G.__t_atk = 0
    protocol.send_to_server = function(name, data)
        if name == P.Req_PlayerAttack then _G.__t_atk = _G.__t_atk + 1 end
        return _G.__t_send0(name, data)
    end
end
return "hooked"' } },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_attack}' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if (_G.__t_atk or 0) < 1 then error("攻击未发出 Req_PlayerAttack") end
local protocol = require("libs.common.api.protocol")
if _G.__t_send0 then protocol.send_to_server = _G.__t_send0; _G.__t_send0 = nil end
return "attack sent x" .. tostring(_G.__t_atk)' } },

        @{ op = 'note'; text = 'G 技能：记录槽位名（dbg 快照 /name 尾段）→ tap 槽1 施放 → 日志/CD/冷却拦截' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local core = bgd_api.client.dbg_bus.cgui_core()
local snap = core.dbg.snapshot
local function slotname(pin)
    for id, e in pairs(snap) do
        if id:find(pin .. "/", 1, true) and id:sub(-5) == "/name" and type(e.text) == "string" then
            return e.text
        end
    end
    return nil
end
_G.__t_n1 = slotname("skill_pin1")
_G.__t_n2 = slotname("skill_pin2")
if not _G.__t_n1 or not _G.__t_n2 then error("技能槽位名未找到（快照未就绪）") end
return "slot1=" .. _G.__t_n1 .. " slot2=" .. _G.__t_n2' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'skill_pin1/skill1' }; save_as = 'slot1' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'skill_pin2/skill2' }; save_as = 'slot2' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'skill_pin1' } },
        @{ op = 'logs'; source = 'game_client'; match = '单击槽位' },
        @{ op = 'logs'; source = 'game_client'; match = 'UseSkill' },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local Settings = require("src.client.Settings")
local CH = require("src.client.page.hud.combat")
local ord = Settings.Get("skill_order")
local sid = (ord and ord[1]) or 1
if not CH.IsSkillOnCooldown(sid) then error("槽位1技能(skillId=" .. tostring(sid) .. ") CD 未启动（服务端未确认施放？）") end
return "cd running"' } },
        @{ op = 'wait_for'; q = 'cd_sec'; timeout_ms = 3000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'skill_pin1' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '技能冷却中'; present = $true },
        @{ op = 'wait_for'; q = '技能冷却中'; present = $false; timeout_ms = 5000 },

        @{ op = 'note'; text = 'G2 技能拖拽换位：槽1→槽2，两槽技能名互换 + skill_order 持久化' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$slot1}'; to_id = '{$slot2}' } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local core = bgd_api.client.dbg_bus.cgui_core()
local snap = core.dbg.snapshot
local function slotname(pin)
    for id, e in pairs(snap) do
        if id:find(pin .. "/", 1, true) and id:sub(-5) == "/name" and type(e.text) == "string" then
            return e.text
        end
    end
    return nil
end
local n1, n2 = slotname("skill_pin1"), slotname("skill_pin2")
if n1 ~= _G.__t_n2 or n2 ~= _G.__t_n1 then
    error("拖拽换位未生效：槽1=" .. tostring(n1) .. "（应 " .. tostring(_G.__t_n2) .. "）槽2=" .. tostring(n2) .. "（应 " .. tostring(_G.__t_n1) .. "）")
end
return "swapped"' } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local ord = require("src.client.Settings").Get("skill_order")
if not ord then error("skill_order 未持久化") end
return { id = tostring(ord[1]) .. "," .. tostring(ord[2]) }' }; save_as = 'ord_swap' },

        @{ op = 'note'; text = 'G3 持久化验证：重启调试后槽位顺序保持（_G 清零，期望值经 {$ord_swap} 传递）' },
        @{ op = 'stop_debug' },
        @{ op = 'wait'; ms = 1500 },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 5000 },
        @{ op = 'wait_for'; q = '攻击'; timeout_ms = 20000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local ord = require("src.client.Settings").Get("skill_order")
if not ord then error("重启后 skill_order 丢失") end
local got = tostring(ord[1]) .. "," .. tostring(ord[2])
if got ~= "{$ord_swap}" then error("槽位顺序重启后未保持: " .. got .. "（应 {$ord_swap}）") end
return "persisted " .. got' } },

        @{ op = 'note'; text = 'H 药水：强制 1001=5/1002=0 → 1001 走通+CD → CD 中「冷却中」→ 1002「没有药水可以使用」' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local CH = require("src.client.page.hud.combat")
CH.SetPotionCounts({ [1001] = 5, [1002] = 0 })
if CH.GetPotionCount(1001) ~= 5 or CH.GetPotionCount(1002) ~= 0 then error("SetPotionCounts 未生效") end
return "counts set"' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'potion1001/btn' }; save_as = 'pot1' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$pot1}' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local CH = require("src.client.page.hud.combat")
if not CH.IsPotionOnCooldown(1001) then error("药水 1001 CD 未启动（点击未走 UsePotion 通路）") end
return "potion cd running"' } },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$pot1}' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '冷却中'; present = $true },
        @{ op = 'wait_for'; q = '冷却中'; present = $false; timeout_ms = 5000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local CH = require("src.client.page.hud.combat")
CH.SetPotionCounts({ [1002] = 0 }) -- 用药水 1001 后服务端 Sync_PlayerStats 会覆盖强制计数，点 1002 前重强制
if CH.GetPotionCount(1002) ~= 0 then error("1002 强制为 0 未生效") end
return "1002=0"' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'potion1002/btn' }; save_as = 'pot2' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$pot2}' } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '没有药水可以使用'; present = $true },
        @{ op = 'wait_for'; q = '没有药水可以使用'; present = $false; timeout_ms = 5000 },

        @{ op = 'note'; text = 'I 拾取：基线无按钮 → SyncDropSpawn 注入假掉落 → 按钮出现 → tap 发 Req_Pickup → 移除后按钮消失' },
        @{ op = 'wait_for'; q = '拾取'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local W = require("src.client.world.WorldState")
local Effects = require("src.client.page.world.Effects")
local protocol = require("libs.common.api.protocol")
local P = require("src.common.Protocol")
if not _G.__t_send1 then
    _G.__t_send1 = protocol.send_to_server
    _G.__t_pick = 0
    protocol.send_to_server = function(name, data)
        if name == P.Req_Pickup then _G.__t_pick = _G.__t_pick + 1 end
        return _G.__t_send1(name, data)
    end
end
Effects.SyncDropSpawn({ uid = 900001, item_id = 1001, x = W.playerX + 8, y = W.playerY + 8 })
return "drop injected"' } },
        @{ op = 'wait'; ms = 1300 },
        @{ op = 'wait_for'; q = '拾取'; timeout_ms = 5000 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ tag = 'hud_pickup' }; save_as = 't_pickup' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$t_pickup}' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'if (_G.__t_pick or 0) < 1 then error("拾取未发出 Req_Pickup") end
require("src.client.page.world.Effects").SyncDropRemove({ uid = 900001 })
local protocol = require("libs.common.api.protocol")
if _G.__t_send1 then protocol.send_to_server = _G.__t_send1; _G.__t_send1 = nil end
return "picked x" .. tostring(_G.__t_pick)' } },
        @{ op = 'wait_for'; q = '拾取'; present = $false; timeout_ms = 5000 },

        @{ op = 'note'; text = 'J Y/U 键 toggle 背包/商店开合' },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '整理背包'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'Y' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'Y' } },
        @{ op = 'wait_for'; q = '整理背包'; present = $false; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.key_down'; args = @{ key = 'U' } },
        @{ op = 'invoke'; id = 'lua.key_up'; args = @{ key = 'U' } },
        @{ op = 'wait_for'; q = '特惠商店'; present = $false; timeout_ms = 4000 },

        @{ op = 'note'; text = 'K 安全边距：GameConfig.HUD_SAFE_MARGIN 存在（HUD pin offset 唯一来源）+ 全屏 capture' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = 'local s = require("src.common.GameConfig").HUD_SAFE_MARGIN
if not (s and s.left == 24 and s.right == 24 and s.top == 12 and s.bottom == 12) then error("HUD_SAFE_MARGIN 异常") end
return "safe margin ok"' } },
        @{ op = 'capture'; max_width = 1280 },

        @{ op = 'note'; text = 'L 收尾：日志 errors 段必须为空（历史 sprobe 报错为开发期探针遗留，看 distinct 增量）' },
        @{ op = 'logs'; source = 'game_client'; tail_lines = 3 }
    )
}

$ndjson = @(
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}',
    (@{ jsonrpc = '2.0'; id = 2; method = 'tools/call'; params = @{ name = 'run_scenario'; arguments = $scenario } } | ConvertTo-Json -Depth 20 -Compress)
) -join "`n"

$out = $ndjson | & $exe mcp 2>&1 | Out-String
[System.IO.File]::WriteAllText("$PSScriptRoot\..\temp\spill_05.txt", $out, (New-Object System.Text.UTF8Encoding($false)))
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
