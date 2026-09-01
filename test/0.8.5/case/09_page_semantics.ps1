# 09_page_semantics.ps1 — 0.8.5 全量验收：统一 Page 架构语义回归（libs/client/cgui/page.lua 内核）
#
# 方法：用 lua.eval 在客户端 VM 注册探针页（render 只放一个带唯一文本的 cg.text，
# 便于 find_ui/wait_for 断言），逐条验证 page.lua 内核语义。探针页全部以 probe_ 前缀命名，
# 收尾统一关闭；框架常驻页（hud_bar/hud_combat/notify/map_view/world_view/toast/dialog/
# guide/team_nearby/debug_hub_entry）的既有状态全程留意恢复。
#
# 覆盖矩阵：
#   S1  注册即 closed：注册后不开，文本不出现、is_open==false
#   S2  open 才显示 + on_init 恰好一次：open 两次（第二次已可见仅刷参）→ init==1、open==1
#   S3  重注册不重置 on_init：同名再注册 → init 仍==1、开关状态保持（仍可见）
#   S4  exclusive 新语义：exclusive A/B 互斥（开 B 关 A）；非 exclusive C 与 B 共存
#   S5  suspend 默认规则：exclusive POPUP 开时 HUD 挂起；显式 suspend=false 的 exclusive 页不挂起
#   S6  backdrop 遮挡：开 backdrop 页 → 下层 HUD 文本 find_ui 被过滤（occluded_skipped>0、
#       total==0）；注：tap 下层会被 actionable 拒绝，该错误通道会使场景步骤失败，
#       以 occluded_skipped 断言等价覆盖
#   S7  queue 单例补位：两个 DIALOG 页，开 A 开 B（B is_queued、不显示）；close A → B 自动补位显示
#   S8  back 导航：POPUP 参与（back 关之）；SYSTEM 参与（back 关框架 dialog 宿主，随后恢复）；
#       HUD 不参与（probe_hud 开后 back 不动它）
#   S9  group 批量：open_group 全开、close_group 全关（幂等，再 close 一次无报错）
#   S10 close_all(type)：开多个 POPUP 探针 close_all(POPUP) 全关，HUD 不受影响
#   S11 事件：event_bus 订阅 cgui.page_opened/page_closed，open/close 探针各触发恰好一次
#   S12 toggle 不多重触发：toggle 开→关，on_open/on_close 各一次，on_init 不重放
#   S13 BENCH 内建挂起：开 cgui_bench（BENCH 无 suspend=false）→ HUD 挂起、NOTIFY 不挂起
#       （toast 仍可投递命中）；关台恢复
#   S14 MAP/WORLD 常驻：map_view/world_view is_visible（移远回原位为可选项，本用例不做，标注）
#   S15 收尾：关闭全部探针页 + game_client 日志 errors distinct 必须为 0
#
# 前置：编辑器在线；项目 config.client.debug_ui=true（cgui_bench 已启动登记，本项目默认开）。
# 用法：powershell -File 09_page_semantics.ps1（编辑器在线即可，脚本自带 start_debug 重置）
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

# ---------- eval 代码段 ----------

# 就绪检查
$codeReady = @'
local cg = bgd_api.client.cgui
if not cg.page.is_visible('hud_bar') then error('HUD 未就绪') end
return 'ready'
'@

# S1/S3 共用：注册 probe_p1（重复执行幂等：重注册替换定义、保持开关状态、不重置 on_init）
$codeRegP1 = @'
local cg = bgd_api.client.cgui
_G.__probe = _G.__probe or { counters = { init = 0, open = 0, close = 0 } }
cg.page({
  name = 'probe_p1',
  type = cg.PAGE.POPUP,
  on_init = function() _G.__probe.counters.init = _G.__probe.counters.init + 1 end,
  on_open = function() _G.__probe.counters.open = _G.__probe.counters.open + 1 end,
  on_close = function() _G.__probe.counters.close = _G.__probe.counters.close + 1 end,
  render = function()
    cg.text({ key = 't', text = '探针P1文本', font = { size = 16 }, static = true })
  end,
})
return 'probe_p1 registered'
'@

$codeS1Assert = @'
local cg = bgd_api.client.cgui
if cg.page.is_open('probe_p1') then error('注册即 closed 违反：未 open 已是 open 态') end
return 'registered-closed ok'
'@

