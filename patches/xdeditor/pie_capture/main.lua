-- pie_capture：修复 PIE 调试视图「拍照」按钮（截「游戏画面+游戏 UI」，支持倍率）。
-- 行为主体不在本文件：slots/xdeditor/<版本>/ui/gameplay_in_editor_view.lua 覆盖官方文件，
-- 其 on_game_snapshot_click 改为调用本应用的 capture CLI（外部 WGC 捕获）。
-- 本模块职责：加载时把注入的 exe 路径放入全局 _G.BGD_CAPTURE_EXE（拍照 handler 直接读全局——
-- run_lua/其他包上下文里 require 本包文件会落到 @common 搜索器而失败，全局变量最稳）。
local ok, exe = pcall(require, 'sce_app_editor-patch.pie_capture._exe_path')
if ok and type(exe) == 'string' and exe ~= '' then
    _G.BGD_CAPTURE_EXE = exe
end
if log_file and log_file.info then
    log_file.info(('[sce_app_editor-patch] pie_capture 已加载（拍照按钮 = 外部捕获，含游戏 UI；exe=%s）')
        :format(tostring(_G.BGD_CAPTURE_EXE)))
end
