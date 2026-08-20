# 星火编辑器完整运行机制研究（脱机调试发布前置）

> 研究日期：2026-08-21
> 状态：静态研究完成（§5.4 有待实测清单）
> 目的：为 bgd_sce_tools 未来新应用「脱机调试发布」打基础——读编辑器已登录凭证/自登录/多凭证切换，脱离编辑器 GUI 完成调试与发布。
> 关联：[editor-debug-control.md](editor-debug-control.md)、[publish-and-capture.md](publish-and-capture.md)、[csharp-module-injection.md](csharp-module-injection.md)、[pc-tester-runtime-reverse.md](pc-tester-runtime-reverse.md)、[pak-extract-guide.md](pak-extract-guide.md)
> 源码证据：`.editor_src_mirror/xdeditor-160`（编辑器 UI 库）、`client_base`（account/ip/utility/upload）、`script-199`（common/upload）

## 0. 研究问题清单

1. 编辑器安装与版本目录结构（哪个 exe 启动什么）。→ §1
2. 登录与凭证：凭证存哪、什么格式、有效期/刷新机制、能否多凭证并存切换。→ §2
3. 发布链路：从菜单「发布/发布项目」到上传完成的完整调用链与 HTTP API。→ §3
4. 调试链路：PIE / 远端调试 host / headless argv 的完整机制。→ §4
5. 「脱机调试发布」的可行路径与方案选型。→ §5

## 1. 安装目录全景（D:\sce_online）

```
星火编辑器.exe            # 启动器（顶层；接收 argv 并路由到 version-<api>/ 的引擎+Shell）
themis_x64.dll / uninst.exe / variation.json
launcher_update/           # 启动器自更新暂存（星火编辑器.exe + themis_x64.dll）
version-12/ version-13/    # 按 api 版本分的引擎+Shell 目录（api 13 → version-13）
version-2000/              # 新一代 Shell（多出 mcpserver.dll/pluginhost.dll/pluginsdk.dll/
                           #   radzen.blazor.dll/datamanager.dll/tinypinyin.dll/ulid.dll——
                           #   官方正在做插件宿主与内置 MCP server！）
  sce / sce.dll / scemodule.dll / sceengine.dll / lua54.dll / coreclr 系 / WinUI 系 ...
  sce.deps.json            # CoreCLR 程序集解析清单（我们的 bridge dll 登记处）
  commandtool.exe          # 待查
Res/maps/<项目>/           # 地图项目（bgd_glzy/bgd_test/lib_lobby...）
User/                      # 编辑器级用户数据：
  user_info-<环境>.json    #   ★ 登录凭证（详见 §2.1；本机实证 user_info-editor-pd.spark.xd.com.json）
  editor_api_version.json  #   编辑器当前 api 版本
  recent_opened_map.json   #   最近打开地图
  mapconfig.json / world_id_map.json / customlayout.json / shortcut.json / starred_trigger.json
  starup                   #   无扩展名，内容就是字符 "1"（启动状态标记，无凭证价值）
  SCECheckpoint/           #   发布/调试的地图复制与备份（publish/ upload_ref/ 调试目录）
  maps/shader_used_catalog
logs/                      # 编辑器日志（lua/ 游戏lua日志、bgd_csharp/ 我们的桥、libhv.* 网络库...）
diagnostics/               # 诊断（readme.md）
update/                    # 编辑器包更新下载缓存（editor-pd.spark.xd.com/res/_m/<包>/<版本>/...）
                           #   client_base 库在 update/editor-pd.../res/client_base（不在 _m 下）
```

## 2. 登录与凭证机制（核心，已挖穿）

### 2.1 凭证文件

- **路径**：`<编辑器根>/User/user_info-<环境域名>.json`（client_base `account.lua:70-76` `get_user_info_file()`：默认 IP 时 `user_info.json`，否则按 `_G.IP` 拼）。**多环境天然分文件**。
- **本机实证**：`D:\sce_online\User\user_info-editor-pd.spark.xd.com.json`，**明文 JSON 不加密**：

