# cgui 分层体系与遮挡语义（0.8.5 终验结论）

> 本文是 0.8.5 分层重构的机制定稿与实测结论。需求与问题清单见
> [../requirements/0.8.5.md](../requirements/0.8.5.md)；真机验证记录见
> [../../test/0.8.x-review/report.md](../../test/0.8.x-review/report.md)。

## 起因（实测现场）

2026-08-30 v0.8.x 审查真机验证：调试台（cgui_bench，90000 层全屏外壳）打开时，
下层 ShopUI/GMUI/HUD 仍在渲染并沉淀 dbg 快照，AI 经 MCP 点技能/买礼包全部"成功"——
「AI 所见 ≠ 玩家所见」，测试失真。根因两分：cgui 全屏外壳无遮挡释放语义；
dbg 快照无可见性概念。

## 机制定稿

### 1. 三套正交状态（panel）

- `open`：逻辑打开意图；`suspended`：被高层/外壳跨层挂起（保持 open 不渲染）；
- `queued`：队列层排队中（等前者关闭补位）。
- 早期版本 `suspended` 一字段两用导致 restore 把排队面板提前挂出——**分字段是硬约束**。
- 查询语义三分：`is_open`（含挂起/排队）/ `is_visible`（正在显示）/ `is_queued`。

### 2. 挂起双通道

- panel 注册面板：`suspended=true + mount.unmount`（恢复时重挂载，保留 open_seq 微增量，
  否则同层"新开的在上面"与 back 导航次序失真）；
- 普通 mount 视图（HUD 常驻件）：`mount.set_hidden`（保持注册跳过渲染，begin/end 成对
  跳过无失配；perf 残值同步清除）。恢复收敛为 `recompute_hidden_views` 单入口，
  在 restore_suspended 与 suspender 注销两处都调（防孤儿路径）。

### 3. 层带匹配（实测踩坑）

项目面板普遍用带内子层（POPUP+100/200），挂起声明用 LAYER 带起点——**匹配必须
`set[layer] or set[band_of(layer)]` 双写**，否则 bench 期间新开的子层面板漏挂
（0.8.5 初审修复在此栽过一次：已开面板靠 mount 隐藏兜住侥幸正确，新开面板全漏）。

### 4. exclusive 框架级自动挂起

`exclusive=true` 且未显式声明 suspend_layers 时默认挂起 `{UI}`（自身带 > UI 带生效），
`suspend_layers=false` opt-out。挂起态下 exclusive 仍互斥（held 分支先关同层其它
open 面板含被挂起的），否则恢复后同层 exclusive 同屏。

### 5. 遮挡不变量（dbg）

- 挂起体系是主机制；`occlusion()` 兜底：可见 exclusive 全屏面板层带之下的全部
  视图 = 遮挡，`hit_test` 跳过、`find_ui` 过滤并回显 `occluded_skipped`。
- **AI 所见 = 玩家所见**成为快照内建不变量。GAME 世界层与 NOTIFY 瞬态反馈不挂起。
- dbg 不能反向 require panel/mount（循环依赖）——遮挡判定经 `occlusion_provider`
  注入（dbg_commands 在 find_ui 时设置）。

### 6. 目录模块化

- 游戏 UI 一律 `ui/<层名>/<模块>.lua`（world/hud/popup/dialog/system/notify/guide/bench
  ↔ LAYER，`panel.LAYER_NAME` 唯一来源）；register/mount 不传层时
  `debug.getinfo(2).source` 路径推导；不一致告警。mount 经 resolver 钩子注入
  （不能反向 require panel）。
- **混层模块必须拆**：TeamHUD（HUD 靠近按钮 + POPUP 队伍面板同文件）拆为
  `ui/hud/TeamHudView.lua` + `ui/popup/TeamHUD.lua`——单文件单层是目录约定成立的
  前提；PlayerLife 的 blackout mount order 归 GAME+500（order 只影响同 z 绘制序，
  跨体系遮挡由 root_z 决定，行为不变）。

### 7. z 值全局校验

`core.check_z`：合法 = 世界 <10000 / 带起点 X0000 / 带内子层 +1~+999 / 浮层 100000+；
死带（10000~19999）与带内偏移 >999 warn_once。mount root_z 同检。

## 实测结论（2026-08-31 真机）

- 商店打开 → HUD 背包按钮消失（挂起）；find 技能书 → `occluded_skipped=853`（世界+HUD 全滤）；
- bench 中 `panel.open('BagUI')` → `open=true/suspended=true` 立即挂起；bench 关 →
  BagUI 恢复渲染，ShopUI 被 exclusive 关闭不同屏；
- TeamHUD（POPUP+200）与商店（POPUP+100）共存：不被 exclusive 关闭、不被遮挡过滤；
- imgui_bench 接入同一挂起体系（eval 确定性 open/close 验证）；
- hub 入口是 toggle 状态盲点——场景脚本开关调试台一律 `eval cgui_bench.open()/close()`。

## 已知边界（本期不做，理由见 0.8.5.md「不做」节）

- 部分遮挡（非全屏面板压 HUD 局部）不做逐像素判定；remember 600 帧回收 vs 长时挂起
  的瞬态槽位丢失；imgui_bench 本体不迁 cgui（接入挂起体系即消除旁路）。
