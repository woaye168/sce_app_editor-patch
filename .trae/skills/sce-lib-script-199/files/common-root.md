# script-199 / common 根级 .lua 逐文件研究记录

> 研究对象：`D:\sce_online\Res\maps\bgd_glzy\.editor_src_mirror\script-199\common\` 根级 12 个 .lua（不含 base/、test/ 等子目录）。
> 全部结论来自真实读取，关键结论标注行号。源文件注释为 GBK 编码。

## 加载链总结

**加载顺序**：`common/init.lua` → `common/main.lua` → `common/isolation.lua`（链式 require，同一 Lua state 内顺序执行）。

1. `common/init.lua:8-10` —— 兜底 `_G.log_file = log`（log_file 本体由 C++ 端注册，此处仅兼容）；`:12` `require 'main'`。
2. `common/main.lua:12` 打日志 `load @common.main begin..`；`:13-17` 依次 `require 'base'`、`require 'json'`、`include 'base.console'`、`require 'update'`（注释明确：**预先加载 update，因为之后 io.write/read 会被阉割**）、`require 'uninstall.generate_count'`；`:19` 设置 `cmsg_pack.set_max_pack_byte_count(102400)`（isolation 之后该函数会被置 nil）；`:21-25` web 平台回调；`:27-30` 若有 `-test` 参数则 `include ('test.' .. 参数)`；`:33-42` 若有 `unit_test` 参数则跑 `@common.base.example.main` 并 **return 中断后续加载**（不走 isolation）。
3. `common/main.lua:44` —— 最后一行 `require 'isolation'`，即 **isolation 是整个 common 加载链的收尾**，在它之后所有危险 API 已被阉割。

**`__lua_state_name` 的含义与取值**：
- 含义：引擎（C++）注入的全局字符串，标识当前代码运行在哪个 Lua state（虚拟机实例）。同一进程内存在多个独立 Lua state，各 state 分别执行一遍这条加载链。
- 本镜像中实际出现的取值（grep 全库）：`'StateGame'`（游戏运行时 state）、`'StateEditor'`（编辑器 state）、`'StateApplication'`（应用/大厅 state，见 base/template/scene.lua:82）。
- **isolation 的阉割只在 `__lua_state_name == 'StateGame'` 时生效**（common/isolation.lua:24）；StateEditor / StateApplication 下 isolation.lua 仅改写 `io.get_user_data_path`（:20-22），不做任何禁用。这就是编辑器补丁的窗口：**编辑器 state 里 io/os/debug/package.loadlib 全部完好**。

**`include` 与 `require` 的区别**：
- `common/reload.lua` 本体只有一行 `return require "@base.reload"`（reload.lua:1），真正的 reload/include 实现已迁移到 **client_base 库（@base.reload），不在本镜像内**，无法从镜像直接取证其实现细节。
- 可从镜像确认的事实：`include` 在 script-199 全库中**没有任何 Lua 定义**（grep 无 `_G.include`/`function include`），与 `log`、`log_file`、`common`、`cmsg_pack` 一样是 **C++ 端注册的引擎全局**。使用点：main.lua:15 `include 'base.console'`、main.lua:29 `include ('test.' .. ...)`。
- 从用法推断（标注为推断，非镜像实证）：`require` 走标准 `package.loaded` 缓存、同名模块只执行一次；`include` 由引擎实现、不走 package.loaded，每次调用重新加载文件，配合 reload 模块支持热更——需要重新执行的入口型脚本（console、test 用例）用 include，库模块用 require。

## isolation.lua 完整梳理

`common/isolation.lua` 分两段：**所有 state 都执行的前置段** + **仅 StateGame 执行的阉割段**。

### 所有 state 生效（:10-22）
- 保存 `main_map = __MAIN_MAP__`（:16）、`debug_traceback = debug.traceback`（:17）。
- 计算 `root_path = <app_dir>/User/maps/<main_map>`（:18-19）。
- **改写 `io.get_user_data_path`** 使其返回地图目录（:20-22）。

### 仅 StateGame 生效（:24 起的 if 块）
- `is_editor_debug = argv.has("editor_server_debug") or argv.has("editor_lobby_debug")`（:25）——**编辑器调试模式下路径限制全放开**（full() 允许 `..` 与绝对路径，:29、:36-37）。
- `full(p)`（:27-44）：非调试态下禁止 `..`、禁止绝对路径，相对路径强制拼到 `User/maps/<main_map>` 下。

**被包装（路径受限）的函数**（全部经 full() 重定向到地图目录）：
1. `io.write`（:47）
2. `io.read`（:57）
3. `io.copy`（:67）
4. `io.rename`（:78）
5. `io.remove`（:89）
6. `io.copy_to_folder`（:99）
7. `io.create_dir`（:110）
8. `io.exist_dir`（:120）
9. `io.exist_file`（:125）
10. `io.walk_dir`（:130）
11. `io.list`（:135）
12. `io.attribute_type`（:140）
13. `io.file_time`（:145）
14. `_G.dofile`（:150）
15. `_G.loadfile`（:155，另强制 mode='t' 禁二进制）
16. `_G.load`（:161，强制 mode='t'）

**被置 nil（直接禁用）的函数**：
- io 资源遍历/包操作：`io.walk_resource_dir`、`io.walk_absolute_dir`、`io.popen`、`io.check_resource_dir`、`io.check_resource_file`（:166-170）
- 序列化：`io.deserialize`、`io.serialize`、`io.is_serializing`、`io.read_cache`（:172-175）
- pak 操作：`io.read_pak_entries`、`io.extract_pak`、`io.extract_pak_file`（:177-179）
- 网络文件：`io.copy_cache_file`、`io.download_file`（:181-182）；`io.upload_file` 仅在没有 `auto_test` 参数时禁用（:183-185）
- 资源路径：`io.add_resource_path`、`io.remove_resource_path`（:189-190）
- 文件对话框/资源管理器：`io.select_file`、`io.select_files`、`io.select_folder`、`io.select_folder_new`、`io.open_path_in_explorer`、`io.show_file_in_explorer`（:192-197）
- **文件监听：`io.add_watch`、`io.remove_watch`（:199-200）**
- `io.empty_method`（:202）、`io.get_package_path`（:204）
- os：`os.execute`、`os.exit`、`os.remove`、`os.rename`、`os.setlocale`、`os.tmpname`（:208-213）
- debug：`debug.getregistry`、`debug.getupvalue`、`debug.setlocal`、`debug.getlocal`、`debug.upvaluejoin`、`debug.sethook`、`debug.setupvalue`、`debug.setuservalue`、`debug.upvalueid`、`debug.gethook`（:215-224）
- `cmsg_pack.set_max_pack_byte_count`（:226）
- `package.loadlib`（:248）；且 `_G.package` 被套 metatable（:230-246），**写 `package.path` 时逐条校验**：禁止 `..`、禁止盘符绝对路径、禁止 `/` 开头。
- 收尾 `io.create_dir('.')`（:206，建地图目录）、日志 `执行绝地天通完成`（:251）。

**补丁含义**：script 库现有 unwatch 类需求若目标是 StateGame，禁用在 Lua 层；但 StateEditor/StateApplication 完全不经此阉割，编辑器补丁在编辑器 state 内可直接使用完整 io/os/debug。

---

## common/init.lua
- 用途：common 库加载入口，log_file 兼容兜底后拉起 main。
- 导出：无导出（执行型脚本）。
- 依赖：`require 'main'`（init.lua:12）。
- 补丁相关：`if not _G.log_file then _G.log_file = log end`（:8-10）——证实 **`log` 与 `log_file` 均为 C++ 预置全局**，log_file 是日志文件通道（C++ 端注册，GBK 注释 :7 大意为「log_file 由 C++ 端创建，这里只是为了兼容」）。本文件是 C++ 创建 Lua state 后执行的第一个脚本（库入口），**入口插槽可插在第 12 行 require 'main' 之前**。

## common/main.lua
- 用途：common 库主引导：加载基础库、按命令行参数分流（test/unit_test），最后落地 isolation。
- 导出：无导出（执行型脚本）；unit_test 分支提前 return（:41）。
- 依赖：`require 'base'`（:13）、`require 'json'`（:14）、`include 'base.console'`（:15）、`require 'update'`（:16）、`require 'uninstall.generate_count'`（:17）、`require 'base.platform'`（:21）、`require 'base.argv'`（:27）、`require '@base.base.util'`（:32，**@ 前缀跨库引用 client_base**）、`require '@common.base.example.main'`（:38）、`require 'isolation'`（:44）。
- 补丁相关：
  - 读取引擎全局 `__MAIN_MAP__` 派生 `_G.__GAME_ID__`（:3-10，去 `_eq` 后缀）。
  - `log_file.info("load @common.main begin..")`（:12）——加载时机日志锚点。
  - :16 注释「预先加载update, 因为之后io.write/read会被阉割」——**实证 isolation 的阉割点在所有业务加载之后**。
  - `argv.get('test')` 分支（:28-30）与 `common.has_arg("unit_test")` 分支（:33-42）——**命令行参数可改变加载路径**，unit_test 会跳过 isolation（:41 return）。
  - 可 hook 点：:44 之前插入补丁代码可在「未阉割」窗口执行。

## common/isolation.lua
- 用途：沙箱隔离（「绝地天通」），StateGame 下阉割危险 io/os/debug API 并重定向文件操作到地图目录。
- 导出：无导出（执行型脚本）。
- 依赖：`require 'base.path'`（:10）、`require 'base.argv'`（:11）、`require 'base.util'`（:12）。
- 补丁相关：见上方「isolation.lua 完整梳理」。关键全局：`__lua_state_name`（:14、:24）、`__MAIN_MAP__`（:16）、`io.get_root_dir()`（:18）、`debug.traceback`（:17）、`log_file`/`log`（:14、:51 等）。**这就是 editor-patch 内核补丁 script 库解锁 isolation 的目标文件**（编辑器 state 不进入 if 块，但若需在游戏内调试，`editor_server_debug`/`editor_lobby_debug` argv（:25）是官方预留的后门开关）。

## common/reload.lua
- 用途：热更模块入口桩，转发到 client_base。
- 导出：`return require "@base.reload"`（:1，跨库 @ 前缀）。
- 依赖：`@base.reload`（client_base 库，不在本镜像）。
- 补丁相关：无直接 hook 点；include/热更语义实现在 client_base，需另行研究。

## common/json.lua
- 用途：JSON 模块入口桩，转发到 client_base。
- 导出：`return require "@base.json"`（:1）。
- 依赖：`@base.json`（跨库）。
- 补丁相关：无。

## common/class.lua
- 用途：OOP class 模块入口桩，转发到 client_base（注释「只保留一份到client_base里」）。
- 导出：`return require '@base.base.class'`（:1）。
- 依赖：`@base.base.class`（跨库）。
- 补丁相关：无。

## common/auto_test.lua
- 用途：自动化测试入口桩（注释「代码迁移到client_base」）。
- 导出：`return require '@base.auto_test'`（:1）。
- 依赖：`@base.auto_test`（跨库）。
- 补丁相关：isolation.lua:183 对 `auto_test` argv 有特判（保留 io.upload_file），二者呼应。

## common/localization.lua
- 用途：本地化语言设置/查询模块。
- 导出：`{ set_language, get_language, add_resource_path, get_text }`（:26-31）；`get_text` 是空实现占位（:22-24）。
- 依赖：无 require；调引擎全局 `common.set_localization_language`（:6、:19）、`common.get_default_language`（:10）。
- 补丁相关：**定义全局 `_G.set_language`**（:3）——语言切换钩子点；`common` 是 C++ 预置全局表。

## common/device_settings.lua
- 用途：设备设置入口桩。
- 导出：`return require '@base.device_settings'`（:1，注释「代码迁移到client_base里了」）。
- 依赖：`@base.device_settings`（跨库）。
- 补丁相关：无。

## common/device_profile.lua
- 用途：设备画像入口桩。
- 导出：`return require '@base.device_profile'`（:1）。
- 依赖：`@base.device_profile`（跨库）。
- 补丁相关：无。

## common/device_config.lua
- 用途：设备配置入口桩。
- 导出：`return require '@base.device_config'`（:1）。
- 依赖：`@base.device_config`（跨库）。
- 补丁相关：无。

## common/suggested_optimizations.lua
- 用途：历史遗留兼容模块，唯一函数已空转（注释「没用了，逻辑挪到C++了，兼容一下，先不删除」:4）。
- 导出：`{ player4_muti_units = player4_muti_units }`（:7-9，空函数）。
- 依赖：`require 'base.platform'`（:1，实际未使用）。
- 补丁相关：无。

---

## 附：入口桩模式小结

12 个根级文件中 7 个（reload/json/class/auto_test/device_settings/device_profile/device_config）是一行 `return require '@base.xxx'` 的**跨库转发桩**——v199 这批模块实现已迁往 client_base 库，script 库只保留入口兼容旧 `require 'json'` 这类写法。真正承载逻辑的根级文件只有 5 个：init.lua、main.lua、isolation.lua、localization.lua、suggested_optimizations.lua。
