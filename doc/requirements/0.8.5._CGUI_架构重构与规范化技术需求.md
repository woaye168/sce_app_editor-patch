# [RFC-0.8.5] CGUI 架构重构与规范化技术需求

## 1. 核心需求要点 (扁平化)

* **层级命名与目录映射对齐**：统一 `src/client/ui` 业务目录与 `.bgd/libs/client/cgui/panel.lua` 的 `LayerType` 枚举定义，消除映射分歧（现存 `world` 目录对齐归正至 `GAME` 二维世界层），确保物理目录与框架层级枚举一一对应。
* **职责聚合目录与包导入机制（引擎机制指导）**：同职责的复杂功能（如 GM 系统、背包系统）按目录进行物理聚合，目录内统一使用 `init.lua` 作为对外暴露入口。明确指导开发人员利用引擎已有的 package 机制，外部消费方直接调用 `require("上级目录.目录名")` 即可自动命中 `init.lua`，无需拼接文件名。
* **基于 LayerType 的特性注入与声明式构建**：
* **起手式与框架驱动**：UI 模块通过注册对应的 `LayerType`（如 `POPUP`、`DIALOG`、`GUIDE`），由 CGUI 框架底层自动赋予并接管该层级特有的运行时调度与生命周期行为（如 `POPUP` 的全屏遮罩、输入吞噬、同层 `exclusive` 互斥、挂起 HUD；`DIALOG` 的单例排队与自动补位弹出等），杜绝业务层手写调度胶水代码。
* **声明式原语与视图组装**：页面内容完全采用纯声明式原语树（Data-as-UI / Table DSL）表达，`render` 函数返回节点配置树，页面由基础原语、带样式的原语及业务 Widget 自由拼装。


* **Widget 组合机制与双层扩展体系**：确立“UI 页面/Layer 本质上是由 Widget 声明式组装构建”的设计模式，在与 `ui` 目录平级处设立 `widget` 模块体系。除 CGUI 内置的基础通用控件外，框架提供标准化的扩展规范，支持业务层自由封装可复用的游戏组件。
* **调试工具链解耦与迁移**：将调试台从 CGUI 核心与业务逻辑中彻底解耦，统一迁移归档至 `libs/client/ui/cgui_bench`，并将原有的 `imgui_bench` 同步收拢移入该 `ui` 目录下，确保框架核心与业务生产包的纯净。

---

## 2. 目录规范结构示例

```text
src/client/
├── ui/                        -- 业务 UI 根目录（子目录严格映射 LayerType）
│   ├── game/                  -- GAME 层（原 world 目录对齐迁移至此）
│   ├── hud/                   -- HUD 层常驻界面
│   └── popup/                 -- POPUP 层弹窗/全屏面板
│       └── gm/                -- 聚合功能模块目录
│           ├── init.lua       -- 聚合对外唯一入口（消费方 require('src.client.ui.popup.gm')）
│           ├── GMMainPanel.lua-- GM 主面板（注册 LayerType.POPUP，承载 Tab 框架）
│           └── GMFurrencyTab.lua -- GM 货币子页面（声明式 View 逻辑）
└── widget/                    -- 业务自定义扩展 Widget 目录（与 ui 平级）
    └── FormRow.lua            -- 声明式可复用组件

```

---

## 3. 标准实现参考代码

### 3.1 业务可复用声明式 Widget (`src/client/widget/FormRow.lua`)

```lua
local cg = bgd_api.client.cgui

---通用的表单项组件（声明式 Widget）
---@param props { label: string, child: table, key?: string }
local function FormRow(props)
    -- 返回声明式节点配置树
    return cg.form_row({
        key = props.key,
        label = props.label,
        child = props.child,
    })
end

return FormRow

```

### 3.2 聚合子页面：货币发放 (`src/client/ui/popup/gm/GMFurrencyTab.lua`)

