local appui = require '@appui'
local SCE = ImportSCEContext()
local MainFrame = GetMainFrame()
local pluginMgr = SCE.GetPluginsManager()
local sceneMgr = SCE.GetSceneManager()
local resolution_data = include 'plugin.tile_editor.ui_resolution_content'
local message_window = require 'ui.components.message_window'
local resolution_change_window = require 'ui.resolution_change_window'
local const = require 'plugin.obj_editor_v2.const'
local obj_manager = require 'plugin.obj_editor_ui.manager.init'
local argv = include '@common.base.argv'
local btn_size = 26
local timer = 1000
local project_manager = require 'project_manager'
local co = require '@common.base.co'
local lobby = require '@common.base.lobby'
local confirm = require '@base.base.confirm'
local module_control_util = require '@common.base.gui.control_util'

local pie_screen_orientation = -1
local resolution_record = -1
local direction_record = 1

local sound_type_map
local settings
local start_pos
local start_relative
local debug_resolution_data = resolution_data.data()


local minute_timer = {
    -- 每次设置的值都是当前的总和才能用这个
    MINUTE = 60,
    set = function(self, key, value)
        if not self[key] then
            self[key] = {next_index = 1}
        end
        self[key][self[key].next_index] = value
        if self[key].next_index == self.MINUTE then
            self[key].next_index = 0
        end
        self[key].next_index = self[key].next_index + 1
    end,
    get_total = function(self, key)
        if not self[key] then return 0 end
        if self[key].next_index >= #self[key] then
            return self[key][#self[key]] - self[key][1]
        end
        local current_index = self[key].next_index - 1
        if self[key].next_index == 1 then
            current_index = 60
        end
        return self[key][current_index] - self[key][self[key].next_index]
    end,
    clear = function(self)
        for key, value in pairs(self) do
            if type(value) == 'table' then
                self[key] = nil
            end
        end
    end,
}

local function get_display_traffic(B)
    local mask = 1024
    local mod_mask = 1023
    if B < mask then
        return string.format('%d B', B)
    end
    local KB = B>>10
    B = B & mod_mask
    if KB < mask then
        return string.format('%.1f KB', KB + B / mask)
    end
    local MB = KB>>10
    KB = KB & mod_mask
    if MB < mask then
        return string.format('%.1f MB', MB + KB / mask)
    end
    local GB = MB>>10
    MB = MB & mod_mask
    return string.format('%.1f GB', GB + MB / mask)
end

local function get_viewport_template(id)
    return base.ui.panel {
        layout = {
            grow_width = 1,
            grow_height = 1,
        },
        bind = {
            layout = {
                grow_width = 'game_view_resolution_grow_width',
                grow_height = 'game_view_resolution_grow_height',
            },
        },
        base.ui.panel {
            name = 'scene_view_border',
            layout = {
                grow_width = 1,
                grow_height = 1,
            },
    
            -- 场景视口，一张渲染场景的纹理
            appui.ui.viewport {
                name = id,
                layout = {
                    grow_width = 1,
                    grow_height = 1,
                },
                radius = {
                    bottom = 5,
                    left = 5,
                    right = 5,
                },
                clip = true,
            },
        },
    }
end


local function get_template(id, debug_user_info)
    local title_name = debug_user_info and string.format('玩家 %d 视图', debug_user_info.player) or '游戏视图'
    local title_icon = debug_user_info and '玩家-触发器1' or 'phone'
    if debug_user_info and debug_user_info.icon_color then
        appui.ui.tabs_bar_proxy.register_icon_color(id, debug_user_info.icon_color)
    end
    return appui.ui.window {
        title_name = title_name,
        title_icon = title_icon,
        id = id,
        dock_width = -1,
        dock_height = -1,
        dock_type = 'center',
        dock_target = 'resource_view',
        fixed = true,
        base.ui.panel {
            layout = {
                grow_width = 1,
                grow_height = 1,
                col_content = 'start',
                direction = 'col',
            },
            clip = true,
            base.ui.panel {
                color = 'rgb(7, 7, 7)',
                layout = {
                    grow_width = 1,
                    height = 2,
                },
            },
            base.ui.panel {
                layout = {
                    grow_width = 1,
                    height = 35,
                    row_content = 'start',
                    direction = 'row',
                },
                -- 分辨率设置
                base.ui.panel  {
                    layout = {
                        width = 140,
                        height = btn_size,
                        row_self = 'start',
                        margin = {left = 7}
                    },
            
                    appui.ui.select {
                        layout = {
                            width = 140,
                            grow_height = 1
                        },
                        name = 'select_game_resolution',
                        enable_drag = false,
                        always_call_on_change = true,
                        options = resolution_data.show_contents(),
                        index = 1,
                        options_width = 'grow',
                        style = scene_button_style,
                        background_opacity = 0.9,
                        bind = {
                            options = 'game_resolution_options',
                            index = 'game_resolution_rate_index',
                            on_change = 'on_select_game_resolution_rate',
                        },
                    },
                },
                -- appui.ui.select {
                --     layout = {
                --         width = 68,
                --         height = btn_size,
                --         margin = {left = 7,},
                --     },
                --     name = 'select_map_resolution',
                --     enable_drag = false,
                --     options = {'横屏', '竖屏'},
                --     index = 1,
                --     options_width = 'grow',
                --     background_opacity = 0.9,
                --     bind = {
                --         index = 'game_resolution_mode_index',
                --         on_change = 'on_select_game_resolution_mode',
                --     },
                -- },
                -- 截图设置
                appui.ui.scene_button {
                    layout = {
                        width = 84,
                        height = btn_size,
                        row_self = 'start',
                        margin = {left = 7},
                    },
                    background_opacity = 0.9,
                    radius = 4,
                    text = '拍照',
                    icon = 'snapshot',
                    style = scene_button_style,
                    bind = {
                        on_click = 'on_game_snapshot_click',
                    },
                },
                appui.ui.select {
                    layout = {
                        -- 此宽度不显示数值
                        width = 30,
                        height = btn_size,
                        row_self = 'start',
                        margin = {left = 0},
                    },
                    name = 'select_map_resolution',
                    enable_drag = false,
                    options = {'x½', 'x1', 'x2', 'x3', 'x4'},
                    index = 2,
                    options_width = 'grow',
                    style = scene_button_style,
                    background_opacity = 0.9,
                    bind = {
                        index = 'game_snapshot_magnification_index',
                        on_change = 'game_snapshot_magnification_on_change'
                    }
                },
                appui.ui.round_corner {
                    layout = {
                        width = -1,
                        height = btn_size,
                        row_self = 'start',
                        margin = {left = 10},
                        padding = {left = 7, right = 7},
                        direction = 'row',
                        row_content = 'start',
                    },
                    radius = 4,
                    color = '#121212',
                    appui.ui.checkbox {
                        layout = {
                            width = 20,
                            height = 20,
                            margin = { right = 10, },
                        },
                        bind = {
                            checked = 'mute_checked',
                            on_change = 'mute_on_change'
                        },
                    },
                    base.ui.label {
                        text = '静音',
                        font = {
                            color = '#E1E1E1'
                        }
                    },
                },
                appui.ui.round_corner {
                    layout = {
                        width = -1,
                        height = btn_size,
                        row_self = 'start',
                        margin = {left = 10},
                        padding = {left = 7, right = 7},
                        direction = 'row',
                        row_content = 'start',
                    },
                    radius = 4,
                    color = '#121212',
                    appui.ui.checkbox {
                        layout = {
                            width = 20,
                            height = 20,
                            margin = { right = 10, },
                        },
                        bind = {
                            checked = 'show_info_checked',
                            on_change = 'show_info_on_change'
                        },
                    },
                    base.ui.label {
                        text = '展示性能与资源统计',
                        font = {
                            color = '#E1E1E1'
                        }
                    },
                },
                appui.ui.round_corner {
                    layout = {
                        width = -1,
                        height = btn_size,
                        row_self = 'start',
                        margin = {left = 10},
                        padding = {left = 7, right = 7},
                        direction = 'row',
                        row_content = 'start',
                    },
                    radius = 4,
                    color = '#121212',
                    base.ui.label {
                        text = '  场景画质：',
                        font = {
                            color = '#E1E1E1'
                        }
                    },
                    appui.ui.select {
                        layout = {
                            width = -1,
                            height = btn_size,
                        },
                        enable_drag = false,
                        options = {'节能', '中', '高'},
                        index = 3,
                        options_width = 'grow',
                        style = scene_button_style,
                        background_opacity = 0.9,
                        bind = {
                            index = 'scene_quality_index',
                            on_change = 'on_select_scene_quality',
                        },
                    },
                },
                -- renderdoc截帧
                (function ()
                    if argv.has('renderdoc_capture') then
                        return
                        appui.ui.scene_button {
                            layout = {
                                width = 84,
                                height = btn_size,
                                row_self = 'start',
                                margin = {left = 7},
                            },
                            background_opacity = 0.9,
                            radius = 4,
                            text = '截帧',
                            icon = 'snapshot',
                            style = scene_button_style,
                            bind = {
                                on_click = 'on_renderdoc_capture_click',
                            },
                        }
                    end
                    return nil
                end)(),
            },
            base.ui.panel {
                layout = {
                    grow_width = 1,
                    grow_height = 1,
                },
                get_viewport_template(id),
                base.ui.panel {
                    z_index = 10000,
                    layout = { width = 273, row_self = 'start', col_self = 'end', direction = 'col', col_content = 'start', margin = 8,},
                    round_corner_radius = 6,
                    clip = true,
                    color = 'rgba(0, 0, 0, 0.85)',
                    base.ui.panel {
                        layout = {
                            row_self = 'end',
                            col_self = 'start',
                            height = 0,
                            direction = 'col',
                            col_content = 'start',
                            margin = 8,
                        },
                        appui.ui.icon {
                            icon = 'drag_icon',
                            size = 20,
                        },
                    },
                    base.ui.panel {
                        layout = {row_self = 'start', grow_width = 1, height = -1, padding = {left = 20, right = 20, bottom = 15}},
                        base.ui.label {
                            text = '',
                            layout = { row_self = 'start', grow_width = 1},
                            font = { color = '#E1E1E1', align = 'left', size = 13},
                            bind = {text = 'info_panel_text' }
                        },
                    },
                    base.ui.panel {
                        layout = {row_self = 'start', grow_width = 1, height = -1, padding = {left = 20, right = 20, top = 10, bottom = 10}},
                        color = '#000000',
                        base.ui.label {
                            layout = { grow_width = 1, height = -1},
                            text = '「调试菜单」选择「模拟手机效果」可以看到更准确的手机端调试数据',
                            font = { size = 13, color = '#FFFFFF', align = 'left'},
                        },
                        bind = {
                            show = 'show_tips',
                        },
                    },
                    bind = {
                        show = 'show_info_panel',
                        event = {
                            on_mouse_down = 'start_drag_panel',
                            on_mouse_up = 'end_drag_panel',
                        },
                        layout = {
                            relative = 'info_panel_relative',
                        },
                    },
                }
            },
        }
    }
end

local function on_select_game_resolution_rate(bind, index)
    local viewport = sceneMgr:get_ui_viewport('GamePlayInEditor')
    if viewport then    -- 获取视口
        -- 先设置C++
        local actualResolution = -1
        if index ~= 1 then  -- 全屏显示
            resolution_record = debug_resolution_data[index].resolution_height / debug_resolution_data[index].resolution_width
            if direction_record == 2 then
                actualResolution = 1 / resolution_record
            else
                actualResolution = resolution_record
            end
        end
        viewport:set_resolution(actualResolution)
        -- 更新ui
        local panel_x, panel_y = viewport:get_outer_panel_size()
        local viewport_x, viewport_y = viewport:get_inner_viewport_size()
        bind.game_view_resolution_grow_width = viewport_x / panel_x
        bind.game_view_resolution_grow_height = viewport_y / panel_y
    end
end


local function get_text_color(value, threshold, upper_limit)
    value         = math.min(value, upper_limit)
    threshold     = math.min(threshold, upper_limit)
    local percent = math.min(math.max(value - threshold, 0) / math.max(upper_limit - threshold, 1), 1.0)
    local gb      = math.ceil(255 * (1 - percent))
    return string.format('#FF%02x%02x', gb, gb)
end


local function mute_on_change(bind, mute)
    if bind then
        bind.mute_checked = mute == true
    end
    if mute then
        common.set_sound_volume(0)
        for _, value in ipairs(sound_type_map) do
            common.set_sound_volume_by_class(value, 0)
        end
        common.set_sound_volume_by_class('Editor', 100)
        common.set_sound_volume_by_class('Master', 100)
    else
        common.set_sound_volume(100)
        for _, value in ipairs(sound_type_map) do
            common.set_sound_volume_by_class(value, 100)
        end
    end
end

local play_in_editor_plugin = {}

function play_in_editor_plugin:init(ui, bind)
    bind.show_tips = not SCE.IsPIEMobileRendererEmulation()
    sound_type_map = {}
    for _, value in ipairs(obj_manager.get_prop_type("$$.Sound_Category").EnumItem) do
        table.insert(sound_type_map, value.Value)
    end
    settings = project_manager.get_editor_project_settings()
    if not settings then
        settings = {debug_settings = {}}
    elseif not settings.debug_settings then
        settings.debug_settings = {}
    end
    local debug_settings = settings.debug_settings

    if debug_settings.custom_resolution then
        debug_resolution_data[#debug_resolution_data].resolution_width = debug_settings.custom_resolution[1]
        debug_resolution_data[#debug_resolution_data].resolution_height = debug_settings.custom_resolution[2]
        debug_resolution_data[#debug_resolution_data].sub_text = string.format('%.0f:%.0f', debug_settings.custom_resolution[1], debug_settings.custom_resolution[2])
    end
    bind.game_resolution_options = resolution_data.show_contents(debug_resolution_data)
    bind.game_resolution_rate_index = debug_settings.game_resolution_rate_index or 1
    -- bind.game_resolution_mode_index = debug_settings.game_resolution_mode_index or 1
    mute_on_change(bind, debug_settings.mute)
    base.next(function()
        self:on_select_game_resolution_rate(bind, bind.game_resolution_rate_index)
        -- self:on_select_game_resolution_mode(bind, bind.game_resolution_mode_index)
    end)
    bind.show_info_checked = debug_settings.show_info_panel
    self:show_info_on_change(bind, bind.show_info_checked)
    local game_snapshot_magnification_index = debug_settings.game_snapshot_magnification_index or 2
    bind.game_snapshot_magnification_index = debug_settings.game_snapshot_magnification_index or 2

    bind.on_select_game_resolution_rate = function(e)
        if debug_resolution_data[e.index].text == '自定义' then
            resolution_change_window.create_ui({debug_resolution_data[e.index].resolution_width, debug_resolution_data[e.index].resolution_height}, function(resolution)
                debug_resolution_data[e.index].resolution_width = resolution[1]
                debug_resolution_data[e.index].resolution_height = resolution[2]
                debug_resolution_data[e.index].sub_text = string.format('%.0f:%.0f', resolution[1], resolution[2])
                self:on_select_game_resolution_rate(bind, e.index)
                debug_settings.game_resolution_rate_index = e.index
                debug_settings.custom_resolution = resolution
                bind.game_resolution_options = resolution_data.show_contents(debug_resolution_data)
                project_manager.set_editor_project_settings(settings)
            end)
        elseif e.index ~= e.old_index then
            self:on_select_game_resolution_rate(bind, e.index)
            debug_settings.game_resolution_rate_index = e.index
            project_manager.set_editor_project_settings(settings)
        end
        self:on_select_game_resolution_rate(bind, e.index)
        debug_settings.game_resolution_rate_index = e.index
        project_manager.set_editor_project_settings(settings)
    end
    -- bind.on_select_game_resolution_mode = function(e)
    --     self:on_select_game_resolution_mode(bind, e.index)
    --     debug_settings.game_resolution_mode_index = e.index
    --     project_manager.set_editor_project_settings(settings)
    -- end

    -- [sce_app_editor-patch/pie_capture] 修复官方拍照：截取「游戏画面 + 游戏 UI」
    -- （官方引擎快照 snapshot_scene_callback 只含 3D 场景，不含游戏 UI 覆盖层）。
    -- 实现：os.execute 调外部捕获（编辑器补丁应用 CLI：WGC 截编辑器主窗口 + PIE 视口控件
    -- get_screen_rect 精准裁剪 + 倍率重采样；编辑器被遮挡/最小化也可截）。
    -- 本文件由 make_pie_slot 注入 gameplay_in_editor_view.lua（替换 on_game_snapshot_click 整段）。
    bind.on_game_snapshot_click = function()
        if game_running_state == 0 then
            sce.system_message_view:new('游戏未启动', 'warn')
            return
        end
        local viewport = sceneMgr:get_ui_viewport(self.ui_name)
        if not viewport then
            sce.system_message_view:new('游戏未启动', 'warn')
            return
        end
        local user_path = MainFrame:GetUserPath()
        if io.exist_dir(user_path .. 'screenShot') == false then
            io.create_dir(user_path .. 'screenShot')
        end
        local magnification = math.max(game_snapshot_magnification_index - 1, 0.5)
        -- 防御：game_snapshot_index 由 update_game_snapshot_index() 初始化，部分路径下尚未执行
        if not game_snapshot_index and update_game_snapshot_index then
            pcall(update_game_snapshot_index)
        end
        game_snapshot_index = (game_snapshot_index or 0) + 1
        local path = ('%sscreenShot/editor_screenShot_%s.png'):format(user_path, game_snapshot_index)

        -- 外部捕获 exe 路径：pie_capture 模块加载时写入全局（注入的 _exe_path.lua）
        local exe = _G.BGD_CAPTURE_EXE
        if type(exe) == 'string' and exe ~= '' then
            local map_path = MainFrame:GetMapPath()
            -- --open-explorer：截完由 CLI 进程用资源管理器选中文件（替代官方打开文件夹行为）
            os.execute(('start "" /min "%s" capture --ratio %s --out "%s" --project-path "%s" --open-explorer')
                :format(exe, magnification, path, map_path))
            sce.system_message_view:new(('拍照中（x%s），完成后自动打开：%s'):format(magnification, path), 'success')
            return
        end

        -- 回退：模块未启用时走官方引擎快照（不含游戏 UI）
        sce.system_message_view:new('pie_capture 未启用，回退引擎快照（不含游戏 UI）', 'warn')
        local viewport_x, viewport_y = viewport:get_inner_viewport_size()
        sceneMgr:snapshot_scene_callback(self.ui_name, 0, 0, viewport_x, viewport_y, path,
            magnification, nil,
            function(result)
                if result > 0 then
                    sce.system_message_view:new(('截图成功:%s'):format(path), 'success')
                    io.open_path_in_explorer(user_path .. 'screenShot')
                else
                    game_snapshot_index = game_snapshot_index - 1
                    sce.system_message_view:new(('截图失败:%s'):format(result), 'error')
                end
            end)
    end

    bind.game_snapshot_magnification_on_change = function(e)
        if e.index ~= game_snapshot_magnification_index then
            game_snapshot_magnification_index = e.index
            debug_settings.game_snapshot_magnification_index = e.index
            project_manager.set_editor_project_settings(settings)
        end
    end

    bind.show_info_on_change = function(e)
        if not e.by_ui then return end
        debug_settings.show_info_panel = e.checked
        project_manager.set_editor_project_settings(settings)
        self:show_info_on_change(bind, e.checked)
    end

    bind.mute_on_change = function(e)
        if not e.by_ui then return end
        mute_on_change(bind, e.checked)
        debug_settings.mute = e.checked
        project_manager.set_editor_project_settings(settings)
    end

    bind.on_renderdoc_capture_click = function(e)
        SCE.RenderDocCaptureOneFrame(true)
    end

    bind.on_select_scene_quality = function(e)
        base.game:send_broadcast('set_render_quality', e.index - 1)
    end

    bind.info_panel_relative = {0, 0}
    
    bind.start_drag_panel = function(button)
        if button == SCE.MOUSEB_LEFT then
            local x, y = common.get_mouse_screen_pos()
            start_pos = {x, y}
            start_relative = bind.info_panel_relative
        end
    end

    bind.end_drag_panel = function(button)
        if button == SCE.MOUSEB_LEFT then
            start_pos = nil
        end
    end
    minute_timer:clear()

    pie_screen_orientation = 1
    self.triggers[#self.triggers + 1] = EDITOR.event_register(EVENT.pie_will_launch, function(_)
        local map_info = obj_manager.obj_base_interface().map_info
        local read_only = map_info.funcs:get_readonly_value()
        local save_entry_id = (read_only or {}).ConfigIni
        if (save_entry_id ~= nil and save_entry_id ~= '') then
            local save_entry = map_info.funcs:get_entry_node(save_entry_id, map_info.path_info.map_name)
            if (save_entry ~= nil) then
                local save_data = save_entry[const.KEYWORD.Cache][const.KEYWORD.PropertyCategory.Game]
                if (save_data.generate_control == true) then
                    pie_screen_orientation = save_data.is_landscape == 1 and 1 or 2
                end
            else
                log.error('找不到用于调试横竖屏config.ini的实例记录：', save_entry_id)
            end
        end

        log.info('current screen orientation:', pie_screen_orientation)
        self:on_select_game_resolution_mode(bind, pie_screen_orientation)
    end)

    self.triggers[#self.triggers + 1] = base.game:event('鼠标-松开', function (trg, button)
        if button == 'button_left' then
            base.next(function()
                self:update_panel_size(bind)
            end)
        end
    end)

    self.triggers[#self.triggers + 1] = base.game:event('游戏-开始', function (trg, button)
        log.error('游戏开始')
    end)

    self.triggers[#self.triggers + 1] = lobby.register_event('luaState广播', function(key, data)
        if key == '退出' then
            log.info('luaState广播 接受到了退出')
            if data.show_confirm and not confirm.is_show() then
                local function find_child(parent, name)
                    for _, child in ipairs(parent.child) do
                        local find = child.name == name and child or find_child(child, name)
                        if find then
                            return find
                        end
                    end
                end
                local viewport = find_child(ui, self.ui_name)
                if viewport then
                    module_control_util.move_to_new_parent(confirm.get_ui(), viewport)
                end
                co.async(function()
                    confirm.set_title(base.i18n.get_text('提示'))
                    confirm.set_text_style({ color = '#dddddd' , size = 20 })
                    local result = confirm.confirm(base.i18n.get_text('确认退出游戏？'), base.i18n.get_text('退出'))
                    if result then
                        sceneMgr:hide_game_in_editor(self.ui_name)
                        pluginMgr:unload_plugin_ui(self.ui_name)
                        pluginMgr:unregister_plugin_ui(self.ui_name)
                    end
                end)
            else
                base.next(function()
                    sceneMgr:hide_game_in_editor(self.ui_name)
                    pluginMgr:unload_plugin_ui(self.ui_name)
                    pluginMgr:unregister_plugin_ui(self.ui_name)
                end)
            end
        end
    end)
end



function play_in_editor_plugin:on_select_game_resolution_rate(bind, index)
    local viewport = sceneMgr:get_ui_viewport(self.ui_name)
    if viewport then    -- 获取视口
        -- 先设置C++
        local actualResolution = -1
        if index ~= 1 then  -- 全屏显示
            resolution_record = debug_resolution_data[index].resolution_height / debug_resolution_data[index].resolution_width
            if direction_record == 2 then
                actualResolution = 1 / resolution_record
            else
                actualResolution = resolution_record
            end
        end
        viewport:set_resolution(actualResolution)
        -- 更新ui
        local panel_x, panel_y = viewport:get_outer_panel_size()
        local viewport_x, viewport_y = viewport:get_inner_viewport_size()
        bind.game_view_resolution_grow_width = viewport_x / panel_x
        bind.game_view_resolution_grow_height = viewport_y / panel_y
    end
end


function play_in_editor_plugin:on_select_game_resolution_mode(bind, index)
    direction_record = index
    local viewport = sceneMgr:get_ui_viewport(self.ui_name)
    if viewport then    -- 获取视口
        -- 先设置C++
        local actualResolution = -1
        if index == 1 or resolution_record < 0 then
            actualResolution = resolution_record
        else
            actualResolution = 1 / resolution_record
        end
        viewport:set_resolution(actualResolution)
        -- 更新ui
        local panel_x, panel_y = viewport:get_outer_panel_size()
        local viewport_x, viewport_y = viewport:get_inner_viewport_size()
        bind.game_view_resolution_grow_width = viewport_x / panel_x
        bind.game_view_resolution_grow_height = viewport_y / panel_y
    end
end

function play_in_editor_plugin:update_panel_size(bind)
    local viewport = sceneMgr:get_ui_viewport(self.ui_name)
    if viewport then
        local panel_x, panel_y = viewport:get_outer_panel_size()
        local viewport_x, viewport_y = viewport:get_inner_viewport_size()
        local new_grow_width = viewport_x / panel_x
        local new_grow_height = viewport_y / panel_y
        if not bind.game_view_resolution_grow_width or math.abs(bind.game_view_resolution_grow_width - new_grow_width) > 0.005 then
            bind.game_view_resolution_grow_width = new_grow_width
        end
        if not bind.game_view_resolution_grow_height or math.abs(bind.game_view_resolution_grow_height - new_grow_height) > 0.005 then
            bind.game_view_resolution_grow_height = new_grow_height
        end
    end
end

function play_in_editor_plugin:show_info_on_change(bind, show)
    bind.show_info_panel = show == true
    if bind.show_info_panel then
        if not self.trigger then
            self.trigger = base.game:event('游戏-更新', function(_, update_delta)
                timer = timer + update_delta
                if timer > 1000 then
                    local send_traffic, pushed_traffic = common.get_traffic()
                    minute_timer:set('traffic', send_traffic + pushed_traffic)
                    local draw_call_detail = sceneMgr:get_draw_call_detail(self.ui_name) or {}
                    local draw_call = draw_call_detail.total or 0
                    local draw_call_detail_text = string.format(
[[  静态模型<#919191:::>%d  骨骼模型<#919191:::>%d  地形<#919191:::>%d
  粒子<#919191:::>%d  Quad<#919191:::>%d  未知<#919191:::>%d]],
                        draw_call_detail.static_mesh or 0,
                        draw_call_detail.skeleton_mesh or 0,
                        draw_call_detail.terrain or 0,
                        draw_call_detail.particle or 0,
                        draw_call_detail.quad or 0,
                        draw_call_detail.unknown or 0)
                    local active_bones = sceneMgr:get_active_bones(self.ui_name) or 0
                    local active_primitives = sceneMgr:get_active_primitives(self.ui_name) or 0
                    bind.info_panel_text = string.format(
[[Ping<#919191:::>  <b>%d</b>
FPS<#919191:::>  <b>%d</b>
当前内存占用<#919191:::>  <b>%.2f%s</b>
服务器负载<#919191:::>  <b>%d%%</b>
渲染批次<#919191:::><b>%d%s</b>
%s
累计服务器流量<#919191:::>  <b>%s</b>
每分钟服务器流量<#919191:::>  <b>%s</b>
云变量请求次数<#919191:::>  <b>%d</b>
单位数量<#919191:::>  <b>%d</b>
等待释放的单位数量<#919191:::>  <b>%d</b>
客户端单位数量<#919191:::>  <b>%d</b>
屏幕内及附近单位数量<#919191:::>  <b>%d</b>
Buff数量<#919191:::>  <b>%d</b>
特效发射器数量<#919191:::>  <b>%d</b>
Jank次数<#919191:::>  <b>%d</b>
服务端GC次数<#919191:::>  <b>%d</b>
调试服务全网使用率<#919191:::>  <b>%.2f%%</b>
活跃骨骼数<#919191:::>  <b>%d%s</b>
活跃模型面数<#919191:::>  <b>%d%s</b>]],
                    common.get_current_ping(),
                    common.get_current_fps() or 0,
                    common.get_current_memory()/1024/1024 or 0 , 'MB',
                    math.ceil(common.get_server_cost() * 100 / 33),
                    draw_call, SCE.IsPIEMobileRendererEmulation() and string.format('(%.f%%建议上限)', draw_call / 500.0 * 100.0) or '',
                    draw_call_detail_text,
                    get_display_traffic(send_traffic + pushed_traffic),
                    get_display_traffic(minute_timer:get_total('traffic')),
                    common.get_score_call_count(),
                    common.get_unit_count(),
                    common.get_unit_wait_gc_count(),
                    common.get_client_unit_count(),
                    common.get_ticked_unit_count(),
                    common.get_buff_count(),
                    common.get_effect_emitters_count(),
                    common.get_jank_count(),
                    common.get_server_GC_count(),
                    common.get_server_cpu_usage(),
                    active_bones, SCE.IsPIEMobileRendererEmulation() and string.format('(%.f%%建议上限)', active_bones / 3700.0 * 100.0) or '',
                    active_primitives, SCE.IsPIEMobileRendererEmulation() and string.format('(%.f%%建议上限)', active_primitives / 180000 * 100.0) or '')
                    timer = 0
                end
                if start_pos then
                    local x, y = common.get_mouse_screen_pos()
                    local new_pos = {start_relative[1] + x - start_pos[1], start_relative[2] + y - start_pos[2]}
                    bind.info_panel_relative = new_pos
                end
            end)
        end
    else
        if self.trigger then
            self.trigger:remove()
        end
    end
end

function play_in_editor_plugin:pre_remove()
    confirm.hide()
    module_control_util.move_to_new_parent(confirm.get_ui(), base.ui.map['main'])
    for i = 1, #self.triggers do
        self.triggers[i]:remove()
    end
    self.triggers = {}
    if self.trigger then
        self.trigger:remove()
    end
    mute_on_change(nil, false)
end

return function(id, debug_user_info)
    local plugin = {
        ui_name = id,
        ui_template = get_template(id, debug_user_info),
        is_new_ui = true,
        slot_id = 'plugin_attribute_slot',
        remove_flag = true,
        triggers = {},
    }
    
    return setmetatable(plugin, {
        __index = function(self, func)
            return rawget(self, func) or play_in_editor_plugin[func]
        end
    })
end
