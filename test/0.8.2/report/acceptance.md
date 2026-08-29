# 0.8.2 验收报告（2026-08-29）

## 范围

需求文档 `doc/requirements/0.8.2.md`：R0 研究（Q1~Q4）→ R2.1 指针收敛 → R2 虚拟指针
→ R1 删 dbg_register + 存量命令迁移 → R3 场景脚本进化 → 存量界面逐面验收。

## 结果：全绿

| 验收 | 用例 | 结果 |
| --- | --- | --- |
| R1 零注册回归 | case/bench_sweep082.ps1（run_scenario 71 步） | 全绿，日志 errors=0 |
| R2 vp 逐命令 | 真机单测（hover tooltip / drag 排序 / scroll / press / set_value / long_press / input / key） | 全过，证据见 doc/research/virtual-pointer.md「实现终验」 |
| R3 场景语法 | case/r3_probe.ndjson + 两个 case 脚本自身 | save_as/{$名}/wait_for/assert_text 全过（含 cargo 单测 25 绿） |
| 存量界面逐面 | case/game_panels082.ps1（34 步） | 商店/背包/HUD 覆盖件/调试台全绿；GM 为 base.ui 旧体系（R4 边界，find 只定位），errors=0 |

## 实测修正（相对定稿方案）

1. 命中测试：容器 finish 晚于子控件会截胡 → static 不标 interactive + 祖先剔除 + order 倒序。
2. 拖放落点：命中子件挡 drop → 上溯最近 enable_drop 祖先（core 记录 frame.drop）。
3. 潜伏 bug 修复：button_impl 的 st 块级作用域导致 on_long_press 从不触发 → st 提升，
   vp 长按实测触发。slider 点击跳值后松手补 on_commit（真实与注入同路径）。
4. bench 套件两处 dummy state 控件（kit_vol/kit_quality）会让「操作生效」断言误判——
   验收断言改用真实状态控件（kit_segs_sl / bag / shop tab）。

## 环境备忘（下次迭代直接可用）

- Trae MCP（mcp_bgd-sce）断连后可全程走桥 HTTP：POST `http://127.0.0.1:39177/rpc`
  `{id:1, method:'invoke_capability', params:{id, args, timeout_ms}}`；start_debug 走
  `{method:'start_debug', params:{project_path}}`；exe CLI `editor start|stop` 同样可用。
- 部署链（沙箱禁写编辑器目录，必须走应用自部署）：cargo build --release →
  复制为 target/deploy-tmp/editor-patch.exe → 先退旧实例（--quit / 兜底 Stop-Process）→
  带 `--project-path <项目> --background` 启动 → `notify project_path=...` 触发 refresh
  部署（dll + lua 补丁）→ `editor start` 重启编辑器。
- ps1 中文必须 UTF-8 BOM + `$OutputEncoding = UTF8`（管道进 exe stdin 的中文不这样就乱码）；
  用编辑工具改 ps1 后 BOM 会丢，跑之前补：`[IO.File]::WriteAllBytes($p, [byte[]](0xEF,0xBB,0xBF)+[IO.File]::ReadAllBytes($p))`。
- 同文件多处编辑不要并行下多个替换调用（实测丢编辑），改完立即 grep 审计关键点。
