本文档是一些未来需求的零散记录，无需关注。

1、
新的版本怎么尝试也可以更新
○ 未应用 xdeditor（编辑器界面） [v169] （无此版本插槽文件，将跳过）
检测到部分库未应用：可能被编辑器升级覆盖，点击「应用补丁」重新应用即可。

2、
"bgd_mcp_bridge.dll": {
            "assemblyVersion": "1.0.0.0",
            "fileVersion": "1.0.0.0"

3、日志中文件路径缺失
[2026-08-16 03:14:44.106][42260][info][135][xdeditor/ini/object_tree/event_manager.lua:184] register_manager_get	nil
[2026-08-16 03:14:44.365][42260][info][135][[C]:-1] [bgd_mcp_bridge] 已注册事件桥 bgd_mcp_cmd


4、README.md  MCP CURL命令罗列


5、实际上需要120+更多，因为项目如果可视化部分越多，这个时间越久
桥调用客户端超时太短（10s），start_debug 需要 120s+。修复 bridge_rpc 加超时参数。