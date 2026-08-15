# xdeditor-160 库研究清单

> 镜像源（明文）：`D:\sce_online\Res\maps\bgd_glzy\.editor_src_mirror\xdeditor-160`（对应 `Res/_m/xdeditor/160/xdeditor`）
> 成果目录：`.trae/skills/sce-lib-xdeditor-160/`
> 规则：逐文件记录（每个 .lua 都要有条目）、结论标注 `相对路径:行号`、成果即时落盘、不臆测。

## 批次

| 批次 | 范围 | 文件数 | 成果文件 | 状态 |
|---|---|---|---|---|
| A | 根目录（main.lua / io_modifier.lua）+ config + console + examples + exception + global + guide | ~41 | files/core-config.md | 待研 |
| B | http_requests + ini + map_generator + map_starter + profiler + project_manager + ref + scene_test_enter_point + sub_process_enter_point + temp + test + texture_merger + texture_viewer + third-party + upload_map + utils | ~63 | files/misc-modules.md | 待研 |
| C | plugin/ 根 + attribute_editor + bloodstrip_editor + gui_editor + light_edit_ui + localization_manager + make_human_plugin + material_editor | ~120 | files/plugin-a.md | 待研 |
| D | plugin/ model_editor + obj_editor_cpp + obj_editor_ui + obj_editor_v2 + particle_editor + physic_editor_plugin + sample + tile_editor | ~190 | files/plugin-b.md | 待研 |
| E | trigger/（rule 目录按文件逐条简录，重点 entry/trigger_manager/trigger_ui_*） | ~96 | files/trigger.md | 待研 |
| F | trigger_editor_v2/ | ~46 | files/trigger-editor-v2.md | 待研 |
| G | ui/（重点 menu_bar / main_view / login / init / window_title_bar 相关） | ~85 | files/ui.md | 待研 |
| H | window/（三批之一） | ~90 | files/window-a.md | 待研 |
| I | window/（三批之二） | ~90 | files/window-b.md | 待研 |
| J | window/（三批之三） | ~85 | files/window-c.md | 待研 |

## 主会话负责（专项）

- 菜单注册机制专项（menu_bgd 修复依据）：window_title_bar 组件机制、register→渲染链路、加载时机、可用事件/回调
- architecture.md：加载流程（main.lua → login → show_editor_main_ui → include 'ui'/'window'/'plugin'）、include/require/@ 机制
- api.md：SCE/EDITOR/eventMgr/common/base/log 等全局 API 签名
- hooks.md：hook 配方（菜单注册、入口插槽等）

## 逐文件记录格式

```markdown
## <相对路径，如 ui/menu_bar.lua>
- 用途：一句话
- 导出：return 内容 / 关键函数签名
- 依赖：require/include（注明 @ 跨库）
- 补丁相关：关键全局/加载时机/可 hook 点（无则写「无」）
```

数据/规则类文件（trigger/rule、test）可一行一条，但必须逐文件列出。
