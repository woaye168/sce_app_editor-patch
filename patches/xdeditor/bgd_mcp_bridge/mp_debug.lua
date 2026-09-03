-- bgd_mcp_bridge / mp_debug：0.8.7 多人调试适配（自 main.lua 拆出，单文件职责纪律）
-- 职责：
--   1. mp_start   多人拉起（纯 Lua 直调 menu_bar.debug_save_as，已 spike 验证；执行序见下）
--   2. mp_switch  切焦（switch_page + set_game_ui_focus，capture 截图内部编排用）
--   3. set_pause  玩家暂停/恢复（disconnect/reconnect_game_in_editor，官方 tab 暂停按钮同款）
--   4. 归属映射表 + 玩家号解析（get_status.clients / ui_loop target 寻址 / 视口选择共用）
-- （分玩家日志 tee 已拆到 mp_log_tee.lua，单文件 500 行职责纪律）
-- 纪律（前科实证）：
--   - 无图不碰 DI：menu_bar/obj_manager 一律在用的时候才取（load_map_done 后），模块加载期零触碰
--   - 不 require 'ui.menu_bar'：用 ctx.get_menu_bar()（main.lua 延迟初始化后持有），
--     obj_manager/ini 等补丁内 require 正常（本模块有 xdeditor 包身份）
--   - is_plugin_ui_loaded 对未注册槽位抛错：枚举一律 pcall（true=在线/false=已卸载/nil=未注册）
-- 机制底档：doc/research/multi-player-debug.md
--
-- 寻址关键实测（2026-09-03 回归实证，修正 dev/研究文档的错误结论）：
--   base.local_player():get_slot_id() 返回的是 PIE 槽位序号（GamePlayInEditor<N> 的 N），
--   不是数编玩家号——players 数组形态（玩家 2→槽位 1）下 eval 日志交叉归属暴露。
--   因此 dbg 协议内部一律用「槽位序号」寻址/配对，数编玩家号只在桥边界经归属映射翻译。

local M = {}

local MAX_SLOTS = 4
-- 官方 tab 图标取色（menu_bar.lua:62-68 局部值，复刻保持一致）
local ICON_COLORS = { '#1BB05D', '#F4CA34', '#339CFE', '#F7649C' }

-- ===== 模块级状态（编辑器 VM 会话级） =====
local ownership = nil      -- 归属映射表 { [player] = { slot_id, user_id, icon_color, paused } }

local function slot_name(i)
    return 'GamePlayInEditor' .. tostring(i)
end

---槽位在线探测（pcall 防御：未注册槽位抛错得 nil）
---@return true|false|nil true=在线 false=已卸载 nil=未注册/不可用
local function slot_online(pluginMgr, slot_id)
    local ok, loaded = pcall(function()
        return pluginMgr:is_plugin_ui_loaded(slot_id)
    end)
    if not ok then
        return nil
    end
    return loaded and true or false
end

---任一调试槽位在线（单人 GamePlayInEditor 或多人 GamePlayInEditor1..4）
function M.any_debugging(ctx)
    local pluginMgr = ctx.get_plugin_mgr()
    if not pluginMgr then
        return false
    end
    if slot_online(pluginMgr, 'GamePlayInEditor') == true then
        return true
    end
    for i = 1, MAX_SLOTS do
        if slot_online(pluginMgr, slot_name(i)) == true then
            return true
        end
    end
    return false
end

---有效归属映射（在线过滤）：mp_start 建立的映射 + 官方对话框手动拉起的兜底合成
---（手动拉起无映射时按「槽位序号 = 玩家号」假设合成，文档化的近似；mp_start 路径精确）。
---@return table|nil { [player] = {slot_id, user_id?, icon_color?, paused?} }（nil = 非多人局）
function M.effective_ownership(ctx)
    local pluginMgr = ctx.get_plugin_mgr()
    if not pluginMgr then
        return nil
    end
    local out = nil
    if ownership then
        for p, e in pairs(ownership) do
            if slot_online(pluginMgr, e.slot_id) == true then
                out = out or {}
                out[p] = e
            end
        end
    end
    if not out then
        for i = 1, MAX_SLOTS do
            if slot_online(pluginMgr, slot_name(i)) == true then
                out = out or {}
                out[i] = { slot_id = slot_name(i) }
            end
        end
    end
    return out
