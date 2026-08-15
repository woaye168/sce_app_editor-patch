# script-199 架构与加载机制

> 证据见 files/ 各文档标注的行号；本文只归纳。

## 库定位

- 包目录：`Res/_m/script/199/script/`；**require 根 = 其下 `common/`**（`require 'base.path'` → `common/base/path.lua`）
- 加载入口：`common/init.lua`（引擎为每个 lua state 加载的第一个脚本）

## 加载链

```
common/init.lua          # log_file 兜底（:8-10）→ require 'main'（:12）
  └─ common/main.lua     # require 'base' → 'json' → include 'base.console'
                         # → require 'update'（预载，isolation 后 io 被阉割）
                         # → argv 分流（test / unit_test，unit_test 提前 return 不走 isolation）
                         # → 最后一行 require 'isolation'（:44）
       └─ common/isolation.lua  # 见 hooks.md「isolation 解锁」
```

## lua state 模型

- `__lua_state_name`（C++ 注入）标识当前 state，实测取值：`'StateGame'`（游戏运行时）、`'StateEditor'`（编辑器）、`'StateApplication'`（应用/大厅，base/template/scene.lua:82）。
- 同一进程多个独立 state，**每个 state 各自执行一遍完整加载链**——script 库的入口插槽代码也会每个 state 各跑一次。
- isolation 的阉割只在 StateGame 生效（isolation.lua:24）；StateEditor/StateApplication 仅改写 `io.get_user_data_path`（:20-22）。

## include vs require

- `require`：标准 Lua，走 `package.loaded` 缓存，模块只执行一次。
- `include`：**C++ 注册的引擎全局**（本库无 Lua 定义），从用法看每次调用重新执行文件（配合热更），用于入口型脚本（base.console、test 用例、xdeditor 的 global/utils/config）。
- `@` 前缀跨库引用：`require '@base.reload'` = client_base 库；`@common.xxx` 也指向 client_base 的 common 部分。本库大量文件只是 `return require '@base.xxx'` 转发桩。

## 与编辑器补丁相关的结构

- `common/sce_app_editor-patch/`（补丁框架目录，editor-patch 应用创建）在 require 根下，入口插槽 `pcall(require, 'sce_app_editor-patch.main')` 注入在 `common/init.lua` 末尾。
- 引擎回调保护链：base/init.lua 建立的 `game_events` 等回调表经 xpcall 包装，补丁回调抛错不会炸引擎。
