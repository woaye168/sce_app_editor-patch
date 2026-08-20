# pak 资源还原操作手册（SCE 地图包/内嵌包解包指南）

> 整理日期：2026-08-21
> 定位：**操作向**手册——给一个 pak/7z，把里面所有资源原样还原成文件树。
> 原理细节见 [pc-tester-runtime-reverse.md](pc-tester-runtime-reverse.md)；本文只管怎么操作。
> 工具均在本仓库 `examples/`，`cargo run --example <名字> -- <参数>` 即用（仓库根目录执行）。

## 0. 30 秒速查

```powershell
# 在 sce_app_editor-patch 仓库根目录执行
$m = "D:\sce_online\Res\maps\bgd_glzy\.editor_src_mirror"   # 研究镜像目录（可换）

# 情况 A：手上就是 .pak 文件（地图包 / 内嵌包拆出的 pak）
cargo run --example decrypt_file  -- "$m\in\map.pak"      "$m\map.pak.dec"    # ① TNND 解密
cargo run --example pak_extract   -- "$m\map.pak.dec"     "$m\map_files"      # ② UPAK 解包

# 情况 B：手上是 .7z（tester 的 embedded_packages）
tar -xf xxx.7z -C "$m\some_dir"                                             # 系统 tar 即可
# 解出来的还是 .pak → 回到情况 A

# 情况 C：编辑器 _m 散目录（lua/json 混合格加密文件）
robocopy <包目录> "$m\xxx" /E                                               # 先备份副本
cargo run --example decrypt_inplace -- "$m\xxx"                             # 全文件就地解密
```

## 1. 你能遇到什么形态

| 形态 | 在哪见过 | 还原路径 |
| --- | --- | --- |
| 地图 pak：`maps/p_xxx/p_xxx.pak` + `libs.json` | tester `update/<env>/Res/res/maps/` | 情况 A（libs.json 是单独 TNND 文件，decrypt_file 直接解） |
| 依赖库 pak：`_m/<包>/<版本>/<包>/<包>.pak` | tester update 目录 | 情况 A |
| 内嵌基础包：`embedded_packages/*.7z` | tester `Win/embedded_packages/` | 情况 B（7z 里还是 pak） |
| 编辑器散目录包：`res/_m/<包>/<版本>/<包>/` | 编辑器更新目录 | 情况 C（文件级 TNND，只解密不解包） |

## 2. 格式速查（还原时必须知道的）

- **TNND 加密**：文件头 4 字节 `TNND`（`54 4E 4E 44`），剩余内容逐字节 XOR `CREATEEASY`。**任何扩展名都可能被加密**（lua/json/pak 都见过）；没头的文件就是明文，工具会自动跳过/原样处理。
- **UPAK（SCE 变体）**：解密后的 pak 内部格式

```
偏移  内容
0     "UPAK"（4 字节 magic）
4     u32 条目数
8     u32 总校验
12    条目索引 × N：
        名字（\0 结尾字符串）
        u32 文件数据偏移
        u32 文件数据大小
        u32 条目校验   ← 🔴 比标准 Urho3D 多这 4 字节，漏掉它解析会逐条漂移
N     文件数据区（明文，lua/json/png/ogg/skel 原样）
```

- **条目内容不再二次加密**：pak 解开就是明文源码/资源，直接可读可用。

## 3. 工具详表

| 工具 | 命令 | 说明 |
| --- | --- | --- |
| decrypt_file | `decrypt_file <输入文件> <输出文件>` | 单文件 TNND 解密；输入无 TNND 头则原样拷贝。80MB pak 秒级 |
| decrypt_inplace | `decrypt_inplace <目录>` | 目录树全文件就地解密；**护栏：路径必须含 `.editor_src_mirror`**，防止误指编辑器原始目录 |
| pak_list | `pak_list <已解密pak> <输出txt>` | 只读索引 → 清单（offset/size/路径），不解出文件，用于快速勘察 |
| pak_extract | `pak_extract <已解密pak> <输出目录>` | 全量解出文件树；防路径穿越（`..` → `__`） |
| strings_dump | `strings_dump <二进制> <输出txt>` | 附赠：从 exe/dll/pak 里导可打印字符串（不解析格式，纯考古用） |

