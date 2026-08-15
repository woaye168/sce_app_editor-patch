# AGENTS.md — sce_app_editor-patch

> 本文件为 AI 助手提供项目上下文。修改代码前请先阅读。

## 项目定位

独立的 egui 桌面应用（中文名「编辑器补丁」）：给**星火编辑器**打补丁，解除编辑器使用限制、扩展编辑器界面，并注入可扩展的补丁框架。通过宿主 [bgd_sce_tools](https://github.com/woaye168/bgd_sce_tools) 的「应用市场」安装分发（registry 在 [bgd_sce_plugins](https://github.com/woaye168/bgd_sce_plugins)），宿主启动时传 `--project-path <项目根>`。

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
│       ├── locate.rs      # 定位链：项目 → 编辑器根 → 任意包目录（多库）
│       ├── crypto.rs      # XOR 加解密（TNND 头 + CREATEEASY 密钥），明文/加密自适应，原子写
│       ├── backup.rs      # 备份机制（编辑器根 bgd_editor_patch/backup/，多库分组）
│       ├── log.rs         # 日志（编辑器根 bgd_editor_patch/log/editor-patch.log）
│       ├── kernel.rs      # 内核补丁点登记/应用/状态检查/还原（含单元/集成测试）
│       └── modules.rs     # 内置补丁模块注册/启停/框架入口重建
├── patches/
│   └── <模块id>/main.lua  # 内置补丁模块（编译期 include_str! 嵌入 exe）
├── .github/workflows/release.yml  # tag 触发构建发布
└── Cargo.toml             # version 固定 0.0.0-dev，CI 按 tag 注入
```

## 核心机制（改代码前必读）

### 编辑器包定位链（locate.rs）

1. `<项目>/project/map_settings.json` → `api_version`（对象或数字两种形态，如 13）
2. `<项目>/script/tsconfig.json` → `compilerOptions.typeRoots` 任意一条含 `Res/_m` 的路径，截取前缀得编辑器根（如 `D:/sce_online/Update/editor-pd.spark.xd.com`）
3. `<编辑器根>/api_pak_version.json` → `[api_version][包名]` 拿包版本号，`#package_path[包名]` 拿路径前缀
4. 包目录 = `<编辑器根>/<路径前缀>/<版本>/<包名>`（`EditorTarget::package_dir(name)`）
   - script 包 → `Res/_m/script/199/script`（其下 `common/` 即 common 包）
   - xdeditor 包 → `Res/_m/xdeditor/160/xdeditor`

### 脚本包加密（crypto.rs）

编辑器包内 `.lua` 大多为 XOR 加密（4 字节 magic `TNND` + 密钥 `CREATEEASY` 循环异或），**但并非全部加密**。`read_lua` 按 magic 头自动识别，`LuaText { text, encrypted }` 记录原格式，`write_lua` 按原格式写回（加密→加密，明文→明文）。**禁止**无脑解密/加密。写盘一律临时文件原子替换。

### 内核补丁（kernel.rs）

- **补丁点登记制**：`PATCH_POINTS` 常量表登记「包名 + 包内相对路径 + 处理方式（PatchKind）」，新增内核补丁只加一行 + 一个注入块函数。当前两点：
  1. `script/common/isolation.lua`（Isolation）：注释所有 `xxx = nil` 禁用行（前缀 `-- [sce_app_editor-patch 解锁] `），末尾注入框架入口 `pcall(require, 'sce_app_editor-patch.main')`
  2. `xdeditor/ui/menu_bar.lua`（MenuBar）：末尾注入 `window_title_bar.register('帮助/bgd_sce_tools', ...)`（`common.open_url` 打开仓库）
- 注入块统一用 `-->> sce_app_editor-patch >>` / `--<< sce_app_editor-patch <<` 标记包裹，应用幂等（先除旧块再转换，解锁行已注释不重复处理）。
- **状态检查**（`check()`）：每个补丁点 已应用/未应用/文件缺失。编辑器升级覆盖后显示「未应用」，UI 提示重新「应用补丁」即可（已启用模块保留）。
- 「还原补丁」：有备份的补丁点逐个字节级还原 + 删除 common 下框架目录。

### 补丁框架（modules.rs）

- 框架目录 `<common>/sce_app_editor-patch/`：`main.lua` 为 AUTO-GENERATED 入口，按启用列表 `pcall(require, ...)` 加载各模块。框架文件是本应用新建的，**一律加密写入**（与包内其他文件一致）。
- **启用状态即文件系统状态**：模块目录存在即启用，无额外状态文件。
- 新增内置模块：`patches/<id>/` 下放 lua 文件 + `modules.rs` 的 `builtin_modules()` 注册。
- 现有模块：`hello`（示例）、`unwatch`（解除项目文件监听：移除并拦截对项目目录的 io.add_watch/remove_watch）。

### 备份与日志（backup.rs / log.rs）

- 都在**编辑器根目录**下（随编辑器数据走，应用卸载/重装不丢）：
  - 备份：`<编辑器根>/bgd_editor_patch/backup/<api版本>/<包名_版本>/<包内相对路径>` + `.manifest.json`，同文件只备首次
  - 日志：`<编辑器根>/bgd_editor_patch/log/editor-patch.log`（定位失败时退回 exe 同级 `log/`）
- 测试用环境变量 `EDITOR_PATCH_BACKUP_DIR` 覆盖备份根。

## 安全红线（最高优先级）

1. **绝不能弄坏编辑器源文件**：修改前必先备份；写盘一律原子替换；保持文件原加密格式。
2. **应用补丁必须可完整还原**：还原后与原始字节完全一致（有集成测试守护）。
3. 编辑器更新后包版本变化 → 新目录是未打补丁的原始状态，重新应用即可，不要跨版本复用注入。

## 构建、测试与发布

```bash
cargo check          # 检查
cargo test           # 单元 + 集成测试（临时目录，不碰真实编辑器）
cargo test -- --ignored --nocapture   # 本机真实项目冒烟测试（只读定位链，含 xdeditor）
cargo build --release
git tag v0.x.0 && git push origin v0.x.0   # 触发 CI：注入版本号 → 构建 → 上传 sce_app_editor-patch.exe
```

- 版本号唯一来源是 git tag（CI 注入 Cargo.toml，源码固定 `0.0.0-dev`）
- **本应用无自我更新**：仓库私有，版本更新由宿主 bgd_sce_tools 应用市场负责（registry.json 走 API 下载 asset，需工具侧配置 GitHub Token——fine-grained PAT 需把本仓库加入授权列表）
- 发版后同步更新 bgd_sce_plugins 的 `registry.json`（`version`/`tag`），`asset_name` 恒为 `sce_app_editor-patch.exe`

## 提交规范

Conventional Commits：`feat: / fix: / docs: / ci: / refactor: / chore:` 前缀（Release notes 依赖）。
