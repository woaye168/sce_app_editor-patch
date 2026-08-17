# sce_app_editor-patch（编辑器补丁）

给**星火编辑器**打补丁的桌面应用：把编辑器库**整库解密为裸露源码**、解除使用限制、注入可扩展的补丁框架，支持按库分组、可勾选启停的补丁模块。

> 当前为闭源项目，许可证采用 AGPLv3（见 [LICENSE](LICENSE)）。

## 功能

- **内核补丁（按库）**：
  - 整库解密：把目标库（script / xdeditor）的加密 lua 全部还原为明文源码——编辑器可直接运行源码，方便查看与二次开发；
  - `script` 库：解锁 `isolation.lua` 中被官方置 `nil` 禁用的 `io`/`os`/`debug` 等函数；
  - 每个库入口注入补丁插槽（`sce_app_editor-patch` 框架），并在库下创建补丁目录。
- **补丁模块（按库分组，可勾选）**：
  - xdeditor / `MCP 桥（外部 AI 控制）`：在编辑器进程内注入并启动 C# 扩展（bgd_mcp_bridge.dll），于 127.0.0.1 暴露 HTTP JSON-RPC 与 MCP 服务，供外部 AI 调用编辑器命令（启动/停止调试、文件操作等）并拉取编辑器事件；勾选时自动部署 dll 并登记 sce.deps.json，取消勾选摘除，「还原补丁」整体恢复；
  - xdeditor / `解除项目文件监听`：移除并拦截编辑器对项目目录的文件监听，外部（如 AI Agent）修改项目文件时不再弹出重载提示；勾选时应用把当前项目根注入模块（_project_root.lua）；
  - xdeditor / `帮助菜单 bgd_sce_tools 入口`（默认开启）：编辑器顶部菜单「帮助」下新增子菜单，点击打开仓库；
  - script / `示例补丁`：验证补丁链路。
- **状态自检**：随时检测各库补丁状态；编辑器升级覆盖补丁后一键重新应用（已启用模块保留）。
- **整库备份**：首次应用前完整备份整个库（备份在编辑器数据目录，应用卸载不丢），「还原补丁」随时整库恢复原状。
- **进度展示**：整库解密/备份/还原多线程并行执行，界面实时进度条。

## 安装与使用

1. 打开 [bgd_sce_tools](https://github.com/woaye168/bgd_sce_tools) 的「应用 - 应用市场」，安装「编辑器补丁」（需在工具设置中配置 GitHub Token，私有仓库走 API 下载；fine-grained PAT 需把本仓库加入授权列表）。
2. 在应用市场启动本应用（宿主会自动传入当前项目路径 `--project-path`）。
3. 「内核」标签页：确认定位信息与库状态，点击「应用补丁」（有进度条）。
4. 「补丁」标签页：按库勾选需要的补丁模块。
5. **重启星火编辑器后生效**（应用、还原、启停模块都需要重启）。

不再需要时，在「内核」标签页点击「还原补丁」即可整库恢复原状。
编辑器升级后打开本应用点「刷新状态」：若库显示「未应用」（被升级覆盖），重新「应用补丁」即可。

## 工作原理

```
项目路径/project/map_settings.json     → api_version（编辑器版本，如 13）
项目路径/script/tsconfig.json          → typeRoots 提取编辑器根目录
<编辑器根>/api_pak_version.json        → [api_version][包名] 得包版本，#package_path 得路径前缀
包目录 = <编辑器根>/<路径前缀>/<版本>/<包名>
```

- 编辑器包内 lua 大多为 XOR 加密（`TNND` 头 + `CREATEEASY` 密钥），也有明文文件；逐文件按 magic 头识别，只解密加密文件。
- 补丁插槽注入在库入口文件末尾顶层 `return` 之前，加载 `<库require根>/sce_app_editor-patch/main.lua`，框架入口按启用列表加载各模块。
- 备份：`<编辑器根>/bgd_editor_patch/backup/<api版本>/<包名_版本>/` 整树，只备首次。
- 日志：`<项目>/.bgd/log/app_editor-patch-YYYY-MM-DD.log`，按日期分文件。

## 安全性

- 动手前必先整库备份；写入采用临时文件原子替换，避免写一半损坏编辑器。
- 加密/明文逐文件识别，明文文件不会被误处理。
- 星火编辑器更新（包版本变化）后补丁自然失效（新目录为原始状态），重新「应用补丁」即可。

## 从源码构建

```bash
cargo build --release   # 产物 target/release/sce_app_editor-patch.exe
cargo test              # 单元 + 集成测试（不碰真实编辑器文件）
```

## 开发（补丁开发）

- `patches/<包名>/<模块id>/`：不改库源码的可勾选模块（版本不敏感）
- `slots/<包名>/<库版本>/`：带插槽的完整新源码文件（版本敏感），用 `cargo run --example make_slots -- <编辑器根> slots` 生成
- `.trae/skills/`：补丁开发技能与库知识库（`sce-lib-script-199` / `sce-lib-xdeditor-160`），AI/人工开发补丁前先查对应库知识库的 hooks.md

## 发布

```bash
git tag v0.x.0 && git push origin v0.x.0
```

CI 自动注入版本号、构建并上传 `sce_app_editor-patch.exe` 到 Release；随后在 [bgd_sce_plugins](https://github.com/woaye168/bgd_sce_plugins) 的 `registry.json` 中 bump `version`/`tag` 即可被应用市场发现（有新版时应用市场会显示「升级」按钮）。
