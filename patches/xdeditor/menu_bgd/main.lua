-- 编辑器顶部菜单「帮助」下增加 bgd_sce_tools 入口
-- 机制（xdeditor 官方事件桥，见 .trae/skills/sce-lib-xdeditor-160/hooks.md）：
--   menu_bar.lua:1134 在模块加载时注册了 EVENT.window_title_bar_register 的监听，
--   任何模块只需 EDITOR.event_notify(EVENT.window_title_bar_register, '菜单/子菜单', callback)
--   即可完成注册，无需自己 require 'ui.menu_bar'（避免组件/时序依赖）。
--   菜单点击链路：C# 触发 'EditorMainTitleMenuBar' → call_command(name)（menu_bar.lua:1066-1069）。

local function logi(m)
    if log and log.info then
        pcall(log.info, m)
    end
end

-- EVENT / EDITOR.event_notify 由 main.lua 加载期的 include 'global' / include 'utils' 注册（main.lua:117-121），
-- 编辑器主流程下此时必然可用；防御式判空以防其他加载路径（scene_test/sub_process 分支）未初始化。
if not (EVENT and EVENT.window_title_bar_register and EDITOR and EDITOR.event_notify) then
    logi('[sce_app_editor-patch] EVENT/EDITOR 未就绪，菜单注册跳过')
else
    local function register()
        EDITOR.event_notify(EVENT.window_title_bar_register, '帮助/bgd_sce_tools', function(item)
            common.open_url('https://github.com/woaye168/bgd_sce_tools')
        end)
        logi('[sce_app_editor-patch] 已注册菜单：帮助/bgd_sce_tools')
    end

    -- 地图加载完成后再注册一次（menu_bar 必已加载并挂好事件监听）；
    -- 同时直接尝试一次（若 menu_bar 已加载则立即生效，无需等地图）。
    if EVENT.load_map_done and EDITOR.event_register then
        EDITOR.event_register(EVENT.load_map_done, register)
    end
    register()
end

local M = {}
return M
