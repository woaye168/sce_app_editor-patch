# 模拟多人调试（Muti-Debug）机制研究与 MCP 化实证

> 2026-09-01。方法 = 静态源码（xdeditor 160 解密源码 + sce.dll 反编译）+ **实机探针**
> （经现有 MCP 的 `lua.run_lua` 编辑器 VM 逃生舱与 lobby 总线广播，在 test_res002
> 真实拉起 2 人多人调试完成 spike）。本文是 0.8.7 需求的事实底座。
> 行号基准：`D:/sce_online/Update/editor-pd.spark.xd.com/Res/_m/xdeditor/160/xdeditor/`；
> C# 反编译：`D:/sce_online/Res/maps/bgd_glzy/.tmp_verify/decomp/sce/`。

## 1. 运行形态（实测定稿）

- 每个启用槽位 = **编辑器进程内一个 PIE 内嵌游戏实例**（`debug_game_in_editor=true`，
  menu_bar.lua:49），不是独立进程。slot_id = `'GamePlayInEditor'..index`（:1799），
  官方对话框固定 4 行（MutiDebugWindow.cs:574-593），单人调试槽位固定
  `GamePlayInEditor`（无序号，且无暂停按钮）。
- 服务端在云端调试 host（assign_host）或本地 host（`use_local_host` → 127.0.0.1:5003），
  不在编辑器进程；**最后一个客户端 tab 关闭即销毁游戏局**（menu_bar.lua:647-649）。

### 1.1 云端 host 分配（2026-09-02 实机取证）

- 链路（menu_bar.lua:285-407）：`debug_game_via_remote_host=true`（:56 默认）→
  `query_assign_host()`：**POST `http://editor-pd.spark.xd.com:9007/api/v1/assign_host`**
  （`_G.IP` = 环境域；body 只带 `api_version`，经 `account.http_request_with_token`
  带账号 token）→ 返回 `host_info{ip, port, token}` →
  `DebugManager.update_host(ip, port, token)`（C++ 侧连接+login，协程等待）。
- 实测响应（test_res002，api 13）：
  `{"result":0,"host_info":{"ip":"106.14.95.227","port":"15435","token":"b0ce88d0-…"}}`——
  **每次调试由服务器侧按需分配一台云端调试 host**（动态 ip/port/一次性 token）；
  无可用 host 时返回无 host_info → 报错「当前调试用户较多，服务器拥挤」。
- 「分配策略定制」的含义 = 干预这个分配（ pinning 本地 host / 自建 host / 选区）；
  唯一官方开关是 argv `use_local_host`（固定 127.0.0.1:5003/token 'qwert'，
  :387-390，需本机已有调试 host 在跑）。0.8.7 沿用默认分配即可，列非目标合理。
- 多人局 N 个客户端连**同一个 host:port**（一局），host 侧按 userid 区分玩家。

### 1.2 调试三角色与 host 相关知识分层

PIE 调试的分布式结构（三个角色，别混淆）：

| 角色 | 谁扮演 | 说明 |
| --- | --- | --- |
| **调试控制台（client of host）** | 编辑器（DebugManager，C++） | 上传地图/起局/收服务端日志（控制协议见 mini-runtime doc/research/scegame-reverse.md §8：EditorLogin → 逐文件上传 → EditorStartGame → 0xF018 拿 session_id） |
| **游戏服务端（host）** | 云端分配的调试 host（默认）或本机 host（use_local_host） | 跑服务端 lua（`logs/server/lua-game-server-*.log` 就是它的输出）；按 userid 区分玩家；局随最后一个客户端断开而销毁 |
| **游戏客户端** | 编辑器内 PIE 实例（每槽位一个 VM）/ 手机真机 | KCP 连 host 进局；客户端 lua 日志混写 `logs/lua/lua-game-*.log` |

- **use_local_host 的价值场景**：离线/无配额环境、协议逆向研究（mini-runtime
  的脱机调试就是自建 host 客户端通道）、定点复现云端分配抖动。代价 =
  本机要先有调试 host 进程在 5003 监听（官方 scegame 以 host 形态启动；
  编辑器本身不提供「一键起本地 host」）。**2026-09-02 更新：mini-runtime 0.4.0
  已提供现成对端**——`debug start --host local` / `host start` 起 127.0.0.1:5003
  中继（TCP 控制面透传 + UDP 5003/5053 KCP NAT 到云端），本菜单即可全链进局；
  注意 KCP 会话端口 = 5003+50（引擎硬编码），任何自建对端都必须双端口监听。
