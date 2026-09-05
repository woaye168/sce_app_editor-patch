# 游戏 VM eval 与「源码形态路径直通」机制

> 2026-09-01 · 起源：0.8.5 统一 Page 架构实施后发现 AI 在 lua.eval 里写的调试代码与游戏源码形态不一致
> （require/res 路径），经与作者多轮讨论定稿「规则归 bgd_sce_tools 所有 + 构建期盖戳 + dbg_bus 收敛翻译」方案。
> 本文档沉淀该过程中全部实测知识与设计决策。**改动相关机制前必读**。
>
> 关联：libs/doc/api/client/cgui_mcp.md（AI 调试闭环）、bgd_sce_tools builder（rewrite/res/rules）。

## 1. 引擎 require 机制（全部真机实测，2026-08-31/09-01）

### 1.1 `@` 是引擎 native 的跨包解析标记

- 先例（doc/research/editor-runtime-mechanism.md）：`require '@base.base.account'` → client_base 库的
  `common/base/account.lua`。
- 游戏 VM 里有两个包：**脚本库包**（`@common/`，引擎 script/199/script/common）与**地图包**
  （`@<ProjectName>/`，ProjectName 来自 project/map_settings.json——实测 p_55a3 项目即 `@p_55a3/`）。
- 地图内模块的运行时键：`@p_55a3/bgd_libs_client/client/cgui/core`（package.loaded 实测枚举）。

### 1.2 点/斜杠双形态均合法

`require('@p_55a3/bgd_libs_client/client/cgui/core')`（斜杠）与
`require('@p_55a3.bgd_libs_client.client.cgui.core')`（点）**实测都成功**——引擎内部规范化为斜杠键；
大小写混合路径段也可（文件查找时小写化，Windows 大小写不敏感兜底）。
**定稿用点形式**（Lua 原生惯例，与编辑器源码 '@base.base.account' 一致）。

### 1.3 裸名解析依赖调用方 chunk 的环境（本机制的总根因）

同一个 `require('bgd_libs_client.client.cgui.core')`：
- 在**地图内代码** chunk：裸名自动映射到地图包 `@p_55a3.`（构建产物正常加载靠的就是它）；
- 在 **dbg eval chunk**（StateGame 默认环境）：裸名被映射到脚本库包 `@common/` →
  `module '@common/bgd_libs_client/...' not found`（实测失败）。

**结论：eval 里 `@<map>` 前缀不可省**；跨包标记的拼接由翻译层完成。

### 1.4 不存在「当前地图」占位符

`@__MAIN_MAP__` 实测被当字面目录解析（`maps/__main_map__/ui/script/...` not found）。勿用。

## 2. res 资源路径：五类五个运行时目的地（bgd_sce_tools 构建规则）

源码字面量（`'libs/res/<类型>/...'` / `'src/res/<类型>/...'`）→ 运行时引用路径：

| 类型 | 运行时引用路径 | 扩展名处理 | 物理落位（工具复制） |
| --- | --- | --- | --- |
| image | `image/image/<前缀>/a.png` | 保留 .png | `ui/image/image/<前缀>/` |
| particle | `res/effect/<前缀>/a.effect` | 保留 .effect | `res/effect/<前缀>/` |
| sound | `res/sound/<前缀>/a` | **去 .ogg** | `res/sound/<前缀>/` |
| spine | `spine/<前缀>/a` | **去 .skel** | `ui/spine/<前缀>/` |
| sprites | `@<ProjectName>/image/sprites/<前缀>/a.png` | 保留扩展名，**@ProjectName 不可省** | `ui/image/sprites/<前缀>/` |

前缀：libs→`bgd_libs_client`，src→`bgd_game_client`（**必须从 cfg 构建目标名派生**——
rewrite.rs 曾写死前缀导致「设置里改了构建目标后模块改写跟随、res 改写不跟随」的撕裂 bug，0.9.0 修复）。

## 3. 引擎 io 沙盒（StateGame）

- `io.write` 等**只接受相对路径**（isolation.lua 拦截绝对路径报错）；相对路径落盘到
  `D:\sce_online\User\maps\<地图包名>\`（实测）。
- 读项目文件需走编辑器 VM（StateEditor）——游戏 VM 够不到项目目录。

## 4. eval 的双通道汇聚点

`lua.eval` 无论走 MCP（run_scenario/invoke）还是**直连桥 HTTP**（127.0.0.1:39177 invoke_capability），
最终都汇聚到游戏 VM 的 dbg_bus `commands.eval`。**翻译逻辑收敛于这一点 = 双通道全覆盖**；
放在 mcp 层（Rust）或桥层（C#）都会漏掉另一通道——这是方案选型的决定性事实。

## 5. 最终架构（盖戳机制）

```
bgd_sce_tools（规则唯一所有者：内建默认 + 设置界面编辑 → bgd.json overlay）
   │ build 时：rewrite/res/clean 消费规则；并盖戳生成 .bgd/src/common/path_rules.lua
   │（_G.bgd_path_rules 全局表：map / modules 前缀对照 / res 前缀对照，全部解析后终值；
   │  文件经 rewrite_excludes 保护不被构建替换损坏）
框架入口 libs/entrance/client.lua 顶部 pcall require（entrance 合并机制 → ui/src/main.lua 最前段，
   最早执行；失败 log.error 响亮暴露）
   ▼
dbg_bus eval：require 转写（'libs.x' → '@'..map..'.'..根名..x）+ res 字面量 gsub 改写；
   盖戳缺失且代码用到源码路径 → 直接报错「请用 bgd_sce_tools 重新构建项目」，无静默兜底
```

## 6. 设计决策记录（ADR）

1. **规则归 bgd_sce_tools 所有**（不引 appsdk/framework 依赖）：关键修改动作（项目管理/构建路径配置）
   都在 tools；规则随 build 盖戳自动下发，作者无「记得同步别处」的负担。
2. **规则是数据不是代码**：`rewrite_excludes`/`res_rules` 进 bgd.json overlay，GUI 可编辑；
   tools 更新默认值 → 下次构建自动传播。
3. **通用排除机制替代特例**：`rewrite_excludes`（构建但不替换）与 `libs_excludes`（不构建）语义正交，
   支持多个条目；tools 自产 artifact 在编排层显式并入，rewrite 引擎零感知具体文件名。
4. **无静默兜底**：盖戳缺失即响亮报错（曾讨论 package.loaded 自发现兜底，被否——藏坑比报错贵）。
5. **加载点选框架 entrance 而非深层 init**：时序最早，先于一切业务与 dbg 通道可用。
6. **modules 存裸根名**（'bgd_libs_client.'），`@`+map 消费端组合：数据干净，`@` 跨包是消费端机制；
   sprites res 条目保留 `@<ProjectName>`（资源路径的必须成分）。

## 7. 附：UI 操控通道边界（同阶段实测，常被误用）

| 操作 | 通道 | 边界 |
| --- | --- | --- |
| `key_down/key_up` | 引擎按键**事件**转发 | Y/U 类事件开关有效；**不置 key_state 轮询态**，WASD 移动无效 |
| 驱动位移 | `press_ui` 摇杆持续输入（摇杆 on_change 写 WorldState） | 实测 1.5s 位移 487 逻辑 px，release_ui 复位 |
| `find_ui` rect | 上一完整帧快照 | 静态 UI 可信；**世界坐标驱动元素**（血条/掉落/瓦片）随镜头每帧变，操作前重 find 或走世界坐标 |
| eval 读游戏模块 | 源码形态 require（经本机制翻译） | 旧的后缀匹配 package.loaded 偏方废弃 |
