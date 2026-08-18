# bgd_mcp_bridge —— 外部 AI 工具接入指南

> 适用版本：sce_app_editor-patch ≥ 0.5.0（海量工具 MCP 架构：Gateway + 能力目录 + 元工具搜索）
> 服务形态：星火编辑器进程内 HTTP 服务，默认地址 `http://127.0.0.1:39177`（配置端口不可用时自动向后避让，实际端口以 `<引擎运行根>/logs/bgd_csharp/port` 文件为准）

启用条件：编辑器补丁应用中勾选「MCP 桥（外部 AI 控制）」（0.5.3 起默认勾选）→ 重启星火编辑器。服务随编辑器进程存亡（编辑器关闭则服务下线）。

## 0.5.3 起：AGENT 首选 bgd_sce_tools mcp 聚合入口

场景一（代码开发调试链路）下，AI 客户端**只需配置一个 stdio MCP**：

```json
{ "mcpServers": { "bgd-sce": { "command": "bgd_sce_tools", "args": ["mcp"] } } }
```

聚合服务恒定 8 工具：`editor_start` / `editor_stop` / `get_logs`（编辑器外本地实现，离线可用）+
`start_debug`（默认 restart_last_debug，失败自动回退全量）/ `stop_debug` / `publish_project` / `capture_game` / `get_status`（在线透传本桥，离线时明确报错引导 editor_start）。
`project_path` 参数缺省取 bgd_sce_tools 最近项目。本指南下文为编辑器进程内桥的直接接入方式（场景二/自定义集成用）。

## 架构：固定元工具 + 可搜索能力目录

0.5.0 起**放弃**「每个能力 = 一个 MCP tool」模型。tools/list **恒定只暴露 10 个固定元工具**；全部能力（菜单命令、DI 服务方法、CppInterface 静态方法、数编读写等数百条）进入可搜索的**能力目录**，能力 ID 即原文（`cmd.调试/调试`、`svc.FileSystem.ScanDir`，中文/斜杠合法）。

AI 调用链路（压低往返轮次）：

1. `search_capabilities` —— 关键词搜索，直接返回**简化签名 + 一句话描述 + 风险级别**；
2. `invoke_capability` —— 按签名直接调用。**参数校验失败时错误响应内嵌 compact schema**（自愈反馈），下一轮修正参数重试即可，无需显式 describe；
3. `describe_capability` —— 仅疑难时深查完整定义（JSON Schema/示例/前置条件）。

能力命名空间（`list_namespaces` 逐层下钻）：

| 前缀 | 通道 |
| --- | --- |
| `svc.*` | DI 单例服务反射调用（主力通道；**服务级准入制**，未准入服务不出现在搜索结果） |
| `datacore.*` | 手写数编读写高层封装（IDataCore，map-scoped） |
| `cpp.*` | SCE.CppInterface 静态基元方法（次要通道） |
| `cmd.*` | 菜单命令（`cmd.调试/调试` 等，运行时从 Lua 侧注入） |
| `lua.*` | Lua 桥 method（get_status/set_suppress/run_lua 等） |
| `sys.*` | 服务自身（server_info 等） |

## 两种接入方式

### 方式一：MCP（Streamable HTTP）——推荐给 AI 工具

端点：`http://127.0.0.1:39177/mcp`（单端点，POST）

支持 MCP 握手（initialize / notifications/initialized / ping）与 tools/list、tools/call。工具集恒定：

`search_capabilities` / `describe_capability` / `invoke_capability` / `list_namespaces` / `get_status` / `start_debug` / `restart_last_debug` / `stop_debug` / `set_suppress` / `get_events`

客户端配置（Cursor / Claude Desktop / 其他支持 MCP URL 的客户端）：

```json
{
  "mcpServers": {
    "sce-editor": {
      "url": "http://127.0.0.1:39177/mcp"
    }
  }
}
```

**Trae / 其他仅支持 stdio 的客户端**：本服务是 HTTP 传输，需 stdio↔HTTP 转发壳；优先用支持 URL 直连的客户端。

### 方式二：HTTP JSON-RPC——脚本/任意语言直接调

