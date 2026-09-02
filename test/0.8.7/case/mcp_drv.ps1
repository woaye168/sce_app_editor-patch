param([Parameter(Mandatory=$true)][string]$InFile)
$exe = "d:\sce_online\Res\maps\sce_app_editor-patch\target\debug\sce_app_editor-patch.exe"
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exe
$psi.Arguments = "mcp"
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
$psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
$p = [System.Diagnostics.Process]::Start($psi)
$lines = [System.IO.File]::ReadAllLines($InFile)
foreach ($l in $lines) { if ($l.Trim()) { $p.StandardInput.WriteLine($l) } }
$p.StandardInput.Flush()
foreach ($l in $lines) {
    if (-not $l.Trim()) { continue }
    $t0 = Get-Date
    $resp = $p.StandardOutput.ReadLine()
    $el = [math]::Round(((Get-Date)-$t0).TotalSeconds, 1)
    Write-Output "----- (${el}s) -----"
    Write-Output $resp
}
$p.StandardInput.Close()
Start-Sleep -Milliseconds 500
if (-not $p.HasExited) { $p.Kill() }
$err = $p.StandardError.ReadToEnd()
if ($err) { Write-Output "STDERR: $err" }
