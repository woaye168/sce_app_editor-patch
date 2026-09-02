-- bgd_mcp_bridge / ui_loop：0.8.0 UI 闭环调试（自 main.lua 拆出，单文件职责纪律）
-- 职责：find_ui / click_ui / click_at / input_text / press_ui / release_ui /
--   long_press_ui / set_value / hover_ui / game_info / eval +
--   0.8.2 增量（drag_ui / scroll_ui / tap / pick / key_down / key_up）——全部薄转发。
-- 游戏侧经引擎 lobby 总线直达 StateGame 的 dbg_bus 端点（bgd 框架游戏侧模块）：
--   编辑器 → 游戏：send_luastate_broadcast('bgd_dbg_cmd', { id, cmd, args, target?, proto=2 })
--   游戏 → 编辑器：bgd_dbg_result { id, ok, json, result, from?, proto? }
-- 0.8.7 多人寻址：lua.* 全命令加 player 参数（缺省 = 多人局 1 号玩家/单人局唯一玩家；
-- 单人局带 player 回退 + note 告知）；多人局恒带 target 定向，回执按 from==target 配对，
-- 收到无 from 的应答 = 对端旧框架 → 响亮报错引导升级（不静默退化）。
-- 编辑器侧 UI（base.ui 持久树）直接在本 VM 查询（find_ui scope=editor）。
-- 机制底档：doc/research/ui-reflection.md + lua-vm-bus.md + multi-player-debug.md

local M = {}