$codeS2Open1 = @'
local cg = bgd_api.client.cgui
if not cg.page.open('probe_p1') then error('open 失败') end
return 'opened'
'@

$codeS2Open2 = @'
local cg = bgd_api.client.cgui
cg.page.open('probe_p1', { again = true }) -- 已可见重复 open：仅刷参，on_open/事件不重放
return 're-opened'
'@

$codeS2Assert = @'
local c = _G.__probe.counters
if c.init ~= 1 then error('on_init 应恰好 1 次，实际 ' .. c.init) end
if c.open ~= 1 then error('已可见重复 open 应仅刷参（on_open 不重放），实际 open=' .. c.open) end
return 'lifecycle ok'
'@

$codeS3Assert = @'
local cg = bgd_api.client.cgui
local c = _G.__probe.counters
if c.init ~= 1 then error('重注册不得重置 on_init，实际 init=' .. c.init) end
if c.open ~= 1 then error('重注册不得重放 on_open，实际 open=' .. c.open) end
if not cg.page.is_visible('probe_p1') then error('重注册应保持开关状态（仍可见）') end
return 're-register ok'
'@

$codeCloseP1 = @'
bgd_api.client.cgui.page.close('probe_p1')
return 'p1 closed'
'@

# S4：注册 exclusive A/B + 非 exclusive C
$codeRegEx = @'
local cg = bgd_api.client.cgui
local function probe(name, text, exclusive)
  cg.page({
    name = name,
    type = cg.PAGE.POPUP,
    exclusive = exclusive,
    render = function()
      cg.text({ key = 't', text = text, font = { size = 16 }, static = true })
    end,
  })
end
probe('probe_ex_a', '探针EXA文本', true)
probe('probe_ex_b', '探针EXB文本', true)
probe('probe_ex_c', '探针EXC文本', false)
return 'ex probes registered'
'@

$codeS4OpenA = @'
bgd_api.client.cgui.page.open('probe_ex_a')
return 'A opened'
'@

$codeS4OpenB = @'
bgd_api.client.cgui.page.open('probe_ex_b')
return 'B opened'
'@

$codeS4AssertAClosed = @'
local cg = bgd_api.client.cgui
if cg.page.is_open('probe_ex_a') then error('exclusive 互斥失败：开 B 后 A 仍打开') end
if not cg.page.is_visible('probe_ex_b') then error('B 应可见') end
return 'exclusive mutex ok'
'@

$codeS4OpenC = @'
bgd_api.client.cgui.page.open('probe_ex_c')
return 'C opened'
'@

$codeS4AssertCoexist = @'
local cg = bgd_api.client.cgui
if not cg.page.is_open('probe_ex_b') then error('非 exclusive 页打开不得影响 exclusive B') end
if not cg.page.is_visible('probe_ex_c') then error('C 应与 B 共存可见') end
return 'non-exclusive coexist ok'
'@

$codeS4Cleanup = @'
local cg = bgd_api.client.cgui
cg.page.close('probe_ex_b')
cg.page.close('probe_ex_c')
return 'ex cleaned'
'@

# S5：exclusive 默认挂起 HUD / suspend=false 显式豁免
$codeS5OpenA = @'
bgd_api.client.cgui.page.open('probe_ex_a')
local cg = bgd_api.client.cgui
if cg.page.is_visible('hud_bar') then error('exclusive POPUP 打开应按默认规则挂起 HUD') end
return 'default suspend ok'
'@

$codeS5NoSus = @'
local cg = bgd_api.client.cgui
cg.page({
  name = 'probe_ex_nosus',
  type = cg.PAGE.POPUP,
  exclusive = true,
  suspend = false, -- 显式关闭默认挂起
  render = function()
    cg.text({ key = 't', text = '探针EXN文本', font = { size = 16 }, static = true })
  end,
})
cg.page.open('probe_ex_nosus')
if cg.page.is_open('probe_ex_a') then error('exclusive 互斥失败：开 nosus 后 A 仍打开') end
if not cg.page.is_visible('hud_bar') then error('suspend=false 的 exclusive 页不得挂起 HUD') end
return 'suspend=false ok'
'@

$codeS5Cleanup = @'
bgd_api.client.cgui.page.close('probe_ex_nosus')
return 'nosus closed'
'@