## 4. 实操示例

### 4.1 还原线上地图包（tester 下载的 p_55a3）

```powershell
$m = "D:\sce_online\Res\maps\bgd_glzy\.editor_src_mirror"
$pak = "D:\sce_pc_tester\tester_1089\Win\update\e.production.spark.xd.com_test\Res\maps\p_55a3\p_55a3.pak"

cargo run --example decrypt_file -- $pak "$m\p_55a3.pak.dec"
cargo run --example pak_list   -- "$m\p_55a3.pak.dec" "$m\p_55a3-list.txt"   # 可选：先看清单
cargo run --example pak_extract -- "$m\p_55a3.pak.dec" "$m\p_55a3"
cargo run --example decrypt_file -- "D:\...\p_55a3\libs.json" "$m\p_55a3-libs.json"  # 旁挂清单
```

产物：`p_55a3/` 下完整地图文件树（`project/map_settings.json`、`ui/image/...`、`ui/atlas/...`、`res/sound/...`、`scene/...`、脚本等）。

### 4.2 还原 tester 内嵌 script 包

```powershell
tar -xf "D:\sce_pc_tester\tester_1089\Win\embedded_packages\script-190.7z" -C "$m\tester_script-190"
# 得到 Script.pak → 情况 A 两步走
cargo run --example decrypt_file -- "$m\tester_script-190\Script.pak" "$m\tester_script-190\s.dec"
cargo run --example pak_extract  -- "$m\tester_script-190\s.dec"      "$m\tester_script-190\extracted"
```

### 4.3 还原编辑器散目录包（如 appui-50）

```powershell
robocopy "D:\sce_online\update\editor-pd.spark.xd.com\res\_m\appui\50\appui" "$m\appui-50" /E
cargo run --example decrypt_inplace -- "$m\appui-50"
```

### 4.4 只看不拆（快速勘察 pak 里有什么）

```powershell
cargo run --example pak_list -- "$m\map.pak.dec" "$m\list.txt"
# 或只要路径名：strings_dump 也行（条目名是明文）
cargo run --example strings_dump -- "$m\map.pak.dec" "$m\strings.txt"
```

## 5. 注意事项 / 坑

1. **只读研究原则**：永远先复制到镜像目录再动手（decrypt_inplace 有路径护栏，但 robocopy 备份这一步别省）；**不要改 tester/编辑器原始目录的任何文件**。
2. **只能解，不能装回去**：条目校验（每条目 4 字节）和总校验的算法未逆向，改内容后无法生成合法 pak；TNND 再加密本身没问题，但 pak 完整性校验过不了。修改运行内容请走正常发布流程。
3. **tar 解 7z**：Windows 自带 bsdtar 直接支持 7z，不要加 `--warning=` 之类 GNU 参数（不支持）。
4. **大文件**：decrypt_file/pak_extract 是全量读内存的，1GB 的 pak（如 p_2xgc）需要 ~2GB 空闲内存；本机实测可跑。
5. **json 也是加密的**：atlas.json、libs.json 这类文件直接 Read 是乱码，先 decrypt_file。
6. **pak 内条目是明文**：不要再对 extract 产物跑 decrypt_inplace（本轮实测 0 个加密文件；跑了也无害，工具会自动跳过）。
7. 镜像目录统一放 `D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/`，已有清单见 pc-tester-runtime-reverse.md §10。

## 6. 工具源码位置（要改行为直接改）

- [decrypt_file.rs](file:///d:/sce_online/Res/maps/sce_app_editor-patch/examples/decrypt_file.rs) / [decrypt_inplace.rs](file:///d:/sce_online/Res/maps/sce_app_editor-patch/examples/decrypt_inplace.rs)：TNND 解密（crypto.rs 同款算法内联）
- [pak_list.rs](file:///d:/sce_online/Res/maps/sce_app_editor-patch/examples/pak_list.rs) / [pak_extract.rs](file:///d:/sce_online/Res/maps/sce_app_editor-patch/examples/pak_extract.rs)：UPAK 解析（条目结构定义就在这里，改格式先改这）
- [strings_dump.rs](file:///d:/sce_online/Res/maps/sce_app_editor-patch/examples/strings_dump.rs)：字符串导出
