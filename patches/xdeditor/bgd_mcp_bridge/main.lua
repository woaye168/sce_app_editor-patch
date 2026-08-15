-- bgd_mcp_bridge：C# 扩展（bgd_mcp_bridge.dll）的 Lua 侧搭档
-- 职责：
--   1. 延迟激活 C# 窗口（csharp_activate_window），失败重试最多 5 次
--   2. 事件桥：监听 'bgd_mcp_cmd'（JSON），执行后 'bgd_mcp_ack' 回传结果
--   3. wrap window_title_bar.register / register_command，记录已注册命令表
--   4. wrap message_window 弹窗抑制：start_debug 前自动开启，pie_will_launch 或 60s 超时自动关闭
-- 注意：SCE 非全局，必须 ImportSCEContext() 自取（xdeditor/main.lua:83）。
-- 计时器：base.wait(timeout, fn) 两参单次延迟（base.timer 签名是 (timeout, count, on_timer)，勿混用）。
-- 红线：本模块加载失败会中断补丁入口循环，任何调用失败只记日志不抛出。

local function logi(m)
    if log and log.info then
        pcall(log.info, '[bgd_mcp_bridge] ' .. m)
    end
end

-- ===== 基础对象（全部防御式获取） =====
local SCE, MainFrame, eventMgr, pluginMgr
pcall(function()
    SCE = ImportSCEContext()
    MainFrame = GetMainFrame()
    eventMgr = SCE.GetEventManager()
    pluginMgr = SCE.GetPluginsManager()
end)
if not SCE then
    logi('ImportSCEContext 失败，模块仅完成加载')
end

-- ===== 职责1：激活 C# dll =====
local function try_activate(attempt)
    local ok, err = pcall(function()
        return SCE.Common.csharp_activate_window('BgdMcpBridge.BridgeWindow, bgd_mcp_bridge')
    end)
    if ok then
        logi(('csharp_activate_window 成功（第 %d 次）'):format(attempt))
    else
        logi(('csharp_activate_window 失败（第 %d 次）：%s'):format(attempt, tostring(err)))
        if attempt < 5 then
            pcall(base.wait, 3000, function()
                try_activate(attempt + 1)
            end)
        else
            logi('csharp_activate_window 重试次数用尽，放弃激活')
        end
    end
end

if SCE then
    pcall(base.wait, 5000, function()
        try_activate(1)
    end)
    logi('模块加载完成，5 秒后尝试激活 BgdMcpBridge')
end

-- ===== 职责3：命令表记录（wrap window_title_bar.register / register_command） =====
-- require 'ui.menu_bar' 直接返回 window_title_bar 组件本体（menu_bar.lua:3166 `return window_title_bar`）
local command_set = {}
local window_title_bar
do
    local ok, mb = pcall(require, 'ui.menu_bar')
    if ok and type(mb) == 'table' then
        window_title_bar = mb
        -- menu_bar.lua:1100 register(name, callback, key, ...)
        if type(mb.register) == 'function' then
            local orig_register = mb.register
            mb.register = function(name, ...)
                local r = orig_register(name, ...)
                if type(name) == 'string' then
                    command_set[name] = true
                end
                return r
            end
        end
        -- menu_bar.lua:1056 register_command(name, callback)
        if type(mb.register_command) == 'function' then
            local orig_register_command = mb.register_command
            mb.register_command = function(name, ...)
                local r = orig_register_command(name, ...)
                if type(name) == 'string' then
                    command_set[name] = true
                end
                return r
            end
        end
        logi('已 wrap window_title_bar.register/register_command')
    else
        logi('require ui.menu_bar 失败，call_command/list_commands 将不可用')
    end
end

