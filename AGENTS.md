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

## 代码规范

- **模块拆分**：单文件代码量接近 500 行时，必须考虑按职责拆分模块，避免单文件过大难以维护。

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
│       ├── kernel.rs      # 库登记（LIBS）/整库应用/slots复制/状态检查/还原（含测试）
│       └── modules.rs     # 补丁模块注册（按库分组/默认勾选）/启停/框架入口重建
├── patches/
│   └── <包名>/<模块id>/main.lua  # 内置补丁模块（不改库源码，编译期 include_str! 嵌入）
├── csharp/
│   ├── bgd_mcp_bridge/    # .NET 9 类库：编辑器进程内 MCP 桥（编译期 include_bytes! 嵌入 exe）
│   │                      #   0.5.0 起：Gateway 架构，catalog.json/annotations.json 嵌入 dll
│   └── make_catalog/      # 工具：反射扫描宿主 dll 生成 catalog.json 骨架（编辑器升级后重跑）
├── slots/
│   └── <包名>/<库版本>/<源码目录结构>/file.lua  # 插槽文件（改库源码，版本敏感，include_dir 嵌入）
├── examples/
│   ├── decrypt_mirror.rs  # 工具：整库解密出明文镜像（研究用）
│   └── make_slots.rs      # 工具：生成 slots 插槽文件（解密+GBK转UTF-8+注入）
├── test/                  # 自测自修（见「自测自修流程」）：case 用例 / temp 临时文件 / knowledge 新知识 / report 测试报告
├── .trae/skills/          # 流程技能（patch-module/lib-onboard）+ 库知识库（sce-lib-<库>-<版本>）
├── doc/requirements/      # 各版本需求文档
├── doc/research/          # 研究成果沉淀（csharp 模块注入、编辑器调试控制等）
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

- **库登记制**：`LIBS` 常量表登记「包名 + require 根 + 入口文件」。应用补丁 = 对每库依次：整库备份（仅首次）→ 整库并行解密 → **复制插槽文件** → 补丁目录/默认模块。
- **插槽文件（0.3.1 起）**：仓库 `slots/<包名>/<库版本>/` 下放「带插槽的完整新源码」，编译期 `include_dir` 嵌入，应用时整树**字节级复制覆盖**（不做编解码，天然免疫 GBK/UTF-8 混合）。无匹配版本子目录则跳过并提示。天然支持多插槽与整文件覆盖。
- 当前插槽：script 库 `common/init.lua`（框架入口插槽，末尾追加 pcall require）+ `common/isolation.lua`（解锁 = nil 行）；xdeditor 库 `main.lua`（入口插槽，顶层 return 之前）。
- 插槽文件由 `examples/make_slots` 生成（解密 → GBK→UTF-8 → 注入/转换），版本更新后重跑即可。
- **状态检查**（`check()`）：入口含插槽标记（script 库另需 isolation 解锁标记）。编辑器升级覆盖后显示「未应用」，重新「应用补丁」即可（已启用模块保留）。
- **还原**：整库备份树覆盖还原 + 删除补丁目录。
- **进度**：应用/还原在后台线程执行，`SharedProgress`（phase/total/done 原子计数）供 UI 画进度条。

### 补丁框架与模块（modules.rs）

- 补丁目录在**库 require 根**下：`<require根>/sce_app_editor-patch/`，`main.lua` 为 AUTO-GENERATED 入口，按启用列表 `pcall(require, 'sce_app_editor-patch.<id>.main')`。整库解密后全部写**明文**。
- **启用状态即文件系统状态**：模块目录存在即启用。
- 模块声明 `default_enabled`：内核补丁**首次创建**补丁目录时自动启用（用户手动关闭后重新应用不会强开；编辑器升级换版本目录后按全新处理会再启用默认）。
- 模块按 `pkg` 归属库，UI 按库分组罗列；该库内核未应用时勾选禁用。
- 新增模块：`patches/<pkg>/<id>/` 放 lua + `builtin_modules()` 注册。
- 现有模块：script/`hello`（示例）、script/`unwatch`（解除项目文件监听）、xdeditor/`menu_bgd`（帮助菜单入口，**默认开启**；用官方事件桥 `EDITOR.event_notify(EVENT.window_title_bar_register, ...)` 注册，不在入口模块 require menu_bar——详见库知识库 hooks.md）。

### 备份与日志（backup.rs / log.rs）

- 备份：`<编辑器根>/bgd_editor_patch/backup/<api版本>/<包名_版本>/` 整树 + `.manifest.json`，同分组只备首次。测试用 `EDITOR_PATCH_BACKUP_DIR` 覆盖。
- 日志：优先 `<项目>/.bgd/log/app_editor-patch-YYYY-MM-DD.log`（与 visual-injector 一致，按日期分文件）；项目无 .bgd 时退回 `<编辑器根>/bgd_editor_patch/log/`。

### bgd_mcp_bridge（C# 扩展注入）

