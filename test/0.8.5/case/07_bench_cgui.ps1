# 0.8.5 全量验收 · 用例 07：CGUI 调试台（cgui_bench）全标签页验收
# 覆盖矩阵：
#   开台   debug_hub_entry「调试」→ debug_hub 面板 →「打开 CGUI 调试台」
#   内置组件  bi_type 切 progress / width 输入改 rect / 监听 on_real_click + tap 预览出日志
#   扩展组件  Widget 注册表计数 / window 开关+inner 点击日志 / popup 开关 / drag&drop 放置 / tileset col,row
#   游戏件   badge/countdown/currency_bar/cooldown/toast/confirm/form_row/vlist/radio_group/
#           red_dot/empty_state/paginator/joystick/number_roll/tree/item_tooltip/vgrid/pscroll/
#           扩展交互(单击/长按)/drag_ghost/sortable_list
#   Tileset  flip_x / scale 2.00x / 整页截图
#   样式编辑  comp_button 预览 / 高度滑杆 / 色板 RGBA / 新增字段+重复告警 / 复制 / 重置全部
#   布局演练  tabs 横排 / lay_row space_between / pin 九宫 tr / pin_dx 滑杆
#   动画演练  scale 播放终值 / 循环 / 换缓动
#   诊断   page.list() 页行 / diag_probe 输入计数 / 双探针截图 / dup_ids 记录
#   关台   关闭调试台 → CGUI 调试台文本消失 + HUD 恢复
# 用法：powershell -File 07_bench_cgui.ps1（编辑器在线即可，脚本自带 start_debug 重置）
# 受限项（脚本内 note 标注）：
#   * 注入点击沿为 on_real_click（vp/state 注入语义），事件日志断言用 '#1 on_real_click'
#   * 游戏件页主体在引擎 scroll（kit_scroll）内，程序滚动不可控（pscroll 文档钉死），
#     翻页用 vp 拖拽（drag_ui dy<0）；vlist 滚到底依赖引擎手势速度，未到底属预期（截图留证）
#   * 扩展交互的双击/右键无注入能力，仅单击/长按可验
#   * 编辑器属性区为引擎 scroll，监听开关/组件属性行可能被裁出视口：
#     步骤先折叠上方分组（tap「通用属性」等组头）再点目标行
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$exe = "D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$proj = "c:\Users\woaye\Documents\SCE Projects\test_res002"

