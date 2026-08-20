# UI 渲染链 / 图集机制 / canvas_texture 实测研究

> 研究日期：2026-08-21
> 状态：本地 PIE 全部实测通过；线上 PC 运行时（tester）实测 canvas_texture 崩溃（平台 bug）
> 关联研究：[pc-tester-runtime-reverse.md](pc-tester-runtime-reverse.md)（线上运行时逆向）、[csharp-module-injection.md](csharp-module-injection.md)、[publish-and-capture.md](publish-and-capture.md)
> 实证环境：编辑器 api 13（version-13，script-199）+ test_res002 项目（MCP start_debug / get_game_logs / capture_game）
> 源码证据：`.editor_src_mirror/script-199`（common 包）、`xdeditor-160`（编辑器 UI）、`appui-50` / `gameui-52` / `lib_lobby-169` / `client_base`（引擎 _m 库）、`sceengine-strings.txt`（编辑器引擎字符串全集）

## 0. 一句话结论

游戏内渲染的 Lua 天花板是 C++ 预置全局 `ui.*`（sceengine.dll `LuaUI.cpp` 注册）；图集是「发布期自动打包 + native 按逻辑路径透明解析」，Lua 无任何子矩形/UV API；`canvas_texture_*` 是「编辑器能用、线上 PCBox 构建 native 硬崩」的半成品功能，**线上不可用**。Tiled/图集类需求走 sprites 网格 / clip 视窗 / 离线 chunk 合成——全部只依赖 pak 感知的 `image` 属性，线上安全。

## 1. 渲染调用链（base.ui → native，逐层实证）

```
base.ui.panel/label/spine/particle/sprites/canvas{...} + base.ui.create(tpl)
  │  —— 控件类型注册：common/base/template/init.lua:1-35
  │      26 种类型表 ui_types = { button canvas dock_area input label panel particle
  │        progress scene spine sprites viewport virtual_joystick(_listener/_slider)
  │        window color_packer color_panel lite_code minimap_canvas video
  │        scroll_rect spline_bg spline_curve bezier_curve webview }
  │      ui[类型] = base.ui.template(include('base.template.'..类型), 类型)
  │  —— 各类型模板（template/panel.lua / spine.lua / particle.lua / sprites.lua...）
  │      只做两件事：base.ui.view{ type='xxx', name, id } + 逐项 base.ui.watch(...)
  │  —— 29 个通用属性由 ui_default 统一 watch（ui.lua:475-504）：
  │      swallow_event(s)/static/disabled/overflow/enable_drag/enable_drop/z_index/clip/
  │      enable/show/color/gray/round_corner_radius/image/mask_image/border/opacity/
  │      focus/scale/rotate/low_level/render_group/flip_x/flip_y/fix_scale/fix_border/
  │      meta_info_str/blur_image  —— image 在这里，类型模板不用管
  ↓ 创建是帧延迟的：控件先进 wait_to_create 队列（ui.lua:442-446 add_wait_to_create_ctrl）
  ↓   下一帧 base.event.on_ui_tick（ui.lua:270-284）→ check_create_new()（:179-217）
  ↓   → gui.add_childs_t(队列) 批量落引擎 + subscribe_now() + emit_on_created()
  ↓ gui = C++ 全局 ui 表的动态代理（ui.lua:69-94）：
  ↓   不在 is_not_prop_set 白名单的 set_xxx → 改写为 ui.set_control_prop(id, k, ...)
  ↓   其余原样转发 C++ ui[key]；每次调用累加 callback_count（base.ui_info() :850 暴露）
  ↓ sceengine.dll（Urho3D 系 native 引擎）LuaUI.cpp 的 lua 注册块 → 真正渲染
```

其它关键机制（ui.lua）：

- `base.ui.create` 已被 hook.lua:116-120 包装一次（字体替换/r_width 分辨率自适应），再包装注意链序。
- `ui_creates[type_name]`（ui.lua:560）是类型→构造器注册表，`base.ui.template` 可注册新类型。
- C++→Lua UI 事件唯一入口：全局 `ui_events` 表（event.lua:298-308 init() 填充），事件清单 event_list（event.lua:1-50）：on_click/on_double_click/on_mouse_enter/leave/down/up/on_real_click/on_drag/on_drop/on_focus(_lose)/on_input/on_long_click(_release)/on_update_scroll/on_scroll_rect_changed/on_text_click/on_vj_*(5)/on_virtual_window_*(5)/on_spline_curve_*/on_bezier_curve_*/on_color_packer_change/on_color_panel_change/on_web_message。改表项可全局拦截。
- 两代 UI 并存：旧式 `base.ui.component`（本库 component/base.lua）vs 新式 `@common.base.gui.component`（实现在 client_base）。
- 控件元表 base.ui.mt（ui.lua:740-833）：on_tick/remove/add_child/get_screen_rect/get_ui_rect/xywh/get_image_wh/set_visible。