```json
{
  "access_token": "1/U9IAQRp...",                  // TapTap OAuth access_token（"1/" 前缀长串）
  "guest_id": "GUEST_2026-02-26_00_20_22_...",     // 游客 uuid（GUEST_日期_时间戳）
  "login": 0,
  "login_token": "30cd5e5b...(64hex)",             // ★ 内部 HTTP API 签名 token（长期有效）
  "login_token_secret": "9b0c47c6...(40hex)",      // ★ 签名私钥（HMAC-SHA1 长度形态）
  "login_type": "",
  "token": "BBAXRAey...(mac_key)$1/U9IAQRp...(kid)", // ★ 登录 token = mac_key$kid
  "token_type": 11,                                 // 11=编辑器TapTap / 13=手机 / 14=安卓容器 / 999=游客
  "version": 1
}
```

- token_type 有效区间 11~14（`token_valid()` account.lua:174-178）；游客 = 999 + 空 token。

### 2.2 登录流程（xdeditor ui/login.lua 全量解读）

登录 = **TapTap OAuth2 device flow（扫码）**：

1. `POST https://www.taptap.com/oauth2/v1/device/code`（client_id/response_type=device_code/scope=public_profile/info={"device_id":"PC"}）→ 拿 `device_code` + `qrcode_url`。
2. **二维码绘制用的是 native 纹理 API**：`common.generate_qrcode(url)` 出矩阵 → `common.create_texture(name, w, h)` → `texture:set_data{x,y,width,height,rgb}` 逐块填色 → 控件 `bind.image = 纹理名`。🔴 这是**引擎另一个「命名纹理」通道的实证**（与 canvas_texture_set_name 同族思路，编辑器 state 在用）。
3. 轮询 `POST /oauth2/v1/token`（grant_type=device_token + code=device_code + secret_type=hmac-sha-1），状态机：`authorization_pending`（未扫）/ `authorization_waiting`（已扫待确认）/ `access_denied`（拒绝）/ 200 成功。
4. 成功 → `set_token(data)`：`token = mac_key..'$'..kid`，`token_type=11`，存 access_token，`account.save()` 落盘。
5. 然后 `account.login()` → `lobby.request_token_login(token_type, token)`（native 大厅连接）→ 等大厅事件 `'登录'`（`lobby.register_once`）→ `on_login_result` 拿到 `login_token`/`login_token_secret`（HTTP 签名对，注释称「长期有效」，且实证会落盘）。
6. 每次启动后还会 `refresh_token()`：`POST /oauth2/v1/token` grant_type=refresh_token + token=access_token → 换新 token 落盘（login.lua:557-600）。

补充：`login()` 的登录方式选择（account.lua:200-227）——PC 平台优先 token 登录（token_valid），否则游客登录；`lobby.request_guest_login()` 游客通道存在。登录状态跨 lua state 同步：StateGame 启动时向 StateApplication 广播「同步账号信息」（account.lua:306-328）。`check_console_wl` 用 `sce.s.score_init(readonly_map, 45)` 查控制台白名单。

### 2.3 内部 HTTP API 鉴权（account.lua:412-441）

`generate_http_token_sign(header)`：

```
pre_sign = noise\n + time_str(unix秒)\n + content_md5\n + login_token\n + login_token_secret
header.token = login_token
header.sign  = md5(pre_sign)
```

内部服务地址 = `calc_http_server_address('<服务名>', <端口>)`（如 bind_account 用 `'login', 9011` → `/api/v1/bind-3rd-account`）。🔴 **拿到 login_token+secret 即可在编辑器外独立签名调内部 API**——脱机工具的鉴权基础。

### 2.4 环境与域名体系（client_base base/ip.lua）

- `_G.IP` 由引擎注入的 `__CE_ENV` 解析（如 `Update/e.production.spark.xd.com_test/...` → `e.production.spark.xd.com`）；argv `server` 可覆盖。
- 环境族：master（内网）/ alpha（外网）/ beta（准线上）/ **pd（线上，e.production.spark.xd.com / editor-pd.spark.xd.com）** / fj-review（提审）/ intl(-beta)。
- OAuth 域名：正式 `www.taptap.com`；rnd 环境 `oauth.api.xdrnd.cn`；国际 `www.tap.io`。
- 更新子路径 `_G.update_subpath` = IP（+`_tapcode`/`_<tag>` 后缀）。

