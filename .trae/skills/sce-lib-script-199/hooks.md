# script-199 已验证 hook 配方

## 1. isolation 解锁（内核补丁，editor-patch 已实现）

- 目标：`common/isolation.lua`
- 做法：注释全部 `xxx = nil` 禁用行（编辑器 state 本不进 if 块，解锁主要为 StateGame 调试场景）。slots 文件：`slots/script/<版本>/common/isolation.lua`
- 注意：StateEditor/StateApplication 本就完整，无需解锁

## 2. 框架入口插槽（内核补丁，editor-patch 已实现）

- 目标：`common/init.lua` 末尾追加 `pcall(require, 'sce_app_editor-patch.main')`
- 时机：每个 lua state 加载链第一步；模块代码会每个 state 各执行一次
- slots 文件：`slots/script/<版本>/common/init.lua`

## 3. 解除项目文件监听（unwatch 模块）

- **已迁移到 xdeditor 包**（0.5.x）：项目目录监听器是 xdeditor 进程 `window/file_monitor_window.lua` 挂的（`io.add_watch(map_path, true, false)`，地图加载/on_map_path_changed 时），script 包侧包装拦截不到；且 script 包拿不到可靠项目路径。
- 现做法（`patches/xdeditor/unwatch/`）：应用在勾选时注入 `_project_root.lua`（当前项目根）→ 模块移除既有监听 + 包装 `io.add_watch` 拦截项目根前缀的后续挂载。
- 历史做法（script 包，`io.get_user_data_path` 推导 + StateGame 下 `io.add_watch/remove_watch` 被 isolation.lua:199-200 置 nil 需先解锁）已废弃。

## 4. 覆盖 C++ 全局函数范本

- `common/base/open_url_wrap.lua:91` 演示了官方如何用 Lua 包装替换 C++ 全局（`common.open_url`）——补丁要改 C++ 函数行为时照此模式：保存原引用 → 赋值新函数 → 内部 pcall 调原函数

## 5. StateGame 下调试后门

- 不补丁也可用：`editor_server_debug` / `editor_lobby_debug` argv 让 full() 放开路径限制（isolation.lua:25-44）
- `base.debugger` 返回调试器启动函数（监听 0.0.0.0:4278）
- `base.proto.__shell`（shell.lua:2）：官方预留的服务端下发任意 Lua 执行通道
