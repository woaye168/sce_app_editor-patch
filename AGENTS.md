# AGENTS.md — sce_app_editor-patch

> 本文件为 AI 助手提供项目上下文。修改代码前请先阅读。

## 项目定位

独立的 egui 桌面应用（中文名「编辑器补丁」）：给**星火编辑器**打补丁——把目标库**整库解密为裸露源码**、在库入口注入补丁插槽、解除使用限制，支持按库分组的可勾选补丁模块。通过宿主 [bgd_sce_tools](https://github.com/woaye168/bgd_sce_tools) 的「应用市场」安装分发（registry 在 [bgd_sce_plugins](https://github.com/woaye168/bgd_sce_plugins)），宿主启动时传 `--project-path <项目根>`。

## 技术栈

- **语言**：Rust 2021
- **UI**：eframe/egui 0.29（即时模式 GUI）
- **序列化**：serde/serde_json
- **CLI**：clap（`--project-path`）
- **并发**：std::thread::scope 分块并行（整库解密/复制），无额外依赖

## 目录结构

```
sce_app_editor-patch/
├── src/
│   ├── main.rs            # UI（egui 三标签：内核/补丁/帮助）+ 进度条 + 入口
│   └── core/
│       ├── mod.rs
│       ├── locate.rs      # 定位链：项目 → 编辑器根 → 任意包目录（多库）
│       ├── crypto.rs      # XOR 识别/解密（TNND 头 + CREATEEASY 密钥），原子写
│       ├── ops.rs         # 批量文件操作：整库解密/整目录复制，多线程并行 + 进度
│       ├── backup.rs      # 整库备份（编辑器根 bgd_editor_patch/backup/<api>/<包_版本>/）
│       ├── log.rs         # 日志（项目 .bgd/log/app_editor-patch-YYYY-MM-DD.log，按日期分文件）
│       ├── kernel.rs      # 库登记（LIBS）/整库应用/状态检查/还原/入口插槽（含测试）
│       └── modules.rs     # 补丁模块注册（按库分组/默认勾选）/启停/框架入口重建
├── patches/
│   └── <包名>/<模块id>/main.lua  # 内置补丁模块（编译期 include_str! 嵌入 exe）
├── doc/requirements/      # 各版本需求文档
├── .github/workflows/release.yml  # tag 触发构建发布
└── Cargo.toml             # version 固定 0.0.0-dev，CI 按 tag 注入
```

## 核心机制（改代码前必读）

### 编辑器包定位链（locate.rs）

1. `<项目>/project/map_settings.json` → `api_version`（对象或数字两种形态，如 13）
2. `<项目>/script/tsconfig.json` → `compilerOptions.typeRoots` 任意一条含 `Res/_m` 的路径，截取前缀得编辑器根（如 `D:/sce_online/Update/editor-pd.spark.xd.com`）
3. `<编辑器根>/api_pak_version.json` **一次性读入缓存**（`EditorTarget.pak`）：`[api_version][包名]` 拿包版本号，`#package_path[包名]` 拿路径前缀
4. 包目录 = `<编辑器根>/<路径前缀>/<版本>/<包名>`（`EditorTarget::package_dir(name)`）
   - script 包 → `Res/_m/script/199/script`（require 根为其下 `common/`）
   - xdeditor 包 → `Res/_m/xdeditor/160/xdeditor`（require 根即包根）

### 加解密（crypto.rs）

包内 `.lua` 大多为 XOR 加密（4 字节 magic `TNND` + 密钥 `CREATEEASY` 循环异或），**但并非全部加密，也可能部分加密部分明文**。所有处理按 magic 头逐文件判断。**0.3.0 起策略为整库解密成明文源码**（编辑器可直接运行裸露源码），写回一律明文；`encrypt` 仅供测试构造样本。

### 内核补丁（kernel.rs）

- **库登记制**：`LIBS` 常量表登记「包名 + require 根 + 入口文件」。应用补丁 = 对每库依次：整库备份（仅首次）→ 整库并行解密 → 库专属文本补丁（script 库解锁 isolation.lua 的 `= nil` 行）→ 入口插槽 → 补丁目录/默认模块。
- **入口插槽**：插在入口文件末尾顶层 `return` 语句**之前**（`find_trailing_return` 定位，括号平衡验证；无 return 则追加末尾）。标记块 `-->> sce_app_editor-patch >>` / `--<< ... <<` 包裹，应用幂等。**切勿把代码追加在 return 之后**（0.2.0 的教训：menu_bar.lua `return window_title_bar` 结尾导致 `<eof> expected`）。
- **状态检查**（`check()`）：入口插槽在位（script 库另需 isolation 解锁标记）。编辑器升级覆盖后显示「未应用」，重新「应用补丁」即可（已启用模块保留）。
- **还原**：整库备份树覆盖还原 + 删除补丁目录。
- **进度**：应用/还原在后台线程执行，`SharedProgress`（phase/total/done 原子计数）供 UI 画进度条。

### 补丁框架与模块（modules.rs）

- 补丁目录在**库 require 根**下：`<require根>/sce_app_editor-patch/`，`main.lua` 为 AUTO-GENERATED 入口，按启用列表 `pcall(require, 'sce_app_editor-patch.<id>.main')`。整库解密后全部写**明文**。
- **启用状态即文件系统状态**：模块目录存在即启用。
- 模块声明 `default_enabled`：内核补丁**首次创建**补丁目录时自动启用（用户手动关闭后重新应用不会强开；编辑器升级换版本目录后按全新处理会再启用默认）。
- 模块按 `pkg` 归属库，UI 按库分组罗列；该库内核未应用时勾选禁用。
- 新增模块：`patches/<pkg>/<id>/` 放 lua + `builtin_modules()` 注册。
- 现有模块：script/`hello`（示例）、script/`unwatch`（解除项目文件监听）、xdeditor/`menu_bgd`（帮助菜单入口，**默认开启**；`require 'ui.menu_bar'` 拿到 window_title_bar 单例后 register）。

### 备份与日志（backup.rs / log.rs）

- 备份：`<编辑器根>/bgd_editor_patch/backup/<api版本>/<包名_版本>/` 整树 + `.manifest.json`，同分组只备首次。测试用 `EDITOR_PATCH_BACKUP_DIR` 覆盖。
- 日志：优先 `<项目>/.bgd/log/app_editor-patch-YYYY-MM-DD.log`（与 visual-injector 一致，按日期分文件）；项目无 .bgd 时退回 `<编辑器根>/bgd_editor_patch/log/`。

## 安全红线（最高优先级）

1. **绝不能弄坏编辑器源文件**：动手前必整库备份；写盘一律原子替换；加密/明文逐文件识别。
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