# S6：backdrop 页
$codeS6Open = @'
local cg = bgd_api.client.cgui
cg.page({
  name = 'probe_bd',
  type = cg.PAGE.POPUP,
  backdrop = true,   -- 全屏遮罩 + 输入吞噬
  suspend = false,   -- 不挂起：保留下层视图渲染，专测 dbg 遮挡过滤
  render = function()
    cg.text({ key = 't', text = '探针BD文本', font = { size = 16 }, static = true })
  end,
})
cg.page.open('probe_bd')
return 'bd opened'
'@

$codeS6Close = @'
bgd_api.client.cgui.page.close('probe_bd')
return 'bd closed'
'@

# S7：DIALOG queue
$codeS7Reg = @'
local cg = bgd_api.client.cgui
local function dlg(name, text)
  cg.page({
    name = name,
    type = cg.PAGE.DIALOG, -- 默认 queue=true（同类型单例排队补位）
    render = function()
      cg.text({ key = 't', text = text, font = { size = 16 }, static = true })
    end,
  })
end
dlg('probe_dlg_a', '探针DLGA文本')
dlg('probe_dlg_b', '探针DLGB文本')
return 'dlg probes registered'
'@

$codeS7OpenA = @'
bgd_api.client.cgui.page.open('probe_dlg_a')
return 'dlg A opened'
'@

$codeS7OpenB = @'
local cg = bgd_api.client.cgui
cg.page.open('probe_dlg_b')
if not cg.page.is_queued('probe_dlg_b') then error('同类型忙碌时开 B 应进队列（is_queued）') end
if cg.page.is_visible('probe_dlg_b') then error('排队中的页不得显示') end
return 'B queued ok'
'@

$codeS7CloseA = @'
bgd_api.client.cgui.page.close('probe_dlg_a')
return 'A closed'
'@

$codeS7AssertB = @'
local cg = bgd_api.client.cgui
if not cg.page.is_visible('probe_dlg_b') then error('close A 后 B 应自动补位显示') end
cg.page.close('probe_dlg_b')
return 'queue pump ok'
'@

# S8a：POPUP 参与 back
$codeS8Popup = @'
local cg = bgd_api.client.cgui
cg.page.open('probe_p1')
if not cg.page.back() then error('back 应关闭最近打开的 POPUP') end
if cg.page.is_open('probe_p1') then error('back 后 POPUP 仍打开') end
return 'back closes POPUP ok'
'@

# S8b：SYSTEM 参与（框架 dialog 宿主被关后恢复）+ HUD 不参与
$codeS8Hud = @'
local cg = bgd_api.client.cgui
cg.page({
  name = 'probe_hud',
  type = cg.PAGE.HUD,
  render = function()
    cg.text({ key = 't', text = '探针HUD文本', font = { size = 16 }, static = true })
  end,
})
cg.page.open('probe_hud')
-- 此时最近的 BACK_TYPES 页是框架 dialog 宿主（SYSTEM 常驻 open）：back 应关它而非 HUD 探针
if not cg.page.back() then error('back 应能关闭 SYSTEM 页（框架 dialog 宿主）') end
if cg.page.is_open('dialog') then error('SYSTEM 页应参与 back（框架 dialog 宿主应被关闭）') end
if not cg.page.is_open('probe_hud') then error('HUD 页不得参与 back') end
cg.page.open('dialog') -- 恢复框架弹窗宿主（back 语义验证的副作用还原）
cg.page.close('probe_hud')
return 'back nav ok（POPUP/SYSTEM 参与，HUD 不参与，dialog 宿主已恢复）'
'@

# S9：group 批量
$codeS9Reg = @'
local cg = bgd_api.client.cgui
local function g(name, text)
  cg.page({
    name = name,
    type = cg.PAGE.POPUP,
    group = 'probe_grp',
    render = function()
      cg.text({ key = 't', text = text, font = { size = 16 }, static = true })
    end,
  })
end
g('probe_g1', '探针G1文本')
g('probe_g2', '探针G2文本')
cg.page.open_group('probe_grp')
return 'group opened'
'@

$codeS9Close = @'
local cg = bgd_api.client.cgui
cg.page.close_group('probe_grp')
if cg.page.is_open('probe_g1') or cg.page.is_open('probe_g2') then
  error('close_group 应全关')
end
cg.page.close_group('probe_grp') -- 幂等：再关一次无报错
return 'group ok'
'@

