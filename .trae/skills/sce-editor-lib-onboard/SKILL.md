---
name: "sce-editor-lib-onboard"
description: "为星火编辑器补丁应用（sce_app_editor-patch）接入新目标库或制作 slots/ 插槽文件（改库源码、版本敏感）。当用户要求补丁 script/xdeditor 之外的库、或需要插槽/覆盖库源文件时调用。"
---

# 接入新目标库 / 制作 slots 插槽文件

在 `d:/sce_online/Res/maps/sce_app_editor-patch` 仓库工作。先读该仓库 `AGENTS.md`。

> slots/ = 「带插槽/修改的完整新源码文件」，按 `slots/<库名>/<库版本号>/<源码目录结构>/file.lua` 组织，应用补丁时整树复制覆盖进库目录。版本敏感是故意的。
> 不改库源码的场景走 patches/ 模块（`sce-editor-patch-module` 技能）。

## A. 制作插槽文件（已有库）

1. **确定目标库与版本**：从 `<编辑器根>/api_pak_version.json` 的 `[api_version][包名]` 拿库版本；当前维护 api 12/13/2000 对应库版本（见 AGENTS.md）
2. **取该版本官方源码**：用 `examples/decrypt_mirror`（`cargo run --example decrypt_mirror -- <包目录> <输出>`）得到明文源码（GBK 注释会乱码，见下）
3. **生成插槽文件**：用 `examples/make_slots`（自动完成 解密 → GBK→UTF-8 转码 → 注入插槽/转换 → 写入 `slots/<库>/<版本>/`）。新增插槽类型时改 make_slots.rs
   - **GBK 处理**：部分官方文件是 GBK 编码，插槽文件统一转 UTF-8（编辑器按字节执行 Lua，UTF-8 源码可正常运行）
   - 入口插槽插在**末尾顶层 return 之前**（`find_trailing_return` 定位 + 括号平衡验证）
4. **纳入测试**：`src/core/kernel.rs` 测试断言新插槽文件效果
5. 发版流程同下

## B. 接入新目标库（扩库）

1. **确认包存在与版本**：`api_pak_version.json` 的 `#package_path.<包名>`（路径前缀）+ `[api_version][包名]`（版本号）；包目录 = `<编辑器根>/<前缀>/<版本>/<包名>`
2. **确定 require 根**：看库内已有 require 写法解析到哪个目录（如 script 库 `require 'base.path'` → `common/base/path.lua`，require 根 = `common`）
3. **确定入口文件**：库被引擎加载的第一个 lua；**必须检查结尾形态**（顶层 return 单行/多行，`find_trailing_return` 需能识别）
4. **登记**：`src/core/kernel.rs` 的 `LIBS` 加 `LibSpec { pkg, name, require_root, entry }`
5. **生成插槽文件**：跑 make_slots（自动覆盖新库各版本）
6. **建库知识库**（**必须**，研究方法论见下）：`.trae/skills/sce-lib-<库名>-<版本>/`
7. **测试**：kernel.rs 集成测试覆盖新库；`cargo test` + `cargo test -- --ignored`
8. **文档同步**：AGENTS.md（LIBS/目录结构）+ README，同次提交
9. **发版**：`git tag v0.x.y && git push origin v0.x.y` + bump bgd_sce_appsdk registry

## 库源码研究方法论（建知识库时硬性遵守）

- **精确到每个源码文件**（逐文件有记录，可简录但不可缺）
- **子代理分批执行**：主会话制定研究清单（`_plan.md`：批次/范围/成果文件），子代理按批研究
- **成果即时独立落盘**：每批结果立刻写 `files/<批>.md`，会话中断可断点续研
- **结论标注来源**（相对路径:行号），不臆测；GBK 乱码注释标注即可
- 知识库结构：`SKILL.md`（导引+索引）/ `architecture.md` / `api.md` / `hooks.md` / `files/`（逐文件记录）/ `_plan.md`

## 关键背景（防幻觉）

- 备份/还原按库整树：`<编辑器根>/bgd_editor_patch/backup/<api版本>/<包名_版本>/`；新库接入自动获得，无需额外代码
- 插槽应用 = `slots/<库>/<版本>/` 整树复制覆盖（字节级，无编解码）；**无匹配版本子目录则跳过并提示，不蛮干**
- 不是所有库/文件都加密：逐文件按 TNND 头判断（ops.rs 已处理）
- 接入前用真实编辑器目录手工验证包目录/入口存在（参考 kernel.rs `smoke_locate_real_project`）

## 安全红线

- 写编辑器文件只在 kernel.rs 既定流程内（备份 → 原子写），不旁路
- 插槽文件必须与官方源码差异最小（只加插槽/目标修改），便于版本对照
