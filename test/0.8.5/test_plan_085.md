# 0.8.5 全量测试计划（统一 Page 架构验收）

> 版本：v1（2026-09-01）。目标：对 0.8.5「cgui 调试台重写 + src 全部界面重写」做
> **全量真实交互验证**——每个界面、每个功能按钮、每种 UI 交互手势都经 MCP 调试闭环
> 真实操作并取证断言。本计划只找问题、不修复。
> 审查基线：test_res002 `48cf008..c13965f` + sce_app_editor-patch `v0.8.4..v0.8.5`。

## 1. 环境与前置

| 项 | 值 |
| --- | --- |
| 编辑器 | 星火编辑器在线（已打 sce_app_editor-patch 补丁，bgd_mcp_bridge 在线） |
| 项目 | `c:\Users\woaye\Documents\SCE Projects\test_res002`（已 build 最新 .bgd） |
| MCP exe | `D:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe` |
| 执行方式 | `powershell -File <用例.ps1>`（编辑器在线即可，脚本自带 start_debug 重置） |
| 一键全量 | `powershell -File test\0.8.5\run_all_085.ps1` |

通过标准（逐用例）：**全部步骤 ok（无 failed_step）+ 末步 logs errors distinct = 0**。
探针式复核用例（10）以输出 `ISSUE-n REPRODUCED / NOT-REPRODUCED` 标记为判定。

## 2. 用例清单

| # | 脚本 | 覆盖对象 | 规模 |
| --- | --- | --- | --- |
| 01 | case/01_bag_full.ps1 | 背包页全交互（开合 4 路径/底栏 3 钮/属性框类型分支/拖拽四分支+预览/双击旋转/长按拆分/exclusive/suspend/backdrop/状态复位） | ~155 步 |
| 02 | case/02_shop_full.ps1 | 商店页（四页签切换/免费领取/付费购买/限购/售罄态/一键购买/货币不足/红点/钱包刷新/倒计时） | ~60 步 |
| 03 | case/03_gm_full.ps1 | GM 面板（表单三控件/空输入/非法数值/成功发放金币+钻石/不在线/表单重置） | — |
| 04 | case/04_team_full.ps1 | 组队页单端可测（创建/重复创建/标题状态机/退出/解散/非 exclusive 共存/非法邀请静默） | — |
| 05 | case/05_hud_combat.ps1 | hud_bar 六钮（音效/BGM 开关翻转+持久化、四入口 toggle 与互斥）+ combat（属性卡片/摇杆按住与复位/键盘移动/攻击/16 技能槽施放+CD+拖拽换位+持久化/药水三态/拾取） | ~158 步 |
| 06 | case/06_notify_framework.ps1 | notify 页全部提示种类 + 框架页 toast（合并/限流/时长/warn）/dialog（确定/取消/遮罩/队列/栈/句柄 close）/guide（高亮洞/整屏/点遮罩回调/guide_hide）+ BENCH 共存语义 | ~141 步 |
| 07 | case/07_bench_cgui.ps1 | cgui 调试台 8 标签页逐项（内置组件/扩展组件(Widget 注册表)/游戏件~20 组件/Tileset/样式编辑/布局演练/动画演练/诊断） | ~130 步 |
| 08 | case/08_bench_imgui.ps1 | imgui 排障台 15 页逐页切页断言 + 交互抽测（raw 不进快照属预期，断言走 eval M.page + capture） | — |
| 09 | case/09_page_semantics.ps1 | Page 内核语义 S1~S15（注册即 closed/on_init 恰好一次/重注册不重置/exclusive 新语义/suspend 默认/backdrop 遮挡/queue 补位/back 导航/group/close_all/事件/toggle 无多重/BENCH 挂起/MAP·WORLD culling） | ~85 步 |
| 10 | case/10_review_issues.ps1 | 静态审查 20 条问题动态复核（#1/#2/#3/#6/#7/#9 探针复现；其余静态已证实） | — |
| — | case/game_panels085.ps1 | （既有 0.8.5 归档用例，保留作对照） | 50 步 |

执行顺序：01→10 任意序（每个脚本独立 start_debug 互不影响）；run_all 按编号序。

