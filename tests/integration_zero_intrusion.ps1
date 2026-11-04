<#
.SYNOPSIS
  零侵入式部署差异验证测试 (TASK-V101-28)
  在装有 PHP/MySQL/IIS 等业务服务的 Windows Server 2019 测试环境执行
#>
param(
    [string]$PackagePath = "",
    [string]$InstallDir = "C:\HTKIS\ShareWeb",
    [int]$Port = 18888,
    [string]$ServiceName = "HTKIS-ShareWeb"
)

$results = @{ scenarios = @(); timestamp = (Get-Date).ToString("o") }
$scriptDir = Split-Path $MyInvocation.MyCommand.Path -Parent
$deployDir = Join-Path $scriptDir "..\deploy\windows"

function Test-Scenario($name, $script) {
    try {
        $r = & $script
        $results.scenarios += @{ name = $name; result = $r; status = "PASS" }
        Write-Host "[PASS] $name" -ForegroundColor Green
    } catch {
        $results.scenarios += @{ name = $name; result = "$($_.Exception.Message)"; status = "FAIL" }
        Write-Host "[FAIL] $name: $($_.Exception.Message)" -ForegroundColor Red
    }
}

Test-Scenario "记录部署前业务服务状态" {
    $before = Get-Service | Select-Object Name, Status | ConvertTo-Json -Depth 2
    $before | Set-Content "test-results\before-services.json"
    "captured"
}

Test-Scenario "执行安装" {
    & powershell -File (Join-Path $deployDir "install.ps1") -PackagePath $PackagePath -Port $Port -ServiceName $ServiceName -InstallDir $InstallDir
    if ($LASTEXITCODE -ne 0) { throw "安装失败 (exit=$LASTEXITCODE)" }
    "installed"
}

Test-Scenario "执行差异验证" {
    & powershell -File (Join-Path $deployDir "verify.ps1") -InstallDir $InstallDir -Port $Port -ServiceName $ServiceName
    if ($LASTEXITCODE -ne 0) { throw "差异验证失败 (exit=$LASTEXITCODE)" }
    "verify PASS"
}

Test-Scenario "执行卸载" {
    & powershell -File (Join-Path $deployDir "uninstall.ps1") -ServiceName $ServiceName -InstallDir $InstallDir
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 30) { throw "卸载失败 (exit=$LASTEXITCODE)" }
    "uninstalled"
}

Test-Scenario "验证业务服务状态一致" {
    $before = Get-Content "test-results\before-services.json" | ConvertFrom-Json
    $after = Get-Service | Select-Object Name, Status
    $diff = Compare-Object $before $after -Property Name, Status
    if ($diff) { throw "业务服务状态有差异: $($diff | Out-String)" }
    "no difference"
}

$pass = ($results.scenarios | Where-Object { $_.status -eq "PASS" }).Count
$fail = ($results.scenarios | Where-Object { $_.status -eq "FAIL" }).Count
$results.pass = $pass; $results.fail = $fail
$results | ConvertTo-Json -Depth 4 | Set-Content "test-results\zero-intrusion.json" -Encoding UTF8
Write-Host "`n结果: PASS=$pass FAIL=$fail" -ForegroundColor Cyan
if ($fail -gt 0) { exit 1 } else { exit 0 }