### 2.5 对「多凭证切换」的直接推论

- **凭证 = user_info-*.json 一个文件**。多账号 = 保留多份该文件，切换 = 换文件后启动编辑器（编辑器启动时 init() 读文件）。
- 自登录（不扫码）= 直接复现 §2.2 的 device flow（纯 HTTPS，编辑器外可做）拿 token/access_token 后自己拼 user_info.json。
- 需要注意：编辑器运行中会 `account.save()` **回写**该文件（refresh_token 后内容会变）——外部工具做「换凭证」要在编辑器关闭时做，或接受被覆盖。

## 3. 发布链路（从菜单到上传完成，全链挖穿）

### 3.1 调用链

```
菜单「发布/发布项目」（menu_bar.lua:2607，弹确认窗）
  → EDITOR.upload_map(log_mark, promise, ignore_save_map, is_upload_ref)   （utils/event.lua:709）
    ① lobby.logined 检查（未登录直接失败）
    ② 保存地图 EVENT.save_map
    ③ upload_backup 备份地图（User/SCECheckpoint 下，保留最近若干份）
    ④ 复制地图到 User/SCECheckpoint/publish/<地图>_<时间戳>/<地图>（白名单 dirs/files）
    ⑤ upload_map_view.upload_target_map(target_path, ...)                 （upload_map_view.lua:809）
       - get_upload_data(target_path)（:258）：读 config.ini（map_type→packet_type 1=游戏/8=大厅、
         is_landscape、player_count 数 user 槽位）+ libs.json（依赖库名列表）+ scene_tag/initial_scene/resource_size
       - upload_params：folder/package_name/path='Res/maps'/auto_ensure=true/with_mob/
         request_id=项目名+时间戳/api_version=编辑器 api 版本/encrypt=1...
       - require '@common.upload'.upload_map(params, nil, 进度cb, 完成cb)（script-199 common/upload/init.lua，非桩是实现）
         ⑥ auto_ensure → ensure_package：POST publisher/update-map-env-info
         ⑦ io.zip_file(folder → <folder><时间戳>.7z)（native 压缩，默认 7z，argv compress_use_7z=0 改 zip）
         ⑧ POST publisher/api/map/upload-map（multipart：[package_name].file = 7z 路径）
         ⑨ 进度通知：native sce.map_publisher.on_process_package_progress_notify（request_id 关联，
            服务器推送 {finish, result, data:{show, errors[], warnings[]}}）
    ⑩ promise:set_result(0=成功/1=失败)（官方 promise 通道，publish-and-capture.md R1 已接入 MCP）
```

### 3.2 上传 HTTP API（脱机可直接复现的部分）

地址推导（client_base base/utility.lua:385-422）：

```
_G.IP = editor-pd.spark.xd.com（编辑器环境）
  → editor- 换 editor. → editor.pd.spark.xd.com
  → 首段换成服务名：publisher-pd.spark.xd.com（production→pd）
  → pd 在 need_use_new_domain 名单 → spark.xd.com 换 tapsce.cn
  → 命中映射走 https：
    publisher = https://publisher-pd.tapsce.cn:9000
    updater   = https://updater-pd.tapsce.cn:9002   （/api/map/api-version 查最新 api）
    login     = https://login-pd.tapsce.cn:9011     （/api/v1/bind-3rd-account 等）
```

| API | 方法 | 说明 |
| --- | --- | --- |
| `/api/map/update-map-env-info` | POST json | 确保 package 创建：mapName/env='test'/path='Res/maps'/autoPublish=1/encrypt=1/patch=1/alias/status=0/map_type |
| `/api/map/upload-map` | POST multipart | 上传地图包：mapName/branch='master'/author/comment/email/request_id/upload_data(json)/tag/api_version/tag_api_version + `[mapName].file=<7z绝对路径>` |
| `/api/map/upload-ref` | POST multipart | 上传引用包（ref 目录 7z） |
| `/api/map/api-version` | POST | 查当前最新 api 版本（updater:9002） |
| 鉴权 | header | §2.3 的 token+sign（md5(noise\ntime\ncontent_md5\ntoken\nsecret)） |

