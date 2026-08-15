---
name: "sce-lib-script-199"
description: "星火编辑器 script 库（common 包）v199 的源码知识库：加载链、isolation 阉割表、UI 框架、逐文件研究。补丁涉及 script 库（游戏脚本/common）时查阅。"
---

# script-199 库知识库

星火编辑器 script 库（`Res/_m/script/199/script`，require 根 = `common/`）v199 的源码研究成果。本目录只讲「这个库是什么」，怎么做补丁看流程技能 `sce-editor-patch-module` / `sce-editor-lib-onboard`。

## 索引

- [architecture.md](architecture.md)：加载链（init→main→isolation）、lua state 模型（StateGame/StateEditor/StateApplication）、include vs require、@ 跨库引用
- [api.md](api.md)：关键全局与 API 签名（log/log_file、common、base、argv、UI 框架）
- [hooks.md](hooks.md)：已验证 hook 配方（isolation 解锁、框架入口插槽、文件监听解除、open_url 包装范本）
- [files/common-root.md](files/common-root.md)：common 根级 12 文件逐文件记录（含 isolation 完整禁用清单）
- [files/common-base.md](files/common-base.md)：common/base/ 111 文件逐文件记录
- [files/common-base-ui-test.md](files/common-base-ui-test.md)：common/base/ui/ + test/ 99 文件逐文件记录
- [_plan.md](_plan.md)：研究清单（批次与状态）

## 补丁开发速查

- **StateGame 才阉割**：isolation 的禁用只在 `__lua_state_name == 'StateGame'` 生效；编辑器 state（StateEditor）下 io/os/debug 全部完好。
- **编辑器补丁的执行环境**：script 库插槽（common/init.lua 末尾）在每个 state 各执行一次；写 state 相关逻辑用 `__lua_state_name` 判断。
- **大量模块是跨库桩**：common 根级 7/12、base/ 31 个文件是 `return require '@base.xxx'`（实现在 client_base 库，不在本库）。
- **UI 框架两代并存**：旧式 `base.ui.component`（本库 base/ui/ui.lua）与新式 `@common.base.gui.component`（client_base）——打 UI 补丁前先辨认目标属于哪代。
