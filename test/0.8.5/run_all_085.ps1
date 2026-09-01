# 0.8.5 全量验收一键执行：按编号顺序跑 01~10 全部用例并汇总
# 用法：powershell -File run_all_085.ps1（编辑器在线即可，各用例自带 start_debug 重置）
# 输出：逐用例 ok/failed_step/errors distinct + 末尾汇总表
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$caseDir = Join-Path $PSScriptRoot 'case'
$cases = @(
    '01_bag_full.ps1',
    '02_shop_full.ps1',
    '03_gm_full.ps1',
    '04_team_full.ps1',
    '05_hud_combat.ps1',
    '06_notify_framework.ps1',
    '07_bench_cgui.ps1',
    '08_bench_imgui.ps1',
    '09_page_semantics.ps1',
    '10_review_issues.ps1'
)

$summary = @()
foreach ($c in $cases) {
    $path = Join-Path $caseDir $c
    [Console]::WriteLine(("`n========== {0} ==========" -f $c))
    if (-not (Test-Path $path)) {
        [Console]::WriteLine("MISSING: $path")
        $summary += [pscustomobject]@{ case = $c; result = 'MISSING'; failed_step = '-'; errors = '-' }
        continue
    }
    $out = & powershell -NoProfile -File $path 2>&1 | Out-String
    [Console]::WriteLine($out.TrimEnd())
    $failed = '-'
    $m = [regex]::Match($out, 'failed_step:\s*(\S+)')
    if ($m.Success) { $failed = $m.Groups[1].Value }
    $errs = '-'
    $m2 = [regex]::Match($out, 'logs errors distinct:\s*(\S+)')
    if ($m2.Success) { $errs = $m2.Groups[1].Value }
    $result = 'PASS'
    if ($out -match '\bERR\b' -or ($failed -ne '-' -and $failed -ne '' -and $failed -ne 'null')) { $result = 'FAIL' }
    if ($out -match 'PARSE FAIL') { $result = 'PARSE_FAIL' }
    $summary += [pscustomobject]@{ case = $c; result = $result; failed_step = $failed; errors = $errs }
}

[Console]::WriteLine("`n================ 汇总 ================")
foreach ($s in $summary) {
    [Console]::WriteLine(("{0,-28} {1,-10} failed_step={2,-6} errors_distinct={3}" -f $s.case, $s.result, $s.failed_step, $s.errors))
}
$pass = ($summary | Where-Object { $_.result -eq 'PASS' }).Count
[Console]::WriteLine(("`n通过 {0}/{1}" -f $pass, $summary.Count))
