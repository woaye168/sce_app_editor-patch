# sce_app_editor-patch（编辑器补丁）

给**星火编辑器**打补丁的桌面应用：解除编辑器使用限制、扩展编辑器界面，注入可扩展的补丁框架，支持可勾选启停的补丁模块。

> 当前为闭源项目，许可证采用 AGPLv3（见 [LICENSE](LICENSE)）。

## 功能

- **内核补丁（多库）**：
  - `script/common/isolation.lua`：解锁被官方置 `nil` 禁用的 `io`/`os`/`debug` 等函数，并注入补丁框架入口；
  - `xdeditor/ui/menu_bar.lua`：编辑器顶部菜单「帮助」下新增「bgd_sce_tools」子菜单（点击打开仓库）。
- **补丁模块**：框架目录 `<common>/sce_app_editor-patch/` 下的功能模块，界面勾选即可启用/关闭：
  - `示例补丁`：验证补丁链路，报告关键函数解禁状态；
  - `解除项目文件监听`：移除并拦截编辑器对项目目录的文件监听，外部（如 AI Agent）修改项目文件时不再弹出重载提示。
- **状态自检**：随时检测各补丁点状态；编辑器升级覆盖补丁后一键重新应用。
- **完备备份**：首次修改编辑器源文件前自动备份原始字节（备份在编辑器数据目录，应用卸载不丢），「还原补丁」随时字节级恢复原状。

## 安装与使用

1. 打开 [bgd_sce_tools](https://github.com/woaye168/bgd_sce_tools) 的「应用 - 应用市场」，安装「编辑器补丁」（需在工具设置中配置 GitHub Token，私有仓库走 API 下载；fine-grained PAT 需把本仓库加入授权列表）。
2. 在应用市场启动本应用（宿主会自动传入当前项目路径 `--project-path`）。
3. 「内核」标签页：确认定位信息与补丁点状态，点击「应用补丁」。
4. 「补丁」标签页：勾选需要的补丁模块。
5. **重启星火编辑器后生效**（应用、还原、启停模块都需要重启）。

不再需要时，在「内核」标签页点击「还原补丁」即可恢复原状。
编辑器升级后打开本应用点「刷新状态」：若补丁点显示「未应用」（被升级覆盖），重新「应用补丁」即可，已启用的模块会保留。

## 工作原理

```
项目路径/project/map_settings.json     → api_version（编辑器版本，如 13）
项目路径/script/tsconfig.json          → typeRoots 提取编辑器根目录
<编辑器根>/api_pak_version.json        → [api_version][包名] 得包版本，#package_path 得路径前缀
包目录 = <编辑器根>/<路径前缀>/<版本>/<包名>
```

- 编辑器包内 lua 大多为 XOR 加密（`TNND` 头 + `CREATEEASY` 密钥），也有明文文件；本应用按 magic 头自动识别并保持原格式写回。
- 内核补丁在 `isolation.lua` 末尾注入 `require 'sce_app_editor-patch.main'`，框架入口按启用列表加载各模块。
- 备份与日志都在编辑器数据目录：`<编辑器根>/bgd_editor_patch/{backup,log}/`，只备首次，保证还原到真正原始文件。

## 安全性

- 修改编辑器源文件前必先备份；写入采用临时文件原子替换，避免写一半损坏编辑器。
- 加密/明文自动识别，不会对明文文件误加密。
- 星火编辑器更新（包版本变化）后补丁自然失效（新目录为原始状态），重新「应用补丁」即可。

## 从源码构建

```bash
cargo build --release   # 产物 target/release/sce_app_editor-patch.exe
cargo test              # 单元 + 集成测试（不碰真实编辑器文件）
```

## 发布

```bash
git tag v0.x.0 && git push origin v0.x.0
```

CI 自动注入版本号、构建并上传 `sce_app_editor-patch.exe` 到 Release；随后在 [bgd_sce_plugins](https://github.com/woaye168/bgd_sce_plugins) 的 `registry.json` 中 bump `version`/`tag` 即可被应用市场发现。
