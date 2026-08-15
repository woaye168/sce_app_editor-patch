---
name: "sce-editor-lib-onboard"
description: "为星火编辑器补丁应用（sce_app_editor-patch）接入新的目标库（内核补丁范围扩库）。当用户要求补丁 script/xdeditor 之外的编辑器库（如 wineditor、appui 等）时调用。"
---

# 接入新的目标库（内核扩库）

在 `d:\sce_online\Res\maps\sce_app_editor-patch` 仓库工作。先读该仓库 `AGENTS.md`，再按本技能执行。

## 内核补丁按库登记

`src/core/kernel.rs` 的 `LIBS` 常量表是全部目标库。应用补丁时对每库执行：整库备份（仅首次）→ 整库并行解密为明文 → 库专属文本补丁（可选）→ 入口插槽 → 创建补丁目录并启用默认模块。接入新库 = 正确填写登记项 + 验证。

## 接入步骤

1. **确认包存在与版本**：读 `<编辑器根>/api_pak_version.json`
   - `#package_path.<包名>` 得路径前缀（缺省 `Res/_m/<包名>`）
   - `[api_version][包名]` 得版本号
   - 包目录 = `<编辑器根>/<前缀>/<版本>/<包名>`
2. **确定 require 根**：package.path 指向的目录（require 路径的起点）。
   - 判据：看库内已有 require 写法解析到哪个目录。如 script 库 `require 'base.path'` → `common/base/path.lua`，故 require 根 = `common`；xdeditor 库 `require 'ui.menu_bar'` → 包根
3. **确定入口文件**：该库被引擎加载的第一个 lua（如 `main.lua` / `init.lua` / `common/init.lua`）。
   - 先解密看结构再定：加密文件 = 前 4 字节 `TNND`，其后与密钥 `CREATEEASY`（字节 67,82,69,65,84,69,69,65,83,89）循环异或
   - **必须检查入口文件结尾形态**：若以顶层 `return` 结尾（单行 `return M` 或多行 `return { ... }`），插槽会插在它之前（kernel.rs 的 `find_trailing_return` 已处理，但要确认它能识别该文件的写法——顶层、行首无缩进、括号平衡）
4. **登记**：`LIBS` 加 `LibSpec { pkg, name, require_root, entry }`
5. **库专属文本补丁**（可选）：如 script 库解锁 isolation.lua 那样，在 `apply_lib` 里按 `lib.pkg` 分支加转换函数，并在 `check()` 里加对应状态校验
6. **测试**：更新/新增 kernel.rs 集成测试（临时目录构造该库结构，验证 应用→状态→幂等→还原 字节级一致），`cargo test` + `cargo test -- --ignored`（真实环境冒烟）全过
7. **文档同步**：AGENTS.md（LIBS/模块清单/机制描述）+ README，同次提交
8. **发版**：`git tag v0.x.y && git push origin v0.x.y` + bump `bgd_sce_plugins/registry.json`

## 关键背景（防幻觉）

- 备份是按库整树备份：`<编辑器根>/bgd_editor_patch/backup/<api版本>/<包名_版本>/`，还原整树覆盖 + 删除补丁目录；新库接入自动获得备份/还原能力，无需额外代码
- 补丁目录 = `<require根>/sce_app_editor-patch/`；插槽代码 `pcall(require, 'sce_app_editor-patch.main')` 能否解析取决于 require 根填得对不对——这是最易错点
- 不是所有库/文件都加密：逐文件按 TNND 头判断（ops.rs 已处理），不要假设
- 接入前用真实编辑器目录手工验证一次包目录存在、入口文件存在（参考 kernel.rs `smoke_locate_real_project` 的做法）

## 安全红线

- 任何写编辑器文件的操作都在 kernel.rs 既定流程内（备份→原子写），不要旁路
- 插槽/文本转换必须幂等（重复应用不产生叠加）