$scenario = @{
    project_path = $proj
    steps = @(
        @{ op = 'note'; text = '用例 07：CGUI 调试台全标签页验收（0.8.5 统一 Page 架构）' },
        @{ op = 'start_debug' },
        @{ op = 'wait'; ms = 4000 },

        # ---------- 开台 ----------
        @{ op = 'note'; text = '开台：HUD「调试」（debug_hub_entry）→ 面板 → 打开 CGUI 调试台' },
        @{ op = 'wait_for'; q = '调试'; timeout_ms = 8000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '调试' } },
        @{ op = 'assert_text'; q = '调试台'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '打开 CGUI 调试台'; expect = '内置组件' } },
        @{ op = 'assert_text'; q = 'CGUI 调试台'; present = $true },

        # ---------- 内置组件 ----------
        @{ op = 'note'; text = '内置组件页（默认页）：bi_type 切 progress → 预览标题变化' },
        @{ op = 'assert_text'; q = '容器类型'; present = $true },
        @{ op = 'invoke'; id = 'lua.pick'; args = @{ q = 'bi_type'; item = 'progress' } },
        @{ op = 'assert_text'; q = '预览（progress）'; present = $true },
        @{ op = 'note'; text = '折叠「通用属性」组腾出属性区视口，改 width=333 → eval 校 bi_ctrl rect.w' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '通用属性' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'bi_width' }; save_as = 'biw' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$biw}'; text = '333' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local core = require('libs.client.cgui.core')`nfor id, e in pairs(core.dbg.snapshot) do`n  if id:find('bi_ctrl') and e.rect then`n    if math.abs(e.rect.w - 333) > 2 then error('bi_ctrl 宽未随输入变化：' .. tostring(e.rect.w)) end`n    return 'width 生效 rect.w=' .. tostring(e.rect.w)`n  end`nend`nerror('快照未找到 bi_ctrl')" } },
        @{ op = 'note'; text = '折叠 组件属性/排版/事件 三组 → 勾选 on_real_click 监听 → tap 预览块 → 日志盒出记录（注入沿=on_real_click）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '组件属性' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '排版' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'grp_event' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'listen_on_real_click' }; save_as = 'lrc' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$lrc}' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'bi_ctrl' } },
        @{ op = 'assert_text'; q = '#1 on_real_click'; present = $true },

        # ---------- 扩展组件 ----------
        @{ op = 'note'; text = '扩展组件页：Widget 注册表（框架件+游戏件同表计数）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '扩展组件' } },
        @{ op = 'assert_text'; q = 'Widget 注册表（共'; present = $true },
        @{ op = 'note'; text = 'window：折叠 通用属性/排版 → 打开窗口 → 勾 on_click 监听 → inner 点击日志 → ✕ 关闭' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '通用属性' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '排版' } },
        @{ op = 'invoke'; id = 'lua.pick'; args = @{ q = 'wc_type'; item = '浮动窗口 window' } },
        @{ op = 'assert_text'; q = '预览（浮动窗口 window）'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '打开窗口'; expect = '演示窗口' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '组件属性' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'grp_event' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'listen_on_click' }; save_as = 'loc' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$loc}' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '里面也能点' } },
        @{ op = 'assert_text'; q = 'on_click = inner'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '✕' } },
        @{ op = 'wait_for'; q = '演示窗口'; present = $false; timeout_ms = 3000 },
        @{ op = 'note'; text = 'popup：打开模态浮层 → 关闭按钮（id 定位防「关闭」歧义）' },
        @{ op = 'invoke'; id = 'lua.pick'; args = @{ q = 'wc_type'; item = '模态浮层 popup' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '打开模态浮层'; expect = '点击遮罩或按钮关闭' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'wc_popup_close' } },
        @{ op = 'wait_for'; q = '点击遮罩或按钮关闭'; present = $false; timeout_ms = 3000 },
        @{ op = 'note'; text = '拖放 drag/drop：拖「物品A x3」到右盒 → 占位文本消失、目标显示物品名' },
        @{ op = 'invoke'; id = 'lua.pick'; args = @{ q = 'wc_type'; item = '拖放 drag/drop' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'wc_drag' }; save_as = 'wdr' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'wc_drop' }; save_as = 'wdp' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$wdr}'; to_id = '{$wdp}' } },
        @{ op = 'wait_for'; q = '（拖左边方块到右边）'; present = $false; timeout_ms = 3000 },
        @{ op = 'assert_text'; q = '物品A x3'; present = $true },
        @{ op = 'note'; text = 'tileset：重新展开组件属性组，改 col=0/row=1 → 坐标文本更新' },
        @{ op = 'invoke'; id = 'lua.pick'; args = @{ q = 'wc_type'; item = '图集图块 tileset' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '组件属性' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'wc_col' }; save_as = 'wcol' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$wcol}'; text = '0' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'wc_row' }; save_as = 'wrow' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$wrow}'; text = '1' } },
        @{ op = 'assert_text'; q = '(0, 1) 1x'; present = $true },

        # ---------- 游戏件 ----------
        @{ op = 'note'; text = '游戏件页：主体在引擎 scroll（kit_scroll）内——程序滚动不可控，翻页用 vp 拖拽（drag_ui dy<0）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '游戏件' } },
        @{ op = 'assert_text'; q = 'badge 红点/角标'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '商店' } },
        @{ op = 'assert_text'; q = '点了商店'; present = $true },
        @{ op = 'note'; text = 'countdown：开始 8 秒 → 0:08 出现 → 2 秒后递减（0:08 消失）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '开始 8 秒倒计时' } },
        @{ op = 'assert_text'; q = '0:08'; present = $true },
        @{ op = 'wait'; ms = 2000 },
        @{ op = 'assert_text'; q = '0:08'; present = $false },
        @{ op = 'assert_text'; q = '0:0'; present = $true },
        @{ op = 'note'; text = 'currency_bar 显示 12,345' },
        @{ op = 'assert_text'; q = '12,345'; present = $true },
        @{ op = 'note'; text = 'cooldown：点「技」→ CD 遮罩（截图留证）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '技' } },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'note'; text = 'toast 两钮' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '弹 toast' } },
        @{ op = 'assert_text'; q = '操作成功'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '弹警示 toast' } },
        @{ op = 'assert_text'; q = '金币不足'; present = $true },
        @{ op = 'note'; text = 'confirm 组件：打开确认框 → 确定 → toast 已确认' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '打开确认框'; expect = '确认操作' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '确定' } },
        @{ op = 'assert_text'; q = '已确认'; present = $true },
        @{ op = 'note'; text = 'form_row：音量 slider set_value 80（生效值在 invoke result.actual）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_vol' }; save_as = 'kvol' },
        @{ op = 'invoke'; id = 'lua.set_value'; args = @{ id = '{$kvol}'; value = 80 } },
        @{ op = 'note'; text = 'vp 拖拽翻页到 vlist/radio_group（受限：引擎 scroll 手势滚动，拖拽量≠精确滚动量）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_toast1' }; save_as = 'scr' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$scr}'; dx = 0; dy = -600 } },
        @{ op = 'wait_for'; q = 'vlist 虚拟列表'; timeout_ms = 4000 },
        @{ op = 'note'; text = 'vlist：从列表行大行程拖拽尝试滚到底（受限：滚动量依赖引擎手势速度，未到底属预期，find+截图留证不硬断言）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = '列表项 1' }; save_as = 'vl1' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$vl1}'; dx = 0; dy = -2000 } },
        @{ op = 'wait'; ms = 500 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = '列表项 500' } },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '高画质' } },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'note'; text = '翻页到 red_dot/empty_state' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_rg' }; save_as = 'krg' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$krg}'; dx = 0; dy = -600 } },
        @{ op = 'wait_for'; q = 'red_dot 红点'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '+1' } },
        @{ op = 'assert_text'; q = '聚合计数：1'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '清空' } },
        @{ op = 'assert_text'; q = '聚合计数：0'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '领取补给' } },
        @{ op = 'assert_text'; q = '已领取补给'; present = $true },
        @{ op = 'note'; text = '翻页到 paginator/joystick' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_rd_add' }; save_as = 'krd' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$krd}'; dx = 0; dy = -600 } },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$krd}'; dx = 0; dy = -600 } },
        @{ op = 'wait_for'; q = 'paginator 分页器'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_pag/2' }; save_as = 'kp2' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$kp2}' } },
        @{ op = 'assert_text'; q = '条目 4'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_joy' }; save_as = 'kjoy' },
        @{ op = 'invoke'; id = 'lua.press_ui'; args = @{ id = '{$kjoy}'; x = 0.6; y = 0 } },
        @{ op = 'assert_text'; q = '按住中'; present = $true },
        @{ op = 'invoke'; id = 'lua.release_ui'; args = @{ id = '{$kjoy}' } },
        @{ op = 'assert_text'; q = '已松开'; present = $true },
        @{ op = 'note'; text = '翻页到 number_roll' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_pag' }; save_as = 'kpag' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$kpag}'; dx = 0; dy = -600 } },
        @{ op = 'wait_for'; q = 'number_roll 数字滚动'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '+ 1000' } },
        @{ op = 'wait'; ms = 1200 },
        @{ op = 'assert_text'; q = '13345'; present = $true },
        @{ op = 'note'; text = '翻页到 tree/item_tooltip' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_roll_add' }; save_as = 'kra' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$kra}'; dx = 0; dy = -600 } },
        @{ op = 'wait_for'; q = 'tree 树形控件'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '英雄' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '战士' } },
        @{ op = 'assert_text'; q = '选中叶子：战士'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_tip_cell1' }; save_as = 'ktc1' },
        @{ op = 'invoke'; id = 'lua.hover_ui'; args = @{ id = '{$ktc1}' } },
        @{ op = 'assert_text'; q = '攻击 +120'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_tip_cell2' }; save_as = 'ktc2' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$ktc2}' } },
        @{ op = 'assert_text'; q = '龙血宝石'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '使用' } },
        @{ op = 'assert_text'; q = '使用了龙血宝石'; present = $true },
        @{ op = 'note'; text = '翻页到 vgrid/pscroll' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_tip_cell1' }; save_as = 'ksc2' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$ksc2}'; dx = 0; dy = -600 } },
        @{ op = 'wait_for'; q = 'vgrid 虚拟网格'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_vgrid/g1' }; save_as = 'kg1' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$kg1}' } },
        @{ op = 'assert_text'; q = '点了格子 g1'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '滚到底部' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'assert_text'; q = '当前偏移：0'; present = $false },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '回顶部' } },
        @{ op = 'wait'; ms = 400 },
        @{ op = 'assert_text'; q = '当前偏移：0'; present = $true },
        @{ op = 'note'; text = '翻页到扩展交互（双击/右键无注入能力=受限，仅验单击/长按）' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_psc_end' }; save_as = 'kpe' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$kpe}'; dx = 0; dy = -400 } },
        @{ op = 'wait_for'; q = '扩展交互（双击/右键/长按）'; timeout_ms = 4000 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '点我：单击' } },
        @{ op = 'assert_text'; q = '最近触发：on_click 单击'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_gesture_btn' }; save_as = 'kgb' },
        @{ op = 'invoke'; id = 'lua.long_press_ui'; args = @{ id = '{$kgb}' } },
        @{ op = 'assert_text'; q = '最近触发：on_long_press 长按'; present = $true },
        @{ op = 'note'; text = '页尾固定区 drag_ghost：拖物品格到放置区 → 内联文本+游戏日志双验证' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_ghost_src' }; save_as = 'kgs' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'kit_ghost_dst' }; save_as = 'kgd' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$kgs}'; to_id = '{$kgd}' } },
        @{ op = 'assert_text'; q = '已放置 ×1（宝石）'; present = $true },
        @{ op = 'logs'; source = 'game_client'; tail_lines = 30; match = 'drop_target 收到放置' },
        @{ op = 'note'; text = '页尾固定区 sortable_list：拖第 1 行到第 3 行 → 最近重排文本' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'row1' }; save_as = 'sr1' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'row3' }; save_as = 'sr3' },
        @{ op = 'invoke'; id = 'lua.drag_ui'; args = @{ from_id = '{$sr1}'; to_id = '{$sr3}' } },
        @{ op = 'assert_text'; q = '最近重排：on_move(row1, row3)'; present = $true },

        # ---------- Tileset 图集 ----------
        @{ op = 'note'; text = 'Tileset 图集页：flip_x 翻转 + scale 拖到 2 → 2.00x（视觉验收截图留证）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'Tileset 图集' } },
        @{ op = 'assert_text'; q = '无缝地图还原'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'ts_flip_x' }; save_as = 'tfx' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$tfx}' } },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'ts_scale' }; save_as = 'tsc' },
        @{ op = 'invoke'; id = 'lua.set_value'; args = @{ id = '{$tsc}'; value = 2 } },
        @{ op = 'assert_text'; q = '2.00x'; present = $true },
        @{ op = 'capture'; max_width = 1280 },

        # ---------- 样式编辑 ----------
        @{ op = 'note'; text = '样式编辑页：comp_button → 预览普通按钮；高度滑杆改预览（截图）；色板出 RGBA 滑杆' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '样式编辑' } },
        @{ op = 'assert_text'; q = '左边选组件'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'comp_button' } },
        @{ op = 'assert_text'; q = '普通按钮'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'n_button_layout.height' }; save_as = 'sbh' },
        @{ op = 'invoke'; id = 'lua.set_value'; args = @{ id = '{$sbh}'; value = 120 } },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'c_button_color_sw' }; save_as = 'scw' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$scw}' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'c_button_color_R' } },
        @{ op = 'note'; text = '新增字段 image → toast 已添加；重复添加 → warn 字段已存在；复制本部件；重置全部' },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'newf_path' }; save_as = 'nfp' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$nfp}'; text = 'image' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '添加' } },
        @{ op = 'assert_text'; q = '已添加 button.image'; present = $true },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$nfp}'; text = 'image' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '添加' } },
        @{ op = 'assert_text'; q = '字段已存在：button.image'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '复制本部件' } },
        @{ op = 'assert_text'; q = '已复制到剪贴板'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '重置全部' } },

        # ---------- 布局演练 ----------
        @{ op = 'note'; text = '布局演练页：tabs 横排 / lay_row space_between（截图）/ pin 九宫 tr / pin_dx 滑杆' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '布局演练' } },
        @{ op = 'assert_text'; q = '锚点探针'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '横排' } },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.pick'; args = @{ q = 'lay_row'; item = 'space_between' } },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'pin_a_tr' } },
        @{ op = 'assert_text'; q = '当前：anchor=tr'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'pin_dx' }; save_as = 'pdx' },
        @{ op = 'invoke'; id = 'lua.set_value'; args = @{ id = '{$pdx}'; value = 30 } },
        @{ op = 'assert_text'; q = 'dx 30'; present = $true },

        # ---------- 动画演练 ----------
        @{ op = 'note'; text = '动画演练页：scale 场景播放 → 终值 缩放 x2.00；开循环再播放（截图）；换缓动再播放（id 点击防文本翻转歧义）' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '动画演练' } },
        @{ op = 'assert_text'; q = '引擎 transition'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = 'scale 缩放' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '播放' } },
        @{ op = 'wait'; ms = 1200 },
        @{ op = 'assert_text'; q = '缩放 x2.00'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'an_loop' }; save_as = 'alp' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$alp}' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '回退' } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.pick'; args = @{ q = 'an_ease'; item = 'out_elastic' } },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'an_play_scale' }; save_as = 'aps' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$aps}' } },
        @{ op = 'wait'; ms = 800 },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'note'; text = '收尾：关循环，防带出不归位动画' },
        @{ op = 'invoke'; id = 'lua.click_ui'; args = @{ id = '{$alp}' } },

        # ---------- 诊断 ----------
        @{ op = 'note'; text = '诊断页：page.list() 区块 + 页行（toast/cgui_bench）；diag_probe 输入计数；双探针截图；dup_ids 记录' },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '诊断' } },
        @{ op = 'assert_text'; q = '已注册 Page（page.list()）'; present = $true },
        @{ op = 'assert_text'; q = 'cgui_bench'; present = $true },
        @{ op = 'assert_text'; q = 'toast'; present = $true },
        @{ op = 'invoke'; id = 'lua.find_ui'; args = @{ q = 'diag_probe' }; save_as = 'dpr' },
        @{ op = 'invoke'; id = 'lua.input_text'; args = @{ id = '{$dpr}'; text = 'x' } },
        @{ op = 'assert_text'; q = 'on_input次数=1'; present = $true },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '校准标记 红块@(100,100)' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '鼠标跟随 红=本帧' } },
        @{ op = 'capture'; max_width = 1280 },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '校准标记 红块@(100,100)' } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '鼠标跟随 红=本帧' } },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local core = require('libs.client.cgui.core')`nlocal st = core.stats()`nlocal msg = '重复 id 告警=' .. tostring(st.dup_ids)`nlog.info('[REVIEW] ' .. msg)`nif st.dup_ids > 0 then return msg .. '（非 0，记录问题）' end`nreturn msg" } },

        # ---------- 关台 ----------
        @{ op = 'note'; text = '关台：先 eval 关 debug_hub 面板（被压在调试台下不可点，且其按钮文本含「CGUI 调试台」会干扰消失断言），再 tap menu_close' },
        @{ op = 'invoke'; id = 'lua.eval'; args = @{ code = "local cg = bgd_api.client.cgui`ncg.page.close('debug_hub')`nreturn 'hub closed'" } },
        @{ op = 'invoke'; id = 'lua.tap'; args = @{ q = '关闭调试台' } },
        @{ op = 'assert_text'; q = 'CGUI 调试台'; present = $false },
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
    [Console]::WriteLine("PARSE FAIL (likely 32KB truncation): $($_.Exception.Message)")
    $blob = $out + "`n" + $_.Exception.Message
    $okCount = ([regex]::Matches($blob, '"ok":\s*true')).Count
    $errMatches = [regex]::Matches($blob, '"ok":\s*false,\s*"error":\s*"([^"]*)"')
    [Console]::WriteLine(("fallback: ok={0} err={1}" -f $okCount, $errMatches.Count))
    foreach ($em in $errMatches) { [Console]::WriteLine('  ERR :: ' + $em.Groups[1].Value) }
    $mf = [regex]::Match($out, '"failed_step":\s*(\S+?)[,\s]')
    if ($mf.Success) { [Console]::WriteLine('failed_step: ' + $mf.Groups[1].Value) }
}