- dll 在勾选补丁模块时部署到 `<运行根>/version-<api>/` 并登记 `sce.deps.json`（备份 `sce.deps.json_bak`）；关闭时摘除；「还原补丁」时整体恢复。
- 编辑器内经 `SCE.Common.csharp_activate_window` 激活，隐藏窗口内跑 HttpListener（127.0.0.1:39177+）暴露 HTTP JSON-RPC 与 MCP `/mcp` 端点；C#↔Lua 双向走事件总线（`bgd_mcp_cmd`/`bgd_mcp_ack`/`bgd_mcp_event`）。
- **0.5.0 起为 Gateway 架构**：tools/list 恒定 10 个元工具，全部能力进能力目录（`catalog.json` 构建期生成 + `annotations.json` 人工标注层，均编译期嵌入 dll）。能力通道：svc（DI 服务反射，服务级准入制）/datacore（IDataCore 手写封装）/cpp（静态基元方法）/cmd+lua（Lua 桥）/sys。安全分级 read/write/danger（danger 需 config.json `danger_allow` 放行），write/danger 调用写审计日志。
- 编辑器升级后：重跑 `dotnet run --project csharp/make_catalog` 重新生成 catalog.json（make_slots 同款约定），再构建 dll。
- 详见 [doc/research/csharp-module-injection.md](doc/research/csharp-module-injection.md) 与 [doc/research/mcp-integration-guide.md](doc/research/mcp-integration-guide.md)。

### 开发工具（examples/）

- `decrypt_mirror`：`cargo run --example decrypt_mirror -- <包目录> <输出目录>` 整库解密出明文镜像（源码研究用）
- `make_slots`：`cargo run --example make_slots -- <编辑器根> <slots目录>` 批量生成插槽文件（解密 → GBK→UTF-8 → 注入插槽/isolation 解锁）

### skills 体系（.trae/skills/）

- **流程技能**：`sce-editor-patch-module`（patches 模块开发）/ `sce-editor-lib-onboard`（扩库与 slots 制作）
- **库知识库**：`sce-lib-<库名>-<版本>/`（架构/加载机制/API 类型声明/hook 配方/逐文件研究记录）——打补丁前必查。当前有 `sce-lib-script-199`、`sce-lib-xdeditor-160`。
- 库知识库研究方法论：逐文件精确、子代理分批、研究清单先行、成果即时落盘（见 sce-editor-lib-onboard/SKILL.md）。

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
- **C# 桥 dll 是构建前置**：CI 需先 `dotnet build csharp/bgd_mcp_bridge`（Release x64）再 `cargo build`（Rust 以 `include_bytes!` 嵌入该 dll）；本地 `cargo build` 前同样需先构建 csharp dll
- **本应用无自我更新**：仓库私有，版本更新由宿主 bgd_sce_tools 应用市场负责（registry.json 走 API 下载 asset，需工具侧配置 GitHub Token——fine-grained PAT 需把本仓库加入授权列表）
- 发版后同步更新 bgd_sce_plugins 的 `registry.json`（`version`/`tag`），`asset_name` 恒为 `sce_app_editor-patch.exe`

## 自测自修流程（开发完成后必走）

### 1. 测试前：建立用例

- 按本轮开发目标编写测试用例，放入 `test/case/`。

### 2. 测试过程：即时调试 + 自测自修

改完代码后按改动对象选择下方生效方式，反复自测自修直至所有用例通过：

| 改动对象 | 即时调试生效方式 |
| --- | --- |
| C# 扩展（bgd_mcp_bridge） | 重新编译后把 dll 复制到 `<编辑器根>../version-13/`（即 `D:/sce_online/version-13`），**需重启编辑器**才能生效 |
| Lua 补丁（xdeditor 包） | 修改后复制到 `<编辑器根>/res/_m/xdeditor/<包版本号>/xdeditor/sce_app_editor-patch/<补丁模块目录>` 同层级 |
| Lua 补丁（script 包） | 修改后复制到 `<编辑器根>/res/_m/script/<包版本号>/script/common/sce_app_editor-patch/<补丁模块目录>` 同层级 |

启动编辑器（自测用示例，api 版本 13）：

```bash
D:/sce_online/星火编辑器.exe -inner -winui_material_editor -winui_resource_store -editor_api_version=13 -file_path="C:/Users/woaye/Documents/SCE Projects/test_res002/project.sce"
```

- 测试过程中的临时文件放入 `test/temp/`（收尾时随版本归档）。
- 测试中产生的新知识、踩坑结论随手记入 `test/knowledge/`（收尾时再完善落盘）。

### 3. 自测自修完毕：收尾

当本轮所有用例全部测试并修复完毕后，依次完成：

1. 输出本轮测试报告，放入 `test/report/`。
2. 检查 `test/knowledge/` 中的新知识：务必验证、研究、完善，形成知识文档存入 `doc/research/`。
3. 把 `test/` 中本轮新增的全部内容归档到版本号文件夹（版本号 = 上个版本号的修定位 + 1）。

## 提交规范

Conventional Commits：`feat: / fix: / docs: / ci: / refactor: / chore:` 前缀（Release notes 依赖）。