- **与 6251 的区别**（编辑器左下角指示器）：`[Disconnect] 0.0.0.0:6251` 是
  **手机真机调试服务**的状态灯（菜单「调试/手机调试」→
  `DebugManager:phone_debug()`；foot.lua:66-91；仅 inner 版显示）——那是
  编辑器=服务端、手机=客户端连入的另一条通道，与 host 分配无关；
  0.0.0.0 = `common.get_local_ip()` 未取到有效本机 IP 的兜底显示。
- 每客户端一个编辑器 tab：标题 `玩家 N 视图`、图标按 muti_debug_info 下标取色
  （绿/黄/蓝/粉，:63-69；gameplay_in_editor_view.lua:123-131）。
- **实测**：2 客户端 Delay=0 同帧拉起一次成功（样本量 1，稳定性结论待开发期
  多次复跑；官方 Delay 默认也是 0，无证据表明必须错峰）。

## 2. 拉起链路与两条编程路径（v1 评审#1 / v2 评审#1 的定案）

官方链：

```
MutiDebugWindow.Debug()（C#，拼 JSON [{Enabled,Player,Team,Delay},...]，尾逗号宽松 JSON）
  → EventManager.SendEvent('CS_muti_debug', json)（MutiDebugWindow.cs:701）
  → menu_bar.lua:1752 处理器：数编校验 + 补 UserId/SlotID + 填 debug_user_info
  → debug_save_as{muti_debug_info=t, use_muti_debug=true}（:1803，:875 任务流水线封装）
  → _debug_save_as（:348）→ DebugManager.debug_game（:601-613，C++ native）
  → 逐槽位 pluginMgr:register_plugin_ui + sceneMgr:show_game_in_editor（:675-693）
```

**事件总线事实（已核实）**：C# `EventManager.SendEvent(任意字符串事件名)` 是引擎
导出 `EventManager_SendEvent` 的薄封装，事件名无白名单；与 Lua 侧
`eventMgr:register_event`（`SCE.GetEventManager()`，引擎绑定）是**同一总线**，
双向实证 = `CS_muti_debug` 与 `EditorMainTitleMenuBar` 两对收发。**Lua 侧没有反向
send 方法**（`MainFrame:SendEvent` 是对象域 Lua→C# 方向，不回环触发 Lua
register_event）。

| | 路径 A：纯 Lua 同 VM 直调 | 路径 B：桥 C# 发任意事件 |
| --- | --- | --- |
| 做法 | `package.loaded['@xdeditor/ui/menu_bar'].debug_save_as{muti_debug_info=t, use_muti_debug=true}`，UserId/SlotID/debug_user_info 自己复刻 :1752 逻辑补全 | 桥 C# 加方法 `_eventManager.SendEvent('CS_muti_debug', json)`（SendDirect 同款写法，EditorBridge.cs:114-124），官方处理器自动补全 |
| 改动面 | 零 C# 改动，桥 Lua 补丁内完成 | 需改 C# 桥 + 重编 dll + 重部署 |
| 实机验证 | **已 spike 成功**（2026-09-01，2 客户端上线、tab 标题/颜色正确） | 未做（总线能力已静态证实） |
| 校验控制 | 预校验逻辑自持，失败原因可精确返回 MCP | 官方处理器失败只写 info 列表，MCP 会假成功 |
| 结论 | **0.8.7 定案采用** | 备选（仅当 A 在后续版本失效时启用） |

### 踩坑（实锤）

- **`run_lua`/`load()` 上下文里 require 相对解析会落到 `@common`（script 包）**：
  xdeditor 的自定义 require 按调用 chunk 的包身份解析，load 出来的 chunk 无身份
  → 默认 `@common/` → `require 'ui.menu_bar'` 报 not found（找去 script/199 包）。
  **正解：`package.loaded['@xdeditor/ui/menu_bar']` 绝对键直取**（package.loaded
  全量键均为 `@<包>/<路径>` 形态）。补丁模块自身代码里 require 正常（有包身份），
  此坑只影响 run_lua/eval 动态代码。
- 「模拟多人调试,使用上次配置[且不编译]」两菜单被 `argv.has('inner')` 门控
  （:2106），且 `last_multi_debug_info` 纯内存会话级（:873）——正式版没有这两个
  菜单，即使有，编辑器重启后调用也静默无效（2026-09-01 实测）。
