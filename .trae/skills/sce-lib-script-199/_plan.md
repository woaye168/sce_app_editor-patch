# script-199 库研究清单

> 镜像源（明文）：`D:\sce_online\Res\maps\bgd_glzy\.editor_src_mirror\script-199`（由 examples/decrypt_mirror 生成，对应 `Res/_m/script/199/script`）
> 成果目录：`.trae/skills/sce-lib-script-199/`
> 规则：逐文件记录（每个 .lua 都要有条目）、结论标注 `相对路径:行号`、成果即时落盘、不臆测。

## 批次

| 批次 | 范围 | 文件数 | 成果文件 | 状态 |
|---|---|---|---|---|
| A | common 根目录全部 .lua（init/main/isolation/reload/json/class/auto_test 等） | ~8 | files/common-root.md | 待研 |
| B | common/base/（不含 ui 子目录） | ~40 | files/common-base.md | 待研 |
| C | common/base/ui/ + common/test/ + test/ | ~50 | files/common-base-ui-test.md | 待研 |

## 主会话负责（专项）

- architecture.md：加载流程（init→main→isolation、lua state 模型、@ 跨库引用、include vs require、reload 机制）
- api.md：关键全局/API 签名（log/log_file、common、base、argv、io/os/debug 解禁项）
- hooks.md：hook 配方（isolation 解锁、框架入口插槽、文件监听解除）

## 逐文件记录格式

```markdown
## <相对路径，如 common/base/util.lua>
- 用途：一句话
- 导出：return 内容 / 关键函数签名
- 依赖：require/include（注明 @ 跨库）
- 补丁相关：关键全局/加载时机/可 hook 点（无则写「无」）
```
