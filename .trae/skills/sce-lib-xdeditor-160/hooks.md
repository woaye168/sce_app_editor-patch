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

## 6. 挂编辑器事件总线监听（EDITOR.event_register，2026-09-01 实机验证）

`EDITOR.event_register(name, callback)` = `base.event_register(editor_events, ...)`（utils/event.lua:804 → script 库 base/event.lua:319），**多监听安全**。三个实锤坑：

1. **回调首参是 trig 自身**：触发器实例方法调用语义（script 库 base/trigger.lua:57 `mt:__call` → `self:callback(...)`）——签名必须 `function(self, arg1, ...) end`，按直觉少写 self 会静默错位；
2. **回调返回非 nil 会中断后续监听投递**（base/event.lua:157-159），纯监听型回调必须返回 nil；
3. **run_lua/load 动态代码里没有 EDITOR/EVENT 全局**（模块 env 隔离）——从已加载模块函数的 `_ENV` upvalue 取：`debug.getupvalue(package.loaded['@xdeditor/ui/menu_bar'].exit_editor, n)` 找 `_ENV`。补丁模块自身代码无此问题。

实战样例（分玩家日志 tee，详见 doc/research/multi-player-debug.md §6）：

```lua
EDITOR.event_register(EVENT.add_info_list, function(self, module, data)
    if module ~= 'debug_client_info' then return end  -- 隐式返回 nil，勿返回值
    local player = data.info_user_info and data.info_user_info.player
    -- ...
end)
```
