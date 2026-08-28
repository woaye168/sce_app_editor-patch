-- bgd_mcp_bridge / ui_loop：0.8.0 UI 闭环调试（自 main.lua 拆出，单文件职责纪律）
-- 职责：find_ui / click_ui / click_at / input_text / press_ui / release_ui /
--   long_press_ui / set_value / hover_ui / game_info 十个 handler + lobby 跨 VM 总线管线。
-- 游戏侧经引擎 lobby 总线直达 StateGame 的 dbg_bus 端点（bgd 框架游戏侧模块）：
--   编辑器 → 游戏：send_luastate_broadcast('bgd_dbg_cmd', { id, cmd, args })
--   游戏 → 编辑器：bgd_dbg_result { id, ok, json, result }（result 为 JSON 字符串或文本）
-- 编辑器侧 UI（base.ui 持久树）直接在本 VM 查询（find_ui scope=editor）。
-- 机制底档：doc/research/ui-reflection.md + lua-vm-bus.md

local M = {}

---注册 UI 闭环 handlers。ctx: { send_ack, deferred, logi, is_debugging }
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
    local dbg_pending = {} -- 游戏侧请求 id -> 桥请求 id
    local dbg_next_id = 0

    local function dbg_on_result(data)
        if type(data) ~= 'table' or data.id == nil then
            return
        end
        local ack_id = dbg_pending[tostring(data.id)]
        if not ack_id then
            return
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
            send_ack(ack_id, true, payload)
        else
            send_ack(ack_id, false, tostring(payload))
        end
    end

    if dbg_bus_ready then
        pcall(dbg_lobby.register_luaState_event, 'bgd_dbg_result', dbg_on_result)
        logi('lobby 跨 VM 调试总线已接入（bgd_dbg_cmd/bgd_dbg_result）')
    else
        logi('lobby 跨 VM 总线不可用，find_ui/click_ui 等游戏侧 UI 能力停用')
    end

    ---经 lobby 总线调游戏侧 dbg_bus 命令（DEFERRED + 3s 无响应超时兜底）
    local function vm_call(cmd, args, ack_id)
        if not dbg_bus_ready then
            error('lobby 跨 VM 总线不可用（编辑器上下文 require lobby 失败）')
        end
        if not ctx.is_debugging() then
            error('游戏未在调试，请先 start_debug')
        end
        dbg_next_id = dbg_next_id + 1
        local req_id = tostring(dbg_next_id)
        dbg_pending[req_id] = ack_id
        pcall(base.wait, 3000, function()
            if dbg_pending[req_id] then
                dbg_pending[req_id] = nil
                send_ack(ack_id, false,
                    '游戏侧无响应（dbg_bus 未就绪？游戏项目需更新框架并重新构建后 restart_last_debug）')
            end
        end)
        local ok, err = pcall(dbg_lobby.send_luastate_broadcast, 'bgd_dbg_cmd',
            { id = req_id, cmd = cmd, args = args })
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
        return vm_call('find_ui', {
            q = type(params) == 'table' and params.q or nil,
            kind = type(params) == 'table' and params.kind or nil,
        }, id)
    end

    -- 游戏侧信息（逻辑分辨率等）：find_ui rect / click_at / capture_game crop 的坐标系基准
    handlers.game_info = function(_params, id)
        return vm_call('game_info', {}, id)
    end

    -- 按 id 点击游戏 UI 控件（注册回调直调 + state 注入兜底；等价真实点击业务效果）
    handlers.click_ui = function(params, id)
        return vm_call('click_ui', { id = type(params) == 'table' and params.id or nil }, id)
    end

    -- 逻辑坐标点击游戏 UI（命中最内层可点击控件）
    handlers.click_at = function(params, id)
        return vm_call('click_at',
            { x = type(params) == 'table' and params.x or nil, y = type(params) == 'table' and params.y or nil },
            id)
    end

    -- 输入框文本输入（直接调 on_input 回调，等价人工输入完整文本）
    handlers.input_text = function(params, id)
        return vm_call('input_text',
            { id = type(params) == 'table' and params.id or nil, text = type(params) == 'table' and params.text or nil },
            id)
    end

    -- 按住（joystick 等持续输入控件，x/y 方向 [-1,1]，持续到 release_ui）
    handlers.press_ui = function(params, id)
        return vm_call('press_ui',
            { id = type(params) == 'table' and params.id or nil, x = type(params) == 'table' and params.x or nil,
              y = type(params) == 'table' and params.y or nil },
            id)
    end

    -- 松开（解除 press_ui 的模拟按住）
    handlers.release_ui = function(params, id)
        return vm_call('release_ui', { id = type(params) == 'table' and params.id or nil }, id)
    end

    -- 长按（调 on_long_press 回调）
    handlers.long_press_ui = function(params, id)
        return vm_call('long_press_ui', { id = type(params) == 'table' and params.id or nil }, id)
    end

    -- 数值直设（slider 滑块）
    handlers.set_value = function(params, id)
        return vm_call('set_value',
            { id = type(params) == 'table' and params.id or nil, value = type(params) == 'table' and params.value or nil },
            id)
    end

    -- 悬停/移入（游戏侧诚实报错：引擎输入拉取式不可脚本注入）
    handlers.hover_ui = function(params, id)
        return vm_call('hover_ui', { id = type(params) == 'table' and params.id or nil }, id)
    end

    return handlers
end

return M