### 3.3 发布链路的 native 依赖与「DLL 直调」评估（2026-08-21 修正）

**结论修正**：先前「纯外部 HTTP 不可行」的表述过于绝对。补证如下：

1. **sceengine.dll 导出面比想象大**：3772 个导出符号（examples/pe_exports.rs 实证），含 CppInterface 全家的 C 风格入口（C# 侧 `LibraryImport("SCEEngine.dll", EntryPoint="MapBuilder_GenerateRefToFile")` P/Invoke 实证，见 .tmp_verify/decomp/sce/SCE.CppInterface/）。`MapBuilder_Create/LoadJsonMap/GenerateRefToFile`、`TileEditor_GenerateRefToFile` 等都在。
2. **但「LoadLibrary 直接调」卡在根上**：所有对象 Create 都要 Urho3D `Context` 指针（MapBuilder.cs:23-27 实证 `Create(context.GetPtr())`），而 **Context 没有导出创建函数**（导出表无 Context_*）——Context 由 exe 启动流程内部创建。外部宿主 bootstrap = 逆向整个启动序列（子系统注册/资源路径/包挂载/反作弊初始化），版本敏感、不可维护。
3. **发布要的两个 native 能力恰不在导出面**：`preprocess_game`（打图集）与 `DebugManager` 无导出（生成 Ref 有导出但要 Context+LoadMap 全链路）。
4. **官方无 GUI 控制台工具已存在：`version-<api>/commandtool.exe`**（13MB，Urho3D 宿主），命令集（字符串实证）：
   `Pack`（打包）/ `MapRef`、`FullRef`、`MergeRef`、`TableRef`、`CopyMapRef`、`DirectoryOrFileRef`（Ref 体系）/ `TextureCompressBatchProcessing`、`TriggerServerTextureCompress`、`TriggerServerAnimCompress`（纹理/动画压缩=打图集重活）/ `ConfuseFiles`（混淆）/ `ModelResolve` / `RemovePrefabLod` / `StatsAssetSize` / `GenerateLiteAsset` / `TextureCube` / `AndroidPakShader` / `GenerateDevicesProfile` / `AnalyzeBacktrace`。
   调用形式（xdeditor console/inner_ui.lua:203 实证）：`CommandTool.exe -ExeFunc=Pack -platform=android -project_path=<路径> -cache_path=... -ref_path=... -shader_cache_path=... -acmap_path=...`；日志落 `logs/tool/CommandTool-<命令>.log`；`print_error_code`/`Write error code to file` 支持错误码落文件。
   🔴 发布流程的 preprocess_game/generate_ref 与 CommandTool 的内部关系待确认（可能 in-process 实现相同逻辑，也可能外壳调用）。

## 4. 调试链路

### 4.1 PIE（编辑器内嵌）与远端 host

已有 [editor-debug-control.md](editor-debug-control.md) 的菜单命令层。本轮补充 host 分配细节（xdeditor map_starter/init.lua 实证）：

- `debug_game_via_remote_host` 默认 true → `query_assign_host()`：`POST http://<_G.IP>:9007/api/v1/assign_host`（带签名 header，body={api_version}）→ 返回 `host_info={ip, port, token}`（云端调试服务器）。
- 拿到 host 后 `DebugManager.update_host(ip, port, token)`（co.call 等连上）→ 复制地图到调试目录（`MainFrame:GetDebugMapPath()`，清空后按白名单拷贝）→ `DebugManager.debug_game{map_path, lua_debug, is_trigger_debug, map_kind, game_in_editor, mobile_renderer_emulation}`。
- 调试画面尺寸：`-width/-height` 参数按项目 debug_settings 分辨率比例计算（上限 2340x1080）。
- **本地后门**：`_G.__fortest_still_use_local_host = true` → host 固定 127.0.0.1:5003 跳过云端（官方自留，editor-debug-control.md §5 已记）。

