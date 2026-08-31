# v0.8.x 实现达标复核 + Code Review + 全量真机验证报告（2026-08-30/31）

> 范围：sce_app_editor-patch v0.8.0~v0.8.3（含 0.8.1 留言）实现达标复核、代码 review、
> 修复与全量真机验证；延伸至 0.8.5 分层体系重构（用户拍板未来方向提前落地）。
> 验证环境：星火编辑器 api13 + test_res002 真机（单线程，编辑器不可多开）。

## 一、达标复核结论

| 版本 | 结果 |
| --- | --- |
| 0.8.0（护栏/引导/find_ui/交互/run_scenario/catalog 瘦身/danger 放行/拆分） | 11/11 达标（截断/crop/match+errors 上浮+×N 收纳/events 切片/hint 全有单测守护） |
| 0.8.1（8 条留言） | 8/8 兑现 |
| 0.8.2（删注册/虚拟指针/场景进化） | 13/13 达标（dbg_register 零残留；catalog 255 条） |
| 0.8.3 A（工具防呆） | 7/7 达标（capture hint 以更优路径：q 复合截图内化进 capture_game） |
| 0.8.3 B/C/D | 文档状态滞后已修正：B 方向反转定稿（sprites 唯一路径）、D 已完成（UIMgr 删除）、E 由本次全量验证覆盖 |

## 二、Code Review 问题与修复（12+2 项，双人交叉验证 12/12 确认）