## 2. 底层直达边界：C/C# 都不可行也不必

- **C（native）**：StateGame 下 `package.loadlib` 被 isolation 置 nil（isolation.lua 阉割表），无 FFI。`ui.*` 就是官方导出的 native 边界全部。再往下只能 detour 引擎二进制（触碰安全红线）。
- **C#**：游戏渲染**不经过 C#**。CoreCLR/sce.dll/scemodule.dll 是编辑器外壳（WinUI 3 界面）；PIE/线上游戏画面由引擎 native 渲染到 SDL 窗口——截图补丁必须走 WGC 截 WinUI 主窗口再裁剪（publish-and-capture.md），就是因为 SDL 内容窗口直接呈现截出来是黑图。`csharp_activate_window` 只存在于编辑器 state（StateEditor），游戏 state（StateGame）无 SCE 上下文。
- **旧式直达 API**（官方 common/test 用例实证，适合调试/测试）：

```lua
ui.add_child('main', json.encode({ type='panel', name='a', image='image/xxx.png', layout={...} }))
ui.set_layout('a', json.encode({ position = {100, 100} }))
ui.set_text('a', 'hello'); ui.set_show('a', true)
ui.register_event('a', 'on_click')      -- 事件落全局 ui_events.on_click(name)
ui.set_control_prop(id, 'image', 'image/xxx.png')  -- 万能属性通道
ui.remove_control('a')
```

- `ui_sound` 同为 C++ 预置全局（LuaUISound.cpp 注册块实证）：`play_ui_sound(sound_id, vol)` / `play_ui_sound_ex(path, vol, type)` / `play_sound(path, vol, loop, time, type)` / `stop_sound(type)` / `get_sound_position(type)` / `is_playing` / `stop_all_sound`。官方 d.lua 文档只写了前三个，注册块里还有后几个（bgd 框架 sound.lua 在用 play_sound/stop_sound/get_sound_position）。
- 调试入口：`base.ui_info()`（ui.lua:850-857，ui_map/tick_map/wait_to_create/callback 计数）。

## 3. 引擎侧控件属性全集（sceengine.dll 字符串考古）

从 49MB sceengine.dll 导出的字符串全集（`.editor_src_mirror/sceengine-strings.txt`）中，LuaUI.cpp 注册块与属性名注册块是成对聚集的（lua 名 + C++ 名相邻）：

- **ui.* 注册块**（:443417-443571）：set_global_scale/set_rich_text_custom_tag/**add_image_search_path**/**clear_image_cache**/set_control_prop/add_childs_t/remove_control/**imgui_begin_view/imgui_end_view/imgui_begin_ui/imgui_end_ui/imgui_begin_wrapper/imgui_end_wrapper/imgui_props/imgui_props2/imgui_data/imgui_state/imgui_view_data/imgui_view_state**/get_rect/get_image_wh/get_control_at_position/SetShow/SetName/RegisterEvent/UnregisterEvent/minimap_to_world/minimap_to_screen/CanvasClear/set_line_width/set_line_color/set_fill_color/draw_line/draw_circle/fill_circle/**fill_triangle**/draw_polygon/fill_polygon/draw_image/CanvasRotate/canvas_path_line_to/canvas_path_stroke/canvas_path_bezier_curve_to/canvas_texture_*(11 个)/switch_page/get_window_dock_type/dock_free/resume_dock/set_window_silent/is_window_show/get_window_tabsbar/set_lite_code_text/get_lite_code_text/switch_long_press_drag_mode/vk_key_click/set_scene_view_scale/set_scene_view_scissor_rect/build_component/ctrl_set_parent/as_sibling_of/get_ctrl_meta_info_str_at_cursor/screen_space_to_ctrl_space/ctrl_space_to_screen_space/get_ctrl_final_scale/get_ctrl_global_scale/ctrl_reflow/set_enabled_in_game/check_webview_environment/move_to_new_parent
  - 🔴 **Lua 层未封装的**：`fill_triangle`、`add_image_search_path`、`clear_image_cache`、整组 `imgui_*`（@appui.imgui 的底座）。