- `use_last_debug_info=true` 且 `last_debug_path==nil` 时**自动降级为全量**（:479-480），
  不报错；但只有 nil 判断，无目录有效性校验。

## 3. muti_debug_info 字段与数编依赖（实测值）

字段（:1752-1803 处理器语义）：

| 字段 | 来源 | 说明 |
| --- | --- | --- |
| Enabled | 调用方 | 启用该槽位 |
| Player | 调用方 | 数编玩家号（player_setting 的 key） |
| Team | 调用方 | 见下「Team 结论」 |
| Delay | 调用方 | 秒，≤0 下一帧拉起，>0 `base.wait(Delay*1000+1)` 错峰（:685-693） |
| UserId | 处理器补 | `Game.user_ids` 按 pairs(player_setting) 遍历序分配给 type='user' 且槽位 opened 的玩家（实测：玩家1→100，玩家2→101） |
| SlotID | 处理器补 | `'GamePlayInEditor'..index`（index = 数组下标，非玩家号） |

另需维护 `menu_bar.debug_user_info[UserId] = {player=N, icon_color=...}`（:1789），
否则 tab 标题退化为「游戏视图」、客户端日志面板无玩家归属。

**test_res002 实测数编（在线模型，经 obj_manager 读）**：
player_setting = 0:computer/t100 + 1~10:user（team 1/2 各 5 人）；
opened_slots = 1~10 全开；user_ids = 30 个（100~129）。
注意：磁盘 `editor/table/entry_data/map_config/$$.map_config.dflt/entry_data.ini`
里 opened_slots 是 2~10 的旧值——**以在线数编模型为准**（config.ini
[debug_setting] 与在线模型一致）。

### 槽位启用是否写数编（v2 评审#2 的精确答案）

MutiDebugWindow.cs:813-819 确认链路 =
`dataCore_.AddGameChange(DefaultMapConfigEntry.FullLink, ["opened_slots"], int列表)`
+ `CommitChanges()` → 引擎导出 `DataEditor_DoEntryNodeSetValue_Lua`——
**这是引擎 DataEditor 的正式数编写通道**：写数编**数据模型**（内存置脏 + 修改通知，
与玩家在「玩家设置」面板手动勾选完全同款），**代码中无直接文件 IO**，落盘走正常
地图保存流程。精确表述：不写文件，但写数编数据模型（会产生未保存改动脏标记）。

### Team 结论（v2 评审#3）

- Lua 侧整条链路**从不读 Team**（menu_bar.lua 全文无消费）；引擎字符串表显示
  `luaex_DebugManager.cpp` 只读 `SlotID`/`UserId`；Team 若在 C++/host 侧被消费，
  该部分无源码可查。
- 星火原生多人逻辑（单位/技能/buff/组队）与 TEAM 相关；但 test_res002 这些逻辑
  全部自持，**本期 Team 直接填数编 player_setting 的默认编队值即可**（玩家 N →
  team 1/2），不做策略。

## 4. 多 VM 总线拓扑（全部实测）

- `send_luastate_broadcast('bgd_dbg_cmd', ...)` **到达全部游戏 VM**：2 人局广播
  eval 收到恰好 2 条 `bgd_dbg_result` 应答。
- **`lobby.vm_name()` 游戏侧返回 nil**（dbg_bus ping 的 vm 字段现无值）——
  不能用作 VM 标识。
- **可用 VM 身份 = `base.local_player():get_slot_id()`**（实测 VM1→1/team1，
  VM2→2/team2，与玩家号一致）。`user_id()` 在 PIE 假用户下报错，不可用。
  `get_team_id()` 正常。
- 回执协议现状无 `from` 字段；编辑器侧 ui_loop 按 id 单配对（ui_loop.lua:34-38），
  多人下同 id 多答会互相覆盖——**0.8.7 要改的寻址缺口实锤**。
- **暂停语义**：`sceneMgr:disconnect_game_in_editor(slot)` 后该 VM **停止应答**
  dbg 命令（实测广播只剩 slot=1 应答）；`reconnect_game_in_editor` 后恢复
  （实测 2 应答回归）。切 tab **不会**自动 reconnect（switch 事件只调
  set_game_ui_focus，menu_bar.lua:668-671）。
- 后台 tab 的游戏 VM **逻辑照常 tick**（未暂停时广播应答正常）——只有画面
  合成受 tab 前后台影响。