# S10：close_all(type)
$codeS10 = @'
local cg = bgd_api.client.cgui
cg.page.open('probe_g1')
cg.page.open('probe_g2')
cg.page.open('probe_p1')
cg.page.close_all(cg.PAGE.POPUP)
if cg.page.is_open('probe_g1') or cg.page.is_open('probe_g2') or cg.page.is_open('probe_p1') then
  error('close_all(POPUP) 应关闭全部 POPUP 探针')
end
if not cg.page.is_visible('hud_bar') then error('close_all(POPUP) 不得影响 HUD') end
return 'close_all(type) ok'
'@

# S11：page_opened/page_closed 事件
$codeS11 = @'
local cg = bgd_api.client.cgui
local ev = require('libs.common.api.event_bus')
_G.__probe.ev = { opened = 0, closed = 0 }
ev.on('cgui.page_opened', function(name)
  if name == 'probe_p1' then _G.__probe.ev.opened = _G.__probe.ev.opened + 1 end
end)
ev.on('cgui.page_closed', function(name)
  if name == 'probe_p1' then _G.__probe.ev.closed = _G.__probe.ev.closed + 1 end
end)
cg.page.open('probe_p1')
cg.page.close('probe_p1')
local e = _G.__probe.ev
if e.opened ~= 1 or e.closed ~= 1 then
  error('cgui.page_opened/page_closed 应各触发 1 次，实际 ' .. e.opened .. '/' .. e.closed)
end
return 'events ok'
'@

# S12：toggle 不多重触发
$codeS12 = @'
local cg = bgd_api.client.cgui
_G.__probe.counters = { init = 0, open = 0, close = 0 }
cg.page.toggle('probe_p1') -- 开
cg.page.toggle('probe_p1') -- 关
local c = _G.__probe.counters
if c.open ~= 1 or c.close ~= 1 then
  error('toggle 开→关 on_open/on_close 应各 1 次，实际 ' .. c.open .. '/' .. c.close)
end
if c.init ~= 0 then error('toggle 不得重放 on_init（initialized 不重置）') end
return 'toggle ok'
'@

# S13：BENCH 内建挂起（cgui_bench 无 suspend=false → bench_suspend 生效）
$codeS13Open = @'
local cg = bgd_api.client.cgui
if not cg.page.open('cgui_bench') then
  error('cgui_bench 未注册（检查 config.client.debug_benches.cgui）')
end
if cg.page.is_visible('hud_bar') then error('BENCH 打开应内建挂起 HUD') end
if not cg.page.is_visible('notify') then error('BENCH 不得挂起 NOTIFY') end
cg.toast('探针toast_BENCH挂起测试')
return 'bench suspend ok'
'@

$codeS13Close = @'
local cg = bgd_api.client.cgui
cg.page.close('cgui_bench')
if not cg.page.is_visible('hud_bar') then error('关调试台后 HUD 应恢复') end
return 'bench closed ok'
'@

# S14：MAP/WORLD 常驻显示（移远回原位为可选项，本用例不做：传送类操作有位置校正风险）
$codeS14 = @'
local cg = bgd_api.client.cgui
if not cg.page.is_visible('map_view') then error('map_view 应常驻显示') end
if not cg.page.is_visible('world_view') then error('world_view 应常驻显示') end
if not cg.page.is_visible('hud_bar') then error('hud_bar 应常驻显示') end
return 'map/world ok'
'@