```lua
local ShopConfig = require('src.common.ShopConfig')
local protocol = require('libs.common.api.protocol')
local P = require('src.common.Protocol')
local CombatHUD = require('src.client.ui.hud.CombatHUD')
local FormRow = require('src.client.widget.FormRow')

local cg = bgd_api.client.cgui

local M = {}
local form = { uid = '', amount = '', currency = 'money' }

local function OnConfirm()
    local targetUid = tonumber(form.uid)
    if not targetUid then
        CombatHUD.ShowToast('GM：请输入目标玩家数字 ID')
        return
    end
    local amount = math.floor(tonumber(form.amount) or 0)
    if amount <= 0 then
        CombatHUD.ShowToast('GM：请输入大于 0 的数量')
        return
    end
    protocol.send_to_server(P.Req_GMAddCurrency, {
        target_uid = targetUid,
        currency = form.currency,
        amount = amount,
    })
end

function M.Reset()
    form.uid = ''
    form.amount = ''
    form.currency = 'money'
end

-- 声明式视图构建函数：返回纯数据树
function M.Render()
    local keys = {}
    for curType in pairs(ShopConfig.CURRENCY) do
        keys[#keys + 1] = curType
    end
    table.sort(keys)

    local names, sel = {}, 1
    for i, k in ipairs(keys) do
        names[i] = ShopConfig.CURRENCY[k].name
        if k == form.currency then sel = i end
    end

    return cg.col({
        key = 'currency_tab_content',
        children = {
            FormRow({
                label = '目标玩家ID',
                child = cg.input({
                    key = 'uid',
                    value = form.uid,
                    layout = { width = 320 },
                    on_input = function(t) form.uid = t end,
                })
            }),
            FormRow({
                label = '货币类型',
                child = cg.radio_group({
                    key = 'currency',
                    items = names,
                    selected_index = sel,
                    horizontal = true,
                    on_select = function(i) form.currency = keys[i] end,
                })
            }),
            FormRow({
                label = '发放数量',
                child = cg.input({
                    key = 'amount',
                    value = form.amount,
                    layout = { width = 320 },
                    on_input = function(t) form.amount = t end,
                })
            }),
            cg.row({
                key = 'confirm_row',
                layout = { grow_width = 1, row_content = 'center', margin = { top = 20 } },
                children = {
                    cg.button_primary({
                        key = 'confirm',
                        text = '确认发放',
                        layout = { width = 240, height = 60 },
                        on_click = OnConfirm,
                    })
                }
            })
        }
    })
end

return M

```

### 3.3 声明式页面主入口与 Layer 注入 (`src/client/ui/popup/gm/GMMainPanel.lua`)

```lua
local W = require('src.client.ui.world.WorldState')
local CurrencyTab = require('src.client.ui.popup.gm.GMFurrencyTab')

local cg = bgd_api.client.cgui

local M = {}
local current_tab = 'currency'

-- 声明式主容器构建
local function RenderGMMain()
    return cg.col({
        key = 'window',
        color = '#2B2B3A',
        round_corner_radius = 16,
        layout = { width = 620, padding = 16 },
        children = {
            cg.row({
                key = 'topbar',
                layout = { grow_width = 1 },
                children = {
                    cg.text({
                        text = string.format('GM 面板（我的 ID: %s）', tostring(W.localPlayerUid)),
                        font = { size = 24, color = '#FFD700', bold = true },
                        layout = { grow_width = 1 },
                    }),
                    cg.button({
                        key = 'close',
                        text = 'X',
                        color = '#A03A3A',
                        layout = { width = 56, height = 44 },
                        on_click = function() cg.panel.close('GMUI') end,
                    })
                }
            }),
            cg.row({
                key = 'tab_bar',
                layout = { margin = { bottom = 12 } },
                children = {
                    cg.button({
                        key = 'tab_cur',
                        text = '货币管理',
                        color = (current_tab == 'currency') and '#4A90E2' or '#333344',
                        on_click = function() current_tab = 'currency' end,
                    })
                }
            }),
            -- 嵌入子 Tab 的声明式节点
            (current_tab == 'currency') and CurrencyTab.Render() or nil
        }
    })
end

-- 注册 POPUP：框架自动接管全屏遮罩、同层互斥与 HUD 挂起
cg.panel.register('GMUI', {
    layer_type = cg.LayerType.POPUP,
    render = RenderGMMain,
    on_open = function()
        CurrencyTab.Reset()
        log.info('客户端：GM 面板已打开')
    end,
    on_close = function()
        log.info('客户端：GM 面板已关闭')
    end,
})

function M.Toggle()
    cg.panel.toggle('GMUI')
end

return M

```

### 3.4 聚合目录统一入口 (`src/client/ui/popup/gm/init.lua`)

```lua
local GMMainPanel = require('src.client.ui.popup.gm.GMMainPanel')
local CurrencyTab = require('src.client.ui.popup.gm.GMFurrencyTab')

return {
    Toggle = GMMainPanel.Toggle,
    ResetForm = CurrencyTab.Reset,
}

```

### 3.5 消费方调用 (`src/client/ui/hud/HudBar.lua` 或 `GameScene`)

```lua
local GM = require('src.client.ui.popup.gm') -- 引擎原生 package.path 机制自动命中 init.lua

HudBar.Register({
    id = 'gm',
    text = 'GM',
    color = '#8E44AD',
    order = 50,
    on_click = function()
        GM.Toggle()
    end,
})

```