local function list_commands()
    local arr = {}
    for name in pairs(command_set) do
        arr[#arr + 1] = name
    end
    table.sort(arr)
    return arr
end

-- ===== 职责4：错误弹窗抑制（wrap message_window） =====
-- require 'ui.components.message_window' 返回表：
--   { message_window=fn, Close=1, Cancel=2, Confirm=3, has_window=fn, close_current_window=fn }
--   （message_window.lua:158-165）；message_window(func, btn_text, prompt_text, title_text, ...)（:138）
local suppress_enabled = false   -- 抑制总开关（set_suppress 控制）
local suppress_auto = false      -- 是否为 start_debug 自动开启（用于自动关闭）
local suppress_gen = 0           -- 代数令牌，防止旧超时回调误关新一次抑制

local function send_event(evt)
    if MainFrame then
        pcall(function()
            MainFrame:SendEvent('bgd_mcp_event', evt)
        end)
    end
end

do
    local ok, mw = pcall(require, 'ui.components.message_window')
    if ok and type(mw) == 'table' and type(mw.message_window) == 'function' then
        local orig_message_window = mw.message_window
        mw.message_window = function(func, btn_text, prompt_text, title_text, ...)
            if not suppress_enabled then
                return orig_message_window(func, btn_text, prompt_text, title_text, ...)
            end
            -- 抑制：不弹窗，立即回调。有确认按钮则视为 Confirm，否则 Close
            local opt = mw.Close
            if type(btn_text) == 'table' and btn_text.confirm_text then
                opt = mw.Confirm
            end
            logi(('弹窗已抑制：title=%s prompt=%s'):format(tostring(title_text), tostring(prompt_text)))
            send_event({
                type = 'message_box_suppressed',
                title = title_text,
                prompt = prompt_text,
            })
            if type(func) == 'function' then
                pcall(func, opt)
            end
        end
        logi('已 wrap ui.components.message_window')
    else
        logi('require ui.components.message_window 失败，弹窗抑制不可用')
    end
end

-- start_debug 自动抑制：开启后等 pie_will_launch 或 60 秒超时自动关闭
local function auto_suppress_off(gen)
    if suppress_auto and suppress_gen == gen then
        suppress_auto = false
        suppress_enabled = false
        logi('弹窗抑制自动关闭')
        send_event({ type = 'suppress_off' })
    end
end

local function auto_suppress_on()
    suppress_enabled = true
    suppress_auto = true
    suppress_gen = suppress_gen + 1
    local gen = suppress_gen
    logi('弹窗抑制自动开启（start_debug）')
    pcall(base.wait, 60000, function()
        auto_suppress_off(gen)
    end)
end

-- pie_will_launch：游戏内调试即将启动（global/global.lua:210，EDITOR/EVENT 为库全局）
if EVENT and EVENT.pie_will_launch and EDITOR and EDITOR.event_register then
    pcall(EDITOR.event_register, EVENT.pie_will_launch, function()
        auto_suppress_off(suppress_gen)
    end)
end

-- ===== 职责2：C# 命令执行代理（事件桥） =====
local function send_ack(id, ok, data_or_err)
    if not MainFrame then
        return
    end
    pcall(function()
        local payload = { id = id, ok = ok and true or false }
        if ok then
            -- 注意：Lua 表经引擎 VariantMap 传给 C# 时会变成 StringVector，C# 端约定把 data 当 JSON 解析。
            -- 因此 data 必须是「JSON 编码后的字符串」，C# 侧再反序列化。
            local encoded = data_or_err
            if type(data_or_err) == 'table' then
                local okj, j = pcall(json.encode, data_or_err)
                if okj then
                    encoded = j
                end
            end
            payload.data = encoded
        else
            payload.error = tostring(data_or_err)
        end
        MainFrame:SendEvent('bgd_mcp_ack', payload)
    end)
end

local handlers = {}

handlers.call_command = function(params)
    if type(params) ~= 'table' or type(params.name) ~= 'string' then
        error('params.name 缺失或非法')
    end
    -- start_debug：执行前自动开启弹窗抑制
    if params.name == '调试/调试' then
        auto_suppress_on()
    end
    -- 兜底通道：直接调组件方法。主通道是 C# 侧对原生 'EditorMainTitleMenuBar' 事件 SendEvent
    -- （官方菜单点击同款，menu_bar.lua:1066 register_event 收到后 call_command(name)）。
    -- 正常情况下 C# 直发原生事件即可，本 handler 作为 C# 选择经 Lua 时的兜底。
    if window_title_bar and type(window_title_bar.call_command) == 'function' then
        window_title_bar.call_command(params.name)
        return true
    end
    error('window_title_bar 不可用（require ui.menu_bar 未成功），请改用 C# 直发 EditorMainTitleMenuBar 事件')
end

handlers.list_commands = function()
    return list_commands()
end

handlers.get_status = function()
    local map_path, debugging
    if MainFrame then
        pcall(function()
            map_path = MainFrame:GetMapPath()
        end)
    end
    if pluginMgr then
        pcall(function()
            debugging = pluginMgr:is_plugin_ui_loaded('GamePlayInEditor')
        end)
    end
    return {
        map_path = map_path,
        debugging = debugging and true or false,
        suppress = suppress_enabled,
    }
end

handlers.set_suppress = function(params)
    local enabled = type(params) == 'table' and params.enabled and true or false
    suppress_enabled = enabled
    if not enabled then
        suppress_auto = false
        suppress_gen = suppress_gen + 1
    end
    logi('弹窗抑制开关：' .. tostring(suppress_enabled))
    return { suppress = suppress_enabled }
end

if eventMgr then
    pcall(function()
        eventMgr:register_event('bgd_mcp_cmd', function(payload)
            -- payload 为 JSON 字符串（json 为编辑器全局库）
            local ok, req = pcall(json.decode, payload)
            if not ok or type(req) ~= 'table' then
                logi('bgd_mcp_cmd 解析失败：' .. tostring(payload))
                return
            end
            local id = req.id
            local handler = handlers[req.method]
            if not handler then
                send_ack(id, false, '未知 method: ' .. tostring(req.method))
                return
            end
            local ok2, res = xpcall(handler, debug.traceback, req.params)
            if ok2 then
                send_ack(id, true, res)
            else
                logi(('method %s 执行失败：%s'):format(tostring(req.method), tostring(res)))
                send_ack(id, false, res)
            end
        end)
    end)
    logi('已注册事件桥 bgd_mcp_cmd')
else
    logi('eventMgr 不可用，事件桥未注册')
end
