#Requires -Version 5.1
<#
.SYNOPSIS
    清理 bgd_bridge_test 冒烟测试产物（0.4.0 验证后由用户本机手动执行）。
.DESCRIPTION
    1. 删除 D:/sce_online/version-13/BgdBridge.dll
    2. 从 D:/sce_online/version-13/sce.deps.json 摘除 BgdBridge/1.0.0（targets 与 libraries 两处）
    3. 删除 xdeditor 补丁模块目录 .../sce_app_editor-patch/bgd_bridge_test/
    4. 从同目录 main.lua 的 modules 表中移除 'bgd_bridge_test' 条目
.EXAMPLE
    ./tools/cleanup_smoke.ps1 -WhatIf   # 预演，只打印将执行的操作
    ./tools/cleanup_smoke.ps1           # 实际执行（默认本机路径）
    ./tools/cleanup_smoke.ps1 -EngineRoot D:/sce_online -ApiVersion 13 -XdVersion 160
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    # 引擎运行根（version-<api> 与 Update/ 的上级），必填或经环境变量 SCE_ENGINE_ROOT 提供
    # （每台机器编辑器安装目录不同，禁止硬编码默认路径）
    [Parameter(Mandatory = $false)]
    [string]$EngineRoot = $(if ($env:SCE_ENGINE_ROOT) { $env:SCE_ENGINE_ROOT } else { throw '请通过 -EngineRoot 或环境变量 SCE_ENGINE_ROOT 指定引擎运行根' }),
    [string]$ApiVersion = '13',
    [string]$XdVersion = '160'
)

$ErrorActionPreference = 'Stop'

$hostDir      = "$EngineRoot/version-$ApiVersion"
$bridgeDll    = "$hostDir/BgdBridge.dll"
$depsJson     = "$hostDir/sce.deps.json"
$patchRoot    = "$EngineRoot/Update/editor-pd.spark.xd.com/Res/_m/xdeditor/$XdVersion/xdeditor/sce_app_editor-patch"
$moduleDir    = "$patchRoot/bgd_bridge_test"
$mainLua      = "$patchRoot/main.lua"
$depsKey      = 'BgdBridge/1.0.0'
$moduleId     = 'bgd_bridge_test'

# ---- 1. 删除 BgdBridge.dll ----
if (Test-Path $bridgeDll) {
    if ($PSCmdlet.ShouldProcess($bridgeDll, '删除 BgdBridge.dll')) {
        Remove-Item $bridgeDll -Force
        Write-Host "[1] 已删除 $bridgeDll"
    }
} else {
    Write-Host "[1] 跳过：$bridgeDll 不存在"
}

# ---- 2. sce.deps.json 摘除 BgdBridge/1.0.0 ----
if (Test-Path $depsJson) {
    $deps = Get-Content $depsJson -Raw -Encoding utf8 | ConvertFrom-Json
    $found = 0
    foreach ($tfmProp in $deps.targets.PSObject.Properties) {
        if ($tfmProp.Value.PSObject.Properties.Name -contains $depsKey) {
            $found++
            if ($PSCmdlet.ShouldProcess("$depsJson -> targets.$($tfmProp.Name).$depsKey", '移除 targets 条目')) {
                $tfmProp.Value.PSObject.Properties.Remove($depsKey)
                Write-Host "[2] 已移除 targets.$($tfmProp.Name).$depsKey"
            }
        }
    }
    if ($deps.libraries.PSObject.Properties.Name -contains $depsKey) {
        $found++
        if ($PSCmdlet.ShouldProcess("$depsJson -> libraries.$depsKey", '移除 libraries 条目')) {
            $deps.libraries.PSObject.Properties.Remove($depsKey)
            Write-Host "[2] 已移除 libraries.$depsKey"
        }
    }
    if ($found -gt 0 -and -not $WhatIfPreference) {
        # ConvertTo-Json 深度默认 2，deps.json 嵌套深，必须显式拉大
        $json = $deps | ConvertTo-Json -Depth 100
        [System.IO.File]::WriteAllText($depsJson, $json, (New-Object System.Text.UTF8Encoding($false)))
        Write-Host "[2] 已回写 $depsJson（共移除 $found 处条目）"
    } elseif ($found -eq 0) {
        Write-Host "[2] 跳过：$depsJson 中未找到 $depsKey 条目"
    }
} else {
    Write-Host "[2] 跳过：$depsJson 不存在"
}

# ---- 3. 删除补丁模块目录 bgd_bridge_test ----
if (Test-Path $moduleDir) {
    if ($PSCmdlet.ShouldProcess($moduleDir, '删除补丁模块目录')) {
        Remove-Item $moduleDir -Recurse -Force
        Write-Host "[3] 已删除 $moduleDir"
    }
} else {
    Write-Host "[3] 跳过：$moduleDir 不存在"
}

# ---- 4. main.lua modules 表移除 'bgd_bridge_test' ----
if (Test-Path $mainLua) {
    $lua = Get-Content $mainLua -Raw -Encoding utf8
    $pattern = "\s*'$moduleId',"
    if ($lua -match [regex]::Escape("'$moduleId'")) {
        if ($PSCmdlet.ShouldProcess("$mainLua -> modules 表", "移除 '$moduleId' 条目")) {
            $newLua = $lua -replace $pattern, ''
            if ($newLua -ne $lua) {
                [System.IO.File]::WriteAllText($mainLua, $newLua, (New-Object System.Text.UTF8Encoding($false)))
                Write-Host "[4] 已从 $mainLua 移除 '$moduleId' 条目"
            } else {
                Write-Warning "[4] 替换未生效，请人工检查 $mainLua"
            }
        }
    } else {
        Write-Host "[4] 跳过：$mainLua 中未找到 '$moduleId' 条目"
    }
} else {
    Write-Host "[4] 跳过：$mainLua 不存在"
}

Write-Host '清理完成。'
