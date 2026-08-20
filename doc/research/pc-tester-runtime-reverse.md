# 线上 PC 运行时（sce_pc_tester）逆向研究

> 研究日期：2026-08-21
> 状态：结构/包格式/日志体系全部打通；地图 pak 与内嵌包已可解密+解包（含正式版他人游戏）
> 关联研究：[ui-render-atlas-canvas.md](ui-render-atlas-canvas.md)（本轮主线问题与实测）、[csharp-module-injection.md](csharp-module-injection.md)（编辑器侧注入，对照用）
> 研究对象：`D:\sce_pc_tester\tester_1089\`（PC 玩家端测试器：tester_1089.exe 启动 → 登录 → 大厅 → 进游戏自动按测试/正式环境下载地图）
> 地图投递语义（用户口述实测）：编辑器发布的地图先进**测试环境** `e.production.spark.xd.com_test`；创作者中心提交审核通过后转**正式环境** `e.production.spark.xd.com`。地图目录最后一级 `p_xxx` = 项目版本号。

## 0. 一句话结论

线上 PC 运行时与编辑器同宗（Urho3D + lua54 + 同一套 script 包体系），但**地图以 TNND 加密的单文件 pak 投递**、引擎是 Build**PCBox** 沙箱构建（与编辑器 BuildPC 有实现差异）、壳是 WebView2+wasm+.NET 混合（**无 sce.dll 架构，编辑器的 csharp_activate_window 注入入口不存在**）、themis 反作弊在场。逆向工具链（解密/解包/字符串考古）已落地本仓库 examples/。

## 1. 目录结构（tester_1089/，全量勘察）

```
tester_1089.exe            # 启动器（登录 → 大厅）
Win/
  scegame                  # ★ 游戏引擎本体（47,920,056 字节，无扩展名 PE；
                           #   构建路径 D:\BuildPCBox\NE_pd\Client\...；启动时会在线更新自己，
                           #   实证：2026-08-21 04:31:18 LastWriteTime 被刷新）
  lua54.dll / themis_x64.dll（网易反作弊）/ lite.dll（内嵌 lite 文本编辑器）/ shaderc.dll /
  d3dcompiler_47.dll / sdk.dll / gmesdk.dll + libgme*.dll（腾讯 GME 语音）/ msvcp140* / vcruntime140* / ucrtbase
  webview2loader.dll + SCEGame.WebView2/   # 大厅壳 = WebView2（EBWebView/Crashpad/GrShaderCache...）
  AppBundle/managed/       # .NET 程序集（InsideSandboxClient/EngineInterface/EngineCommon/
                           #   ClientInterfaceDefinition/DotNext + 裁减版 System.*）+ dotnet.wasm +
                           #   node.mjs + run-node.sh/run-wasmtime.sh + icudt.dat——wasm/node 混合运行时
  embedded_packages/       # 内嵌基础包（7z 压缩包）：
                           #   script-190.7z / client_base-78.7z / appui-28.7z /
                           #   startup-364.7z / xdeditor_startup-21.7z / embedded_package_version.json
  update/
    e.production.spark.xd.com_test/Res/    # ★ 测试环境
      maps/<p_xxx>/<p_xxx>.pak + libs.json #   地图包
      _m/<包>/<版本>/<包>/<包>.pak          #   依赖库按版本分目录（script 185/190/199、appui 28/46/47/48、
                                           #   defaultui、lib_lobby 168、smallcard_*、lib_control...）
      script/script.pak / client_base/client_base.pak / appui/appui.pak / fonts / uistyle /
      shadercache_windows_game_dxbc(_extra) / xdeditor_startup
    e.production.spark.xd.com/res/         # ★ 正式环境（结构同上）
  User/
    maps/<p_xxx>/          # 运行时生成物（bgd_settings.json=bgd 框架落盘、shader_used_catalog、
                           #   user_online_time.txt、woayeTest111.json 等游戏自写文件）
                           #   ——🔴 地图内容不在这里！这里只有引擎/游戏运行时写的文件
    maps/app_box/          # 应用盒子（小游戏容器地图）
    exit_capsule.json / starup / starup_game / user_info-*.json / game_settings-*.json
  imagecache/              # image_cache 网络图片下载缓存（md5 文件名，部分带 .png 扩展）
  logs/                    # 见 §7 日志体系
