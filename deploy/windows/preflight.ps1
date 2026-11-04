#Requires -RunAsAdministrator
<#
.SYNOPSIS
  ShareWeb 部署前环境基线采集与预检
.DESCRIPTION
  零侵入式部署：所有操作只读，不修改系统。
  退出码：0=通过；10=端口冲突；11=服务已存在；12=权限不足；13=OS版本不匹配
#>
param(
    [int]$Port = 18888,
    [string]$ServiceName = "HTKIS-ShareWeb",
    [string]$InstallDir = "C:\HTKIS\ShareWeb"
)

$ErrorActionPreference = "Stop"
$exitCode = 0

function Write-Baseline($path, $data) {
    $dir = Split-Path $path -Parent
    if (!(Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $data | ConvertTo-Json -Depth 5 | Set-Content -Path $path -Encoding UTF8
}

$timestamp = (Get-Date).ToString("yyyyMMdd-HHmmss")
$baselinePath = Join-Path $InstallDir "data\baseline\baseline-$timestamp.json"

try {
    $os = Get-CimInstance Win32_OperatingSystem
    $osCaption = $os.Caption
    $osArch = $os.OSArchitecture
    if ($osCaption -notmatch "Server 2019") {
        Write-Host "[FAIL] OS 版本不匹配: $osCaption (要求 Windows Server 2019)" -ForegroundColor Red
        exit 13
    }
    if ($osArch -notmatch "64") {
        Write-Host "[FAIL] 要求 64 位系统" -ForegroundColor Red
        exit 13
    }
    Write-Host "[OK] OS: $osCaption ($osArch)" -ForegroundColor Green

    $portConn = Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue
    if ($portConn) {
        $procId = $portConn[0].OwningProcess
        $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
        Write-Host "[FAIL] 端口 $Port 被占用 (PID=$procId, $($proc.ProcessName))" -ForegroundColor Red
        exit 10
    }
    Write-Host "[OK] 端口 $Port 可用" -ForegroundColor Green

    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($svc) {
        Write-Host "[FAIL] 服务 $ServiceName 已存在" -ForegroundColor Red
        exit 11
    }
    Write-Host "[OK] 服务名 $ServiceName 可用" -ForegroundColor Green

    $services = Get-Service | Select-Object Name, Status, StartType | ConvertTo-Json -Depth 2
    $ports = Get-NetTCPConnection -State Listen | Select-Object LocalPort, OwningProcess | ConvertTo-Json -Depth 2
    $smbShares = Get-SmbShare -ErrorAction SilentlyContinue | Select-Object Name, Path, Description | ConvertTo-Json -Depth 2
    $smbAccess = @{}
    Get-SmbShare -ErrorAction SilentlyContinue | ForEach-Object {
        $a = Get-SmbShareAccess -Name $_.Name -ErrorAction SilentlyContinue
        $smbAccess[$_.Name] = $a | Select-Object AccountName, AccessControlType, AccessRight | ConvertTo-Json -Depth 2
    }
    $envVars = Get-ChildItem Env: | Select-Object Name, Value | ConvertTo-Json -Depth 2
    $firewall = Get-NetFirewallRule | Where-Object { $_.Enabled -eq 'True' } | Select-Object DisplayName, Direction, Action | ConvertTo-Json -Depth 2

    $baseline = @{
        timestamp = $timestamp
        os = @{ caption = $osCaption; arch = $osArch }
        port = $Port
        serviceName = $ServiceName
        services = $services
        listeningPorts = $ports
        smbShares = $smbShares
        smbAccess = $smbAccess
        envVars = $envVars
        firewallRules = $firewall
    }

    Write-Baseline $baselinePath $baseline
    Write-Host "[OK] 基线快照已保存: $baselinePath" -ForegroundColor Green
    Write-Host "BASELINE_PATH=$baselinePath"
    exit 0
}
catch {
    Write-Host "[ERROR] $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}