### 4.2 官方 headless argv 全集（main.lua 启动分发，实证）

编辑器 exe 的 argv 入口（main.lua:30-65 + 710-718）：

| argv | 效果 |
| --- | --- |
| `-sub_process` | 子进程模式（sub_process_enter_point；io.set_package_io_mode(0)） |
| `-upload_map=<地图路径>` | **无头发布**：登录 → load_map → save → upload_map → os.exit(0=成功/1=失败) |
| `-generate_map=<路径>` | 无头生成（触发器→lua 并保存） |
| `-upload_lib=<名字,逗号分隔>` / `-upload_lib_abs=<路径,名字[,api][,主库];...` | 无头发布依赖库 |
| `-generate_and_debug_map -file_path=<路径>` | **无头调试**：登录回调 → map_starter（加载地图→generate_lua_only→复制调试目录→assign_host→debug_game→os.exit(0)） |
| `-skip_login` | 跳过扫码登录（游客身份，login.lua:626/661-663） |
| `-autotest` | 免登录（autotest_app:do_argv_task 自动测试框架） |
| `-local_test[=2]` | 跳过更新流程（=2 时仍允许更新） |
| `-no_clear_shaders` | 跳过启动清 shader 缓存 |
| `-server=<域名>` / `-http=<地址>` | 覆盖 _G.IP / http 基址 |
| `-commit_message=<文本>` | 发布 comment 字段 |
| `-compress_use_7z=0` | 发布压缩包用 zip 不用 7z |
| `-editor_api_version=<n>` / `-file_path=<project.sce>` / `-inner` / `-winui_material_editor` / `-winui_resource_store` | 常规启动参数（bridge editor_start 实证形态，见下） |

- 无头发布超时护栏：generate_cmd 启动后 2 分钟内未进编辑器主流程 → os.exit(1)（main.lua:61-65）；menu_bar.lua:1228-1259 负责分发到 map_generator 对应函数并以返回值作为进程退出码（0/1/2=部分成功）。
- **已实证的编辑器启动命令**（sce_app_editor-patch editor.rs:94-100，0.5.4 起生产在用）：

```
D:\sce_online\星火编辑器.exe -inner -winui_material_editor -winui_resource_store \
    -editor_api_version=<api> -file_path=<项目>\project.sce
```

## 5. 「脱机调试发布」方案设计（基于以上实证）

### 5.1 可行路径对比

| 方案 | 形态 | 优点 | 缺点 |
| --- | --- | --- | --- |
| A. MCP 驱动在线编辑器（现状） | editor_start + start_debug/publish_project | 已生产可用 | 编辑器 GUI 常开 |
| B. **官方 headless argv** | `星火编辑器.exe -upload_map=<路径>` / `-generate_and_debug_map -file_path=<项目>\project.sce` | 官方通道、零补丁依赖、退出码即结果 | 仍起引擎进程（但无交互）；发布进度推送依赖 map_publisher 长连接（进程内仍全） |
| C. 纯外部 HTTP 复现 | 工具自己 zip+签名+POST publisher API | 完全不启编辑器 | 打图集/generate_ref 是 native 能力复现不了；进度推送缺失；压缩格式细节（7z 参数）需对齐——**不推荐独立使用** |
| D. **CommandTool.exe 组合拳**（§3.3 新发现） | headless argv（编辑器 exe）+ `CommandTool.exe -ExeFunc=...`（Ref/压缩/混淆）+ 外部 HTTP 上传 | 不开 GUI 且全部走官方二进制 | CommandTool 各命令参数细节待实测；与发布流程的真实依赖关系待确认 |

**推荐：B 为主（官方 headless argv + 凭证文件）；D 是 B 的增强（要拆细发布步骤时用 CommandTool 单独跑重活）；C 只用于查询类 API（api-version 等）。「LoadLibrary sceengine.dll 直接调」已证不可行（§3.3 第 2 条：Context 无导出）。**

