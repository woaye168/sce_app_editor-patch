# xdeditor-160 已验证 hook 配方

## 1. 加顶部菜单（menu_bgd 模块已实现，本配方即 0.3.1 需求2 的解）

**正确机制：事件桥 `EVENT.window_title_bar_register`，不要在入口模块 require 'ui.menu_bar'。**

```lua
-- EVENT/EDITOR 在 main.lua:117-121 初始化链后就绪（插槽在 main.lua 末尾，必然可用）
local function register()
    EDITOR.event_notify(EVENT.window_title_bar_register, '帮助/xxx', function(item)
        common.open_url('https://example.com')
    end)
end
if EVENT.load_map_done and EDITOR.event_register then
    EDITOR.event_register(EVENT.load_map_done, register)  -- menu_bar 必已加载时再注册一次
end
register()  -- menu_bar 已加载则立即生效
```

**为什么**：
- menu_bar 在登录成功后才加载（main.lua:473），入口插槽执行时它多半未加载；
- `window_title_bar` 是组件类工厂产物（menu_bar.lua:14），提前 require 会新建组件类且触发重依赖链；
- menu_bar.lua:1134 加载时注册了 `EVENT.window_title_bar_register` 监听，事件 notify 即完成注册（register → `call_cs_function('RegisterItem')` 直达 C# 菜单栏），与加载顺序完全解耦；
- 官方同款用法：trigger_editor_app.lua:1660、utils/event.lua:314。

**反例（0.3.0 踩过的坑）**：入口模块 `pcall(require, 'ui.menu_bar')` 然后 `register`——要么 require 失败被 pcall 吞掉，要么注册到错误的组件类上，菜单不出现且无日志。

## 2. 感知窗口创建

包装 `base.ui.create_ui_root`（window/window_app.lua:27）或 `_G.WINDOW_APP_MANAGER:handle_window`（win_app_manager.lua:162），可感知全编辑器窗口创建。

## 3. io 行为定制

编辑器 state 下可随意包装 `io.*` 全局函数；官方样板 `io_modifier.lua`（show_editor_main_ui 内 main.lua:471 加载，包装 io.write/rename/remove 等加 skip_watch）。补丁在其后叠加再包一层即可。

## 4. 插件扩展

- 编辑器级：`class(X, SCE.Plugin)` + `register_plugin`（plugin/sample/ 样例）
- 地图级：地图目录 `ui/script/plugin/init.lua` 提供 load/unload（plugins_manager.lua:446-503）
- 地编创建面板加页签：`tile_editor/create_panel.create_panel.add_panel` / `plugin_template.add_ui`

## 5. 主/子进程区分

`ProcessInfo.is_main_process`（main.lua:124 后可用）；菜单/窗口类补丁只在主进程有意义（register 的 process_type 参数或直接判 ProcessInfo）。
