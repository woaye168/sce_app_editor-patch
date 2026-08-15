pcall(collectgarbage, 'generational')

if app.set_version_key_value then
    app.set_version_key_value('open_package_holding', 'value')
end
require '@common.base'
local argv = include '@common.base.argv'
local util = require '@common.base.util'


if common.has_arg("unit_test") then
    local s = argv.get("unit_test")
    local li = util.split(s, ';')

    local unit_test = require("@common.base.example.main")
    unit_test(li)
    return
end

if argv.has('scene_test') then
    include 'global'
    -- 这些会注册一些全局变量, 注意变量会依赖
    include 'utils'
    include 'config'
    require 'console'
    base.ui.auto_scale.disable()
    require 'scene_test_enter_point.main'
    return
end

if argv.has('sub_process') then
    if io.set_package_io_mode then
        log.info('sub process => set io mode to 0.')
        io.set_package_io_mode(0)
    end
    include 'global'
    -- 这些会注册一些全局变量, 注意变量会依赖
    include 'utils'
    include 'config'
    require 'console'
    base.ui.auto_scale.disable()
    require 'sub_process_enter_point.main'
    return
end

if argv.has('upload_map') then
    _G.generate_cmd = 'upload_map'
elseif argv.has('upload_lib') then
    _G.generate_cmd = 'upload_lib'
elseif argv.has('upload_lib_abs') then
    _G.generate_cmd = 'upload_lib_abs'
elseif argv.has('generate_map') then
    _G.generate_cmd = 'generate_map'
end

if argv.has('trigger_editor_v2_developer') then
    _G.trigger_editor_v2_developer = true
end

if _G.generate_cmd  then
    -- 进入生成流程之前给两分钟的加载编辑器时间
    _G.generate_cmd_timer = base.timer(2 * 60 * 1000, function()
        log.info('['.._G.generate_cmd..'] failed: 打开登录编辑器超时')
        os.exit(1)
    end)
end
-- 有时只想查看个模型每次要编很久的Shader，加个参数支持跳过
if not argv.has('no_clear_shaders') then
log.info('clear shaders')
common.clear_shaders() -- 现在这个shader只是个兜底：照说我们预编译好的shadercache要覆盖所有的shader的。所以每次删ResCache里的shader没什么问题（不删的话，如果某些预编译不覆盖的shader更新了，就可能不匹配导致坏）
log.info('clear shaders finish')
end

local lobby = require '@common.base.lobby'
base.ui.auto_scale.disable()

local scelobby = include '@common.base.lobby'
local account = include '@common.base.account'
local co = include '@common.base.co'
local check_log = include '@common.base.check_log'
local platform = include '@common.base.platform'
require '@common.base.ip'
local SCE = ImportSCEContext()
local undo_redo_mgr = SCE.GetUndoRedoManager()
local event_mgr = SCE.GetEventManager()
local pluginMgr = SCE.GetPluginsManager()


---------------------------------------------------
-- 本文件里会用到的一些局部函数
---------------------------------------------------

-- 立即连接 entrance
local function connect_entrance_immediately(ip)
    lobby.set_entrance_ip(ip)
    lobby.connect()
    log.info('connecting to entrance..')
end

---------------------------------------------------
-- 尽早先连下Entrance，，毕竟等连上还要点时间，等连接的期间同步加载些别的lua
---------------------------------------------------

log.info(('编辑器版本 : %d'):format(common.get_binary_version()))
log.info('script env : ', __CE_ENV);
log.info('from', argv.get('from') or 'no from')
log.info('platform', common.get_platform())

-- 加载保存着的用户名，密码，还有guest_id(没有的话生成一个)
-- 所谓guest_id叫device_id也许更准确，就是不管有没用手机号登陆，生成个id存机器上，标识这个机器
account.init()

---------------------------------------------------
-- 初始化一些东西
---------------------------------------------------

include '@common.class'
include 'global'
-- 这些会注册一些全局变量, 注意变量会依赖
include 'utils'
include 'config'
require 'console'
include 'exception'
include 'sub_process_enter_point.init_process_info'

-- 会修改 这里拷贝下
local modules_to_update = EDITOR.utils.deep_copy(require '@xdeditor_startup.modules_to_update')

