# xdeditor-160 架构与加载机制

> 证据见 files/ 各文档标注的行号；本文只归纳。

## 库定位

- 包目录：`Res/_m/xdeditor/160/xdeditor/`；**require 根 = 包根**（`require 'ui.menu_bar'` → `ui/menu_bar.lua`）
- 加载入口：`main.lua`

## main.lua 加载流程

```
main.lua
  ├─ argv 分支（各自提前 return）：
  │    unit_test（:11-18）/ scene_test（:20-28）/ sub_process（:30-43）
  │    —— scene_test/sub_process 各自 include 'global'/'utils'/'config' + require 'console'
  │    generate_cmd（upload_map/upload_lib*/generate_map，:45-64 起超时定时器）
  ├─ 主流程全局初始化（:117-124）：
  │    include '@common.class' → include 'global'（EDITOR/EVENT 常量）
  │    → include 'utils'（utils/event.lua 挂 EDITOR.event_register/notify，:804-805）
  │    → include 'config' → require 'console' → include 'exception' → ProcessInfo
  ├─ 资源更新（update_modules_to_update_in_xdeditor / update_editor_resource_dict）
  ├─ 登录：include 'ui.login'，login(function() ... show_editor_main_ui() end)（:773-784）
  │    show_editor_main_ui()（:460 起）：
  │      require 'io_modifier'（:471，io hook 样板）
  │      → include 'ui'（:473 → ui/init.lua → main_view → menu_bar 等组件创建）
  │      → include 'window'（:475）→ include 'plugin'（:477）
  └─ 末尾顶层 return { continue_launch_editor, argv_has_scene_test, argv_has_sub_process }（:961-965）
     —— editor-patch 的入口插槽就插在这个 return 之前
```

## 关键机制

- **include vs require vs @**：与 script 库同一套引擎语义（include 每次重执行、@ 跨库）。xdeditor 大量使用 `@common.base.xxx`（client_base 的 common 部分）。
- **menu_bar 加载时机**：在登录成功后的 `show_editor_main_ui()`（main.lua:473）才加载——这就是 0.3.0 在 main.lua 加载期 require menu_bar 失败的原因。
- **window_title_bar 是组件类**：`base.ui.component('window_title_bar')`（ui/menu_bar.lua:14）每次调用新建组件类，非单例。
- **主/子进程**：部分 app 文件头有 `ProcessInfo.is_main_process` 分支（子进程只拿消息转发桩）——补丁需区分进程。
