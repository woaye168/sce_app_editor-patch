# 编辑器调试三大链路详解（LuaDebug / 本地服务器 / 手机调试）

> 2026-09-02。方法 = 静态源码（xdeditor/menu_bar + 引擎字符串表 + LuaPanda 协议样例）
> + **实机验证**（test_res002 真拉起 + 5003 自建 host 桩全链跑通）。
> 本文与 multi-player-debug.md 互补：那边讲多人 PIE，这边讲调试器/本地 host/手机通道。

## 1. LuaDebug（菜单「调试/调试(启动LuaDebug)」）

### 是什么

引擎内嵌的 **LuaPanda 3.2.0 定制 fork（SCELuaDebugger）**——不是 MobDebug、
不是 EmmyLua、不是标准 DAP。菜单传 `lua_debug="server;game"`（menu_bar.lua:2228），
C++ DebugManager 拆成两个 argv 分别下发：
- `server` → 调试 host 的服务端 VM：host 侧打印
  `[common/preload/luapanda.lua:235] Debugger start as SERVER. bind host:0.0.0.0 port:15635`
  （2026-09-02 实测，SERVER 模式 = 被调试端监听等客户端）；
- `game` → 游戏客户端进程：`-lua_debug_application=%d`（CLIENT 模式 =
  主动连 VSCode 扩展监听口，引擎字符串 `try connect SCELuaDebugger begin. address: %s:%d`）。

### 协议与客户端

- 协议 = LuaPanda 私有 JSON 指令集（`{"callbackId":N,"cmd":"...","info":{...}}`：
  initSuccess/setBreakPoint（普通/条件/日志点三种 type）/getVariable
  （varRef 10000~2万局部、2万~3万全局、3万+ upvalue）/getWatchedVariable/
  continue/stopOnStep(F10)/stopOnStepIn(F11)/stopOnStepOut/stopOnEntry/
  stopOnBreakpoint/refreshLuaMemory），逐条样例在 xdeditor
  `trigger/debug/init.lua:33-129` 注释里（编辑器触发器调试面板说同一协议，
  经 C++ DebugTrigger 服务中转）。
- **协议形态**：消息 = json4lua 编码的 JSON + `|*|` 分隔符（luapanda.lua:113，
  与原版相同）；握手初始化字段（adapterVer/luaFileExtension/cwd/TempFilePath/
  logLevel/pathCaseSensitivity/useCHook/distinguishSameNameFile/truncatedOPath/
  autoPathMode/stopOnEntry/isNeedB64EncodeStr，1476-1494 行）与原版逐字段一致；
  版本不一致只提示不阻断（586-589 行）。
- 配套客户端 = **VSCode 扩展 SCELuaDebugger（stuartwang.sceluadebugger）**；
  编辑器会生成 `.sce_workspace.code-workspace` 并拉起 VSCode（引擎字符串
  `run vscode cmd: {}`）。**本机未安装该扩展**（2026-09-02 查证
  .vscode/.trae-cn extensions 只有 EmmyLua——注意 EmmyLua 是另一套调试体系，
  与 SCELuaDebugger 不通用）。
- **编辑器触发器调试面板说同一协议**：xdeditor `trigger/debug/init.lua`
  （1154 行）经 `DebugManager:get_debug_trigger(SCE.EDITOR_AS_SERVER /
  EDITOR_AS_CLIENT)` 收发同款 JSON 消息，由 C++ DebugTrigger 服务中转
  （`DebugTrigger Listen {}:{}`，等 vscodeClient 连入）——即编辑器自带一个
  免装扩展的触发器调试客户端，但只覆盖触发器不覆盖 lua 源码断点。
- 端口：SERVER 模式实测固定 15635（host 服务端 VM）；CLIENT 模式的扩展监听口
  未在现有产物中定位（扩展未安装，抓不到）。编辑器 DebugServer 另监听
  [6325,6375) 段第一个空口做地址转发（`DebugServer Listen {}:{}` +
  PullDebugAddressInfo/NotifyDebugAddressInfo 消息组，与本机 netstat 实测
  6325-6328 一致）。
- C++ 侧另有 **lua-cmsgpack 0.4.0**（MessagePack 序列化备选通道，
  `lua_debug_cmsgpack.inl`，线协议仍走 json4lua）。
- 旧废弃通道（勿混淆）：script 库 `common/base/debugger.lua`（dbg:io
  'listen:0.0.0.0:4278'）在 main.lua 里是注释掉的死代码，与本链路无关。

### 我们的现状判断

