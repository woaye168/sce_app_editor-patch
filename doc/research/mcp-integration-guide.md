# bgd_mcp_bridge —— 外部 AI 工具接入指南

> 适用版本：sce_app_editor-patch ≥ 0.4.3
> 服务形态：星火编辑器进程内 HTTP 服务，固定地址 `http://127.0.0.1:39177`（**固定端口，不自动跳变**）

启用条件：编辑器补丁应用中勾选「MCP 桥（外部 AI 控制）」→ 重启星火编辑器。服务随编辑器进程存亡（编辑器关闭则服务下线）。

## 两种接入方式

### 方式一：MCP（Streamable HTTP）——推荐给 AI 工具

端点：`http://127.0.0.1:39177/mcp`（单端点，POST）

支持 MCP 握手（initialize / notifications/initialized / ping）与 tools/list、tools/call。工具分两类：

- **固定工具**：`call_command` / `list_commands` / `get_status` / `start_debug` / `restart_last_debug` / `stop_debug` / `set_suppress` / `get_events`
- **动态工具**：编辑器全部已注册菜单命令（80+，如「文件/保存」「调试/调试」）各暴露为一个 `cmd_...` 工具，`description` 是原始中文命令名

各工具接入示例：

**Cursor / Claude Desktop / 其他支持 MCP URL 的客户端**——在 MCP 配置中加入：

```json
{
  "mcpServers": {
    "sce-editor": {
      "url": "http://127.0.0.1:39177/mcp"
    }
  }
}
```

**Trae / 其他仅支持 stdio 的客户端**：本服务是 HTTP 传输。若客户端只支持 stdio，需一个 stdio↔HTTP 转发壳（后续版本可选提供）；优先用支持 URL 直连的客户端。

### 方式二：HTTP JSON-RPC——脚本/任意语言直接调

端点：`POST http://127.0.0.1:39177/rpc`，body `{"id":1,"method":"<方法>","params":{...}}`。

```bash
# 编辑器状态
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":1,\"method\":\"get_status\"}"

# 启动调试（地图需已打开，否则返回友好错误）
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":2,\"method\":\"start_debug\"}"

# 停止调试
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":3,\"method\":\"stop_debug\"}"

# 任意菜单命令
curl -X POST http://127.0.0.1:39177/rpc -H "Content-Type: application/json" \
  -d "{\"id\":4,\"method\":\"call_command\",\"params\":{\"name\":\"文件/保存\"}}"
```

事件拉取：`GET http://127.0.0.1:39177/events?since=<seq>` 增量返回编辑器事件（地图加载、调试启动、弹窗抑制等）。

## 端口占用说明（Q4-2 答复）

服务使用**固定端口 39177，刻意不做自动跳变**——因为 AI 工具/编辑器是按 URL 静态配置的，端口一变所有已配置客户端全部失效。

- 端口被占用时：服务**不启动**并在 `logs/bgd_csharp/bgd_csharp-*.log` 记录明确错误（含占用提示），`server_info` 不可用。
- 排查：Windows 下 `netstat -ano | findstr 39177` 找到占用 PID，`tasklist | findstr <PID>` 确认进程，关闭后重启编辑器即可。
- 保证可用性的做法：39177 是较少被占用的端口；只要没有别的程序长期占用它，配置一次即可永久使用。若你的环境确实被占，联系我们把固定端口换成别的固定值（重新编译）。

## 排错

- C# 侧日志：`<引擎运行根>/logs/bgd_csharp/bgd_csharp-YYYY-MM-DD-HH.log`（按小时滚动）
- 端口文件：`<引擎运行根>/logs/bgd_csharp/port`（内容为 `39177`）
- Lua 侧日志：编辑器主日志中 `[bgd_mcp_bridge]` 前缀行
- 连接失败即视为「编辑器不在线」（服务随编辑器进程存亡）
