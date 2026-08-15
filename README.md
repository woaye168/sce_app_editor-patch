# sce_app_editor-patch（编辑器补丁）

给**星火编辑器**打补丁的桌面应用：解除编辑器使用限制，注入可扩展的补丁框架，支持可勾选启停的补丁模块。

> 当前为闭源项目，许可证采用 AGPLv3（见 [LICENSE](LICENSE)）。

## 功能

- **内核补丁**：解密并解锁编辑器 `common` 包中的 `isolation.lua`（恢复被官方置 `nil` 禁用的 `io`/`os`/`debug` 等函数），并在文件末尾注入补丁框架入口。
- **补丁模块**：框架目录 `<common>/sce_app_editor-patch/` 下的功能模块，界面勾选即可启用/关闭。
- **完备备份**：首次修改编辑器源文件前自动备份原始字节，「还原补丁」可随时字节级恢复原状。

## 安装与使用

1. 打开 [bgd_sce_tools](https://github.com/woaye168/bgd_sce_tools) 的「应用 - 应用市场」，安装「编辑器补丁」（需在工具设置中配置 GitHub Token，私有仓库走 API 下载）。
2. 在应用市场启动本应用（宿主会自动传入当前项目路径 `--project-path`）。
3. 「内核」标签页：确认定位信息（编辑器版本 / script 包版本 / common 目录），点击「应用补丁」。
4. 「补丁」标签页：勾选需要的补丁模块。
5. **重启星火编辑器后生效**（应用、还原、启停模块都需要重启）。

不再需要时，在「内核」标签页点击「还原补丁」即可恢复原状。

## 工作原理

```
项目路径/project/map_settings.json     → api_version（编辑器版本，如 13）
项目路径/script/tsconfig.json          → typeRoots 提取编辑器根目录
<编辑器根>/api_pak_version.json        → [api_version].script 得包版本（如 199）
common 目录 = <编辑器根>/Res/_m/script/<版本>/script/common
```

- 编辑器脚本包所有 `.lua` 均为 XOR 加密（`TNND` 头 + `CREATEEASY` 密钥），补丁读写均做加解密。
- 内核补丁注入 `require 'sce_app_editor-patch.main'`，框架入口按启用列表加载各模块。
- 备份目录在应用安装目录下 `backup/<api版本_script版本>/`，只备首次，保证还原到真正原始文件。

## 安全性

- 修改编辑器源文件前必先备份；检测到文件格式不符立即中止。
- 写盘采用临时文件原子替换，避免写一半损坏编辑器。
- 星火编辑器更新（script 包版本变化）后补丁自然失效（新目录为原始状态），重新「应用补丁」即可。

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