LuaDebug 对 bgd 工作流**非必需**：我们有 dbg_bus/lua.eval 逃生舱 + cgui 反射
+ 日志排障，覆盖其 80% 场景；真正独占的价值是「断点单步 + 变量检查」。
要用它：装 VSCode + SCELuaDebugger 扩展（扩展市场搜 SCELuaDebugger），
调试时走「调试(启动LuaDebug)」菜单（或 debug_save_as 传 lua_debug='server;game'）。
0.8.7 与其零冲突（debug_save_as 加 lua_debug 参数即可叠加）。

## 2. 调试(本地服务器)（use_local_host → 127.0.0.1:5003）

### 官方设计

- 菜单（inner 专属，menu_bar.lua:2124）= `debug_save_as{use_local_host=true,
  muti_debug_info=getDebugUserInfo()}` → 跳过 assign_host，直连
  127.0.0.1:5003 token='qwert'（:387-390）。
- 对端官方形态 = NE 服务端 **host.exe**（GameHost，lua 服务运行时），由内部
  开发树的 GameHostServerLauncher 拉起：`bin-release/host.exe -d -startupmode=2
  -nogfw -noserviceagent -port=5003 -config_path="...?.lua" -log_folder="logs/host"`
  （sceengine 字符串 :442236-442239）。**该二进制不随正式编辑器分发，本机没有
  （实物核查 D:/sce_online 全树）**——所以「怎么起本地服务器」的官方答案 =
  正式版用户起不了，这是内部开发设施。

### 自建 host 的可行性（2026-09-02 全链实证）

用 mini-runtime 逆向的控制协议（scegame-reverse.md §8）自建 5003 桩
（本仓库 examples/host_stub.rs，~200 行），实测编辑器「use_local_host 调试」
全链跑通：

```
EditorLogin(token='qwert') → 桩回 0xF001 result=0
→ 编辑器上传 312 文件（SendWriteFile，路径 p_55a3/... 全小写）→ 桩逐文件回 0xF010 ack
→ EditorStartGame(p_55a3, api=13, 依赖表) → 桩回 0xF018 result=0 session_id
→ 编辑器拉起 PIE 客户端 ✓（is_plugin_ui_loaded('GamePlayInEditor')=true）
→ 桩持续收 0xF01F（编辑器心跳）/ 0xF01B（停止调试时的销毁通知）
```

- **token 校验**：host 端自持——自建桩收下 f2 即放行（EditorLogin 实测帧
  `... 08 e6 b2 b8 12 12 05 71 77 65 72 74`：f1=966758, f2='qwert'）。
- **0xF01F 身份**：停止调试后编辑器持续发 0xF01F（13 字节心跳/保活）+
  0xF01B（18 字节，停局/销毁通知）——该消息号不在 mini-runtime 原消息表中，
  本次新登记。
- **ack 时序实证**：SendWriteFile 后编辑器**等待** 0xF010 ack 才继续发下一个
  文件（v1 桩不回 ack 时编辑器卡在 update_host 阶段静默等待；v2 桩回 ack 后
  312 文件 5.9 秒传完）。与 mini-runtime 文档「编辑器流水发送不等 ack」的
  记录相反——**本地连接（127.0.0.1）实测是逐文件等 ack 的**，云端高延迟下的
  行为可能不同，此差异需后续核实修正 scegame-reverse.md。
- **客户端侧边界（2026-09-02 深夜更新：已被 mini-runtime 中继打通）**：PIE 客户端以
  `-host_ip=127.0.0.1 -host_port=5003` 进局时，**KCP 会话端口 = 控制端口 + 50
  （引擎硬编码，实际 dial 5053）**——桩只监听 5003 所以进不了局（lua-game 0 字节
  实证的真正原因）。mini-runtime local_host.rs 中继（TCP 控制面协议感知转发 +
  UDP 5003/5053 双端口 KCP NAT 到云端）已端到端验证：本菜单 → 中继 → 云端 →
  PIE 真实进局。KCP 会话协议已破解（CE1 握手族/标准 KCP/3B 流分帧/c2h 明文
  protobuf+cmsg_pack/h2c=ZCompress 压缩无加密），详见 mini-runtime
  doc/research/scegame-reverse.md §13 与 self-host.md。
- **价值**：编辑器侧全链（登录/上传/起局/停局/心跳）可完全脱云自测；
  多人调试的 host 行为研究、上传协议排障、离线环境开发都有用。
  排障速查：编辑器只在「拷贝地图 + debug_game 上报失败」时经
  debug_exception_handler 弹错；连接异常静默断线（无重连日志）。