- **属性名注册块**（:452240 起）：image 相关仅 `image/mask_image/border/flip_x/flip_y`；sprites 仅 `frame_count/row_frame_count/start_frame/end_frame/sprite_size/playing`；panel 滚动 `enable_scroll/scroll_direction/scroll_image/scroll_color/scroll_hover_color/scroll_drag_color/scroll_end_margin/scroll_track_image/scroll_track_color/scroll_width/scroll/scroll_pos/scroll_pos_hard/scroll_elasticity/scroll_deceleration/scroll_threshold`；scene 控件 `camera_info/buff/rotation_ue/rotation_qua/zoom/scale3D/anim_fade_time/independent`；还有 `progress_type/progress_rotate/slider_*`、`viewport_name/viewport_msaa`、`html/run_js/web_message/web_type/web_dev_tools/web_import_script/isolated`、`sr_*`（scroll_rect）、`vj_*`（摇杆）等。
  - 🔴 **结论：控件属性全集里没有 UV/源矩形/裁剪区域属性**，「取图集子图」在控件属性层不存在通道。

## 4. 图集机制（引擎原生，发布期自动）

### 4.1 格式（appui-50 包 ui/atlas/ 实证）

- `ui/atlas/atlas.json`：图集注册表，数组 `[{ AtlasPath: "atlas/ui.png", ConfigPath: "atlas/ui.json" }]`（多图集就多条目）。
- `ui/atlas/<name>.json`：映射表，数组条目：

```json
{
  "RelativePath": "ui/image/basic/circle_16.png",   // 原始逻辑路径 = 运行时引用名
  "CompressType": 0,          // 0=普通；4=九宫格（条目拆成 Grid-0..8 九个子矩形，边格带 BorderWidth）
  "X": 1, "Y": 1, "Width": 32, "Height": 32,        // 在大图里的像素矩形
  "Border": "84.0 30.0 83.0 0.0",   // 可选，"left top right bottom"
  "OriginSize": "172 34", "HasAlpha": true
}
```

九宫格条目实例（radius_1.png，CompressType=4）：Grid-0..8 九个 {X,Y,Width,Height,OriginWidth,OriginHeight,CompressType[,BorderWidth]}——九宫格的图在打包时被拆成 9 块独立入图集。

### 4.2 生产与消费

- **生产**：发布/上传时 `debug_manager:preprocess_game(target_path)`（native；xdeditor upload_map_view.lua:583/638，进度文案「打图集」）扫描 `ui/image` 自动打包。**手写 atlas.json 会在发布时被覆盖，不可手造**。
- **消费**：完全在 native。查遍 gameui/lib_lobby/appui/client_base 的 Lua，**零处引用 atlas**——运行时按原逻辑路径引用（`image='image/xxx.png'`），引擎自动从图集取子图，对代码透明。
- **发布只收界面编辑器数编引用的图**：p_55a3.pak 实证——atlas 里只收了 1 张数编 GUI 引用的图（组件_020.png，82 字节 atlas.json + 206 字节 atlas_1.json + 16KB atlas_1.png）；Lua 代码引用的散图（shop/item 等 bgd 资源）**全部原样保留在 pak 里**（ui/image/image/bgd_game_client/... 逐文件条目在）。
- **旁挂配置** `<图>.png.json`（xdeditor gui_editor/tools/image_config.lua 全量解读）：
  - 字段：`UnBuildAtlas`（不打进图集，保留原图）/ `Border`（"l t r b" 字符串）/ `CompressTo`（"w h" 压缩到指定尺寸）/ `SkipUISize`（跳过按 UI 尺寸自动算 CompressTo）。
  - 旧格式自动升级：`.ff`（{UnBuildAtlas}）+ `.conf`（CompressType=4 时 Grid-1/3/5/7 的 BorderWidth 四边）→ 合并为 .json 后删旧文件（image_config.lua:87-120）。
  - `CompressTo` 自动推算：保存数编时按引用该图的控件 layout 尺寸取最大值（save_map_image_configs :202-301；sprites 控件类型 `$$.gui_ctrl.sprites` 不参与）。
  - `SCE.CalcImageBorder(path)` native 可自动计算九宫格边距（image_config.lua:194-199）。
  - `border` 控件属性文档原话「不设置时读取图集中的边框宽度」——即来自这里的配置。