| # | 问题 | 修复 | 真机验证 |
| --- | --- | --- | --- |
| 1 | vp 一次性序列跑完 D.vp 残留，真实鼠标被永久遮蔽 + hover 卡死 | 序列耗尽自动复位（补抬起/leave 沿；hover/press 保持态除外） | eval 验证 vp_is_nil ✓ |
| 2 | wait_for 桥错误直接传播，PIE 重启窗口假性失败 | 轮询体内桥错误视为未命中，继续等到 timeout | stop_debug 后轮满 3s 聚合报错 ✓ |
| 3 | expect 同时匹配 id/text 假阳性 | 命中方式区分（verified_via=text/id + 弱断言 note） | verified_via 两种取值实测 ✓ |
| 4 | max_width=0 静默绕过降采样护栏 | 显式 0 拒绝（actionable）+ 单测 | MCP 报错 ✓ |
| 5 | 桥 eval 仍是白名单（与全参透传纪律矛盾） | 改 passthrough | eval 全链路 ✓ |
| 6 | vp 按住中直接发新命令状态错乱 | settle_down 入口守卫 | press/release 全周期 ✓ |
| 7 | 变量值含 {$ 被二次展开 | 替换后从插入点继续扫 + 单测 | 单测 ✓ |
| 8 | 注入静默丢弃无反馈 | D.dropped 追踪 + drag_ui 延迟回读警告 | frames 正常无丢弃 ✓ |
| 9 | danger 陈旧文案（Gateway.cs/Program.cs→catalog.json） | 修文案 + 重跑 make_catalog + 重编 dll 部署 | catalog 255 条 ✓ |
| 10 | capture q 多命中静默取首条 | candidates/note 回显 | q=txt candidates=13 ✓ |
| 11 | FileSystem 剔除注释与 cpp.* 扫描行为不符 | 注释补说明（有意保留只读 getter） | — |
| 12 | UIMgr 陈旧注释 + 0.8.3.md 状态滞后 | 清理 + 状态更新 | — |
| 13 | （验证中新发现）drag_ui frames=0：vp_run 表别名被逐帧 remove 消费 | total_steps 先存 + vp_run 内部复制 | frames=5 ✓ 排序生效 ✓ |
| 14 | （验证中新发现）vp 命令对 rect=0x0 被裁剪控件无防护 | resolve_interactive 拒绝 + actionable | 代码核验（当帧无可复现裁剪件） |

## 三、0.8.5 分层体系重构（用户拍板：未来方向一口气落地，不做兼容）

起因：真机验证时调试台开着，AI 在被全覆盖的界面上点技能/买礼包全部"成功"——测试失真。

### 实施（全部落地并真机验证）

| 项 | 内容 | 验证 |
| --- | --- | --- |
| 挂起体系 | set_external_suspender + mount.set_hidden + suspend_layers_now 双通道 + layer_held 层带/精确双写 + queued/suspended 分字段 + exclusive 挂起态互斥 + open_seq 恢复 + is_queued 三分 | bench 开→商店/HUD 消失；新开 BagUI 立即挂起；关 bench→BagUI 恢复 ShopUI 不同屏 ✓ |
| exclusive 自动挂起 | 框架默认挂 {UI}（opt-out=false）；ShopUI/BagUI/GMUI 落地 | 商店开→HUD 消失 ✓ |
| imgui_bench 收口 | 接入挂起体系 + Z.BENCH 注册表 + cg_overlay require 修复 | eval open/close 挂起恢复 ✓ |
| dbg 遮挡不变量 | occlusion_provider + hit_test 跳过 + find_ui 过滤 + occluded_skipped 回显 | occluded_skipped=853 实测 ✓ |
| z 全局校验 | core.check_z（死带/越带 warn_once）+ mount root_z 同检 | 启动零越带告警 ✓ |
| 目录模块化 | ui/<层名>/<模块> 全量迁移（18 文件 + require 改写 17 文件）；TeamHUD 拆层（TeamHudView 拆出）；src_template 同步；路径推导层 + 一致性告警（抓到 blackout/TeamHUD 两处真实越带并修正） | 启动零不一致告警 ✓ |

### 文档

- 0.8.5.md（全量问题清单 P0/P1/P2 + 方案 + 「不做」评估）/ doc/research/layering.md（机制定稿）
- cgui.md §2 层模型重写（目录约定/挂起/遮挡/校验）；cgui_mcp.md 0.8.5 增量（遮挡语义 + 调试台确定性开关）
- test_res002 AGENTS.md 模块速查路径 + UI 目录约定（第 6 条硬性约定）；src_template README 与结构
- 0.8.3.md 状态修正（B 方向反转 / D 完成 / E 覆盖）

## 四、全量真机验证矩阵（test_res002，两处日志 errors=0）

- MCP 工具链：status/search（别名命中）/describe/namespaces/events/logs（match+errors+收纳）/capture（crop/max_width/downscaled/candidates）/run_scenario（变量/wait_for/assert/容错）
- UI 闭环：find/tap/pick/click/input_text/press/release/long_press/set_value(actual 回读)/hover/drag（sortable 重排）/scroll（pscroll+引擎容器 actionable）/key_down/up/eval
- 游戏面板：商店（页签/购买/钱包/限购）、背包（整理/格子/开关）、GM（发放+服务端日志）、组队（创建/队伍面板与商店共存）、技能按钮、音效/BGM 开关
- 调试台：hub、cgui bench 全 8 页、imgui bench 全屏
- 分层：挂起/恢复/exclusive 互斥/遮挡过滤/挂起期新开面板/目录推导零告警

## 五、结案复核记录（三轮子代理独立复核）

- **一轮**：12 项新问题（含 1 项黑屏级：imgui_bench 关闭按钮绕过 M.close 致业务 UI 永久挂起；
  调试台开/关按钮绕过统一路径 ×3、遮挡 provider 注入缺口、toast 被挂起、千级注释残留、
  anim 死带、大写 UI 目录、u32 截断、list_views hidden、文档漂移、occluded_skipped 语义）。
  全部修复并真机回归（toast 挂起期可见 / 遮挡守卫 actionable / anim 页零告警 / cargo test 27 绿）。
  注：一轮代理所述「用户 23:36 四点反馈 2.1/2.3/2.4」（init.lua 入口约定/widget 平级/
  调试台挪目录）经二轮核查**无出处，属误判**，本会话无此用户消息。
- **二轮**：10/12 完全落实；遗留 4 项低危（pick 遮挡漏滤、resolve 后缀分支漏拒、
  page_diag 排队态显示、cgui changelog 缺失+旧路径残留）+ 1 项提示（debug_hub order/root_z
  跨带设计权衡，保留）。
- **三轮**：4 项全部修复确认，无新问题，**结论：可以结案**。
  已知弱一致（不阻塞）：find_ui 零命中建议名单不过滤遮挡（真操作时 resolve 会明确拒绝）。

## 六、遗留（已知边界，见 0.8.5.md「不做」节）

部分遮挡逐像素判定、remember 回收 vs 长挂起、imgui_bench 本体迁移、F14 真机复现待裁剪件场景、
find_ui 建议名单遮挡弱一致、debug_hub order/root_z 跨带（设计权衡）。
