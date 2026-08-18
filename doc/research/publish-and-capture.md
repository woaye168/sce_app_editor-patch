# 0.5.3 M0 实测研究：发布完成感知（R1）与游戏画面截取（R2）

> 研究日期：2026-08-18
> 关联需求：doc/requirements/0.5.3.md（M0 实测研究任务 R1/R2）
> 素材：xdeditor-160 明文镜像（`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/xdeditor-160/`）

## R1：发布项目的完成/失败感知

### 结论

**不用菜单命令透传，也不用轮询日志——官方流程自带 promise 结果通道。**
在 bgd_mcp_bridge 的 Lua 桥新增 `publish_project` handler，直接调 `EDITOR.upload_map(log_mark, promise, ignore_save_map)` 并等 promise 结果，异步 ack / 事件回传。

### 证据链

1. 菜单命令 `发布/发布项目`（menu_bar.lua:2607）：弹 message_window 确认后调 `EDITOR.upload_map()`（无参）。我们的弹窗抑制 wrap 会自动 Confirm，但菜单路径拿不到完成信号。
2. `EDITOR.upload_map` 定义在 utils/event.lua:709，签名 `upload_map(log_mark, promise, ignore_save_map, is_upload_ref)`：
   - `promise`：全程每个成功/失败出口都 `promise:set_result(0|1)`（0=成功，1=失败；覆盖未登录、保存失败、复制失败、上传失败、异常 catch 全部分支）。
   - `log_mark`：传入后全程输出结构化日志 `[<log_mark>]发布地图[<path>]成功/失败: msg`、`进度:...`（upload_map_view.lua:727-768），作为兜底感知通道。
3. promise 的官方用法实证（map_generator/init.lua:37-40，无头发布流程）：
   ```lua
   local promise = base.promise()
   EDITOR.upload_map('upload_map', promise, true)
   local upload_ret = promise:co_result()  -- 协程内挂起等待，0=成功 1=失败
   ```

### 落地设计（0.5.3 M4 前的桥接改造）

- Lua 桥 `handlers.publish_project`：协程内 `base.promise()` → `EDITOR.upload_map('bgd_mcp', promise)`（不传 ignore_save_map，与菜单行为一致先保存地图；确认弹窗天然绕开——它在菜单 handler 里，不在 upload_map 内）→ `co_result()` 拿到 0/1。
- ack 策略：发布耗时分钟级，超出常规 ack 超时。采用「立即 ack `{started=true}` + 完成时 `bgd_mcp_event` 推 `{type='publish_done', ok, code}`」；C# 侧元工具 `publish_project` 收到 started 后等事件（长超时，默认 600s 可配）。
- 兜底：`log_mark='bgd_mcp'` 的结构化日志沉在编辑器主日志，事件通道失效时可查日志。
- danger 分级保持不变（annotations.json 标注 + danger_allow 放行）。

## R2：调试态游戏画面截取

### 结论

**选定路线 a 的加强版：引擎原生 `sceneMgr:snapshot_scene_callback`，经 Lua 桥直调，PNG 落盘任意路径。**
这是 PIE 视图「截图」按钮的官方实现，引擎自己渲染出图，无 D3D 黑屏问题，无需 WGC/PrintWindow。路线 b/c 仅作备选不再投入。

### 证据链

1. PIE 视图截图按钮（ui/gameplay_in_editor_view.lua:537-564）：
   ```lua
   local viewport = sceneMgr:get_ui_viewport(self.ui_name)          -- ui_name = 槽位 id
   local viewport_x, viewport_y = viewport:get_inner_viewport_size()
   sceneMgr:snapshot_scene_callback(self.ui_name, 0, 0, viewport_x, viewport_y, path,
       math.max(game_snapshot_magnification_index - 1, 0.5), nil,
       function(result) -- result > 0 成功
           ...
       end)
   ```
   - `sceneMgr = SCE.GetSceneManager()`（:5）。
   - `ui_name` = 插件槽位 id（:842 `ui_name = id`），单开调试即 `'GamePlayInEditor'`（多开为 GamePlayInEditor1..4）。
   - path 任意（官方用 `MainFrame:GetUserPath()..'screenShot/editor_screenShot_N.png'`），png。
   - 倍率参数：官方 UI 选项映射 `magnification_index - 1`（默认 index 2 → 1.0 原倍），下限 0.5。
   - 游戏未启动时 `get_ui_viewport` 返回 nil（:566 「游戏未启动」提示）——天然的失败前置判断。
2. CppInterface/catalog 反编译检索无 Screenshot/Capture/ReadPixels 导出——托管侧没有同等能力，Lua 桥是唯一通道（与 sceneMgr 其他调用一致走 xdeditor Lua）。

### 落地设计（capture_game）

- Lua 桥 `handlers.capture_game`：
  1. `pluginMgr:is_plugin_ui_loaded('GamePlayInEditor')` 判定调试中，否则报错「游戏未启动」。
  2. `sceneMgr:get_ui_viewport('GamePlayInEditor')` 取内视口尺寸。
  3. 落盘路径由参数指定（C# 侧生成 `<项目>/.bgd/log/screenshots/capture_<时间戳>.png`），`snapshot_scene_callback` 异步回调拿 result。
  4. ack 策略：截图回调通常在数帧内返回，可同步等短超时（如 10s）直接 ack `{ path, width, height }`；超时降级为事件回传。
- C# 侧元工具 `capture_game`：经桥调用，把编辑器内落盘的 png 路径返回给 AI（AI 自行 Read 查看）。