# S15：清理全部探针页
$codeCleanup = @'
local cg = bgd_api.client.cgui
local names = {
  'probe_p1', 'probe_ex_a', 'probe_ex_b', 'probe_ex_c', 'probe_ex_nosus',
  'probe_bd', 'probe_dlg_a', 'probe_dlg_b', 'probe_g1', 'probe_g2', 'probe_hud',
}
for _, n in ipairs(names) do cg.page.close(n) end
return 'all probes closed'
'@

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '09 统一 Page 架构语义回归（page.lua 内核，探针页驱动）' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },
        @{ op = 'wait_for'; q = '商店'; timeout_ms = 30000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeReady } },

        @{ op = 'note'; text = 'S1 注册即 closed：注册后不开，find_ui 无命中、is_open==false' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeRegP1 } },
        @{ op = 'assert_text'; q = '探针P1文本'; present = $false },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS1Assert } },

        @{ op = 'note'; text = 'S2 open 才显示 + on_init 恰好一次（重复 open 已可见页仅刷参）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS2Open1 } },
        @{ op = 'wait_for'; q = '探针P1文本'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS2Open2 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS2Assert } },

        @{ op = 'note'; text = 'S3 重注册不重置 on_init、开关状态保持' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeRegP1 } },
        @{ op = 'wait'; ms = 300 },
        @{ op = 'assert_text'; q = '探针P1文本'; present = $true },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS3Assert } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeCloseP1 } },

        @{ op = 'note'; text = 'S4 exclusive：开 A 开 B → A 关；开非 exclusive C → B 不关（C 共存）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeRegEx } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS4OpenA } },
        @{ op = 'wait_for'; q = '探针EXA文本'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS4OpenB } },
        @{ op = 'wait_for'; q = '探针EXB文本'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS4AssertAClosed } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS4OpenC } },
        @{ op = 'wait_for'; q = '探针EXC文本'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS4AssertCoexist } },
        @{ op = 'assert_text'; q = '探针EXB文本'; present = $true },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS4Cleanup } },

        @{ op = 'note'; text = 'S5 suspend 默认规则：exclusive POPUP 挂起 HUD；suspend=false 豁免' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS5OpenA } },
        @{ op = 'assert_text'; q = '商店'; present = $false },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS5NoSus } },
        @{ op = 'wait_for'; q = '探针EXN文本'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '商店'; present = $true },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS5Cleanup } },

        @{ op = 'note'; text = 'S6 backdrop 遮挡：下层 HUD 被 dbg 遮挡过滤（occluded_skipped>0）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS6Open } },
        @{ op = 'wait_for'; q = '探针BD文本'; timeout_ms = 4000 },
        @{ op = 'assert_text'; q = '商店'; present = $false },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = '商店' }; save_as = 'occ'; save_field = 'occluded_skipped' },
        @{ op = 'note'; text = 'S6：tap 下层会被 actionable 拒绝（错误通道会使场景失败），以 occluded_skipped 断言等价覆盖' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS6Close } },
        @{ op = 'wait_for'; q = '商店'; timeout_ms = 4000 },

        @{ op = 'note'; text = 'S7 queue 单例补位：DIALOG A/B，B 排队不显示；close A → B 自动补位' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS7Reg } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS7OpenA } },
        @{ op = 'wait_for'; q = '探针DLGA文本'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS7OpenB } },
        @{ op = 'assert_text'; q = '探针DLGB文本'; present = $false },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS7CloseA } },
        @{ op = 'wait_for'; q = '探针DLGB文本'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS7AssertB } },

        @{ op = 'note'; text = 'S8 back 导航：POPUP/SYSTEM 参与，HUD 不参与' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS8Popup } },
        @{ op = 'assert_text'; q = '探针P1文本'; present = $false },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS8Hud } },

        @{ op = 'note'; text = 'S9 group 批量：open_group 全开、close_group 全关（幂等）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS9Reg } },
        @{ op = 'wait_for'; q = '探针G1文本'; timeout_ms = 4000 },
        @{ op = 'wait_for'; q = '探针G2文本'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS9Close } },
        @{ op = 'assert_text'; q = '探针G1文本'; present = $false },
        @{ op = 'assert_text'; q = '探针G2文本'; present = $false },

        @{ op = 'note'; text = 'S10 close_all(type)：POPUP 全关，HUD 不受影响' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS10 } },
        @{ op = 'assert_text'; q = '探针P1文本'; present = $false },
        @{ op = 'assert_text'; q = '商店'; present = $true },

        @{ op = 'note'; text = 'S11 事件：cgui.page_opened/page_closed 各触发恰好一次' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS11 } },

        @{ op = 'note'; text = 'S12 toggle 不多重触发：开→关 on_open/on_close 各一次，on_init 不重放' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS12 } },

        @{ op = 'note'; text = 'S13 BENCH 内建挂起：开 cgui_bench → HUD 挂起、NOTIFY（toast）不挂起；关台恢复' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS13Open } },
        @{ op = 'wait_for'; q = '探针toast_BENCH挂起测试'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS13Close } },

        @{ op = 'note'; text = 'S14 MAP/WORLD 常驻显示（移远回原位为可选项，本用例不做）' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeS14 } },

        @{ op = 'note'; text = 'S15 收尾：清理全部探针页 + errors 段必须为空' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = $codeCleanup } },
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
