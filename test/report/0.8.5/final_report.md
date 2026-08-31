# 0.8.5 统一 Page 架构（cg.page）—— 终验报告

> 日期：2026-08-31；实施现场：test_res002（框架共建验证场）
> 依据：doc/requirements/0.8.5.md + 0.8.5_dev.md
> 场景脚本：test/0.8.5/case/game_panels085.ps1（50 步全绿）

## 验收总表逐项

| 验收项 | 结论 | 证据 |
| --- | --- | --- |
| 注册/开关/互斥/挂起/排队/页组 | ✅ | T1 探针（probe_a~g）全路径断言：注册即 closed、open 才显示、exclusive 同类型互斥、queue FIFO 补位、group 批量开关 |
| on_init 恰好一次、重注册不重置 | ✅ | 重注册（含可见态重注册）后 init_a=1 不变，on_open 不重放 |
| toggle 无多重触发 / 首帧无闪显 | ✅ | toggle 单路径分发；注册零挂载 |
| backdrop 遮罩可见/吞噬下层点击/关闭恢复 | ✅ | 截图变暗 + tap 被拦（actionable 错）+ 关闭恢复 |
| dbg 对被遮下层报遮挡（occluded_skipped） | ✅ | backdrop 页打开时 find_ui 返回 occluded_skipped=1 |
| exclusive 新语义（商店开→背包关；组队非 exclusive 不受影响） | ✅ | T6.4 共存实测：shop 可见时 team 仍 open+visible |
| 挂起/恢复往返后 exclusive 仍互斥、同档次序不失真 | ✅ | 终验场景：shop 开→HUD 挂起（open 保持、不可见）→关→恢复 |
| BENCH 内建挂起业务档（调试台开→HUD/DIALOG/GUIDE 挂起，MAP/NOTIFY 不动） | ✅ | 终验场景 step 17：bench 开时 hud/dialog/guide 不可见，map/toast 存活 |
| MAP/WORLD 拆分 + 视口 culling | ✅ | T6.7：map_view（瓦片整层）/world_view（实体/血条/飘字/黑幕，IsInView 早退）；渲染次序（瓦片<世界<HUD<NOTIFY）截图确认 |
| id 前缀连锁：场景脚本全量更新跑绿 | ✅ | game_panels085.ps1 50 步全绿 |
| AI 闭环零回归（快照/虚拟指针/state 注入/scope/occluded_skipped） | ✅ | 全程 tap/input_text/press_ui/release_ui/drag_ui/pick/scope/tag 链路使用 |
| find_ui scope（页名前缀真参数）/tag（任一命中）/返回 tags | ✅ | T2 探针逐项验证 + 面板脚本 tag 定位（bag_close/gm_close） |
| 两处日志 errors=0 | ✅ | 各会话新增错误均为 0（日志中 4 条 distinct 历史错误为开发期探针遗留：11:42 raw 控件名带 '/' 实验、16:51 T6.9 中途已修 bug） |

## 任务级提交索引（test_res002 仓库，未推远端）

| commit | 任务 |
| --- | --- |
| a61901b | T1/T1b Page 内核 + backdrop |
| da6f7e3 | T2/T3 新 Widget 库 + tag/scope/raw/状态新实现 |
| 22eec32 | T4 toast/dialog/guide 四概念分离 |
| cb9d5af | T5 调试台 cg.page 重构 |
| 2346657 | T6.0 cg 表统一切换 + 摘除旧挂载点 |
| 102dfd2 / 86bdd46 / 4c52835 / aa97af8 | T6.1~T6.4 背包/商店/GM/组队页 |
| 4f8d23f / a02f502 | T6.5 HUD（摇杆+安全边距+美化）/ T6.6 notify 页 |
| 8d730a6 | T6.7 MAP/WORLD 拆分 |
| 8a2236d | T6.8 游戏 widget + T6.9 装配点清理 |
| 4d34aa5 | T6.9 框架页统一 cg.* 终态 |
| 1581093 | T7 src_template 同步 |
| 1090c6e | T8 删除旧物（-4929 行） |
| c13965f / ad33434 | T10 文档收口（两侧） |

editor-patch 仓库：7ba19f3（场景脚本）、16e7859（find_ui scope/tag 目录配套）。

## 已认可的语义变化（评审拍板，非 bug）

- debug_hub 入口（HUD 档）随 exclusive 全屏页打开被挂起
- 全局对话框页档位由旧 BENCH+100 归位 SYSTEM：调试台打开时系统弹窗不再压过调试台（终验实证：bench 开时 dialog 页挂起）
- 引导蒙层归位 GUIDE 档（旧 BENCH+50 特例废除）：调试台打开时引导随业务档挂起
- confirm_async 点遮罩 = 取消（组队邀请即拒绝；旧版 close_on_mask=false 不关闭）

## 遗留与边界

- 摄像机跟随/键盘移动在 PIE 环境无法经 MCP 注入验证（引擎 key_state 不吃注入/窗口焦点），需人工按 WASD 复核一次（UpdateCamera 公式逐字未改，主循环存活有日志铁证）
- 背包多格拖拽端到端（drag_ui 落格）建议后续补一条专项场景
- 旧商店贴图资源（src/res/image/shop/ 10 张）已无引用未删除（资源清理不在本期范围）
- types/bgd_cgui.d.lua 已按新 API 重写；后续组件增删需同步
