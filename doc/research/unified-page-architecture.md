# 统一 Page 架构（0.8.5）实战沉淀

> 2026-08-31 · test_res002 实施全录的经验提炼。规范本身见
> doc/requirements/0.8.5_dev.md；本文档只收**实施中实测发现**的、规范里没有的知识。

## 1. 双轨切换纪律的实战形态（大改不中断的方法论）

0.8.5 全量重写 UI 框架而游戏始终可用，靠的不是兼容层而是这条纪律：

1. **cg 表上的一切签名变更归属同一个切换点**——新旧签名无法可靠重载共存
  （remember 的 initial 与 key 撞型；col/row/box 新旧同名不同义），cg 表只能绑一套。
  新库先建（libs/client/widget/ 与旧 cgui/widget/ 并存），框架自身 Page 先用
  模块级 require 直调新库真机验证（仪器先绿），再选批首一次性切 cg 表。
2. **批首切换 + 批内中间态可坏**：T6.0 切 cg 表后游戏旧 UI 暂时全坏是预期
  中间态——「不中断」的精确定义 = 每批结束系统可用、仪器常绿，不是每行
  改动后可用。同批先摘除旧视图挂载/注册点（防每帧报错风暴拖慢真机迭代），
  再逐页以终态恢复。
3. **每任务一次原子提交**（可回退粒度 = 任务）是这套打法的安全网。

## 2. 引擎实测坑（本轮新锤）

- **颜色串必须 8 位 #RRGGBBAA**：6 位 #RRGGBB 引擎静默失效（backdrop 不上色，
  页面像没遮罩）。page.backdrop 声明归一化时自动给 6 位补 FF 兜底。
- **raw 逃生口内 imgui 控件名必须局部简单名（不带 '/'）**：引擎按名字解析
  容器栈，带路径分隔符的名字导致 begin/end 配对失配（mismatched ui +
  当帧渲染中断 + 后续每帧级联异常）。
- **模块级 `local cg = bgd_api.client.cgui` 有加载期循环**：被 api/cgui.lua
  加载链 require 的模块（page/toast、dialog、guide、debug_hub 等在 cgui 加载
  期内执行）模块级拿不到 cg（nil）。纪律：框架页要么 render 闭包内惰性取
  `bgd_api.client.cgui`，要么确认自己的加载序晚于 cgui（api/init.lua 注册序）。
- **内容塌缩**：center 定宽面板内的外层 flow 必须 `layout.grow_width = 1`，
  否则按内容自适应塌成一条竖带（子控件 grow 相对 0 宽全灭）。

## 3. Page 内核设计要点（实现备忘）

- **挂起 = 卸载不渲染**是唯一机制：「suspended 不进快照」「零每帧开销」
  「dbg 不可见不可操作」全部由它推导，无独立代码路径。
- **多挂起体系共存靠 mount 的来源 tag 计数隐藏**（set_hidden_tag）：
  任一来源持有即隐藏，全部解除才恢复—— panel/page 双轨期互不误解除。
- **exclusive 新语义 = 同类型 exclusive 页互斥**，非 exclusive 页天然共存——
  旧版 TeamHUD 靠 POPUP+200 档内偏移躲互斥的写法彻底消失，档内偏移随之废除。
- **BENCH 内建挂起业务档**（HUD/POPUP/DIALOG/SYSTEM/GUIDE；NOTIFY/MAP/WORLD
  不挂）替代旧 external_suspender 登记；`suspend = false` 可关闭（debug_hub
  面板页这类纯工具面板用）。
- **dbg 遮挡判定**（occlusion_provider）：可见 exclusive/backdrop 页的最高
  档位之下的视图 = 玩家不可见。backdrop 页借此与 exclusive 同权打通 AI 视野。

## 4. 迁移 playbook（旧 panel/mount → cg.page，逐条对照）

| 旧 | 新 |
| --- | --- |
| cg.panel.register(name, draw, { layer, exclusive, suspend_layers, on_open, on_close }) | cg.page({ name, type, exclusive, suspend, backdrop, on_open, on_close, render }) |
| cg.mount(name, draw, { order, root_z }) 常驻视图 | cg.page({ name, type, render }) + 启动编排区 open 一次 |
| 手画全屏遮罩 + 点外关闭 | backdrop = true（或 { color, opacity }） |
| 页内 z_index 手写 + 档内偏移（POPUP+100） | 删除（root_z/渲染序框架自动；页内 z 只表达相对序 0~9999） |
| HUD 入口注册机制（HudBar.Register） | 各 HUD 页直接画入口按钮（同类型多页共存、锚点自避） |
| 协议回调里弹窗 | cg.confirm_async / cg.dialog_async（投递 → page/dialog.lua SYSTEM 页） |
| 页面级状态 cg.remember(key, initial, true) | 模块局部表优先；控件级 cg.remember(initial, { key?, persist? }) |
| cg.patterns.form_page | form_row 直接组合（patterns 已删） |

## 5. 未做与原因（防后人误当 bug）

- 部分遮挡不做逐像素判定（全屏页是主流形态，主路径已闭合）
- remember 瞬态槽位 600 帧回收是已知边界（要「关再开不丢」走 persist/模块局部表）
- on_suspend/on_restore 钩子不加（挂起=不渲染动画自停；无真实场景，语义位置预留）
- Tiled 完整运行时不做（MAP 档是宿主预留；游戏侧 MAP 页是其雏形）
