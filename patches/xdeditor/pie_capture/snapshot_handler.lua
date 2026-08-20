-- 注意：本文件仅供 examples/make_pie_slot 使用（注入模板），**不随 pie_capture 模块部署**
-- （模块部署文件见 module_files 只挂 main.lua）。
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
