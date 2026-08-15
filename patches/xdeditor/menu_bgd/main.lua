-- 编辑器顶部菜单「帮助」下增加 bgd_sce_tools 入口
-- menu_bar.lua 返回 window_title_bar 组件实例（require 模块缓存，单例），此处复用同一实例注册

local function logi(m)
    if log_file and log_file.info then
        pcall(log_file.info, m)
    end
end

local ok, wtb = pcall(require, 'ui.menu_bar')
if ok and type(wtb) == 'table' and wtb.register then
    wtb.register('帮助/bgd_sce_tools', function(item)
        common.open_url('https://github.com/woaye168/bgd_sce_tools')
    end)
    logi('[sce_app_editor-patch] 已注册菜单：帮助/bgd_sce_tools')
else
    logi('[sce_app_editor-patch] 菜单注册失败: ' .. tostring(wtb))
end

local M = {}
return M
