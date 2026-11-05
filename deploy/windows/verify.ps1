#Requires -RunAsAdministrator
<#
.SYNOPSIS
  ShareWeb 部署后差异验证（零侵入式检查）
.DESCRIPTION
  对比部署前基线快照，仅允许 ShareWeb 自身资源变更。
  退出码：0=PASS；60=BASELINE_VIOLATION
#>
param(
    [string]$BaselinePath = "",
    [string]$InstallDir = "C:\HTKIS\ShareWeb",
    [int]$Port = 18888,
    [string]$ServiceName = "HTKIS-ShareWeb"
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrEmpty($BaselinePath)) {
    $dir = Join-Path $InstallDir "data\baseline"
    if (Test-Path $dir) {
        $latest = Get-ChildItem $dir -Filter "baseline-*.json" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if ($latest) { $BaselinePath = $latest.FullName }
    }
}

if (!(Test-Path $BaselinePath)) {
    Write-Host "[FAIL] 基线快照未找到: $BaselinePath" -ForegroundColor Red
    exit 60
}

$baseline = Get-Content $BaselinePath -Encoding UTF8 | ConvertFrom-Json
$violations = @()

$currentServices = Get-Service | Select-Object Name, Status, StartType
$baseServices = $baseline.services
$addedSvc = $currentServices | Where-Object { $baseServices.Name -notcontains $_.Name }
$removedSvc = $baseServices | Where-Object { $currentServices.Name -notcontains $_.Name }
$allowedAdded = $ServiceName
foreach ($s in $addedSvc) {
    if ($s.Name -ne $allowedAdded) { $violations += "新增服务: $($s.Name)" }
}
foreach ($s in $removedSvc) { $violations += "移除服务: $($s.Name)" }

$currentPorts = Get-NetTCPConnection -State Listen | Select-Object LocalPort
$basePorts = $baseline.listeningPorts
$newPorts = $currentPorts | Where-Object { $basePorts.LocalPort -notcontains $_.LocalPort }
foreach ($p in $newPorts) {
    if ($p.LocalPort -ne $Port) { $violations += "新增监听端口: $($p.LocalPort)" }
}

$currentSmb = Get-SmbShare -ErrorAction SilentlyContinue | Select-Object Name, Path
$baseSmb = $baseline.smbShares
$addedSmb = $currentSmb | Where-Object { $baseSmb.Name -notcontains $_.Name }
$removedSmb = $baseSmb | Where-Object { $currentSmb.Name -notcontains $_.Name }
foreach ($s in $addedSmb) { $violations += "新增 SMB 共享: $($s.Name)" }
foreach ($s in $removedSmb) { $violations += "移除 SMB 共享: $($s.Name)" }

$currentEnv = Get-ChildItem Env: | Select-Object Name, Value
$baseEnv = $baseline.envVars
$addedEnv = $currentEnv | Where-Object { $baseEnv.Name -notcontains $_.Name }
$removedEnv = $baseEnv | Where-Object { $currentEnv.Name -notcontains $_.Name }
foreach ($e in $addedEnv) { $violations += "新增环境变量: $($e.Name)" }
foreach ($e in $removedEnv) { $violations += "移除环境变量: $($e.Name)" }

$fwRuleName = "HTKIS-ShareWeb-$Port"
$currentFw = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.Enabled -eq 'True' } | Select-Object DisplayName, Direction, Action
$baseFw = $baseline.firewallRules
$addedFw = $currentFw | Where-Object { $baseFw.DisplayName -notcontains $_.DisplayName }
foreach ($f in $addedFw) {
    if ($f.DisplayName -ne $fwRuleName) { $violations += "新增防火墙规则: $($f.DisplayName)" }
}

$report = @{
    baselinePath = $BaselinePath
    timestamp = (Get-Date).ToString("yyyyMMdd-HHmmss")
    violations = $violations
    result = if ($violations.Count -eq 0) { "PASS" } else { "BASELINE_VIOLATION" }
}
$reportPath = Join-Path $InstallDir "data\verify-report.json"
$report | ConvertTo-Json -Depth 3 | Set-Content -Path $reportPath -Encoding UTF8

if ($violations.Count -eq 0) {
    Write-Host "[PASS] 零侵入验证通过，无未授权变更" -ForegroundColor Green
    exit 0
} else {
    Write-Host "[FAIL] 检测到未授权变更:" -ForegroundColor Red
    foreach ($v in $violations) { Write-Host "  - $v" -ForegroundColor Yellow }
    exit 60
}