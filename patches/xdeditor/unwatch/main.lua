-- 解除项目文件变更监听（xdeditor 包）：外部修改项目文件时编辑器不再弹重载提示。
-- 为什么必须在 xdeditor 包：项目目录的监听器是 xdeditor/window/file_monitor_window.lua
-- 在编辑器 UI 进程内挂的（io.add_watch(map_path, true, false)，地图加载/切换时挂载），
-- script 包侧的 io.add_watch 包装拦截不到它。
-- project_root 由「编辑器补丁」应用在启用本模块时注入（_project_root.lua，AUTO-GENERATED），
-- 比脚本侧 io.get_user_data_path 等运行时推导可靠。

local function logi(m)
    if log and log.info then
        pcall(log.info, '[sce_app_editor-patch] ' .. m)
    end
end

local ok_root, project_root = pcall(require, 'sce_app_editor-patch.unwatch._project_root')
if not ok_root or type(project_root) ~= 'string' or project_root == '' then
    logi('unwatch: 未注入 project_root（请通过「编辑器补丁」应用重新勾选本模块以注入），跳过')
    return
end

local function normalize(p)
    local s = tostring(p):gsub('\\', '/'):lower()
    if #s > 1 and s:sub(-1) == '/' then
        s = s:sub(1, -2)
    end
    return s
end

local root = normalize(project_root)

local function under_root(p)
    local ps = normalize(p)
    return ps == root or ps:sub(1, #root + 1) == root .. '/'
end

-- 移除已存在的项目目录监听（补丁入口早于 file_monitor_window 挂载，通常无既有监听，此处为兜底；
-- remove_watch 需与 add 时字符串一致，尝试几种变体）
if io.remove_watch then
    pcall(io.remove_watch, project_root)
    pcall(io.remove_watch, project_root:gsub('\\', '/'))
    pcall(io.remove_watch, project_root:gsub('\\', '/') .. '/')
    logi('unwatch: 已移除项目文件监听: ' .. project_root)
end

-- 拦截后续对项目目录（含子路径）的监听挂载
--（file_monitor_window 在地图加载/切换 on_map_path_changed 时挂载；拦截后其日志会记「监视地图目录失败」，属预期）
if io.add_watch then
    local raw_add_watch = io.add_watch
    io.add_watch = function(p, ...)
        if under_root(p) then
            logi('unwatch: 已拦截项目文件监听: ' .. tostring(p))
            return
        end
        return raw_add_watch(p, ...)
    end
end