### 4.3 image 属性的四种来源

1. 相对资源路径：`'image/xxx.png'`（相对地图 ui/ 目录）；
2. `'@地图名/image/xxx.png'` 跨图命名空间前缀（image_config.lua:138-159 的 header 解析实证）；
3. http(s) URL：经 client_base `base/ui/image_cache.lua`（见 §5）；
4. **绝对文件路径**（image_cache 的 cache.get 实证：下载到 `<root>/imagecache/<md5>.<ext>` 后把绝对路径直接赋给 image 显示）。

## 5. image_cache（网络图片通道，client_base 全量解读）

`client_base/common/base/ui/image_cache.lua`（跨库桩 `require '@base.base.ui.image_cache'` 的实现体）：

- **「isolation 前捕获 native 函数」官方范本**：模块加载时缓存 `io_download_file / io_add_resource_path / io_exist_file`（:8-10）并立刻 `io_add_resource_path('imagecache')`（:13）——模块随 ui.lua:242 在 isolation 之前加载，之后 isolation 把这些 io.* 置 nil 也不影响已捕获的引用。
- 触发点：ui.lua:247-248——watch 到 image 属性值以 `http` 开头时走 image_cache.run（下载 → 赋缓存名）。
- `cache.run`（:32-56）：URL → 短名（avatar 函数对微信头像 qlogo.cn 强制 64 尺寸 + 名字截尾 10 字符小写）→ `io_download_file(url, 'imagecache/'..name)` → 成功后 `ui[k]=name` 并回调 `func(ui.id, name)`。
- `cache.get(url, func, name, use_cache)`（:73-159）：URL→md5→`imagecache/<md5><ext>` 绝对路径；已存在直接用；并发防击穿用 coroutine.promise；下载走 `sce.httplib.request{url, output=save_path}`（co.call 包装）；手机端强制 https→http（:79-81，注释「手机上应该不能用https」）；`replace_update_url` 做 URL 替换钩子。
- 配套目录：tester 运行时 `Win/imagecache/` 下大量 md5 文件（部分带 .png 扩展）实证该通道线上在用。

## 6. sprites 控件图集模式（网格取子图）

GUI 编辑器实证（xdeditor ctrl_container.lua:1438-1547, 2622）：

- sprites 有「文件来源」二选一：**图集 atlas**（image 填单张图片）/ **文件夹 folder**（image 填目录）。
- 运行时判定逻辑：`string.find(image, '.')`——**image 字符串含 `.` = 图集模式，不含 = 文件夹模式**（:2622）。
- 图集模式专属属性：`sprite_size`（每帧宽高）、`row_frame_count`（每行帧数）；两模式共有：`frame_count/start_frame/end_frame/interval/loop/playing`。
- **帧号行优先从 1 开始**：frame = row * row_frame_count + col + 1（粒子编辑器 SubUV 模块同款算法实证：particle_scene_ui.lua:4846-4848 `row = idx//col_count; col = idx%col_count`）。
- **定格姿势（本地 PIE 实测）**：`playing=false` 会忽略 start_frame 停在第 1 帧；正确定格 = `playing=true + loop=false + start_frame=end_frame=N + interval 大值`（四象限验证：4 个 sprites 各定格一帧，红/绿/蓝/黄全部正确）。
- **Tiled 对齐**：frame = gid - firstgid + 1（Tiled tileid 也是行优先）；**tileset 的 margin/spacing 必须为 0**（SCE 网格按 sprite_size 紧密步进，无间距概念）。
- 性能顾虑消解：sprites 当静态图用的问题不在「序列帧控件」本身，而在于每控件独立纹理加载——若所有瓦片引用**同一张图集路径**，引擎资源层复用同一纹理，开销可控；真正的数量瓶颈用 §9 的 chunk 方案解决。

## 7. clip 视窗法（任意子矩形，支持 margin/spacing）