```

## 2. 引擎与壳

- **scegame = Urho3D 系引擎**（PackageFile/ResourceCache/NanoVG/SDL 字符串实证），UI 渲染 DX11（`shadercache_windows_game_dxbc*/dx11/...`，日志实证 `Loaded cached compressed vertex shader ui(DIFFMAP MIXCOLOR VERTEXCOLOR)`）。
- 与编辑器引擎（version-13/sceengine.dll，BuildPC）**同源不同构建**：LuaUI.cpp 注册块**逐字节一致**（ui.*/ui_sound.*/canvas_texture_* 名称与次序全同，tester 字符串 :535377-535398 vs 编辑器 :443496-443517），但实现行为有差异——canvas_texture_set_size 在 PCBox 构建 native 硬崩、编辑器正常（详见 ui-render-atlas-canvas.md §8.3）。
- 壳：WebView2（大厅 UI）+ wasmtime/node + .NET managed。**没有** sce.dll/scemodule.dll/sce.deps.json 体系 → 编辑器那套 C# dll 注入（csharp-module-injection.md）**在 tester 上不可用**。
- themis_x64.dll（反作弊）在场，进程级 detour/调试器 attach 可能被干扰（未实测）。

## 3. 包格式（全链路打通，含格式修正过程）

三层套娃：

```
7z（embedded_packages/*.7z；Windows 自带 tar.exe -xf 可解，无需装 7-Zip）
  └─ *.pak（TNND 加密，整包）
       └─ 解密后 = Urho3D UPAK 变体：
            头："UPAK" + u32 条目数 + u32 总校验
            每条目 = 名字\0 + u32 offset + u32 size + u32 条目校验
            🔴 比标准 Urho3D PackageFile 每条目多 4 字节尾校验——
               发现过程：初版 pak_list 按「名字\0+offset+size」解析，第二条目起名字漂移
               出垃圾前缀（"H␣␣map_ref_res/..."）；对 hex dump 逐字节比对后发现
               size 之后还有 4 字节（48 17 FC A8 之类）才到下一个名字
       └─ 条目内容：lua/json 明文（pak 内不再二次加密）
```

- **TNND 加密**：4 字节 magic `TNND` + 剩余字节逐字节 XOR `CREATEEASY`——**不限 .lua**：json（appui atlas.json、libs.json）、整个 pak 都是。识别方法 = 文件头 4 字节。
- UPAK 头 hex 实证（p_55a3.pak 解密后前 96 字节）：

```
55 50 41 4B 39 05 00 00 90 48 88 5E 70 72 6F 6A  UPAK9... .H^proj
65 63 74 2F 6D 61 70 5F 73 65 74 74 69 6E 67 73  ect/map_settings
2E 6A 73 6F 6E 00 83 5C 01 00 F1 01 00 00 48 17  .json.\..ñ..H.
FC A8 6D 61 70 5F 72 65 66 5F 72 65 73 2F 74 65  ü¨map_ref_res/te
```

  读法：`UPAK` → 条目数 0x539=1337 → 总校验 0x5E884890 → 条目1 名字 `project/map_settings.json\0` → offset 0x00015C83 → size 0x1F1 → 条目校验 0xA8FC1748 → 条目2 名字 `map_ref_res/texture/...`。

## 4. 地图 pak 解剖（p_55a3.pak = 用户地图线上包，80MB/1337 条目）

条目布局实例（offset/size/路径）：

```
     89716      21969  map_ref_res/texture/effect/dj/t_fx_smoke_02_dj.png
  23298444    2910797  res/sound/bgd_game_client/bgm_yiban.ogg
  32545661         82  ui/atlas/atlas.json          ← 发布期自动图集注册表
  32545743      16453  ui/atlas/atlas_1.png
  32562196        206  ui/atlas/atlas_1.json
  60071811      49220  ui/image/sprites/bgd_game_client/desert_packed.png
  60906029      44169  ui/image/sprites/bgd_libs_client/unit_left/unit_left_2.png
  60950198     856869  ui/image/image/bgd_game_client/shop/icon_gift.png
  ...                scene/default/unit_save.lua / area_save.lua / config.lua 等
```

- **散图原样保留**：ui/image/ 下逐文件条目都在（shop/item 全套）——打图集不删原图。
- **自动图集只收了 1 张数编 GUI 引用的图**：atlas.json 内容（82 字节）`[{"AtlasPath":"atlas/atlas_1.png","ConfigPath":"atlas/atlas_1.json"}]`；atlas_1.json 只有 `ui/image/组件_020.png` 一条（CompressType=0，带 Border/OriginSize/HasAlpha）。
- `libs.json`（地图 pak 旁，TNND 加密单文件）：地图依赖库清单，**不含版本钉**：

```json
{ "lib_control": "script_libs/lib_control", "lib_game_options": "script_libs/lib_game_options",
  "lib_common_ai": "ai_templates/lib_common_ai", "defaultui": "defaultui",
  "default_units_ts": "default_units_ts", "smallcard_inventory": "script_libs/smallcard_inventory",
  "lib_common_sounds": "script_libs/smallcard_common_sounds", ... }
```

## 5. 脚本包版本体系

- tester 内嵌基础版：embedded_packages = script-**190** / client_base-**78** / appui-**28** / startup-364 / xdeditor_startup-21。
- **按地图在线下载对应版本**：update 目录里 script 185/190/199、appui 28/46/47/48 并存；用户地图（api 13）跑的是下载的 script-**199** pak（测试/正式两环境都有 `_m/script/199/script/script.pak`）。
- 190 vs 199 已知差异（tester_script-190/extracted 与 script-199 镜像对比）：
  - canvas 模板：190 **没有** texture_brush（canvas_texture 封装是 190→199 之间加的）；
  - isolation.lua：190 用 `log.warn`，199 用 `log_file.warn`（崩溃日志里的 `common/isolation.lua:61` 行号两版一致）。
- 编辑器侧包版本对照：_m 下有 script 179-199、xdeditor 142-169、appui 28-50、gameui 47-52、lib_lobby 169。

## 6. 线上资源加载与 IO 边界

- 引擎资源加载走 **ResourceCache → PackageFile**（pak 感知）：`image` 属性、require、include 全正常。
- **StateGame 的 io.read 读不到地图内容**（tester 实测日志原文）：

```
[04:35:30.955][warning][common/isolation.lua:61] io.read failed,
  full path[D:/sce_online/User/maps/p_55a3/ui/image/image/bgd_game_client/item/helmet_demon.png],
  error_code[1]
```

  （注：此行是编辑器 PIE 复测同款失败——调试图也不在 User/maps 下。isolation 把相对路径重定向到 `<root>/User/maps/<地图>/` 裸文件系统，而地图内容在 pak 里。）
- pak 读取 API（`io.read_pak_entries/extract_pak/extract_pak_file/read_cache`）StateGame 全被 isolation 置 nil；tester 跑自己的 script 包，**编辑器补丁管不到玩家端**。
- **唯一穿 pak 的 Lua 通道 = require**（引擎 lua 加载器 pak 感知）。需要把字节带进 pak：base64 → `.lua` 数据模块 `return "..."`。
- io.write 线上可用实证：`User/maps/p_55a3/bgd_settings.json` 是 bgd 框架运行时落盘的。
- image_cache 的「isolation 前捕获 native 函数」范本与绝对路径显示能力见 ui-render-atlas-canvas.md §5。

## 7. 生产游戏实证（正式版 pak 解包）

解包正式版游戏 **p_2xgc（1.06GB/6904 条目）**、**p_1ax1（344MB/3401 条目）** + 公共库 **lib_ui_48**、**defaultui_63**，全量 Lua 搜索 canvas/draw_image/canvas_texture：

- canvas 使用**只有 `minimap_canvas`**（引擎原生小地图控件，纯 C++ 渲染，数编模板 `components['$$.template@gui_ctrl.minimap_canvas']` 配置）；
- **零处** draw_image / canvas_texture / canvas 自绘；
- 推论：线上动态/自定义图像的官方通道就是 image 属性（资源路径/绝对路径/http），**没有像素级通道的生产先例**——canvas_texture 是未发布的半成品。

## 8. 日志体系（Win/logs/）与崩溃特征

| 目录 | 内容 |
| --- | --- |
| `lua/lua-game-*.log` | ★ 游戏 StateGame 的 log_file 落点（探针主战场） |
| `lua/lua-application-*.log` / `lua-base-*.log` | 大厅（StateApplication）/基础 state |
| `game/game-*.log` | ★ native 引擎日志（INFO 级：shader 加载、游戏阶段事件 GE_GAME_LOADING 等） |
| `Network/` `downloader/` `updater/` `ziper/` `lobby/` `login/` `im/` `global_chat/` `ui/` `ExecTime/` `FileRemover/` `regionSelector/` `wasmtime-base/` | 各子系统（wasmtime-base 证实壳内 wasm 运行时） |
| `libhv.<date>.log` | libhv 网络库 |

**canvas_texture 线上崩溃实录**（lua-game-2026-08-21 04_35_15_535.log 末尾）：

```
[04:35:30.859][warning][726][.../canvasprobe.lua:73] [CanvasProbe] canvas created, id=ui-2062-nil
[04:35:31.022][info][728][common/base/game.lua:698] on_enter_game
[04:35:31.409][warning][751][.../canvasprobe.lua:48] [CanvasProbe] >>> B1 set_size
（日志到此终止，无异常行；game 日志同步终止于 04:35:31.022 "after GE_ENTER_GAME event"）
```

**native 硬崩特征**：lua-game 日志静默终止、game 日志同步终止、**无 dump 文件、无 WER Application Error 事件**（Get-WinEvent 实证）——官方未留崩溃诊断口子。定位只能靠 Lua 侧 marker 日志配对法（ui-render-atlas-canvas.md §11）；指令级定位需 x64dbg attach（themis 可能干扰，未实测；且即使查清也改不了玩家端二进制，仅研究价值）。

## 9. 逆向工具链（本仓库 examples/，演进史即需求史）

> 操作向手册（拿到 pak 怎么一步步还原资源）：[pak-extract-guide.md](pak-extract-guide.md)

| 工具 | 用法 | 来历/用途 |
| --- | --- | --- |
| `decrypt_mirror`（既有） | `decrypt_mirror <包目录> <镜像输出>` | 最早：整库解密 .lua 到镜像；**局限：只处理 .lua**——本轮发现 atlas.json/libs.json 也加密后不够用 |
| `decrypt_inplace` | `decrypt_inplace <目录>` | 全扩展名 TNND 就地解密（护栏：只允许 .editor_src_mirror 内路径） |
| `decrypt_file` | `decrypt_file <输入> <输出>` | 单文件 TNND 解密（pak/json 通吃，80MB pak 秒解） |
| `pak_list` | `pak_list <已解密pak> <out.txt>` | UPAK 索引转清单（offset/size/名字）——初版格式解析漂移促成「4 字节尾校验」发现 |
| `pak_extract` | `pak_extract <已解密pak> <目录>` | UPAK 全条目解包（含尾校验跳过、防路径穿越 `..`→`__`） |
| `strings_dump` | `strings_dump <二进制> <out.txt>` | 二进制可打印字符串导出（引擎注册块/属性表考古：lua 名与 C++ 名成对聚集，注册块即 API 全集） |
| `pe_exports` | `pe_exports <pe文件>` | PE 导出符号表列举（sceengine.dll 3772 个导出即由它实证） |

典型流程：

```powershell
tar -xf xxx.7z -C out\                                  # 7z 用系统 tar
cargo run --example decrypt_file -- out\xxx.pak out\xxx.dec
cargo run --example pak_extract -- out\xxx.dec out\extracted\
cargo run --example decrypt_inplace -- out\extracted\   # 若条目仍加密（本轮未遇到，条目是明文）
```

## 10. 研究镜像清单（D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/）

| 镜像 | 来源 | 内容 |
| --- | --- | --- |
| `script-199/` / `xdeditor-160/` | 编辑器包（早前解密） | 游戏脚本库 / 编辑器 UI 库 |
| `appui-50/` / `gameui-52/` / `lib_lobby-169/` | 编辑器 _m 包（628+237 个 TNND 文件已解密；lib_lobby 本明文） | appui 含 ui/atlas 图集格式实证 + imgui 模块源码 |
| `client_base/` | `update/editor-pd.../res/client_base`（77 个 TNND 已解密） | @base/@common 桩的实现库：image_cache、update 体系 |
| `tester_script-190/` `tester_client_base-78/` `tester_appui-28/` `tester_startup-364/` | tester embedded_packages（7z→pak→解密→解包，348/77/564/226 条目） | 运行时内嵌包 |
| `game_p_2xgc/` / `game_p_1ax1/` | 正式版游戏 pak（1GB/344MB，6904/3401 条目） | 生产游戏代码实证 |
| `lib_ui_48/` / `defaultui_63/` | 正式版公共库 pak（445/231 条目） | 生产 UI 库 |
| `sceengine-strings.txt` | 编辑器引擎 49MB 字符串全集 | ui.* 注册块 :443417-443571、属性表 :452240+ |
| `scegame-tester-strings.txt` | tester 引擎字符串全集 | canvas_texture 注册块 :535377-535398、NanoVGCanvas 创建失败日志格式串 :541466 |
| `p_55a3.pak.dec` / `p_55a3-pak-list.txt` / `p_55a3-libs.json` | 用户地图线上包（80MB 解密件/索引清单/依赖清单） | 地图 pak 结构实证 |

## 11. 与编辑器差异速查 / 注入可行性

| 维度 | 编辑器 | tester（线上 PC） |
| --- | --- | --- |
| 引擎构建 | BuildPC（version-13/sceengine.dll，49MB） | BuildPCBox（Win/scegame，47.9MB 无扩展名） |
| 壳 | CoreCLR + sce.dll（可 C# 注入） | WebView2 + wasm + .NET managed（无注入入口） |
| 反作弊 | 无 | themis_x64.dll 在场 |
| 脚本包 | Res/_m 散目录 | embedded 7z（190 系）+ 按地图在线下载 pak（可到 199） |
| 地图形态 | 项目目录散文件 | 单文件 TNND-UPAK pak + libs.json |
| StateGame io.read 地图资源 | 不可达（调试图在调试目录） | 不可达（内容在 pak） |
| canvas_texture | 可用 | native 硬崩（平台 bug） |
| 补丁影响面 | sce_app_editor-patch 全能力 | **补丁管不到**（独立安装；改 embedded 包仅影响本机研究） |

## 12. 未解之谜 / 后续方向

- canvas_texture 崩溃的指令级定位（x64dbg attach tester 复现，拿崩溃模块+偏移对照反汇编）——仅研究价值。
- startup-364 / xdeditor_startup-21 / app_box 包内容未细看（startup=54MB/226 条目，含大厅启动逻辑）。
- themis 反作弊对调试器/注入的实际干扰程度未实测。
- 地图 pak 的条目校验算法（4 字节尾校验的算法未逆向，目前解包不需要校验）。
- 引擎在线更新机制：scegame 自身会被更新（LastWriteTime 实证），更新通道/差分格式未研究。
