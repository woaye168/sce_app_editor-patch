-- 解除项目文件变更监听：避免外部修改项目文件时编辑器弹出重载提示
-- （内核解锁后 io.add_watch / io.remove_watch 已恢复可用）

local function project_root()
    if not io.get_user_data_path then
        return nil
    end
    local ok, path = pcall(io.get_user_data_path)
    if ok and path then
        return tostring(path)
    end
    return nil
end

-- 移除已存在的项目目录监听
local root = project_root()
if root and io.remove_watch then
    pcall(io.remove_watch, root)
    log_file.info('[sce_app_editor-patch] 已移除项目文件监听: ' .. root)
end

-- 拦截后续对项目目录（含子路径）的监听挂载（编辑器可能在本模块加载后再挂监听）
if io.add_watch then
    local raw_add_watch = io.add_watch
    io.add_watch = function(p, ...)
        local r = project_root()
        local ps = tostring(p)
        if r and ps:sub(1, #r) == r then
            log_file.info('[sce_app_editor-patch] 已拦截项目文件监听: ' .. ps)
            return
        end
        return raw_add_watch(p, ...)
    end
end

local M = {}
return M
