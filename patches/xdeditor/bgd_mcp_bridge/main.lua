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
local SCE, MainFrame, eventMgr, pluginMgr, sceneMgr
pcall(function()
    SCE = ImportSCEContext()
    MainFrame = GetMainFrame()
    eventMgr = SCE.GetEventManager()
    pluginMgr = SCE.GetPluginsManager()
    sceneMgr = SCE.GetSceneManager()
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

-- C# dll 激活。注意：C# 侧 BridgeWindow 会把 DI 触碰推迟到地图加载后（WaitForMapLoadedAsync），
-- 避免污染宿主 AddMapScoped 缓存导致官方模块（模拟多人调试）崩溃。
if SCE then
    pcall(base.wait, 5000, function()
        try_activate(1)
    end)
    logi('模块加载完成，5 秒后尝试激活 BgdMcpBridge')
end

-- ===== 职责3+4：延迟初始化（命令表记录 + 弹窗抑制 wrap） =====
-- 关键时序约束（与 menu_bgd 一致）：绝不能在本模块加载期（补丁入口，时机很早）就
-- require 'ui.menu_bar' / 'ui.components.message_window' —— menu_bar.lua 顶部有大量
-- SCE.GetMainWindow()/GetSceneManager() 等顶层副作用，过早 require 会打乱官方初始化顺序，
-- 导致后续流程被破坏（实证：应用补丁后「调试/模拟多人调试」因 MutiDebugWindow 的
-- AddMapScoped<IDataCore> 在 CurrentMapName==null 时崩溃而退出编辑器）。
-- 因此这两个 require + wrap 一律推迟到「地图加载完成（load_map_done）」之后执行，
-- 此时官方各模块已完全初始化，wrap 安全且 list_commands 能拿到全量命令。
local command_set = {}
local window_title_bar
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

local deferred_inited = false
local function init_deferred()
    if deferred_inited then
        return
    end
    deferred_inited = true

    -- 职责3：wrap window_title_bar.register / register_command，记录已注册命令表
    -- require 'ui.menu_bar' 直接返回 window_title_bar 组件本体（menu_bar.lua:3166 `return window_title_bar`）
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
        -- 兜底：menu_bar 已完全加载，callback_map 里已有大量命令，直接吸收进命令表
        -- （register 是组件方法，window_title_bar.callback_map 在 menu_bar.lua:3160 暴露）
        if type(mb.callback_map) == 'table' then
            for name in pairs(mb.callback_map) do
                if type(name) == 'string' then
                    command_set[name] = true
                end
            end
        end
        logi('已 wrap window_title_bar.register/register_command，并吸收 callback_map 现有命令')
    else
        logi('require ui.menu_bar 失败，list_commands 将不完整')
    end

    -- 职责4：wrap message_window 弹窗抑制
    -- require 'ui.components.message_window' 返回表：
    --   { message_window=fn, Close=1, Cancel=2, Confirm=3, has_window=fn, close_current_window=fn }
    local ok2, mw = pcall(require, 'ui.components.message_window')
    if ok2 and type(mw) == 'table' and type(mw.message_window) == 'function' then
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

-- 地图加载完成后初始化（此时 menu_bar/message_window 已完全初始化，wrap 安全）
if EVENT and EVENT.load_map_done and EDITOR and EDITOR.event_register then
    pcall(EDITOR.event_register, EVENT.load_map_done, function()
        init_deferred()
    end)
end
-- 兜底：若地图已开（事件错过）或事件不可用，延迟后也尝试一次
pcall(base.wait, 15000, function()
    init_deferred()
end)

-- 命令表快照（去重后排序）
local function list_commands()
    local arr = {}
    for name in pairs(command_set) do
        arr[#arr + 1] = name
    end
    table.sort(arr)
    return arr
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

-- run_lua 兜底逃生舱（0.5.0 M7）：pcall 执行任意 Lua。默认 danger 级关闭，
-- 需在 <引擎运行根>/logs/bgd_csharp/config.json 的 danger_allow 放行「lua.run_lua」。
handlers.run_lua = function(params)
    if type(params) ~= 'table' or type(params.code) ~= 'string' then
        error('params.code 缺失或非法')
    end
    local fn, load_err = load(params.code, 'run_lua')
    if not fn then
        error('Lua 编译失败: ' .. tostring(load_err))
    end
    local ok, res = xpcall(fn, debug.traceback)
    return { ok = ok and true or false, result = tostring(res) }
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

-- ===== 0.5.3 场景一：发布项目 / 游戏画面截取 =====

-- 延迟 ack 约定：handler 返回 DEFERRED 时不立即 ack，
-- 由 handler 在异步回调里自行 send_ack（C# 侧按 id 配对等待，超时由调用方 timeout_ms 控制）
local DEFERRED = { __deferred = true }

-- 发布项目（R1 定稿：EDITOR.upload_map 官方 promise 结果通道，doc/research/publish-and-capture.md）
-- 绕开菜单 handler 的确认弹窗（弹窗在菜单层，upload_map 本身无交互）；与菜单行为一致先保存地图。
-- log_mark='bgd_mcp' 让官方流程输出结构化日志（[bgd_mcp]发布地图[...]成功/失败）作兜底感知。
handlers.publish_project = function(params, id)
    if not (EDITOR and EDITOR.upload_map) then
        error('EDITOR.upload_map 不可用（xdeditor 未就绪）')
    end
    local map_path
    pcall(function()
        map_path = MainFrame and MainFrame:GetMapPath()
    end)
    if not map_path or map_path == '' then
        error('地图未打开：请先在编辑器中打开项目/地图，再发布')
    end
    coroutine.wrap(function()
        local ok, err = xpcall(function()
            local promise = base.promise()
            EDITOR.upload_map('bgd_mcp', promise)
            local ret = promise:co_result()
            logi('发布项目完成：code=' .. tostring(ret))
            send_event({ type = 'publish_done', ok = ret == 0, code = ret })
            send_ack(id, true, { ok = ret == 0, code = ret })
        end, debug.traceback)
        if not ok then
            logi('发布项目异常：' .. tostring(err))
            send_event({ type = 'publish_done', ok = false, error = tostring(err) })
            send_ack(id, false, err)
        end
    end)()
    return DEFERRED
end

-- 获取 PIE 游戏视口在编辑器窗口中的矩形（0.5.3 修订版截图方案的 lua 环节）。
-- 返回引擎 UI 逻辑坐标矩形 + 逻辑分辨率；外部（bgd_sce_tools WGC 截窗）按比例换算物理裁剪框。
-- 原理：PIE 视口是 base.ui 控件树里的 viewport 控件（ui-<n>-GamePlayInEditor），
-- 控件元表自带 get_screen_rect()（编辑器主区 main rect=(0,0,逻辑宽,逻辑高)）。
handlers.get_game_view_rect = function()
    if not (base and base.ui and base.ui.map) then
        error('base.ui 不可用')
    end
    local ui
    for k, v in pairs(base.ui.map) do
        if tostring(k):match('^ui%-%d+%-GamePlayInEditor$') then
            ui = v
            break
        end
    end
    if not ui or type(ui.get_screen_rect) ~= 'function' then
        error('游戏视口控件不存在（游戏未在调试？）')
    end
    local ok, x, y, w, h = pcall(function()
        return ui:get_screen_rect()
    end)
    if not ok or not x then
        error('读取视口矩形失败: ' .. tostring(x))
    end
    local lw, lh
    pcall(function()
        lw, lh = common.get_resolution()
    end)
    if not lw or not lh then
        error('无法获取编辑器 UI 逻辑分辨率')
    end
    return { x = x, y = y, width = w, height = h, logical_width = lw, logical_height = lh }
end

-- 截取游戏画面（R2 定稿：引擎原生 snapshot_scene_callback，PIE 截图按钮官方实现）
-- 注意：该引擎快照只含 3D 场景、不含游戏 UI 覆盖层；MCP capture_game 默认走
-- 「get_game_view_rect + 外部 WGC 整窗裁剪」路线（含游戏 UI），本 handler 留作兜底。
-- params.path：png 落盘绝对路径（缺省 <用户目录>/screenShot/bgd_capture_<时间戳>.png）
handlers.capture_game = function(params, id)
    if not sceneMgr then
        error('SceneManager 不可用')
    end
    local ui_name = 'GamePlayInEditor'
    local debugging = false
    pcall(function()
        debugging = pluginMgr and pluginMgr:is_plugin_ui_loaded(ui_name)
    end)
    if not debugging then
        error('游戏未在调试（GamePlayInEditor 槽位不存在），请先 start_debug')
    end
    local viewport = sceneMgr:get_ui_viewport(ui_name)
    if not viewport then
        error('游戏视口不可用（游戏未启动？）')
    end
    local w, h = viewport:get_inner_viewport_size()
    local path = type(params) == 'table' and params.path or nil
    if not path or path == '' then
        local base_dir = MainFrame:GetUserPath() .. 'screenShot'
        if not io.exist_dir(base_dir) then
            io.create_dir(base_dir)
        end
        path = ('%s/bgd_capture_%s.png'):format(base_dir, os.date('%Y%m%d_%H%M%S'))
    end
    sceneMgr:snapshot_scene_callback(ui_name, 0, 0, w, h, path, 1.0, nil, function(result)
        if result and result > 0 then
            logi('截图成功：' .. path)
            send_ack(id, true, { path = path, width = w, height = h })
        else
            logi('截图失败：result=' .. tostring(result))
            send_ack(id, false, 'snapshot_scene_callback 返回 ' .. tostring(result))
        end
    end)
    return DEFERRED
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
            -- handler 第二个参数为请求 id：返回 DEFERRED 时由 handler 异步回调里自行 send_ack
            local ok2, res = xpcall(handler, debug.traceback, req.params, id)
            if ok2 then
                if res ~= DEFERRED then
                    send_ack(id, true, res)
                end
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
