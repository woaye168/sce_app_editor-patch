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

## R2：调试态游戏画面截取（终版：get_screen_rect + WGC 显示器裁剪）

### 结论（真机实证，2026-08-19）

**终版方案：lua 桥读 PIE 视口控件的 `get_screen_rect()`（引擎 UI 逻辑坐标）→ 外部 WGC 截取显示器 → 按「客户区物理/逻辑」比例裁剪**。
产出 = 纯游戏画面 + 游戏 UI（不含编辑器界面），test_res002 真机验证通过（血条/技能/背包等游戏 UI 完整入镜）。

### 为什么不是引擎 snapshot_scene_callback（初版方案的实证否定）

初版选定引擎原生 `sceneMgr:snapshot_scene_callback`（PIE 截图按钮官方实现，gameplay_in_editor_view.lua:537）。
真机验证发现它**只截 3D 场景渲染、不含游戏 UI 覆盖层**，且对自定义相机场景不代表实际呈现帧
（test_res002 截出纯色底）。截图工具的核心目的是验证 UI，该方案不成立，降级为 `lua.capture_game` 兜底能力。

### 终版方案的证据链（逐步实证）

1. **编辑器主区不是 XAML**：EditorMainWindow 只挂 TitleMenuBar，内容区是 re-parent 进来的
   引擎自绘窗口（EditorMainWindow.cs:85-94 SetParent childWindow_）；视觉树里找不到游戏视口的
   SwapChainPanel（C# 视觉树方案不可行）。
2. **SDL 窗口不可直接 WGC**：编辑器内容窗口类名 `SDL_app`，对其创建 GraphicsCaptureItem 实测失败
   （Failed to convert item）——改为截取**所在显示器**（HMONITOR 恒定可截）。
3. **PIE 视口位置有官方 API**：视口是 base.ui 控件树中的 viewport 控件（`base.ui.map['ui-<n>-GamePlayInEditor']`），
   控件元表自带 `get_screen_rect()` 返回引擎 UI 逻辑坐标矩形；`common.get_resolution()` 返回逻辑分辨率
   （编辑器主区 main rect = (0,0,逻辑宽,逻辑高)）。lua 桥 `lua.get_game_view_rect` 直接暴露。
4. **坐标换算**：视口屏幕物理坐标 = SDL 窗口客户区屏幕原点（ClientToScreen）+ 逻辑矩形 × (客户区物理/逻辑) 比例；
   裁剪框 = 屏幕物理坐标 − 显示器原点（GetMonitorInfo，多显示器安全）。

### 落地实现

- lua 桥 `handlers.get_game_view_rect`（read）：返回 `{x, y, width, height, logical_width, logical_height}`；
  视口控件不存在即「游戏未在调试」。
- bgd_sce_tools `editor::capture_editor_window`（MCP `capture_game`）：桥取矩形 → 找编辑器 SDL 内容窗口
  （进程内最大 SDL_app 顶层窗口）→ WGC 截显示器 → `frame.buffer_crop` GPU 裁剪 → png 落盘
  `<项目>/.bgd/log/screenshots/`，返回 `{path, width, height}`。
- 已知边界：① 屏幕抓取读实际呈现内容，编辑器窗口被遮挡时该区域会被遮挡物覆盖（调试时保持编辑器前台）；
  ② 编辑器窗口最小化时截取失败/超时。