```lua
base.ui.panel {           -- 父：瓦片大小视窗
    layout = { width = tile_w, height = tile_h, position_type = 'absolute', position = {x, y} },
    clip = true,          -- 引擎通用属性：裁剪子控件超出部分
    static = true,
    base.ui.panel {       -- 子：整张图集，负偏移把目标矩形对齐到视窗
        layout = { width = atlas_w, height = atlas_h,
                   position_type = 'relative', position = { -tile_x, -tile_y } },
        image = '<图集资源路径>', static = true,
    },
}
```

本地 PIE 实测通过（四象限图集 + 偏移 {-128,-128} 只显示右下黄色象限）。代价每瓦片 2 个最轻量 panel（无动画定时器、无事件）。适合 Tiled 带 margin/spacing 的图集或非均匀子图。

## 8. canvas_texture 家族（实测全录）

### 8.1 API 清单

LuaUI.cpp 注册块 11 个：`canvas_texture_set_name / set_size / set_fill_color / fill_pixel / fill_rect / fill_circle / clear_circle / get_pixel_color / set_compressed_data / get_compressed_data / set_blur`。官方唯一 Lua 封装是 script-199 `template/canvas.lua` 的 texture_brush（canvas 控件实例 `ui:get_brush()`）；**tester 内嵌 script-190 的 canvas 模板没有它**（功能是在 190→199 之间出现的）。

### 8.2 探针迭代实录（test_res002 CanvasProbe，2026-08-21）

探针基建：base64 内嵌 PNG（helmet_demon.png 17057B → 22744 字符）+ 纯 Lua b64 解码 + `>>> step` / `<<< step ok ret` marker 日志 + `场景-加载完成` 挂钩 + base.wait(1000) 等控件落引擎。

| 版本 | 改动 | 结果 |
| --- | --- | --- |
| v1 | b64 自检 / A: io.read 地图图片 / B: set_size+set_fill_color+fill_rect+fill_pixel / C: set_compressed_data(PNG) / D: get_pixel_color+get_compressed_data | b64 解码 ✓（头 `89 50 4E 47`）；**A 失败 err=1（本地 PIE 也读不到地图图片）**；B 全活；**C ret=false（拒绝 PNG，不崩）**；D1 像素仍红；D2 回调 70 字节 |
| v2 | 读 PNG IHDR 宽高；F: draw_image(资源路径)；D2 改 hex 转储；E: round-trip（填蓝→喂回 blob→恢复） | F ok；C 复核 false；**D2 头 `28 B5 2F FD` = LZ4 frame 魔数**，blob 可见红色像素字面量 `FF 00 00 FF`；**E3 round-trip ret=true**（格式自洽）；E1 填蓝后 E2 像素仍红 → **set_fill_color 是画刷色不是 flood fill**；draw_image 后像素不变 → **draw_image 是矢量层** |
| v3 | canvas 背景改透明；G: set_name + panel `image='probe_canvas_tex'` | 截图：**canvas 显示红色纹理（v1/v2 背景 #202020 盖住了纹理！）；panel 引用命名纹理显示红块 ✓** |
| v5 | 四象限填色（红绿蓝黄）；H: 4 个 sprites 各定格一帧；I: clip 视窗偏移 {-128,-128} | 截图：**canvas 四象限 ✓；sprites 四帧各取各的 ✓（v4 曾用 playing=false 失败停帧 1，改 playing=true 后正确）；clip 纯黄 ✓** |

关键截图存证：test_res002/.bgd/log/screenshots/（capture_1787256943/1787257132/1787257267/1787257454/1787257604.png）。

### 8.3 结论

- `set_compressed_data` 吃 **LZ4 frame 压缩的原始 RGBA**（标准 lz4 帧可用任意 lz4 工具链离线生成），纹理尺寸由 `set_size` 决定。
- `canvas_texture_set_name(id, '名字')` 后，**任意控件的 `image='名字'` 都能引用该动态纹理**（panel/sprites/clip 均实测）——理论上的「运行时上传图集 + 网格取帧」闭环成立。
- 编辑器/PIE 全部可用；**线上 PC 运行时 set_size 第一步 native 硬崩**（见 pc-tester-runtime-reverse.md §2/§8）——平台 bug，不作为任何方案的依赖。
- canvas 显示纹理前提：背景透明（`color='rgba(0,0,0,0)'`），否则背景色盖住纹理。

## 9. Tiled 接入推荐（线上安全）

