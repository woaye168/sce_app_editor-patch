-- bgd_mcp_bridge / mp_log_tee：0.8.7 分玩家日志 tee（自 mp_debug.lua 拆出，单文件职责纪律）
-- 职责：常驻监听 EVENT.add_info_list 的 debug_client_info 投递（调试信息面板同款数据源，
-- 面板已把 userID 映射成玩家号）→ 每玩家环形缓冲；mp_logs handler（查询/清空）。
-- 踩坑实锤（研究文档 §6）：
--   - 回调首参是 trig 自身（方法调用语义），必须 function(self, module, data)
--   - 返回非 nil 会中断后续投递，必须返回 nil
--   - 只能拿到面板管线放行的日志（file_location 为 nil 的被面板过滤，如部分 C++ 日志）
--   - 无历史（注册时点起）；单人局日志无 userID 映射，归「未标记」缓冲 0
-- 机制底档：doc/research/multi-player-debug.md

local M = {}

local TEE_CAP = 1000 -- 每玩家环形缓冲条数（新一局拉起时由 mp_debug 调 reset 清空）

-- ===== 模块级状态（编辑器 VM 会话级） =====
local tee = {}             -- { [player(0=未标记)] = { {player,type,message,frame,location}, ... } }
local tee_registered = false
local logi_fn = nil        -- setup 时持有

local function tee_push(player, entry)
    local key = player or 0
    local buf = tee[key]
    if not buf then
        buf = {}
        tee[key] = buf
    end
    buf[#buf + 1] = entry
    if #buf > TEE_CAP then
        table.remove(buf, 1)
    end
end

---注册 tee 常驻监听（幂等）。EDITOR/EVENT 不可用时静默跳过（mp_start 拉起时会补挂）。
function M.ensure()
    if tee_registered then
        return
    end
    if not (EDITOR and EDITOR.event_register and EVENT and EVENT.add_info_list) then
        return
    end
    local ok = pcall(EDITOR.event_register, EVENT.add_info_list, function(_, module, data)
        if module ~= 'debug_client_info' or type(data) ~= 'table' then
            return
        end
        local player = data.info_user_info and data.info_user_info.player or nil
        local loc = nil
        if type(data.info_location) == 'table' then
            loc = data.info_location.text
            if data.info_location.detail then
                loc = (loc or '') .. ':' .. tostring(data.info_location.detail)
            end
        elseif type(data.info_location) == 'string' then
            loc = data.info_location
        end
        tee_push(player, {
            player = player,
            type = data.info_type,
            message = data.info_message,
            frame = data.info_frame,
            location = loc,
        })
    end)
    if ok then
        tee_registered = true
        if logi_fn then
            logi_fn('分玩家日志 tee 已挂接（debug_client_info 投递）')
        end
    end
end

---新一局拉起时清空 tee 缓冲（mp_debug.mp_start 调用）
function M.reset()
    tee = {}
end

---注册 handlers。ctx: { logi, get_ownership }（get_ownership → mp_debug.effective_ownership，
---在线过滤后的归属映射或 nil=非多人局；闭包延迟解析避免装配顺序耦合）
---@return table<string, function> handlers（挂到 main.lua 的 handlers 表）
function M.setup(ctx)
    logi_fn = ctx.logi
    M.ensure() -- 模块加载即尝试挂 tee

    local handlers = {}

    ---分玩家日志 tee 查询/清空（tee 只含面板管线放行的日志；无历史，注册时点起）
    handlers.mp_logs = function(params)
        params = type(params) == 'table' and params or {}
        M.ensure()
        if not tee_registered then
            error('日志 tee 不可用（EDITOR 事件总线未就绪）')
        end
        -- 单人局日志无 userID 映射（player=nil 归「未标记」缓冲 0）：单人局 player=1 查询命中未标记行
        local own = ctx.get_ownership and ctx.get_ownership() or nil
        local key = own and (params.player or 1) or 0
        -- 多人局定向查询不在线且无任何 tee 记录的玩家 → 报错引导（防误判「该玩家无日志」）；
        -- tee 有记录（如曾在线/已暂停玩家）仍放行查历史
        if own and params.player ~= nil and not own[params.player] and not tee[params.player] then
            local list = {}
            for k in pairs(own) do
                list[#list + 1] = tostring(k)
            end
            table.sort(list)
            error(('玩家 %s 不在线（当前在线：%s，get_status 可查）'):format(
                tostring(params.player), table.concat(list, ',')))
        end
        if params.clear == true then
            if params.player then
                tee[key] = nil
            else
                tee = {}
            end
            return { cleared = true }
        end
        local buf = tee[key] or {}
        local tail = tonumber(params.tail) or 100
        local lines = {}
        local start = math.max(1, #buf - tail + 1)
        for i = start, #buf do
            lines[#lines + 1] = buf[i]
        end
        local ret = {
            lines = lines,
            total = #buf,
            player = params.player,
            note = 'tee 通道自编辑器本次调试起收集（无历史）；仅含面板管线放行的客户端日志，服务端日志请省略 player 走文件型',
        }
        if not own and params.player ~= nil then
            ret.note = '当前为单人调试，player 参数已忽略（日志归未标记通道）；如需多人，请 start_debug{players=N}'
        end
        return ret
    end

    return handlers
end

return M