端点：`POST http://127.0.0.1:39177/rpc`，body `{"id":1,"method":"<方法>","params":{...}}`。
method 与元工具同名（`search_capabilities`/`describe_capability`/`invoke_capability`/`list_namespaces`/`get_status`/`start_debug` 等）。**0.4.x 的 `call_command`/`list_commands`/`list_tool_categories`/`list_category_tools` 已移除**（无兼容层）。

```bash
# 搜索能力（返回简化签名+风险级别+total_hits）
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":1,\"method\":\"search_capabilities\",\"params\":{\"query\":\"文件存在\"}}"

# 调用能力：svc 通道（DI 服务）
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":2,\"method\":\"invoke_capability\",\"params\":{\"id\":\"svc.FileSystem.FileExists\",\"args\":{\"fileName\":\"D:/tmp/a.txt\"}}}"

# 调用能力：菜单命令（能力 ID 即原文，中文/斜杠直接传）
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":3,\"method\":\"invoke_capability\",\"params\":{\"id\":\"cmd.文件/保存\"}}"

# 数编批量写（全部暂存后一次提交，不引发界面反复刷新）
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":4,\"method\":\"invoke_capability\",\"params\":{\"id\":\"datacore.batch_write\",\"args\":{\"changes\":[{\"link\":\"$$.map_config.dflt.root\",\"path\":[\"Game\",\"xxx\"],\"value\":123}]}}}"

# 启动调试（高频操作直暴露快捷路径，无需走目录）
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":5,\"method\":\"start_debug\"}"
```

事件拉取：`GET http://127.0.0.1:39177/events?since=<seq>` 增量返回编辑器事件（地图加载、调试启动、弹窗抑制、能力调用失败、danger 拒绝、目录版本漂移等审计事件）。

## 安全分级

| 级别 | 判定 | 策略 |
| --- | --- | --- |
| read | 人工标注白名单 / 高置信纯查询前缀（Is*/Get*/Query*/Exists* 等） | 默认开放 |
| write | **未知一律 write** | 默认开放，describe 明示 |
| danger | Exit/Delete/SystemCommand/Publish/Upload 关键词 + 人工黑名单 + **svc 未准入服务一律视同** | 默认拒绝，需配置放行 |

放行方式：`<引擎运行根>/logs/bgd_csharp/config.json`（与 mcp_port 同文件）加 `danger_allow` 数组，元素为精确能力 id 或 `前缀*`：

```json
{ "mcp_port": 39177, "danger_allow": ["lua.run_lua", "svc.FileSystem.Delete*"] }
```

审计：所有 write/danger 调用的「能力 id + 入参 + 耗时 + 结果」异步写 `<引擎运行根>/logs/bgd_csharp/audit-YYYY-MM-DD.log`；danger 拒绝同时推 `/events`（`danger_denied`）。

## 版本漂移

catalog.json 构建期生成并嵌入 dll（头记录引擎版本）。编辑器升级后：启动检测到版本不符、或条目运行期惰性校验失败时，推 `/events` 事件 `catalog_version_mismatch`，且 search 结果头部固定标注「目录版本漂移」。处置：重跑生成工具（`dotnet run --project csharp/make_catalog`）→ 重新构建 dll → 重新应用补丁。

## 排错

- C# 侧日志：`<引擎运行根>/logs/bgd_csharp/bgd_csharp-YYYY-MM-DD-HH.log`（按小时滚动）
- 审计日志：`<引擎运行根>/logs/bgd_csharp/audit-YYYY-MM-DD.log`
- 端口文件：`<引擎运行根>/logs/bgd_csharp/port`（内容为实际监听端口，如 `39177`）
- Lua 侧日志：编辑器主日志中 `[bgd_mcp_bridge]` 前缀行
- 连接失败即视为「编辑器不在线」（服务随编辑器进程存亡）
- 端口不可用时服务**自动向后避让**（最多探测 100 个候选，跳过系统保留段），实际端口见端口文件与 WARN 日志；客户端以端口文件为准
- 「端口被占用但 netstat 查不到进程、换临近端口也失败」= 端口落在**系统保留端口段**（Hyper-V/WSL/winnat 动态保留）内：`netsh int ipv4 show excludedportrange tcp` 查看保留段，把 mcp_port 配置到保留段之外；真被进程占用时 `netstat -ano | findstr <端口>` 找占用 PID