end

---get_status 的 clients 数组（同步槽位枚举 + 归属映射，不做广播收集）
function M.clients(ctx)
    local arr = {}
    local own = M.effective_ownership(ctx)
    if own then
        local ps = {}
        for p in pairs(own) do
            ps[#ps + 1] = p
        end
        table.sort(ps)
        for _, p in ipairs(ps) do
            local e = own[p]
            arr[#arr + 1] = {
                player = p,
                slot_id = e.slot_id,
                user_id = e.user_id,
                online = true,
                paused = e.paused or false,
            }
        end
        return arr
    end
    local pluginMgr = ctx.get_plugin_mgr()
    if pluginMgr and slot_online(pluginMgr, 'GamePlayInEditor') == true then
        arr[1] = { player = 1, slot_id = 'GamePlayInEditor', online = true, paused = false }
    end
    return arr
end

---玩家号解析（ui_loop 定向用）：多人局 → target=玩家号；单人局 → nil + 告知
---@return number|nil target, string|nil note, string|nil err
function M.resolve_player(ctx, player)
    local own = M.effective_ownership(ctx)
    if own then
        local p = player or 1 -- 缺省 = 多人局 1 号玩家（确定性缺省，用户拍板）
        if type(p) ~= 'number' then
            return nil, nil, 'player 须为数字（1~4）'
        end
        if not own[p] then
            local list = {}
            for k in pairs(own) do
                list[#list + 1] = tostring(k)
            end
            table.sort(list)
            return nil, nil, ('玩家 %s 不在线（当前在线：%s，get_status 可查）'):format(
                tostring(p), table.concat(list, ','))
        end
        return p, nil, nil
    end
    if player ~= nil then
        return nil, '当前为单人调试，player 参数已忽略；如需多人，请 start_debug{players=N}', nil
    end
    return nil, nil, nil
end

---玩家号 → dbg 协议 target（PIE 槽位序号）。槽位序号 ≠ 数编玩家号（见文件头实测结论）：
---dbg_bus 游戏侧 get_slot_id() 拿到的是槽位序号，协议寻址/回执配对必须用槽位序号，
---玩家号在此完成边界翻译（经归属映射表 slot_id 尾号）。
---@return number|nil slot_index（nil=单人局不带 target）, number|nil player, string|nil note, string|nil err
function M.dbg_target(ctx, player)
    local p, note, err = M.resolve_player(ctx, player)
    if err then
        return nil, nil, nil, err
    end
    if p == nil then
        return nil, nil, note, nil -- 单人局（含带 player 回退）：不带 target
    end
    local own = M.effective_ownership(ctx)
    local slot_id = own and own[p] and own[p].slot_id or nil
    local idx = slot_id and tonumber(slot_id:match('^GamePlayInEditor(%d+)$')) or nil
    if idx == nil then
        return nil, nil, nil, ('玩家 %d 的槽位序号解析失败（slot_id=%s）'):format(p, tostring(slot_id))
    end
    return idx, p, note, nil
end

---玩家号 → PIE 槽位 id（get_game_view_rect 视口选择用）
---@return string|nil slot_id（nil=单人无序号槽位）, string|nil note, string|nil err, number|nil player
function M.slot_for_player(ctx, player)
    local target, note, err = M.resolve_player(ctx, player)
    if err then
        return nil, nil, err
    end
    if target then
        local own = M.effective_ownership(ctx)
        return own[target].slot_id, nil, nil, target
    end
    return nil, note, nil, nil
end

-- ===== 预校验辅助 =====

---read_map_type 等价实现（menu_bar.lua:1674 局部函数不可达，逐行对齐自持）
local function read_map_type()
    local ok, fm = pcall(require, 'ini.file_manager')
    if not ok or type(fm) ~= 'table' or type(fm.get_file_info) ~= 'function' then
        return nil, 'ini.file_manager 不可用'
    end
    local file_info = fm.get_file_info('config.ini')
    if file_info and file_info.ini_old_table and file_info.ini_old_table['map'] then
        return file_info.ini_old_table['map']['map_type'] or ''
    end
    return ''
end

---数编读取 + 官方同款 UserId 分配（pairs(player_setting) 遍历序，:1765 同款）
---@return table|nil ctx_out { player_setting, opened_slots, user_ids, player_map, candidates }
local function read_multi_config()
    local ok, obj_manager = pcall(require, 'plugin.obj_editor_ui.manager.init')
    if not ok or type(obj_manager) ~= 'table' then
        return nil, 'obj_manager 不可用（地图未完全加载？）'
    end
    local getv = obj_manager.get_entry_node_value
    local NODE = obj_manager.const.ENTRY_NODE_MAP_CONFIG
    local user_ids = getv(NODE, 'Game.user_ids')
    local player_setting = getv(NODE, 'Game.player_setting')
    local opened_slot = getv(NODE, 'Game.opened_slots')
    if type(player_setting) ~= 'table' then
        return nil, '数编 player_setting 读取失败（地图未打开或数据异常）'
    end
    local opened_slots = {}
    if type(opened_slot) == 'table' then
        for _, value in ipairs(opened_slot) do
            opened_slots[value] = true
        end
    end
    -- 官方分配序：pairs(player_setting) 遍历，type='user' 且槽位 opened 的玩家依次分 user_ids
    local player_map = {}
    local candidates = {}
    local user_idx = 1
    for key, value in pairs(player_setting) do
        if value[1] == 'user' and opened_slots[key] then
            local user_id = type(user_ids) == 'table' and user_ids[user_idx] or nil
            if user_id ~= nil then
                player_map[key] = user_id
            end
            candidates[#candidates + 1] = key
            user_idx = user_idx + 1
        end
    end
    return {
        player_setting = player_setting,
        opened_slots = opened_slots,
        user_ids = user_ids,
        player_map = player_map,
        candidates = candidates,
    }, nil
end

-- ===== handlers =====

---注册 handlers。ctx: { send_ack, deferred, logi,
---  get_plugin_mgr, get_scene_mgr, get_main_frame, get_menu_bar, auto_suppress, tee }
---（tee = mp_log_tee 模块：ensure()/reset() 拉起时补挂与清空）
---@return table<string, function> handlers（挂到 main.lua 的 handlers 表）
function M.setup(ctx)
    local send_ack = ctx.send_ack
    local DEFERRED = ctx.deferred
    local logi = ctx.logi

    -- 模块加载即尝试挂 tee（EDITOR/EVENT 此刻不可用则等 mp_start 时补挂）
    if ctx.tee then
        ctx.tee.ensure()
    end

    local handlers = {}

    ---多人拉起（执行序：互斥先停 → 缺省选人 → 预校验 → 字段补全 → 拉起 → 归属映射 + 逐槽位轮询）
    handlers.mp_start = function(params, id)
        params = type(params) == 'table' and params or {}
        local pluginMgr = ctx.get_plugin_mgr()
        local MainFrame = ctx.get_main_frame()
        if not (pluginMgr and MainFrame) then
            error('编辑器管理器不可用（插件/主框架未就绪）')
        end

        -- 解析 players：2~4 整数（自动选人）或 [{player?, delay?}...]（显式玩家 + 逐玩家延迟秒）
        local want_count, explicit = nil, nil
        local pv = params.players
        if type(pv) == 'number' and pv % 1 == 0 then
            if pv < 2 or pv > MAX_SLOTS then
                error(('players 人数须为 2~%d（players=1 请省略该参数，走现状单人调试）'):format(MAX_SLOTS))
            end
            want_count = pv
        elseif type(pv) == 'table' then
            if #pv < 1 or #pv > MAX_SLOTS then
                error(('players 数组长度须为 1~%d（每项 {player?, delay?}，delay 缺省 0）'):format(MAX_SLOTS))
            end
            explicit = pv
        else
            error('players 缺失或非法：2~4 的整数，或 [{player?, delay?}...] 数组')
        end

        ---逐槽位轮询上线（pcall 防御；true=在线/false=已卸载/nil=未注册），全部在线或超时
        ---sel 为补全后的 muti_debug_info 条目（字段 SlotID/Player）
        local function poll_all_online(sel, round, deadline_rounds, done)
            local all = true
            for _, s in ipairs(sel) do
                if slot_online(pluginMgr, s.SlotID) ~= true then
                    all = false
                    break
                end
            end
            if all then
                done(true)
                return
            end
            if round >= deadline_rounds then
                done(false)
                return
            end
            -- base.wait 自身失败（引擎计时器不可用）时兜底终止，防静默挂起无应答
            local ok_wait = pcall(base.wait, 500, function()
                poll_all_online(sel, round + 1, deadline_rounds, done)
            end)
            if not ok_wait then
                done(false)
            end
        end

        local stopped_previous = false

        local function finish()
            local clients = M.clients(ctx)
            local ret = { clients = clients }
            if stopped_previous then
                ret.stopped_previous = true
            end
            logi(('多人调试拉起完成：%d 个客户端在线'):format(#clients))
            send_ack(id, true, ret)
        end

        ---选人 + 预校验 + 字段补全 + 拉起（同步段；失败精确报错不进官方静默降级）
        local function launch()
            local mb = ctx.get_menu_bar()
            if not (mb and type(mb.debug_save_as) == 'function' and type(mb.debug_user_info) == 'table') then
                error('menu_bar 未就绪（地图未打开？请先在编辑器打开项目地图）')
            end
            local map_path = ''
            pcall(function()
                map_path = MainFrame:GetMapPath() or ''
            end)
            if map_path == '' then
                error('地图未打开：请先在编辑器中打开项目/地图，再启动多人调试')
            end
            local mt, mt_err = read_map_type()
            if mt == nil then
                error('地图类型读取失败：' .. tostring(mt_err))
            end
            if mt == 'lobby' then
                error('"大厅地图"目前仅支持"大厅调试"（多人调试不支持 lobby 地图）')
            end

            local cfg, cfg_err = read_multi_config()
            if not cfg then
                error('多人配置读取失败：' .. tostring(cfg_err))
            end

            -- 缺省选人：官方遍历序前 N 个 user+opened 玩家
            local sel = {}
            if want_count then
                if #cfg.candidates < want_count then
                    error(('可用 user 玩家槽位不足：需 %d 个，当前仅 %d 个已启用——请去「玩家设置」面板启用更多槽位')
                        :format(want_count, #cfg.candidates))
                end
                for i = 1, want_count do
                    sel[i] = { player = cfg.candidates[i], delay = 0 }
                end
            else
                for i, e in ipairs(explicit) do
                    local p = type(e) == 'table' and e.player or nil
                    if p == nil then
                        p = cfg.candidates[i] -- player 缺省按数组位置取自动选人结果的第 i 个
                    end
                    if type(p) ~= 'number' then
                        error(('players[%d] 无法确定玩家号（显式给 player 字段）'):format(i))
                    end
                    local d = type(e) == 'table' and tonumber(e.delay) or nil
                    sel[i] = { player = p, delay = d or 0 }
                end
            end

            -- 预校验（拒绝即报错，不触发官方静默降级）
            local seen = {}
            for _, s in ipairs(sel) do
                local p = s.player
                if seen[p] then
                    error(('玩家 %d 重复指定'):format(p))
                end
                seen[p] = true
                local ps = cfg.player_setting[p]
                if ps == nil then
                    error(('玩家 %d 在数编 player_setting 中不存在'):format(p))
                end
                if ps[1] ~= 'user' then
                    error(('玩家 %d 类型是 %s 不是 user（多人调试仅支持 user 玩家）'):format(p, tostring(ps[1])))
                end
                if not cfg.opened_slots[p] then
                    error(('玩家 %d 调试槽位未启用——请去「玩家设置」面板启用槽位 %d'):format(p, p))
                end
                if cfg.player_map[p] == nil then
                    error(('数编 user_ids 库存不足，玩家 %d 分不到 UserId（请在「玩家设置」补充 user_ids）'):format(p))
                end
            end

            -- 字段补全 + 拉起（Team = 数编默认编队值 player_setting[player][2]；SlotID = 数组下标）
            local t = {}
            local new_own = {}
            for i, s in ipairs(sel) do
                local uid = cfg.player_map[s.player]
                t[i] = {
                    Enabled = true,
                    Player = s.player,
                    Team = cfg.player_setting[s.player][2],
                    Delay = s.delay,
                    UserId = uid,
                    SlotID = slot_name(i),
                }
                -- 缺失则 tab 标题退化、日志面板无玩家归属（tee 也依赖该映射）
                mb.debug_user_info[uid] = { player = s.player, icon_color = ICON_COLORS[i] }
                new_own[s.player] = {
                    slot_id = slot_name(i),
                    user_id = uid,
                    icon_color = ICON_COLORS[i],
                    paused = false,
                }
            end

            ctx.auto_suppress() -- start_debug 同款弹窗抑制（pie_will_launch 或 60s 自动关闭）
            local ok, err = pcall(mb.debug_save_as, {
                muti_debug_info = t,
                use_muti_debug = true,
                -- full 语义对齐单人：false 且有上次调试目录 → 跳过编译（nil 时官方自动降级全量不报错）
                use_last_debug_info = params.full ~= true,
            })
            if not ok then
                error('debug_save_as 拉起失败：' .. tostring(err))
            end
            ownership = new_own
            if ctx.tee then
                ctx.tee.reset() -- 新一局清空 tee 缓冲
                ctx.tee.ensure()
            end

            -- 逐槽位轮询上线（对齐 start_debug 120s 超时；轮询用补全后的 t，含 SlotID）
            poll_all_online(t, 0, 240, function(all_ok)
                if all_ok then
                    finish()
                else
                    local detail = {}
                    for _, s in ipairs(t) do
                        detail[#detail + 1] = ('玩家%d(%s)=%s'):format(
                            s.Player, s.SlotID, tostring(slot_online(pluginMgr, s.SlotID)))
                    end
                    send_ack(id, false, '多人调试启动超时（120s 内槽位未全部上线：'
                        .. table.concat(detail, ' ') .. '）。可 get_status 查 clients 后重试')
                end
            end)
        end

        -- 执行序 1：互斥（任一 GamePlayInEditor* 在线 → 先「调试/停止」等全部卸载）
        local any_online = M.any_debugging(ctx)
        if not any_online then
            launch()
        else
            local mb = ctx.get_menu_bar()
            if not (mb and type(mb.call_command) == 'function') then
                error('已有调试会话在线，且 menu_bar 不可用无法停止（请手动「调试/停止」后重试）')
            end
            stopped_previous = true
            ownership = nil
            pcall(mb.call_command, '调试/停止')
            -- deferred 路径的 launch 在计时器回调里执行，error 无人捕获会静默挂起
            -- （调用方只能等 150s 超时且丢失预校验精确报错）——统一 xpcall 兜底回 nack
            local function launch_guarded()
                local ok, e = xpcall(launch, debug.traceback)
                if not ok then
                    send_ack(id, false, tostring(e))
                end
            end
            local rounds = 0
            local function wait_stopped()
                if not M.any_debugging(ctx) then
                    -- 槽位全部卸载后再留 3s 余量：C++/服务端 teardown 滞后于 UI 卸载，
                    -- 立即重拉会让上一局的销毁广播落到新局（实测 rapid 重拉致编辑器崩溃，
                    -- 1.5s 余量在连续 5 次重拉第 5 次仍崩，3s 再验证）
                    if not pcall(base.wait, 3000, launch_guarded) then
                        launch_guarded()
                    end
                    return
                end
                rounds = rounds + 1
                if rounds > 30 then -- 15s
                    send_ack(id, false, '停止上一次调试超时（15s）：请手动「调试/停止」后重试')
                    return
                end
                if not pcall(base.wait, 500, wait_stopped) then
                    send_ack(id, false, '引擎计时器不可用（base.wait 失败），无法确认停止状态——请手动「调试/停止」后重试')
                end
            end
            if not pcall(base.wait, 500, wait_stopped) then
                wait_stopped()
            end
        end
        return DEFERRED
    end

    ---切焦（capture 截图内部编排用；write 但非能力面）：返回 { switched, player, previous, paused, note? }
    handlers.mp_switch = function(params)
        params = type(params) == 'table' and params or {}
        local own = M.effective_ownership(ctx)
        if not own then
            return {
                switched = false,
                note = '当前为单人调试，player 参数已忽略；如需多人，请 start_debug{players=N}',
            }
        end
        local p = params.player or 1
        local entry = own[p]
        if not entry then
            local list = {}
            for k in pairs(own) do
                list[#list + 1] = tostring(k)
            end
            table.sort(list)
            error(('玩家 %s 不在线（当前在线：%s，get_status 可查）'):format(tostring(p), table.concat(list, ',')))
        end
        local sceneMgr = ctx.get_scene_mgr()
        -- 找当前焦点槽位（is_scene_focus 无文档，pcall 防御；拿不到则 previous=nil 调用方跳过还原）
        local previous = nil
        if sceneMgr then
            for pl, e in pairs(own) do
                local ok, f = pcall(function()
                    return sceneMgr:is_scene_focus(e.slot_id)
                end)
                if ok and f == true then
                    previous = pl
                    break
                end
            end
        end
        if previous == p then
            return { switched = false, player = p, previous = previous, paused = entry.paused or false }
        end
        local ok1, err1 = pcall(function()
            _G.ui.switch_page(entry.slot_id)
        end)
        if not ok1 then
            error(('切换玩家视图失败（switch_page %s）：%s'):format(entry.slot_id, tostring(err1)))
        end
        if sceneMgr then
            pcall(function()
                sceneMgr:set_game_ui_focus(entry.slot_id)
            end)
        end
        return { switched = true, player = p, previous = previous, paused = entry.paused or false }
    end

    ---玩家暂停/恢复（官方 tab 暂停按钮同款：disconnect/reconnect_game_in_editor）。
    ---语义 = 断线/重连模拟：disconnect 后该 VM 停 tick（dbg 命令不应答、日志无新行、
    ---画面定格最后一帧）；host 局不销毁；reconnect 后客户端重新连入。
    handlers.set_pause = function(params)
        params = type(params) == 'table' and params or {}
        if type(params.paused) ~= 'boolean' then
            error('set_pause 缺 paused（boolean：true=暂停/断线模拟，false=恢复/重连）')
        end
        local own = M.effective_ownership(ctx)
        if not own then
            error('单人调试槽位无暂停能力（官方单人 PIE 无暂停按钮）；断线/重连测试请 start_debug{players=N} 后用 lua.set_pause')
        end
        local p = params.player or 1
        local entry = own[p]
        if not entry then
            local list = {}
            for k in pairs(own) do
                list[#list + 1] = tostring(k)
            end
            table.sort(list)
            error(('玩家 %s 不在线（当前在线：%s，get_status 可查）'):format(tostring(p), table.concat(list, ',')))
        end
        local sceneMgr = ctx.get_scene_mgr()
        if not sceneMgr then
            error('SceneManager 不可用')
        end
        if params.paused then
            sceneMgr:disconnect_game_in_editor(entry.slot_id)
        else
            sceneMgr:reconnect_game_in_editor(entry.slot_id)
        end
        entry.paused = params.paused -- 桥自持标注（手动点 tab 暂停按钮不经过桥会漂移，只影响标注）
        return { ok = true, player = p, paused = params.paused }
    end

    return handlers
end

return M