local _editor_resource_dict = {}
---@return table<string, update_info_row>
function _G.editor_resource_dict()
    return _editor_resource_dict
end

local download_manager = include '@common.update.download_manager'  ---@type DownloadManager

local function convert_and_add_to_res_dict(value)
    local _, end_index = value.map_name:find('_mob')
    if (end_index ~= #(value.map_name)) then
        _editor_resource_dict[value.map_name] = {
            path = value.path,
            name = value.map_name,
            packet_type = math.floor(value.map_type),
            alias = value.alias,
            version = value.version or 1,
            map_source = value.map_source,
        }
    end
end

local function update_editor_resource_dict()
    if argv.has('no_update') then
        return
    end

    local to_update_list = download_manager:update_version_info {
        update_list = modules_to_update,
    }

    _editor_resource_dict = {}
    for _, v in ipairs(to_update_list) do
        _editor_resource_dict[v.name] = v
    end

    local get_my_map_list = require 'http_requests.goods'.get_my_map_list

    local my_models = get_my_map_list({ 9, 11, 1011, 14 }, true)
    for index, value in ipairs(my_models or {}) do
        convert_and_add_to_res_dict(value)
    end
end

local editor_local_resource = require 'window.editor_local_resource'
local function auto_remove_preview_resource()
    editor_local_resource:load()
    local overtime_preview_packages = editor_local_resource:get_overtime_preview_packages()
    if #overtime_preview_packages == 0 then
        return
    end
    -- EDITOR.utils.print_table(overtime_preview_packages)

    -- 需要删除的预览包及依赖包
    local overtime_package_list = download_manager:update_version_info {
        update_list = overtime_preview_packages
    }
    -- EDITOR.utils.print_table(overtime_package_list)

    local local_packages = editor_local_resource:get_local_packages()
    for _, module in ipairs(modules_to_update) do
        local_packages[#local_packages + 1] = module
    end
    -- 本地需保留的包及依赖包
    local local_package_list = download_manager:update_version_info {
        update_list = local_packages,
    }
    -- EDITOR.utils.print_table(local_package_list)

    local local_package_flag = {}
    for _, local_package in ipairs(local_package_list) do
        local_package_flag[local_package.name] = true
    end

    local need_delete_packages = {}
    for _, overtime_package in ipairs(overtime_package_list) do
        if not local_package_flag[overtime_package.name] then
            need_delete_packages[#need_delete_packages + 1] = overtime_package
        end
    end
    -- EDITOR.utils.print_table(need_delete_packages)

    local local_version = require '@common.update.core.local_version'
    local update_path = io.get_app_dir()..'/Update/'.._G.IP .. '/'
    -- 删除包
    for _, need_delete_package in ipairs(need_delete_packages) do
        -- log.alert(need_delete_package.path, need_delete_package.name, need_delete_package.suffix)
        io.remove(update_path .. need_delete_package.path .. '/' .. need_delete_package.name)
        editor_local_resource:remove_preview_resource_package(need_delete_package.name)
        -- 删除包后 设置包版本为0
        local_version:set(need_delete_package.name, 0, need_delete_package.suffix)
    end
    local_version:save()
end

local is_download_map_ref_resource = false
local function download_map_ref_resource(map_name)
    if is_download_map_ref_resource then
        return
    end

    is_download_map_ref_resource = true
    local EProgressBar = SCE:GetEProgressBar()
    local manager
    if argv.has('objv1') then
        manager = require 'plugin.obj_editor'
    else
        manager = require 'plugin.obj_editor_v2'
    end

    -- 拷贝Update或Res目录下地图到User目录, 避免生成Ref相关影响原始地图
    local map_path = io.get_package_path(map_name)
    local user_map_path = EDITOR.utils.add_tailing_slash(io.get_root_dir()) ..
        'User/init_download_maps/' .. map_name .. '/'
    io.remove(user_map_path)
    local ref_dir = user_map_path  .. 'ref/'

    -- 添加地图和libs的pak路径到path seatcher
    SCE.Common.set_package_to_path_searcher(map_name, 'maps/' .. map_name)
    require 'plugin.plugins_manager'
    EDITOR.event_notify(EVENT.add_lib_path, map_path)

    -- 加载数遍进度条
    EProgressBar:move_to_window_center(SCE.GetMainWindow())
    EProgressBar:begin()
    EProgressBar:update_title('加载数据编辑器')
    local get_update_progress_func_clear = EDITOR.utils.get_update_progress_func_clear
    local progress = get_update_progress_func_clear('加载数据编辑器')

    -- 调用数遍相关
    SCE.MAPINFO.init_map_info(map_path)
    local map_info = manager.init_map_info(MapInfo.xdeditor_path, map_path, MapInfo.package_path,
        map_name, true, false, nil, progress)
    map_info.funcs:init_res_data(false)
    local res_data = map_info.funcs:get_res_data()

    local user_editor_ref_path = ref_dir .. 'editor_objref.txt'
    local res_text = ''
    for _, res in ipairs(res_data) do
        res_text = res_text .. res .. '\r\n'
    end
    -- 保存editor_objref
    local save_objref_ret = io.write(user_editor_ref_path, res_text)
    if save_objref_ret ~= 0 then
        log.infof('save editor obj ref failed, code[%s], path[%s]', tostring(save_objref_ret), user_editor_ref_path)
        EProgressBar:end_progress()
        return
    end

    require 'utils.map_download_refs'

    EProgressBar:update_title('计算地图引用')
    local update_progress = get_update_progress_func_clear('计算地图引用')
    local debug_manager = SCE.GetDebugManager()
    -- debug_manager:set_scene_list(EDITOR.utils.get_scene_list())
    debug_manager:generate_ref_async(map_path, false, ref_dir, 'editor')
    local percentage = 0
    local ready = 0
    local success = true
    -- 返回1代表正常完成,返回0代表进行中,返回-1代表执行完了但是出错了
    while ready == 0 do
        ready = debug_manager:check_ref_async()
        percentage = percentage + 0.01
        if ready == 0 and percentage < 0.99 then
            update_progress(percentage)
            coroutine.sleep(100)
        elseif ready == 0 then
            success = false
            ready = -1
        elseif ready == -1 then
            success = false
        end
        if ready ~= 0 then
            update_progress(1)
        end
    end
    if success then
        EProgressBar:update_title('下载资源')
        EProgressBar:update_content('准备下载')
        -- editor_full.ref 在user_map_path下
        EDITOR.event_notify(EVENT.download_map_ref_resources, user_map_path)
    else
        log.infof('generate ref failed')
    end
    EProgressBar:end_progress()
end

---------------------------------------------------
-- 生成原生lua流程
---------------------------------------------------

local function init_map_path_serarcher_event()
    local project_manager = require "project_manager"
    local loaded_project_name
    local loaded_project_id
    -- 按理来说这个应该在加载ui编辑器之前调用，这样ui编辑器才嫩require到当前地图的 ui/script
    EDITOR.event_register(EVENT.load_map, function(trigger, map_path)
        local get_update_progress_func_clear = EDITOR.utils.get_update_progress_func_clear
        local progress = get_update_progress_func_clear('读取地图文件')
        progress(0)
        project_manager.load_project_file(map_path)
        -- SCE.GetProjectSettings():load(map_path)
        progress(0.5)
        SCE.MAPINFO.init_map_info(map_path)
        MapInfo.project_name = project_manager.get_project_name()
        loaded_project_name = util.path_last_part(map_path)
        loaded_project_id = project_manager.get_project_name()
        if type(loaded_project_name) == "string" then
            SCE.Common.set_package_to_path_searcher(loaded_project_name, map_path)
        end
        if type(loaded_project_id) == "string" then
            SCE.Common.set_package_to_path_searcher(loaded_project_id, map_path)
        end
        progress(1)
    end)
    EDITOR.event_register(EVENT.unload_map, function(trigger)
        log.info("EVENT.unload_map path_searcher start")
        if type(loaded_project_name) == "string" then
            SCE.Common.set_package_to_path_searcher(loaded_project_name, nil)
        end
        if type(loaded_project_id) == "string" then
            SCE.Common.set_package_to_path_searcher(loaded_project_id, nil)
        end
        log.info("EVENT.unload_map path_searcher end")
    end)
end

---------------------------------------------------
-- 初始化一些东西
---------------------------------------------------

local update = include '@common.update'

common.get_editor_api_version = function()
    local editor_version_manager = require '@base.update.core.api_version_config'.editor_version_manager
    local api_version_cfg, _ = editor_version_manager:get()
    local api_version = api_version_cfg.api_version
    return api_version
end

-- 阻塞时间较长的操作会导致entrance断开发不出去，要等entrance连上再发
common.stat_sender_co = function(name, tab)
    base.next(coroutine.will_async(function()
        while not scelobby.is_entrance_connected() do
            coroutine.sleep(3000)
        end
        common.stat_sender(name, tab)
    end))
end


local modules_to_update_in_xdeditor = {}
local function update_modules_to_update_in_xdeditor()
    if argv.has('no_update') then
        return
    end
    if #modules_to_update_in_xdeditor > 0 then
        return
    end

    -- 动画预览模型
    local anim_conf = require 'window.art_workbench.anim_libs.anim_conf'
    for index, module in ipairs(anim_conf.get_preview_map()) do
        table.insert(modules_to_update_in_xdeditor, module)
    end

    -- steam 启动编辑器不下载用户捏人资源, 动画预览模型还是需要下载的
    if not argv.has('from_steam_launcher') then
        --追加要更新的模型包，一次更新
        local update_characters1 = require 'window.art_workbench.modeling_editor.common.update_characters1' 
        local my_models = update_characters1.get_my_model()
        -- log.alert('update_modules_to_update_in_xdeditor', #my_models)
        for index, value in ipairs(my_models or {}) do
            local _, end_index = value:find('_mob')
            if end_index ~= #value then  -- 过滤掉_mob格式. 查询时我们只查原始名字, 如果客户端是手机, 则会自动下载_mob
                modules_to_update[#modules_to_update+1] = value
                table.insert(modules_to_update_in_xdeditor, value)
            end
        end
        local official_models = update_characters1.get_official_characters1()
        for index, value in ipairs(official_models or {}) do
            --modules_to_update[#modules_to_update+1] = value  -- TODO @麻兆豫, 这里有问题, 是不是value的格式改了?
        end
    end

    -- 更新xdeditor资源包
    local progress_bind = require '@xdeditor_startup.ui.progress'
    local error_msg = nil
    xpcall(update.try_update, function(err)
        error_msg = err
        log.warn(debug.traceback(err))
    end, {
        maps = modules_to_update_in_xdeditor,
        forbidden_check_binary = true,
        default_part = 3,
        as_editor_resource_map = true,
        reason = 'xdeditor',
        progress_bind = progress_bind
    })
    local sleep = co.wrap(base.wait)
    if error_msg then
        common.send_user_stat('update_error', error_msg)
        log.warn(error_msg)
    end
end

function _G.update_editor_resource_dict()
    print('更新一次已发布包列表')
    co.async(function()
        update_editor_resource_dict()
    end)
end

local desktop_x, desktop_y = common.get_desktop_resolution()

local function show_editor_main_ui()
    -- 把屏幕恢复成全屏

    local argv_w = argv.get('width')
    local argv_h = argv.get('height')
    log.info('args width height:', argv_w, argv_h)
    if argv_w and argv_h then -- 若启动参数有宽高，后面的set_resolution会失败，需要显示窗口
        SCE.GetMainWindow():set_visible(true)
    end

    -- 传true的话就是真全屏(并且把屏幕分辨率改成你传的数)了，现在一般不用真全屏
    -- 获取桌面除任务栏外工作区域的大小
    local workarea_x, workarea_y = common.get_desktop_workarea()
    common.set_resolution(workarea_x, workarea_y, false)
    if argv.has('obj_test') and argv.has('show_on_left') then
        common.set_window_position(- workarea_x, 0)
    else
        common.set_window_position(0, 0)
    end

    _G.DURING_SPLASH_WINDOW = false

    ---------------------------------------------------
    -- 编辑器主界面相关代码
    ---------------------------------------------------
    log.info('X.D.Editor start..')

    require 'io_modifier'

    include 'ui'

    include 'window'

    include 'plugin'

    local trigger_manager = require "trigger.trigger_manager"
    local project_manager = require "project_manager"

    EDITOR.event_register('reload', function()
        if trigger_manager then
            trigger_manager.clear_changes()
            trigger_manager.clear_files()
        end
        if undo_redo_mgr then
            undo_redo_mgr:clear()
        end
        -- local tileEditor = pluginMgr:get_plugin('TileEditor')
        -- if tileEditor then
        --     tileEditor:clear_saved_scenes()
        -- end
        app.reload()
    end)

    SCE.MAPINFO.init_map_info('')

    init_map_path_serarcher_event()
    -- EDITOR.event_register(EVENT.load_map_done, function(trigger, map_path)
    --     base.next(function()
    --         if EDITOR.guide_config == nil then
    --             return
    --         end
    --         local projectSettings = SCE.GetProjectSettings()
    --         local mapSettings = projectSettings:get_module_settings('MapSettings')
    --         local url = mapSettings:get_guide_url()
    --         if #url > 0 and not EDITOR.guide_config.disabled_guide_url[url] then
    --             local show_guide = require 'window.guide_window'.show_guide
    --             local _, connection
    --             _, connection = show_guide('项目介绍', url, function(ctrl)
    --                 if ctrl.checked then
    --                     EDITOR.guide_config.disabled_guide_url[url] = true
    --                     EDITOR.save_guide_config()
    --                 end
    --                 ctrl:disconnect(connection)
    --             end)
    --             return
    --         end
    --         if EDITOR.___NOT_FIRST_TIME_LOADED_MAP then
    --             return
    --         end
    --         EDITOR.___NOT_FIRST_TIME_LOADED_MAP = true
    --         local guide = require 'window.guide_window'.guide
    --         guide('欢迎来到SCE编辑器', EDITOR.guide_config.editor_app)
    --     end)
    -- end)
    -- EDITOR.event_register(EVENT.save_map_progress, function(trigger, map_path)
    --     map_path = map_path or GetMainFrame():GetMapPath()
    --     SCE.GetProjectSettings():save(map_path)
    -- end)

    -- 编辑器初始化之后关闭更新页
    if SCE.Common.async_download_progress then
        SCE.Common.async_download_progress('show',{
            show = false
        })
    end

    -- 获取一下文档链接
    EDITOR.utils.get_doc_url()

    local function load_map()
        local map_path = GetMainFrame():GetMapPath()
        log.info('check_init_path_completeness', map_path)
        EDITOR.update_map_libs(map_path)
        -- local check_result, check_result_message = EDITOR.utils.map_completeness_check(map_path, trigger_manager.get_libs())
        -- if check_result == 0 then
        --     EDITOR.load_map(map_path, true)
        -- elseif map_path and map_path ~= '' then
        --     log.info('[completeness_check] 打开地图失败，'..check_result_message)
        --     local message_window = require 'ui.components.message_window'
        --     message_window.message_window(function(opt)
        --         if opt == message_window.Close then
        --             return
        --         end
        --         if opt == message_window.Confirm then
        --             return
        --         end
        --     end , {confirm_text = '确定', close_text = '取消'}, '打开失败，目标文件不符合SCE项目文件规格。'..check_result_message, '警告')
        --     EDITOR.load_map(GetMainFrame():GetMapPath(), true)
        -- end
        EDITOR.load_map(map_path, true, nil, function(save_map_impl)
            if map_path and map_path ~= '' then
                save_map_impl()
            end
        end)
        if (argv.has('obj_test')) then
            local map_path = argv.get('obj_test_map')
            if (map_path ~= nil and map_path ~= '') then
                EDITOR.load_map(map_path, true)
                local util = require 'plugin.obj_editor_v2.utility'
                local log_path = util.get_newest_editor_log_path()
                if (log_path ~= nil) then
                    util.OS.execute(string.format('code -r -g "%s":99999', log_path))
                end
            elseif (argv.has('obj_test_auto')) then
                local obj_manager = require 'plugin.obj_editor_v2'
                if (obj_manager.unit_test ~= nil) then
                    obj_manager.unit_test:DoTest()
                end
            end
        end
    end

    local function init_tips(need_load_map)
        -- 初始化进度条的提示文本，子进程读不到积分所以主进程读了传过去
        -- base.next(function()
        local co = coroutine.running()
            local function func(score)
                local tips = {}
                for key, value in pairs(score or {}) do
                    table.insert(tips, value.text)
                end
                SCE:GetEProgressBar():init_tips(tips)
                if need_load_map then
                    coroutine.resume(co)
                end
            end
            sce.s.score_init(sce.s.readonly_map, 59, {
                ok = function(score, iscore, sscore)
                    log.debug('tips query success')
                    func(score)
                end,
                error = function(code, reason)
                    log.debug('tips query error :', code, reason)
                    func(nil)
                end,
                timeout = function()
                    log.debug('tips query timeout')
                    func(nil)
                end
            })
            if need_load_map then
                coroutine.yield()
                load_map()
            end
        -- end)
    end

    --初始化加载一次
    if _G.generate_cmd == nil then
        init_tips(true)
    else
        init_tips()
    end

    --sets input focus to the window.
    common.raise_window()

    require 'window.art_workbench'
    require 'window.art_workbench.modeling_editor'

    --这里编辑器加载完了吧，可以执行自动测试了？
    local autotest_app = require 'window.autotest_app'
    base.next(function()
        autotest_app:do_argv_task()
    end)

    --这里编辑器加载完了吧，可以播操作记录了？
    local record_player = require 'window.record_player'
    base.next(function()
        record_player:do_argv_task()
    end)

    -- 更新完把字体换成Regular
    local EMessageBox = ImportSCEContext():GetEMessageBox()
    if EMessageBox then
        EMessageBox:set_font_family('Regular')
    end


    -- 设置编辑器状态为显示主页面
    if type(GetMainFrame().SetSCEEditorState) == "function" then
        GetMainFrame():SetSCEEditorState(97)
    end
end

-- 仅当shader有更新或者本地PC编辑器有Res/Shaders目录时候清理shader cache
-- local clear_shaders = false
-- base.game:event('update shader res', function()
--     log.info('Res/Shaders has new version, will clear shader cache.')
--     clear_shaders = true
-- end)
-- local function check_and_clear_shaders()
--     local shader_res_dir = app.get_resource_root_dir() .. '/Shaders'
--     log.info(('Shader resource path： %s'):format(shader_res_dir))
--     if platform.is_win() and io.exist_dir(shader_res_dir) then
--         log.info('platform is win and shader res dir exists, will clear shader cache')
--         clear_shaders = true
--     end
--     if clear_shaders then
--         log.info('clear shader cache...')
--         SCE.Common.clear_shaders()
--     end
-- end


local guide_config_filename = common.get_app_dir()..'/User/guide_config.json'
EDITOR.save_guide_config = function()
    io.write(guide_config_filename, base.json.encode(EDITOR.guide_config))
end
local err, str = io.read(guide_config_filename)
local default_guide_config = require 'config.preferences.guide_config'
if err ~= 0 then
    -- 使用默认配置
    EDITOR.guide_config = EDITOR.utils.deep_copy(default_guide_config)
else
    local loaded_config = base.json.decode(str)
    local config = EDITOR.utils.deep_copy(default_guide_config)
    for key, value in pairs(loaded_config) do
        config[key] = value
    end
    EDITOR.guide_config = config
end

---------------------------------------------------
-- 如果没加-local_test，就立刻连entrance。加上-local_test的话就会跳过更新流程，一般和-use_local_res结合使用
---------------------------------------------------
local local_test = argv.has('local_test')
local allow_update = false
if local_test then
    local s = argv.get('local_test')
    if s ~= '' then
        local n = tonumber(s)
        allow_update = n == 2 and true or false
    end
end

local function continue_launch_editor()
    if argv.has('generate_and_debug_map') then
        log.info('arg file_path:', argv.get('file_path'))
        local login = include 'ui.login'
        login(function()
            require 'map_starter'
        end)
        return
    end
    if local_test and not allow_update then
        log.info('本地测试，不连接entrannce')

        common.set_resolution(480, 280, false)
        common.raise_window()
        shortcut.load_shortcut_configs()

        show_editor_main_ui()  -- 直接拉起主界面
        --check_and_clear_shaders()
        return
    end
    
    --StartUp里已经把这个函数的前半段执行完了
    local function try_update_and_launch_editor()
        log.info('before check_log.start()')
        --更新完毕清理旧log，等一秒是为了reload让虚拟机重启，避免清理两次log
        check_log.start()
        log.info('will load shortcut configs')
        shortcut.load_shortcut_configs()

        auto_remove_preview_resource()

        if argv.has('test_resource') then
            local flag_path = io.get_app_dir() .. '/User/starup_lua'
            if not io.exist_file(flag_path) then
                -- 下载default_units地图资源
                download_map_ref_resource('default_units')
                io.write(flag_path, '')
            end
        end
        if argv.has('doctor') then
            log.info('开始检测renderdoc动态库')
            local path = io.get_app_dir()
            local renderdoc_path = path..'/Update/' .. _G.update_subpath .. '/Res/renderdoc/renderdoc.dll' 
            if not io.exist_file(path..'/renderdoc.dll') then
                if io.exist_file(renderdoc_path) then
                    local copysuccess = io.copy(renderdoc_path,path..'/renderdoc.dll')
                    if copysuccess then
                        log.info('renderdoc动态库加载成功')
                        local EMessageBox = ImportSCEContext():GetEMessageBox()
                        EMessageBox:set_size(300, 160)
                        EMessageBox:set_font_family('Regular')
                        EMessageBox:begin('编辑器诊断模式已加载成功，请重启编辑器使用;;重启编辑器')
                        common.force_exit()
                    end
                end
            end
        elseif not argv.has('inner') then
            local path = io.get_app_dir()..'/renderdoc.dll'
            if io.exist_file(path) then
                io.remove(path)
            end
        end

        if not local_test then
            local login = include 'ui.login'
            login(function()
                update_modules_to_update_in_xdeditor()
                log.info('update_editor_resource_dict')
                EDITOR.utils.update_http_ip()
                update_editor_resource_dict()  -- 获取资源商店用的数据
                show_editor_main_ui()
            end)
        else
            show_editor_main_ui()
        end
        -- check_and_clear_shaders()


    end

    try_update_and_launch_editor();
    
end

-- 开启输入事件的监听
SCE.StartListenInputEvent()

if event_mgr then
    local callback_map = {}
    local function get_quick_function_lists()
        if sce and sce.ui and sce.ui.main_view and sce.ui.main_view.menu_bar then
            callback_map = sce.ui.main_view.menu_bar.callback_map
        end
    end
    get_quick_function_lists()

    shortcutMgr.register(shortcutMgr.UNDO, function()
        undo_redo_mgr:undo()
    end)
    shortcutMgr.register(shortcutMgr.REDO, function()
        undo_redo_mgr:redo()
    end)
    shortcutMgr.register(shortcutMgr.UPDATE_ASSIST_GRID_SCALE, function()
        local tileEditor = pluginMgr:get_plugin('TileEditor')
        if tileEditor then
            tileEditor:show_assist_grid()
            EDITOR.event_notify(EVENT.change_show_grid_collision)
        end
    end)
    shortcutMgr.register(shortcutMgr.SHOW_COLLISION, function()
        local tileEditor = pluginMgr:get_plugin('TileEditor')
        if tileEditor then
            tileEditor:show_collision()
            EDITOR.event_notify(EVENT.change_show_grid_collision)
        end
    end)
    shortcutMgr.register(shortcutMgr.SHOW_FOG, function()
        local tileEditor = pluginMgr:get_plugin('TileEditor')
        if tileEditor then
            tileEditor:set_show_fog()
            EDITOR.event_notify(EVENT.change_show_fog)
        end
    end)
    shortcutMgr.register(shortcutMgr.SHOW_INDICATORS, function()
        local tileEditor = pluginMgr:get_plugin('TileEditor')
        if tileEditor then
            local show_atmosphere_indicator = tileEditor:is_atmosphere_indicator_showing()
            local show_music_indicator = tileEditor:is_music_indicator_showing()
            local show_camera_indicator = tileEditor:is_camera_indicator_showing()
            local show_light_indicator = tileEditor:is_light_indicator_showing()
            local show_partical_indicator = tileEditor:is_partical_indicator_showing()
            if show_atmosphere_indicator and show_music_indicator and show_camera_indicator and show_light_indicator and show_atmosphere_indicator then  -- 只有当全显示的时候，才会直接全切成不显示
                tileEditor:show_atmosphere_indicator()
                tileEditor:show_music_indicator()
                tileEditor:show_camera_indicator()
                tileEditor:show_light_indicator(false)
                tileEditor:show_partical_indicator(false)
            else    -- 否则只要有一个不显示，就让它显示
                if not show_atmosphere_indicator then
                    tileEditor:show_atmosphere_indicator()
                end
                if not show_music_indicator then
                    tileEditor:show_music_indicator()
                end
                if not show_camera_indicator then
                    tileEditor:show_camera_indicator()
                end
                if not show_light_indicator then
                    tileEditor:show_light_indicator(true)
                end
                if not show_partical_indicator then
                    tileEditor:show_partical_indicator(true)
                end
            end
            EDITOR.event_notify(EVENT.change_show_indicator)
        end
    end)
	shortcutMgr.register(shortcutMgr.NEW, function()
        if callback_map['文件/新建'] == nil then
            get_quick_function_lists()
        end
        if callback_map['文件/新建'] then
            callback_map['文件/新建']()
        end
    end)
    shortcutMgr.register(shortcutMgr.OPEN, function()
        if callback_map['文件/打开'] == nil then
            get_quick_function_lists()
        end
        if callback_map['文件/打开'] then
            callback_map['文件/打开']()
        end
    end)
    shortcutMgr.register(shortcutMgr.SAVE, function()
        if callback_map['文件/保存'] == nil then
            get_quick_function_lists()
        end
        if callback_map['文件/保存'] then
            callback_map['文件/保存']()
        end
    end)
    -- shortcutMgr.register(shortcutMgr.SAVEAS, function()
    --     if callback_map['文件/另存为'] == nil then
    --         get_quick_function_lists()
    --     end
    --     if callback_map['文件/另存为'] then
    --         callback_map['文件/另存为']()
    --     end
    -- end)
    shortcutMgr.register(shortcutMgr.RELOAD_SHADERS, function()
        SCE.Common.reload_shaders()
    end)


    -- message box event
    base.game:broadcast('send_logs', function()
        local upload_log = require '@common.base.upload_log'
        upload_log()
    end)
end

-- winui message box event
_G.upload_log = require '@common.base.upload_log'

if argv.has('show_examples') then
    require 'examples'
end

-- 测试内存泄漏的代码
if debug.dump_traceback then
    base.loop(1000, function()
        collectgarbage('collect')
    end)

    local dir = ('%s/logs/memory_check/%s'):format(io.get_app_dir(), os.date('%Y%m%d_%H%M%S', os.time()))
    log.info(('exists debug.dump_traceback, dir: %s'):format(dir))
    io.create_dir(dir)
    base.wait(120 * 1000, function()
        collectgarbage('collect')
        collectgarbage('collect')
        collectgarbage('collect')
        collectgarbage('collect')
        local file = ('%s/base.txt'):format(dir)
        log.info(('debug.dump_traceback: %s'):format(file))
        debug.dump_traceback(file)
    end)

    local id = 1
    base.loop(60 * 60 * 1000, function()
        collectgarbage('collect')
        collectgarbage('collect')
        collectgarbage('collect')
        collectgarbage('collect')
        local file = ('%s/%d.txt'):format(dir, id)
        log.info(('debug.dump_traceback: %s'):format(file))
        debug.dump_traceback(file)
        id = id + 1
    end)
else
    log.info('debug.dump_traceback not exists')
end

if SCE.GetTiledSceneManager then
    local tiledSceneMgr = SCE.GetTiledSceneManager()
    -- 创建块状场景，这个调用时机应该很早
    tiledSceneMgr:create_tiled_scene(EFFECT_TILED_SCENE_NAME, 128, 128)
end

argv.add('kcp_stream', '1')

-- require('test.scorearchive')

-->> sce_app_editor-patch >>
-- 编辑器补丁插槽（由 sce_app_editor-patch 应用注入，请勿手改）
local __ep_ok, __ep_err = pcall(require, 'sce_app_editor-patch.main')
if not __ep_ok and log_file and log_file.info then
    log_file.info('[sce_app_editor-patch] 框架入口加载失败: ' .. tostring(__ep_err))
end
--<< sce_app_editor-patch <<
return {
    continue_launch_editor=continue_launch_editor,
    argv_has_scene_test=argv_has_scene_test,
    argv_has_sub_process=argv_has_sub_process,
}