---注册 UI 闭环 handlers。ctx: { send_ack, deferred, logi, is_debugging, dbg_target }
---dbg_target(player) → target(number|nil), note(string|nil), err(string|nil)（0.8.7 多人寻址）
---@return table<string, function> handlers（挂到 main.lua 的 handlers 表）
function M.setup(ctx)
    local send_ack = ctx.send_ack
    local DEFERRED = ctx.deferred
    local logi = ctx.logi

    local dbg_lobby
    pcall(function()
        dbg_lobby = require '@base.base.lobby'
    end)
    local dbg_bus_ready = type(dbg_lobby) == 'table'
        and type(dbg_lobby.send_luastate_broadcast) == 'function'
        and type(dbg_lobby.register_luaState_event) == 'function'
    local dbg_pending = {} -- 游戏侧请求 id -> { ack=桥请求id, target=玩家号|nil, note=告知|nil }
    local dbg_next_id = 0

    local function dbg_on_result(data)
        if type(data) ~= 'table' or data.id == nil then
            return
        end
        local entry = dbg_pending[tostring(data.id)]
        if not entry then
            return
        end
        -- 0.8.7 定向配对：多人请求只收 from==target 的应答；其他玩家的同 id 应答忽略（继续等）
        if entry.target ~= nil then
            if data.from == nil then
                -- 无 from = 对端旧框架：响亮报错引导升级，不等超时不静默退化
                dbg_pending[tostring(data.id)] = nil
                send_ack(entry.ack, false,
                    '游戏侧框架过旧（dbg 回执无 from 字段，无法多人寻址）：请 bgd_sce_tools update-framework + build 后 restart_last_debug')
                return
            end
            if data.from ~= entry.target then
                return
            end
        end
        dbg_pending[tostring(data.id)] = nil
        local payload = data.result
        if data.json and type(payload) == 'string' then
            local ok, t = pcall(json.decode, payload)
            if ok then
                payload = t
            end
        end
        if data.ok then
            if type(payload) ~= 'table' then
                payload = { result = payload }
            end
            -- 单人局带 player 的回退告知（统一文案模板）
            if entry.note then
                payload.note = entry.note
            end
            send_ack(entry.ack, true, payload)
        else
            send_ack(entry.ack, false, tostring(payload))
        end
    end

    if dbg_bus_ready then
        pcall(dbg_lobby.register_luaState_event, 'bgd_dbg_result', dbg_on_result)
        logi('lobby 跨 VM 调试总线已接入（bgd_dbg_cmd/bgd_dbg_result）')
    else
        logi('lobby 跨 VM 总线不可用，find_ui/click_ui 等游戏侧 UI 能力停用')
    end

    ---经 lobby 总线调游戏侧 dbg_bus 命令（DEFERRED + 3s 无响应超时兜底）
    ---0.8.7：args.player 经 ctx.dbg_target 译 target 进 bgd_dbg_cmd（dbg_commands 零改动，
    ---过滤在 dbg_bus 总线层）；player 字段不随 args 下发（游戏侧无此概念）
    local function vm_call(cmd, args, ack_id)
        if not dbg_bus_ready then
            error('lobby 跨 VM 总线不可用（编辑器上下文 require lobby 失败）')
        end
        if not ctx.is_debugging() then
            error('游戏未在调试，请先 start_debug')
        end
        local target, note, terr
        if ctx.dbg_target then
            target, note, terr = ctx.dbg_target(args.player)
            if terr then
                error(terr)
            end
        end
        if args.player ~= nil then
            -- player 是桥侧寻址参数，不下发给游戏侧 dbg_commands
            local cleaned = {}
            for k, v in pairs(args) do
                if k ~= 'player' then
                    cleaned[k] = v
                end
            end
            args = cleaned
        end
        dbg_next_id = dbg_next_id + 1
        local req_id = tostring(dbg_next_id)
        dbg_pending[req_id] = { ack = ack_id, target = target, note = note }
        pcall(base.wait, 3000, function()
            local pend = dbg_pending[req_id]
            if pend then
                dbg_pending[req_id] = nil
                -- 超时文案统一含暂停恢复引导（暂停 VM 不应答 dbg 命令，实测）
                local hint = pend.target
                    and ('无响应（dbg_bus 未就绪？或目标玩家已暂停——可 lua.set_pause{player='
                        .. tostring(pend.target) .. ', paused=false} 恢复；或游戏侧框架需更新：update-framework + build + restart_last_debug）')
                    or '无响应（dbg_bus 未就绪？或目标玩家已暂停——可 lua.set_pause{player=N, paused=false} 恢复；或游戏侧框架需更新：update-framework + build + restart_last_debug）'
                send_ack(ack_id, false, '游戏侧' .. hint)
            end
        end)
        -- 多人局恒带 target + proto=2；单人局不带 target（协议字节级兼容旧游戏框架）
        local payload = { id = req_id, cmd = cmd, args = args }
        if target ~= nil then
            payload.target = target
            payload.proto = 2
        end
        local ok, err = pcall(dbg_lobby.send_luastate_broadcast, 'bgd_dbg_cmd', payload)
        if not ok then
            dbg_pending[req_id] = nil
            error('lobby 广播失败: ' .. tostring(err))
        end
        return DEFERRED
    end

    ---编辑器侧 UI 查询（base.ui 持久树，调编辑器界面时用）
    local function find_editor_ui(params)
        if not (base and base.ui and base.ui.map) then
            error('base.ui 不可用')
        end
        local q = type(params) == 'table' and params.q
        if type(q) ~= 'string' or q == '' then
            error('params.q 缺失（按控件名/id 子串模糊查询）')
        end
        local ql = q:lower()
        local items = {}
        local total = 0
        for name, ctrl in pairs(base.ui.map) do
            local ns = tostring(name)
            if ns:lower():find(ql, 1, true) then
                total = total + 1
                if #items < 20 then
                    local rect
                    local ok, x, y, w, h = pcall(function()
                        return ctrl:get_screen_rect()
                    end)
                    if ok and x then
                        rect = { x = x, y = y, w = w, h = h }
                    end
                    items[#items + 1] = { id = ns, sys = 'editor', type = ctrl.type, rect = rect }
                end
            end
        end
        table.sort(items, function(a, b)
            return a.id < b.id
        end)
        return { items = items, total = total, truncated = total > #items }
    end

    local handlers = {}

    -- 查询 UI 控件：默认游戏侧（cgui 快照 + base.ui 树，返回逻辑坐标 rect/可点/可输入标记）；
    -- scope='editor' 查编辑器自身 UI。结果上限 20 条 + truncated。
    handlers.find_ui = function(params, id)
        local scope = type(params) == 'table' and params.scope or nil
        if scope == 'editor' then
            return find_editor_ui(params)
        end
        return vm_call('find_ui', params, id)
    end

    -- 游戏侧 UI 命令一律全参透传（真薄转发）：参数白名单曾导致游戏侧新增参数
    -- （如 0.8.3 tap/click_ui 的 expect/expect_absent 操作后验证）被桥静默吞掉——
    -- 参数校验是游戏侧命令自己的职责（缺参/非法各自报 actionable 错误）。
    local function passthrough(cmd)
        return function(params, id)
            return vm_call(cmd, type(params) == 'table' and params or {}, id)
        end
    end

    handlers.game_info = passthrough('game_info')         -- 游戏侧信息（逻辑分辨率等坐标系基准）
    handlers.click_ui = passthrough('click_ui')           -- 按 id 点击（支持 expect/expect_absent 验证）
    handlers.click_at = passthrough('click_at')           -- 逻辑坐标点击（命中最内层可点击控件）
    handlers.input_text = passthrough('input_text')       -- 输入框文本输入（触发 on_input）
    handlers.press_ui = passthrough('press_ui')           -- 按住（joystick 持续输入）
    handlers.release_ui = passthrough('release_ui')       -- 松开（解除模拟按住）
    handlers.long_press_ui = passthrough('long_press_ui') -- 长按（hold_ms 可选，默认 800）
    handlers.set_value = passthrough('set_value')         -- 数值直设（slider）
    handlers.hover_ui = passthrough('hover_ui')           -- 悬停（虚拟指针驻留保持态）
    handlers.drag_ui = passthrough('drag_ui')             -- 拖拽：{from_id,to_id} 或 {from_id,dx,dy}
    handlers.scroll_ui = passthrough('scroll_ui')         -- 受控滚动（pscroll scroll_to 直驱）
    handlers.tap = passthrough('tap')                     -- 复合：找文本→跟祖先→点击（支持 expect）
    handlers.pick = passthrough('pick')                   -- 复合：dropdown 展开+选项
    handlers.key_down = passthrough('key_down')           -- 键盘按下（不调 key_up 则保持）
    handlers.key_up = passthrough('key_up')               -- 键盘松开

    -- 游戏侧 eval 逃生舱（StateGame VM 内 pcall 任意 Lua；danger 级，与编辑器侧 run_lua 同级。
    -- 同走全参透传：参数校验是游戏侧 eval 命令自己的职责，白名单曾静默吞参数的教训见上）
    handlers.eval = passthrough('eval')

    return handlers
end

return M