## 3. 覆盖矩阵（界面 × 交互类型）

| 界面 | 点击 | 输入 | 拖拽 | 长按 | 滚动 | 键盘 | hover | 开关/切换 | 弹窗 | 断言通道 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| bag 背包 | ✓ | — | ✓ 移动/合并/锻造/交换 | ✓ 拆分 | — | Y | — | ✓ 批量丢弃 | ✓ 属性框 | 协议日志/Sync_BagData/find_ui |
| shop 商店 | ✓ | — | — | — | — | U | — | ✓ 四页签 | — | 钱包数值/toast/服务端日志 |
| gm | ✓ | ✓ uid/数量 | — | — | — | — | — | ✓ 货币单选 | — | toast/日志/负断言 |
| team 组队 | ✓ | — | — | — | ✓ 邀请列表 | — | — | ✓ 状态机 | ✓ 邀请弹窗(受限) | 标题/成员行/toast |
| hud_bar | ✓ 六钮 | — | — | — | — | — | — | ✓ 音效/BGM | — | 文本翻转/eval 开关态 |
| hud_combat | ✓ 攻击/技能/药水/拾取 | — | ✓ 摇杆/技能换位 | — | — | WASD/Y/U | — | ✓ CD 遮罩 | — | WorldState eval/日志/capture |
| notify | — | — | — | — | — | — | — | — | ✓ 警告/toast/BUFF 条 | assert_text/eval W 字段 |
| toast/dialog/guide | ✓ | — | — | — | — | — | — | — | ✓ 队列/栈/遮罩 | eval 标志位/find_ui/capture |
| cgui_bench ×8 页 | ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ | ✓ 全部 | ✓ | find_ui/assert_text/日志 |
| imgui_bench ×15 页 | ✓ click_at | ✓ | ✓(受限) | — | ✓ | — | — | ✓ | — | eval M.page/capture |

## 4. 受限项（已识别，报告单列）

1. **双人组队流程**（邀请/接受/拒绝/合并/踢出/邀请 TTL 失效）：单 PIE 实例无双端，04 仅覆盖单端路径。
2. **双击/右键注入**：vp 无双击与右键能力——背包双击旋转走协议层验证；imgui 核心API 右键计数不测。
3. **imgui_bench 内容**（cg.raw）不进 dbg 快照：find_ui 找不到属预期，断言降级为 eval 页索引 + capture。
4. **游戏件页引擎 scroll**：vlist 滚动走 vp 拖拽，滚到底不做硬断言。
5. **付费购买按钮定位**：同 tag 多行无唯一文本，走 eval 直发协议；UI 点击覆盖由免费礼包承担。
6. 倒计时归零（8h 周期）不实等，仅断言倒计时文本渲染。

## 5. 与静态审查问题清单的对照

| 审查问题 | 复核方式 |
| --- | --- |
| #1 队列×挂起卡死 / #2 钩子失衡 / #3 close_all 脉冲 | 10 号用例探针复现 |
| #6 slider on_commit 失效 / #7 countdown 槽位共享 / #9 pscroll 高度覆盖 | 10 号用例探针复现 |
| #4 重注册 suspend 不重评估 | 09 号 S3 顺带覆盖（重注册语义） |
| #10 pscroll 句柄稳定性 | 静态已证实（每帧新建表），07 pscroll 功能回归顺带 |
| #5/#14/#16/#17/#18/#19 注释与死代码残留 | 静态已证实（grep 零调用方），无需动态 |
| #11/#12 team 边距/编排、#13 行数纪律 | 静态已证实 |
| #15 imgui_bench debug_ui 闸门 | 08 号用例开台路径顺带观察 |
| #20 验收证据链缺口 | 本轮全量用例归档即补闭环 |
| #8 grid 异常路径 | 异常注入无通道，静态已证实 |

## 6. 报告产出

执行完毕后汇总：`test/report/0.8.5/full_validation_report.md`——逐用例结果
（ok/failed_step/errors distinct）+ 新发现问题清单（复现步骤/证据截图或日志/严重度）
+ 受限项清单 + 与静态审查报告的合并结论。