### 5.2 凭证层设计（直接落地的结论）

- **读已登录凭证**：直接读 `User/user_info-<环境>.json`（明文）。
- **多凭证**：凭证=单文件，应用侧做「凭证库」（多份 json 存档 + 账号备注），切换=编辑器关闭时换文件。
- **自登录**：复现 TapTap device flow（§2.2 纯 HTTPS）拿 token/access_token，拼 user_info.json 落盘——免去扫码开编辑器。
- **注意回写**：编辑器运行中会 refresh_token 并 `account.save()` 覆盖凭证文件；外部凭证库应在每次编辑器关闭后重新收割（harvest）最新文件。
- 凭证有效期：login_token 注释称「长期有效」；access_token 走 refresh；实测有效期需观察（失效表现=启动时弹扫码）。

### 5.3 脱机调试的特殊点

- 无头调试依赖云端 assign_host（9007 端口签名 POST）——凭证有效即可，无需 GUI。
- `_G.__fortest_still_use_local_host` 后门只在 map_starter 流程里（headless 调试可用本地 host 5003，如果本地有 host 服务的话）。
- 无头调试 map_starter 跑完 `os.exit(0)`——**调试局的生命周期与编辑器进程解耦**（游戏由 host/独立进程承载），具体进程形态待实测。
- headless 模式下弹窗风险：map_starter 的地图完整性检查失败会弹 message_window 等人点（map_starter/init.lua:179-189）——脱机工具要监控超时并杀进程兜底。

### 5.4 待实测清单（下一步）

1. `星火编辑器.exe -upload_map=<test_res002 路径>` 真跑一次：确认无头发布端到端通（观察 logs/ 与退出码）。
2. `-generate_and_debug_map -file_path=...` 无头调试：确认 assign_host 成功、游戏起来、进程何时退出。
3. 凭证有效期观察 + 换凭证文件的编辑器启动验证。
4. 纯 HTTP 签名调 `updater/api-version` 验证 §2.3 签名在编辑器外成立（最低风险的外部 API 试验）。
5. CommandTool.exe 逐个命令摸参数（先 `-ExeFunc=MapRef` / `TextureCompressBatchProcessing` 在测试项目上跑，看 logs/tool/CommandTool-*.log），并确认发布流程是否会自己调它（hook 一次正常发布观察 logs/tool/）。

## 6. 自托管「最小编辑器」评估（用户线索深挖，2026-08-21 追加）

> 线索来源：用户提供的依赖清单路径 + restore_game.py（examples/ 新增的一键还原工具，含伪 KTX 解码）。

### 6.1 包依赖清单与定位链（全部实证）

- **总清单**：`update/editor-pd.spark.xd.com/api_pak_version.json`（720KB）——api 版本 → {包名: 版本}。api 13 实证：appui=48, script=199, gameui=48, lib_lobby=168, defaultui=63, lib_control=46, lib_game_options=105, lite=2, map_templates=42...（数十个）。
- **包实体两种形态**：
  1. 散目录：`update/<env>/res/_m/<包>/<版本>/<包>/`（编辑器在线更新下载的）；
  2. 内嵌 7z：`version-13/embedded_packages/`（实证：script-199.7z / script-190.7z / client_base-78.7z / appui-28.7z+48.7z / startup-364.7z / xdeditor_startup-21.7z）——编辑器也内嵌包，与 tester 同构。
- **引擎的库注册表**（sceengine.dll 字符串实证）：`script;client_base;startup;xdeditor;xdeditor_startup;refconfig;appui;engineres;ui;fonts`——引擎按名字维护多库资源根。
- **`@` 跨库解析是 native**：`require '@base.base.account'` → client_base 库的 `common/base/account.lua`（字符串 `client_base/common`、`@xdeditor_startup.main` 实证）。自托管时用自定义 package.searcher 复刻即可（纯查表映射，容易）。
- **启动链**：`星火编辑器.exe`（launcher）→ `version-<api>/sce`（739KB 引导 stub）→ CoreCLR(sce.dll) + lua54 + sceengine.dll → xdeditor_startup/main.lua（登录/更新）→ xdeditor/main.lua（编辑器本体）。startup 包 = 大厅/登录/支付/防沉迷 UI（StateApplication）。

