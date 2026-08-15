# AGENTS.md — sce_app_editor-patch

> 本文件为 AI 助手提供项目上下文。修改代码前请先阅读。

## 项目定位

独立的 egui 桌面应用（中文名「编辑器补丁」）：给**星火编辑器**打补丁，解除编辑器使用限制并注入可扩展的补丁框架。通过宿主 [bgd_sce_tools](https://github.com/woaye168/bgd_sce_tools) 的「应用市场」安装分发（registry 在 [bgd_sce_plugins](https://github.com/woaye168/bgd_sce_plugins)），宿主启动时传 `--project-path <项目根>`。

## 技术栈

- **语言**：Rust 2021
- **UI**：eframe/egui 0.29（即时模式 GUI）
- **序列化**：serde/serde_json
- **CLI**：clap（`--project-path`）

## 目录结构

```
sce_app_editor-patch/
├── src/
│   ├── main.rs            # UI（egui 三标签：内核/补丁/帮助）+ 应用状态 + 入口
│   └── core/
│       ├── mod.rs
│       ├── locate.rs      # 定位链：项目 → 编辑器 common 包目录
│       ├── crypto.rs      # 脚本包 XOR 加解密（TNND 头 + CREATEEASY 密钥）+ 原子写
│       ├── backup.rs      # 备份机制（exe 同级 backup/<apiX_scriptY>/）
│       ├── kernel.rs      # 内核补丁：解锁 isolation.lua + 注入框架入口（含单元/集成测试）
│       └── modules.rs     # 内置补丁模块注册/启停/框架入口重建
├── patches/
│   └── <模块id>/main.lua  # 内置补丁模块（编译期 include_str! 嵌入 exe）
├── .github/workflows/release.yml  # tag 触发构建发布
└── Cargo.toml             # version 固定 0.0.0-dev，CI 按 tag 注入
```

## 核心机制（改代码前必读）

### 编辑器 common 包定位链（locate.rs）

1. `<项目>/project/map_settings.json` → `api_version`（对象或数字两种形态，如 13）
2. `<项目>/script/tsconfig.json` → `compilerOptions.typeRoots` 任意一条含 `Res/_m` 的路径，截取前缀得编辑器根（如 `D:/sce_online/Update/editor-pd.spark.xd.com`）
3. `<编辑器根>/api_pak_version.json` → `[api_version].script` 得 script 包版本号（如 199）
4. common 目录 = `<编辑器根>/Res/_m/script/<版本>/script/common`

### 脚本包加密（crypto.rs）

`Res/_m/script/<版本>/script/` 下所有 `.lua` 均为 XOR 加密：前 4 字节 magic `TNND`，其余字节与密钥 `CREATEEASY` 循环异或。**写入编辑器的任何 lua 文件必须加密**；写入一律走临时文件原子替换。

### 内核补丁（kernel.rs）

- 「应用补丁」（幂等）：解密 `isolation.lua` → 备份原始加密字节（仅首次）→ 注释所有 `xxx = nil` 禁用行（前缀 `-- [sce_app_editor-patch 解锁] `）→ 末尾追加注入块（标记 `-->> sce_app_editor-patch >>` / `--<< sce_app_editor-patch <<` 包裹，`pcall(require, 'sce_app_editor-patch.main')`）→ 加密写回 → 重建框架入口。
- 「还原补丁」：用备份**字节级**还原 `isolation.lua`，删除 common 下整个 `sce_app_editor-patch/` 目录。
- isolation.lua 由 common 包 `main.lua` 末尾 `require 'isolation'` 加载，禁用只发生在 `StateGame` 态。

### 补丁框架（modules.rs）

- 框架目录 `<common>/sce_app_editor-patch/`：`main.lua` 为 AUTO-GENERATED 入口，按启用列表 `pcall(require, 'sce_app_editor-patch.<id>.main')`。
- **启用状态即文件系统状态**：模块目录存在即启用，无额外状态文件。
- 新增内置模块：`patches/<id>/` 下放 lua 文件 + `modules.rs` 的 `builtin_modules()` 注册（id/名称/描述/文件清单）。

### 备份机制（backup.rs）

`<exe目录>/backup/<api版本_script版本>/` 存原始文件 + manifest.json；同分组只备首次，保证还原到真正原始。测试可用环境变量 `EDITOR_PATCH_BACKUP_DIR` 覆盖备份根。

## 安全红线（最高优先级）

1. **绝不能弄坏编辑器源文件**：修改前必先备份；格式不符（无 TNND 头）立即中止；写盘一律原子替换。
2. **应用补丁必须可完整还原**：还原后 isolation.lua 与原始字节完全一致（有集成测试守护）。
3. 编辑器更新后 script 包版本变化 → 新目录是未打补丁的原始状态，重新应用即可，不要跨版本复用注入。

## 构建、测试与发布

```bash
cargo check          # 检查
cargo test           # 单元 + 集成测试（临时目录，不碰真实编辑器）
cargo test -- --ignored --nocapture   # 本机真实项目冒烟测试（只读定位链）
cargo build --release
git tag v0.x.0 && git push origin v0.x.0   # 触发 CI：注入版本号 → 构建 → 上传 sce_app_editor-patch.exe
```

- 版本号唯一来源是 git tag（CI 注入 Cargo.toml，源码固定 `0.0.0-dev`）
- **本应用无自我更新**：仓库私有，版本更新由宿主 bgd_sce_tools 应用市场负责（registry.json 走 API 下载 asset，需工具侧配置 GitHub Token）
- 发版后同步更新 bgd_sce_plugins 的 `registry.json`（`version`/`tag`），`asset_name` 恒为 `sce_app_editor-patch.exe`

## 提交规范

Conventional Commits：`feat: / fix: / docs: / ci: / refactor: / chore:` 前缀（Release notes 依赖）。
