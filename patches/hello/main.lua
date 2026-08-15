-- 示例补丁：验证补丁链路可用
-- 由 sce_app_editor-patch 框架入口加载（内核解锁后，io/os/debug 等函数已恢复）

_G.__EDITOR_PATCH__ = true

local function state(v)
    if v == nil then
        return '禁用'
    end
    return '可用'
end

log_file.info('[sce_app_editor-patch] 示例补丁已加载')
log_file.info(('[sce_app_editor-patch] io.popen[%s] os.execute[%s] debug.getupvalue[%s]')
    :format(state(io.popen), state(os.execute), state(debug and debug.getupvalue)))

local M = {}
return M