## 3. 手机调试（菜单「调试/手机调试」+ 左下角 [Disconnect] x.x.x.x:6251）

### 已查证的事实

- PC 侧：inner 菜单 → save_map → `DebugManager:phone_debug()`（C++）——
  编辑器=服务端监听 **0.0.0.0:6251**（本机 netstat 实测：4 个 sce 相关进程
  全部 LISTENING 6251，即服务**常驻自动启动**，不点菜单也在听；
  状态灯只反映「有无设备连入」：server_is_active() + connect_state_changed
  事件，foot.lua:66-91，仅 inner 版显示）。
- 显示 0.0.0.0 = `common.get_local_ip()` 未取到有效本机 IP 的兜底；
  能显示局域网 IP 时的语义 = 「手机连这个地址」→ **手机需与 PC 同局域网**。
- 点菜单弹「未连接」= 当前没有任何手机客户端接入 6251（与状态灯 Disconnect
  同义）。
- 端口附注：DebugServer 另在 [6325,6375) 挑空口监听（本机 6325-6328 实测），
  与 6251 的关系未在源码中确认（疑似 6251=连接口、63xx=调试数据转发口）。

### 无据边界（如实）

**手机端入口叫什么、怎么发现 PC、是否扫码——研究范围内全部无据**
（引擎字符串表/xdeditor 全库/script 库/mini-runtime 全部 research 文档均无
星火手机 APP 名、配对流程或 connect_editor 客户端实现）。可执行的取证方向：
手机上星火/对战平台 APP 的「连接编辑器/真机调试」入口（TapTap 账号体系，
credential-userid.md 记 token_type=13=手机端）；或 Frida hook phone_debug 看
真实握手。在此之前，手机调试对本工作流不可用也不阻塞（PIE 已覆盖）。

## 4. 还原可行性深度评估（2026-09-02 补充）

### 4.1 LuaDebug 还原（结论：不用还原，直接用）

**LuaPanda 本体就是明文开源 Lua 躺在包内**：`common/preload/luapanda.lua`
（TNND 加密但密钥已知，解密副本 `.editor_src_mirror/client_base/common/preload/luapanda.lua`，
3766 行，头部腾讯 BSD 版权 + `debuggerVer = "3.2.0"` + `TCPSplitChar = "|*|"`）。
与开源 Tencent/LuaPanda 3.2.0 的 fork 差异仅约 5 处几十行：
① 日志同时写星火 log_file；② `hookLib = luapanda_chook`（Lua5.4 源码级内嵌，
不 require libpdebug.dll）；③ socket 换引擎自实现 async_tcp；④
add_blocked_time_callback 卡顿统计；⑤ preload 自动加载 glue。
**协议零改动**（json4lua + `|*|` 分隔，握手字段逐一致）。

- chook（libpdebug C 部分）编译进 sceengine.dll，导出函数名与开源完全一致
  （sync_breakpoints/sync_debugger_path/stopOnStep 等全套），无需还原。
- **接入路径**：装开源 LuaPanda 3.2.x VSCode 扩展（或 stuartwang/SCELuaDebugger
  fork），launch.json 指向 game:15635（LuaPanda 原生支持 lua 作 SERVER 的模式，
  `startServer` 就是原版功能）；若扩展不支持外连则加一个 ~30 行 TCP 双向桥
  （game:15635 ↔ 127.0.0.1:8818）。断点路径映射用原版 cwd/truncatedOPath/
  autoPathMode 调（`@` 前缀是 Lua source 标准前缀非星火定制）。
- 验证清单（运行时一次性确认）：adapter 外连配置项名、chook getPath 对
  `@包/` 虚拟路径的实际输出。工作量 = 配置级半天。

### 4.2 本地服务器演变（mini-runtime → 完整服务端）

已在 §2 实证编辑器侧控制台协议全链可行（312 文件上传/起局/心跳/销毁）。
距「完整可用的本地调试 host」还差（按工作量排序）：

| 层 | 内容 | 现状 | 量级 |
| --- | --- | --- | --- |
| 控制台 TCP | EditorLogin/上传/起局/日志/心跳/销毁 | **已完成**（host_stub.rs 实证） | 小（已做） |
| 载荷管理 | staging 生成、依赖库按 api_pak_version 注册表落位、增量缓存 | mini-runtime 已有（staging.rs/payload.rs，B 模式全链自托管已跑通） | 中（已有） |
| 游戏会话 KCP | 客户端进局握手、玩家会话、帧同步/状态同步、心跳保活 | **协议已破解 + 中继转发已上线**（2026-09-02：CE1 握手/KCP/3B 流分帧/c2h 明文/h2c ZCompress；local_host 中继双链路进局实证） | 中（自研会话面剩 ZCompress 格式复刻） |
| 服务端运行时 | 跑服务端 lua 的引擎（sceengine server 模式） | ~~理论可复用本机 SCE 壳~~ **已证伪**（2026-09-02 self-host.md §3/§6：客户端引擎无 GamePlayServer，debug_via_remote=0 实验无 host.exe 可拉）；对自研逻辑项目可用 GameHost.lua 复刻 + 薄 native shim 替代（self-host.md §8 B+ 路线） | 中-大 |