### 6.2 发布链路要用的 native 面（自托管宿主需提供/替代的清单）

按 common/upload/init.lua + upload_map_view.lua + account.lua 的调用面清点：

| 依赖 | 性质 | 自托管替代方案 |
| --- | --- | --- |
| lua54 运行时 | 现成 dll（version-13/lua54.dll） | 直接 P/Invoke 嵌入（Rust/C# 宿主） |
| `sce.httplib`（request/create/stream/create_stream） | native HTTP client | 用 reqwest/.NET HttpClient 实现同名 Lua 绑定（面很窄：url/method/input/query/json/output stream） |
| `io.*`（read/write/list/zip_file/copy_to_folder...） | native 文件 IO | 自己实现；`io.zip_file` 用外部 7z.exe 或 CommandTool Pack 替代 |
| `common.*`（get_md5/get_system_time/generate_qrcode...） | native 杂项 | 逐个实现（md5/时间戳等 trivial） |
| `base.*`/`coroutine`/`co`/`timer`/`wait` | **script 库纯 Lua**（client_base/script-199） | 官方代码原样跑，只需宿主提供帧循环/定时器驱动 |
| `account` | client_base 纯 Lua | 原样跑；凭证从 user_info-*.json 预填 |
| `lobby`（logined/request_token_login/事件） | **native 长连接**（最难替代） | 发布链只读 `lobby.logined`→可打桩 true；登录态从凭证注入，不走真实大厅连接 |
| `sce.map_publisher`（进度推送） | native | 放弃实时进度，只等 upload-map 的 HTTP 结果 + 超时轮询 |
| 打图集/Ref（preprocess_game/generate_ref） | native（DebugManager/MapBuilder） | CommandTool.exe（-ExeFunc=MapRef/TextureCompress...）或先跳过实测 |
| 编辑器 UI 层（upload_map_view 等） | xdeditor 纯 Lua | **不需要**——直接调 common/upload 的 upload_map(params)，绕开 UI 层 |

### 6.3 发布对资源的转换（restore_game.py 佐证的新事实）

- **发布产物里的 `.png` 全部是伪 KTX**（p_55a3.pak 三处抽查，魔数 `AB 4B 54 58 20 31 31 BB 0D 0A 1A 0A` + `01 02 03 04`）：BC7/DXT/RGBA8 纹理格式，BC 系 R/B 通道互换。restore_game.py 已含解码器（偏移 28=internalFormat，36/40=宽高，64=imgSize（最低位=pad 标志），68+pad=数据）。
- 未定性：PNG→伪KTX 的转码发生在客户端发布期（CommandTool TextureCompress？）还是服务端。**本地 PIE 读明文 PNG 正常**，说明运行时兼容明文——自托管发布若跳过转码，服务端/运行时是否接受需实测（最坏情况：用 CommandTool 的 TextureCompressBatchProcessing 补这一步）。

### 6.4 结论

**「自己实现一个简单的编辑器」对发布/调试链路可行**，形态 = lua54 窄宿主 + 官方 Lua 包原样跑（startup/xdeditor_startup 不需要）+ 窄 native 面仿真（httplib/io/common）+ lobby 打桩 + 凭证注入 + CommandTool 补重活。不需要渲染子系统。对编辑功能（场景/数编 UI）不可行也不必。

相比方案 B（headless argv 官方通道）：自托管的优势是进程完全自控（可嵌入 bgd_sce_tools 应用内、无弹窗风险、可多开并行）；代价是要维护 native 面仿真与版本漂移（api_pak_version.json 变化时跟进）。

**建议路线**：先 B 跑通（§5.4-1/2 实测）拿到基准 → 再做自托管宿主（以 common/upload/init.lua 的 upload_map 为唯一调用点，向上自己组装 params）作为 v2 增强。自托管的验证标准 = 与方案 B 发布结果逐字节对比 pak。

（完）