```
离线：Tiled 导出 lua（纯数据，放 src/common/ require）+ tileset png → .bgd/src/res/image/tiled/xxx.png
      （有 margin/spacing 的图集：离线重排成零间距，或走 clip 视窗法）
运行时（全部只走 image 属性，pak 感知，零文件 IO）：
  · 均匀网格：sprites 控件 image + sprite_size + row_frame_count + start=end=帧号 + playing=true
  · 任意矩形：clip 视窗法（§7）
  · 大地图性能：离线把静态层按 16×16 瓦片合成 chunk 大图，一个 chunk 一个 image 控件；
    视口外不创建/复用控件
```

## 10. bgd 资源路径映射（工具链视角，rewrite.rs/res.rs 实证）

源码里写 `'src/res/<类型>/...'` / `'libs/res/<类型>/...'`，构建时改写+同步：

| 类型 | 同步目标（项目根下） | 运行时引用改写 | 备注 |
| --- | --- | --- | --- |
| image | `ui/image/image/<prefix>/` | `image/image/<prefix>/<name>.png` | 期望 .png |
| particle | `res/effect/<prefix>/` | `res/effect/<prefix>/<name>.effect` | |
| sound | `res/sound/<prefix>/` | `res/sound/<prefix>/<name>`（去 .ogg） | |
| spine | `ui/spine/<prefix>/` | `spine/<prefix>/<name>`（去 .skel） | |
| sprites | `ui/image/sprites/<prefix>/` | `@<ProjectName>/image/sprites/<prefix>/<name>`（目录无扩展名/文件保留） | 前缀取 map_settings.json 的 ProjectName |

prefix：libs→`bgd_libs_client`，src→`bgd_game_client`。资源不存在时构建日志黄色警告不阻断。

## 11. 探针方法学与 MCP 实测链（可复用）

- **native 崩溃定位**：每个被测调用前后打 `>>> name` / `<<< name ok=.. ret=..` 配对日志（log_file.warn 落盘即刷），崩溃时最后一条 `>>>` 即凶手——pcall 拦不住 native 硬崩，marker 是唯一线索。
- **MCP 实测链**：探针模块挂 `场景-加载完成` → `bgd_sce_tools build` → `editor_start`（幂等）→ `start_debug`（默认 restart_last_debug，秒级重载）→ 等 ~20s → `get_game_logs`（`D:/sce_online/logs/lua/lua-game-*.log`）→ `capture_game` 截图目视（编辑器和游戏同进程，截图是后台 WGC 截取，不需要焦点）。
- **线上验证链**：编辑器发布测试版 → tester 进游戏 → tester 侧 `Win/logs/lua/lua-game-*.log`。
- 探针遗留物：test_res002 的 `.bgd/src/client/CanvasProbe.lua` + `CanvasProbePngData.lua` + GameClient.lua 两行（**不摘除则线上必崩**）。

## 12. 其它散落发现（本副产品）

- `SCE.Common.regenerate_seconduvs()`（xdeditor console/inner_ui.lua:255）——第二 UV 重建 native 入口。
- minimap_canvas 控件 API：`ui.minimap_to_world(id, x, y)` / `ui.minimap_to_screen`（template/minimap_canvas.lua）。
- progress 控件 progress_type：'left/right/up/down/clockwise/counter_clockwise/bordered left' 等（单轴裁剪图片，可当残废版子矩形用）。
- scene 控件可在 UI 里渲 3D 模型/粒子/buff 表现（template/scene.lua：set_model/set_camera_info/set_particle/set_buff/set_light）。
- 粒子编辑器 SubUV 模块（CEParticleModuleSubUV）证实引擎粒子也有网格子图概念，算法与 sprites 一致。
- video/webview 控件、自定义字体 argv（hook_font：微信/QQ/custom_font 时强制 font.family='Custom'）。
- npot_texture 测试用例证实非 2 幂纹理可用（网络图片）。
- 引擎是 Urho3D 二次开发（NE_pd 代码树），canvas = NanoVG（GUINanoVGCanvas），UI shader DX11。
- include vs require：include 是 C++ 注册的引擎加载器（热更语义，每次重新执行文件）；`@` 前缀跨库引用（'@base.xxx'/'@common.xxx' → client_base 库）。
- 三 lua state 模型：StateGame（游戏）/StateEditor（编辑器）/StateApplication（大厅），isolation 阉割只在 StateGame 生效。