**判断（2026-09-02 深夜更新）**：mini-runtime 已交付「中继 host」（0.4.0）——编辑器侧控制面 + KCP 会话面双链路实证可玩（§2）。KCP 协议已破解，剩余壁垒从「协议逆向」缩小为「ZCompress 压缩格式复刻 + GameHost.lua 编排复刻 +（自研逻辑项目）薄 native shim」；通用服务端引擎本体不可得（客户端引擎无 server 半身，已证伪复用 SCE 壳的猜想）。详见 mini-runtime doc/research/self-host.md（§8 架构 A/B/B+/C）。

### 4.3 手机调试还原（lib_lobby 线索已证伪 + 官方链路已还原）

**用户记忆的 lib_lobby 入口 = 误记（实证）**：lib_lobby（166~171 各版本全查）
与调试相关的只有游戏内日志浮窗（page_debug.lua，F9/F12）和 Ctrl+D 表达式
求值页；`connect_editor/6251/连接编辑器` 全库零命中。真正的手机端入口在
**tester 启动壳**（星火对战平台 APP 的 startup Lua）：
`application/entrance/main.lua` —— argv 同时有 `-editor_server_debug` 和
`-local_test` 时启动即自动 `lobby.connect_host()`（CONNECT_HOST_IMMEDIATELY），
且壳层原生 UI 有「输 IP/端口 + 连接 host」入口（`on_connect_host`/
`on_input_host_port` 绑定，1545-1558/2201-2204 行；`local_host` argv 可指定）。

**官方「手机调试」用户链路（官方文档 + 源码对齐）**：
编辑器菜单「调试/手机调试」→ 存盘 → 编辑器监听 6251 → 手机上装**星火对战
平台 APP**（doc.sce.xd.com 官方文档「通过星火对战平台实机调试」）→
内测模式加入（邀请码）/连 PC → TCP 连 6251 → EditorLogin 握手 →
编辑器经 0xF004/0xF008 逐文件**一次性推送当前地图**到手机 →
EditorStartGame 起局 → 手机作为 game client 走 GamePlayOnline 进
**编辑器本机充当的局 host**（不经云端）；日志经 0xF00C 回传编辑器。
游戏内调试工具 = lib_lobby 的日志浮窗/表达式页（用户记得的「入口」真相）。

**「还原」可行性（两个方向都已可行）**：
- **PC 假装手机**（自建客户端连真实编辑器 6251）：协议 = mini-runtime 已破解
  的同一套（EditorLogin + 收文件 + 起局），把 host 指向编辑器 6251 即可；
  待确认项 = 手机调试场景 host_token 是否校验（抓包一次定稿）。
- **自建 PC 服务端**（接受官方手机 APP 连入）：host_stub 已证编辑器侧协议
  对称可实现，监听 6251 + 补地图推送即可；前提 = APP「连接 host」入口
  能指到自建 IP（壳层有绑定实证，但该原生按钮的可见条件未证实）。
- **自写手机 APP**：不可行（scegame 无公开移动端构建），也不必要。

## 5. 三链路关系图

```
编辑器（inner 版）
├─ PIE 调试（默认）：assign_host 分配云端 host → N 个 PIE 客户端 KCP 进局
│   └─ 叠加 lua_debug='server;game' → host VM 15635 监听 + 客户端回连 VSCode 扩展
├─ 调试(本地服务器)：跳过分配，直连 127.0.0.1:5003（官方 host.exe 不随正式版分发；
│   mini-runtime 0.4.0 中继 host 已打通全链：控制面转发 + KCP（5053=5003+50）NAT 到云端，
│   PIE 真实进局；纯本地服务端仍需 GameHost.lua 复刻，见 mini-runtime self-host.md）
└─ 手机调试：编辑器常驻监听 6251，等同局域网的手机 APP（星火对战平台 tester 壳，
    `-editor_server_debug` 入口见 §4.3）连入，编辑器一次性推图后起局
```
