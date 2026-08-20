# AGENTS.md — sce_app_editor-patch

> 本文件为 AI 助手提供项目上下文。修改代码前请先阅读。维护准则见 [doc/research/agents-md-guidelines.md](doc/research/agents-md-guidelines.md)。

## 项目定位

独立的 egui 桌面应用（中文名「编辑器补丁」）：给**星火编辑器**打补丁——把目标库**整库解密为裸露源码**、在库入口注入补丁插槽、解除使用限制，支持按库分组的可勾选补丁模块。通过宿主 [bgd_sce_tools](https://github.com/woaye168/bgd_sce_tools) 的「应用市场」安装分发（registry 在 [bgd_sce_appsdk](https://github.com/woaye168/bgd_sce_appsdk)），宿主启动时传 `--project-path <项目根>`。

**同时是 AGENT 操作编辑器的唯一 MCP 入口**：`sce_app_editor-patch mcp`（stdio，恒定 8 工具：editor_start/editor_stop/get_game_logs/capture_game 本地实现 + start_debug/stop_debug/publish_project/get_status 在线透传编辑器内 bgd_mcp_bridge）。同名 CLI 子命令 `editor start|stop`、`logs`、`capture`、`notify <key>=<value>`（宿主解耦通知：切项目时更新运行时共享常量 bgd_runtime.lua + 最近项目 + 通知 GUI 刷新）供人类/脚本直接用。应用单实例；`--background` 静默驻留（看守线程 Win32 SW_HIDE/SW_RESTORE 驱动主窗口——egui 隐藏时事件循环休眠，不能依赖 ViewportCommand），驻留模式窗口 X = 隐藏（假关闭，服务常开，再次打开经 show 信号唤出），真退出只走宿主 `--quit`；前台启动窗口 X = 正常退出。bridge dll 部署失败（编辑器占用）时置待重部署标志，update 每 5s 自动重试。

## 技术栈与规范

- Rust 2021；eframe/egui 0.29；windows-capture + image（截图）；reqwest（桥 HTTP）；CLI 手写分发（无 clap）
- **bgd_appsdk**（crates.io 公开包 `bgd_appsdk = "0.2"`，仓库 [bgd_sce_appsdk](https://github.com/woaye168/bgd_sce_appsdk)）：单实例/看守线程/日志/应用配置/**通用窗口壳 AppShell** 等公共基建，禁止在本仓库重复实现（UI 经 `ShellApp` trait 注册标签页即可）。单实例/信号前缀一律由 appsdk 按 exe 名推导（`app::default_si_prefix`），禁止硬编码
- **模块拆分**：单文件接近 500 行必须按职责拆分。

## 目录结构

```
src/main.rs            # 入口（业务 CLI 分发 + bgd_appsdk::app::run 统一入口）+ 应用状态/壳实现
src/ui/{kernel,patches,settings,help}.rs  # 四个标签页业务 UI（impl EditorPatchApp）
src/cli.rs             # CLI 子命令（editor/logs/capture）
src/mcp.rs             # stdio MCP 聚合服务（NDJSON）
src/core/              # locate/crypto/ops/backup/log/modules/slot_inject/slots（内嵌插槽文件）
                       # editor（编辑器生命周期/日志/设置）/bridge_client/capture
                       # kernel.rs（库登记/状态检查/进度聚合）+ kernel/{apply,restore,tests}.rs
patches/modules.json   # 模块清单元数据（id/pkg/名称/描述/默认勾选/部署dll/注入声明）
patches/<包>/<id>/     # 补丁模块 lua（编译期 include_str! 嵌入）
csharp/bgd_mcp_bridge/ # .NET 9 进程内 MCP 桥（Gateway 架构，编译期嵌入 exe）
csharp/make_catalog/   # 能力目录生成工具（编辑器升级后重跑）
slots/<包>/<版本>/     # 插槽文件（改库源码，版本敏感）+ slot.manifest.json
examples/              # decrypt_mirror / make_slots / make_pie_slot / wgc_probe
test/                  # 自测自修归档（case/temp/knowledge/report）
.trae/skills/          # 流程技能 + 库知识库（sce-lib-<库>-<版本>，打补丁前必查）
doc/requirements|research/  # 版本需求文档 / 研究成果
```

## 核心机制（改代码前必读）

### 编辑器包定位链（locate.rs）

项目 `map_settings.json` api_version → `tsconfig.json` typeRoots 推编辑器根 → `api_pak_version.json`（缓存）查包版本/路径前缀 → 包目录。script 包 require 根 = `script/common/`，xdeditor 包 require 根 = 包根。

### 加解密（crypto.rs）

包内 `.lua` 按 4 字节 magic `TNND` 逐文件识别 XOR 加密（密钥 `CREATEEASY`）；**策略为整库解密成明文**，写回一律明文。

### 内核补丁（kernel.rs / slot_inject.rs）

- **库登记制**（`LIBS`）：应用 = 整库备份（仅首次）→ 整库并行解密 → 应用插槽 → 补丁目录/默认模块。
- **插槽**：`slots/<包>/<版本>/` 整树字节级复制覆盖；版本漂移三级回退——① 精确版本；② 最近低版本 manifest 全哈希一致则复用；③ 运行时模式注入（锚点失败明确报错提示重跑 make_slots）。注入/剥离幂等。
- **状态检查**：入口含插槽标记（script 库另需 isolation 解锁标记）；编辑器升级覆盖后重新「应用补丁」即可。
- **还原**：整库备份树覆盖还原 + 删除补丁目录（有集成测试守护字节一致）。

### 补丁模块（modules.rs）

- 补丁目录 = `<require根>/sce_app_editor-patch/`，`main.lua` 为 AUTO-GENERATED 入口按启用列表 pcall require；**目录存在即启用**。
- 元数据在 `patches/modules.json`（默认勾选/部署 dll/注入项目根/注入 exe 路径）；新增模块：放 lua + 登记 + `module_files` 挂文件。
- refresh 自同步：按嵌 exe 内容重写已启用模块文件、刷新运行时共享常量/exe 路径、dll 内容比对重部署（编辑器内运行时升级需重启编辑器）。
- **运行时共享常量**：`<require根>/sce_app_editor-patch/bgd_runtime.lua`（`return { project_path = ... }` 表结构，所有模块可 require）——由启用注入声明的模块时写入、refresh 同步、宿主 notify（切项目）实时更新；模块读常量不走各自注入文件。
- 现有模块：`hello`（示例）、`unwatch`（解除项目文件监听，默认开）、`menu_bgd`（帮助菜单，默认开，用官方事件桥注册）、`bgd_mcp_bridge`（MCP 桥，默认开）、`pie_capture`（拍照按钮修复=外部捕获，默认开；行为主体在 slots 覆盖的 gameplay_in_editor_view.lua，模块加载时把注入的 exe 路径写入全局 `_G.BGD_CAPTURE_EXE`）。

### bgd_mcp_bridge（C# 扩展注入）

- dll 勾选模块时部署到 `<运行根>/version-<api>/` 并登记 `sce.deps.json`；版本号跟随插件 tag（CI `-p:Version` 注入）。
- 编辑器内经 `SCE.Common.csharp_activate_window` 激活，隐藏窗口 HttpListener（127.0.0.1:39177+）暴露 HTTP JSON-RPC；C#↔Lua 走事件总线（`bgd_mcp_cmd`/`bgd_mcp_ack`/`bgd_mcp_event`）。
- **Gateway 架构**：tools/list 恒定 10 个元工具，全部能力进能力目录（catalog.json 生成 + annotations.json 人工标注，均嵌入 dll）。通道：svc/datacore/cpp/cmd+lua/sys。安全分级 read/write/danger（danger 需 config.json `danger_allow` 放行；write/danger 写审计日志）。Lua 桥支持延迟 ack。
- 编辑器升级后：`dotnet run --project csharp/make_catalog -- --project <项目根>` 重生成 catalog.json 再构建 dll。
- 详见 [doc/research/csharp-module-injection.md](doc/research/csharp-module-injection.md)、[doc/research/mcp-integration-guide.md](doc/research/mcp-integration-guide.md)、[doc/research/publish-and-capture.md](doc/research/publish-and-capture.md)。

### 截图（capture.rs）

lua 桥 `lua.get_game_view_rect` 取 PIE 视口控件 get_screen_rect 逻辑矩形 → WGC 截 WinUI 主窗口（SDL 内容窗口直接呈现截出黑图，不可直接截）→ 帧内等比裁剪 → 倍率重采样。窗口最小化/隐藏时离屏恢复（SHOWNOACTIVATE + 屏外坐标 + placement 还原），遮挡无需处理。

## 安全红线（最高优先级）

1. **绝不能弄坏编辑器源文件**：动手前必整库备份；写盘一律原子替换；加密/明文逐文件识别。
2. **应用补丁必须可完整还原**：还原后与原始字节完全一致。
3. 编辑器更新后包版本变化 → 新目录是未打补丁的原始状态，重新应用即可，不要跨版本复用注入。

## 构建、测试与发布

```bash
cargo check          # 检查
cargo test           # 单元 + 集成测试（临时目录，不碰真实编辑器）
cargo test -- --ignored --nocapture   # 本机真实项目冒烟（只读定位链）
cargo build --release
git tag v0.x.0 && git push origin v0.x.0   # CI 注入版本号 → 构建 → 上传 sce_app_editor-patch.exe
```

- 版本号唯一来源是 git tag（Cargo.toml 固定 `0.0.0-dev`）。
- **C# 桥 dll 是构建前置**：先 `dotnet build csharp/bgd_mcp_bridge`（Release x64）再 cargo build（include_bytes! 嵌入）。
- **本应用无自我更新**：版本更新由宿主应用市场负责（应用仓库 CI 合成的 `app-release.json` 提供版本/描述/版本说明，宿主凭极简 registry 发现本应用；发版不再改任何清单）。

## 自测自修流程（开发完成后必走）

1. 按本轮目标写用例放 `test/case/`。
2. 即时调试：C# 改动 → 重编 dll 复制到 `D:/sce_online/version-13/`（重启编辑器生效）；lua 补丁改动 → 复制到 `<编辑器根>/res/_m/<包>/<版本>/<包>/sce_app_editor-patch/<模块目录>` 同层级。
3. 临时文件放 `test/temp/`，新知识记 `test/knowledge/`。
4. 收尾：测试报告入 `test/report/`；知识验证完善后落盘 `doc/research/`；`test/` 本轮内容归档到版本号文件夹。

## 提交规范

Conventional Commits：`feat: / fix: / docs: / ci: / refactor: / chore:` 前缀（Release notes 依赖）。