- **暂停态无官方查询接口**（2026-09-02 实证）：sceneMgr 方法全清单 =
  hide/show/disconnect/reconnect/set_game_ui_focus/is_scene_focus——没有
  is_paused 类查询。桥侧只能自持 paused 标志（用户手动点 tab 暂停按钮会漂移，
  只影响标注不影响功能）。

### 4.1 离场与重进（2026-09-02 两轮实机实验，定案）

**问题**：玩家完整离场（关 tab）后能否局内重新加入？

- **离场语义（服务端真实生效）**：关 tab 路径（hide_game_in_editor(slot, false)
  + unload + unregister，menu_bar.lua:646-653）→ 服务端日志实见
  「玩家 101 断开连接，数据已清理」——是真离场不是假断开。hide 单调
  （不 unload/unregister）同样导致服务端离场（第二轮实验证实）。
- **重进 = 不可行（两轮实验一致结论）**：离场后对同槽位
  `show_game_in_editor`（无论是否重新 register/load）不会召回原玩家——
  实测行为是**挤掉在局的其他玩家**（服务端：玩家 100 断开）并启动**一个**
  全新客户端（拿到默认第一个 userid，slot=1），最终全局只剩 1 个 VM。
  即 C++ 侧 slot 调试会话在 hide 时已销毁，再 show 等于重建会话组且只含
  当前槽位。
- **结论**：官方流程不支持局内加人/重进（实测定案，非猜测）。
  **断线/重连测试的唯一手段 = 暂停/恢复**（disconnect/reconnect，VM 与
  会话都保留，见 §4）。0.8.7 非目标条款据此从「未实测」改为「实测不可行」。

### 4.2 切焦后画面合成延迟（2026-09-02 像素采样实测，定稿 wait_ms）

方法：VM2 开商店页（视觉差异）→ 焦点切到玩家 1 → 屏幕像素采样基线 →
`switch_page` + `set_game_ui_focus` 切到玩家 2 → 切后 45/180/315/514/833/1516ms
六个点采样视口中心 patch 哈希。
结果：**45ms 首个采样点已是玩家 2 画面且全程稳定**；切回玩家 1 后哈希精确
回到基线值（逐位相等）。结论：切焦合成近乎即时，**capture 的 wait_ms 缺省
取 200ms 即有充足余量**（WGC 路径另有窗口感知开销，此处测的是编辑器内合成）。
另注意：本测试走屏幕像素（CopyFromScreen），编辑器窗口需可见；WGC 离屏路径
不受影响。

## 5. 视口/窗口/截图（实测）

- base.ui.map 键两种：窗口根 `GamePlayInEditor1`/`GamePlayInEditor2` + 视口控件
  `ui-<n>-GamePlayInEditor1`/`ui-<n>-GamePlayInEditor2`（视口控件 name = slot_id，
  gameplay_in_editor_view.lua:105-117）。
- 现状 `get_game_view_rect`（桥 main.lua:343-371）正则 `^ui%-%d+%-GamePlayInEditor$`
  不匹配带序号后缀 → **多人局下 capture_game 必报错「游戏视口控件不存在」**（实测）。
- 两视口 `get_screen_rect()` 返回**完全相同的矩形**（328,132,1864,1047，同 dock
  区叠放），且控件上无可用的可见性 getter——**UI 树层面无法区分前后台 tab**；
  WGC 截的是编辑器主窗口合成画面，只可能含前台 tab。
  **结论：分玩家截图 = 先切 tab 再截，无免切方案。**
- 切 tab 编程路径（实机调通）：`_G.ui.switch_page(slot_id)` +
  `sceneMgr:set_game_ui_focus(slot_id)`（前者切显示，后者定键鼠输入焦点，
  menu_bar.lua:679-682/668-671 官方同款）。切后「等帧」时长未测定，开发期 spike
  定下界（建议起点 300-500ms）。
- **状态门禁缺口（实测）**：`get_status` 与 ui_loop `is_debugging()` 都用
  `is_plugin_ui_loaded('GamePlayInEditor')`（无序号）→ 纯多人局恒 false，
  lua.* 全部报「游戏未在调试」。另：`is_plugin_ui_loaded` 对**从未注册过**的槽位
  会抛错（pcall 得 nil），枚举 1..4 必须 pcall 防御（true=在线 / false=已卸载 /
  nil=未注册）。

## 6. 日志（实测）

