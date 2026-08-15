# script-199 关键全局与 API

> 签名实证见 files/ 文档行号；`@base` 桩的真实实现在 client_base 库（未研究），此处为调用点归纳。

## C++ 预置全局（无 Lua 定义，直接用）

| 全局 | 说明 |
|---|---|
| `log` / `log_file` | 日志通道（log_file 由 C++ 创建，common/init.lua:8-10 兜底 `log_file = log`）。`log.info/debug/error`、`log_file.info/warn` |
| `common` | C++ 全局表：`common.open_url(url)`、`common.set_localization_language`、`common.get_default_language`、`common.has_arg(name)`、`common.clear_shaders()`、`common.get_binary_version()`、`common.get_editor_api_version()` 等 |
| `include` | 引擎加载器（见 architecture.md） |
| `cmsg_pack` | 消息打包（`set_max_pack_byte_count`，isolation 后置 nil） |
| `__MAIN_MAP__` / `__lua_state_name` | 引擎注入的地图名 / state 名 |

## Lua 层关键模块

| 模块 | 要点 |
|---|---|
| `base` | base/init.lua 总装配：`base.event_register/event_dispatch`、`base.ui`、`base.timer(ms, fn)`、`base.event.on_ui_tick` |
| `base.argv`（桩） | `argv.has(name)` / `argv.get(name)`——命令行参数判断（如 `editor_server_debug`、`auto_test`） |
| `base.util`（桩） | `util.split` 等工具 |
| `base.path`（桩） | 路径对象（`/ `拼接、`is_absolute`、`str`） |
| `base.co`（桩） | `co.wrap/async/sleep` 协程 |
| `base.platform`（桩） | `is_app()/is_mobile()/is_web()` |
| `base.ui` | 见下「UI 框架」 |
| `localization` | `set_language/get_language/add_resource_path`；定义全局 `_G.set_language`（common/localization.lua:3） |

## UI 框架（base/ui/ui.lua）

- `base.ui.component(type_name, base)`（:584）：组件**类工厂**——每次调用新建组件类（`cui_<name>`，重名自动加计数后缀），非单例注册表
- `base.ui.create(view)` / `base.ui.create_ui_root` / `base.ui.panel{}` / `base.ui.template`（:561）/ 控件元表 `mt`（:740）
- **控件创建是帧延迟的**：create 进 wait_to_create 队列，`base.event.on_ui_tick` 统一实例化——当帧查 `base.ui.map` 拿不到
- C++→Lua UI 事件唯一入口：全局 `ui_events` 表（base/ui/event.lua:298-308），改表项可全局拦截
- 两代 UI 并存：旧式 `base.ui.component`（本库）vs 新式 `@common.base.gui.component`（client_base）

## isolation 阉割速查（StateGame 限定）

- 被包装（路径重定向到 User/maps/<地图>）：`io.write/read/copy/rename/remove/copy_to_folder/create_dir/exist_dir/exist_file/walk_dir/list/attribute_type/file_time`、`dofile/loadfile/load`（loadfile/load 强制 mode='t'）
- 被置 nil：`io.popen`、`io.add_watch/remove_watch`、`io.select_file(s)/select_folder(_new)/open_path_in_explorer/show_file_in_explorer`、`io.add_resource_path/remove_resource_path`、`io.read_pak_entries/extract_pak(_file)`、`io.serialize` 系、`os.execute/exit/remove/rename/setlocale/tmpname`、`debug.getregistry/getupvalue/setlocal/getlocal/upvaluejoin/sethook/setupvalue/setuservalue/upvalueid/gethook`、`package.loadlib`、`cmsg_pack.set_max_pack_byte_count`
- `package` 套了 metatable：写 `package.path` 逐条校验（禁 `..`/盘符/`/` 开头）
- 官方后门：`editor_server_debug` / `editor_lobby_debug` argv 放开路径限制（isolation.lua:25）