- 多人局所有 PIE 客户端**混写同一文件** `D:/sce_online/logs/lua/lua-game-<时间>.log`
  （本次 2 人局未产生新文件，持续写编辑器启动时那份）。
- 行格式 `[时间][pid][级别][帧号][file:line] 消息`：pid 同进程相同；第 4 段是
  **帧号**（同帧多行同号，跨帧递增），**不是 VM 标识**；行内无玩家标签。
- **分玩家日志可行方案（2026-09-01 实机 spike 验证通过）**：复用调试信息面板
  同款机制——不碰文件、不碰 `log.set_log_handler`（引擎原生单槽位，包装有顶掉
  面板的风险），直接在 xdeditor 事件总线上挂监听：

  ```lua
  -- 监听调试信息面板的投递事件（面板已把 userID 映射成玩家号/颜色）
  EDITOR.event_register(EVENT.add_info_list, function(self, module, data)
      if module ~= 'debug_client_info' then return end
      local player = data.info_user_info and data.info_user_info.player  -- 玩家号
      -- data.info_type / info_message / info_location / info_frame 皆可用
  end)
  ```

  实测：2 人局中两个客户端各发一条 `log.info`，tee 精确捕获并按玩家归属
  （player=1/2 各 1 条）。**事件源链**：游戏 VM 日志 → 引擎按 PIE 会话 userid
  标记 → 面板 `log.set_log_handler`（debug_client_info_panel.lua:60，第 4 参
  userID）→ 查 `menu_bar.debug_user_info[userID]` 映射玩家号（:132）→
  `EDITOR.event_notify(EVENT.add_info_list, 'debug_client_info', {...})`（:126）。
- **踩坑（实锤）**：
  ① **事件监听回调首参是 trig 自身**（trigger.lua:57 `mt:__call` →
  `self:callback(...)`，方法调用语义）——签名必须是 `function(self, module, data)`，
  按直觉写 `(module, data)` 会静默错位收不到有效数据；
  ② dispatch 遇任一监听**返回非 nil 即中断后续投递**（base/event.lua:157-159），
  tee 回调必须返回 nil；
  ③ run_lua/load 动态代码里没有 EDITOR/EVENT 全局——从已加载模块函数的 `_ENV`
  upvalue 里取（如 `debug.getupvalue(menu_bar.exit_editor, ...)` 找 `_ENV`），
  补丁模块自身代码无此问题（有正常包身份）；
  ④ tee 只能拿到**面板管线放行**的日志（file_location 为 nil 的被面板 :75-77
  过滤，如部分 C++ 日志）；单人调试日志无 userID 映射（player=nil，归为未标记）；
  ⑤ tee 从注册时点开始收集，无历史；需自持每玩家环形缓冲（建议 1000 条/人）。
- 对 MCP 的落地形态：桥 Lua 常驻 tee + 新 handler（如 `mp_logs {client?, tail?,
  match?}`）按玩家过滤返回；与本地文件型 get_game_logs 互补（文件日志仍用于
  服务端/编辑器日志与全量排查）。

## 7. 停止（实测）

MCP `stop_debug`（菜单「调试/停止」）对多人局完整生效：GamePlayInEditor1/2 全部
unload 无残留（operation_menu.lua:39-56 遍历 GamePlayInEditor/1..4 槽位）。
**无需新停止链路**。

## 8. 现有 MCP 15 工具清单（v2 评审#4 备查）

| 分组 | 工具 |
| --- | --- |
| 本地实现（5） | editor_start / editor_stop / get_game_logs / capture_game / run_scenario |
| 在线透传桥（4） | start_debug / stop_debug / publish_project / get_status |
| Gateway 元工具（6） | search_capabilities / describe_capability / invoke_capability / list_namespaces / get_events / set_suppress |

## 9. 前科纪律（改动红线）

- **无图不碰 DI**：桥 C# 提前触碰 map-scoped DI 会导致 MutiDebugWindow ctor 崩溃
  （test/requirements/debug-muti-debug-crash.md）；本补丁所有新 handler 的初始化
  必须推迟到地图加载后（与现有 init_deferred 同纪律）。
- 桥 Lua 模块加载期**不得** require 'ui.menu_bar'（顶层副作用打乱官方初始化，
  实证导致模拟多人调试崩溃退出）——一律 load_map_done 后经
  `package.loaded['@xdeditor/ui/menu_bar']` 或延迟 require 获取。
