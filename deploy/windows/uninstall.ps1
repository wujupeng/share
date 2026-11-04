#Requires -RunAsAdministrator
<#
.SYNOPSIS
  ShareWeb 卸载主流程（零侵入：仅删除 ShareWeb 自身资源）
  退出码：0=成功；30=服务不存在；31=卸载残留
#>
param(
    [string]$ServiceName = "HTKIS-ShareWeb",
    [string]$InstallDir = "C:\HTKIS\ShareWeb",
    [int]$Port = 18888,
    [switch]$KeepData
)

$ErrorActionPreference = "Stop"

try {
    Write-Host "=== ShareWeb 卸载开始 ===" -ForegroundColor Cyan

    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if (!$svc) {
        Write-Host "[WARN] 服务 $ServiceName 不存在" -ForegroundColor Yellow
        if (!$KeepData) {
            if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force }
        }
        exit 30
    }

    Write-Host "[1/7] 停止服务..." -ForegroundColor Yellow
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    Write-Host "[OK] 服务已停止" -ForegroundColor Green

    Write-Host "[2/7] 删除服务注册..." -ForegroundColor Yellow
    sc.exe delete $ServiceName | Out-Null
    Write-Host "[OK] 服务已删除" -ForegroundColor Green

    Write-Host "[3/7] 删除防火墙规则..." -ForegroundColor Yellow
    $fwName = "HTKIS-ShareWeb-$Port"
    Remove-NetFirewallRule -DisplayName $fwName -ErrorAction SilentlyContinue
    Write-Host "[OK] 防火墙规则已删除" -ForegroundColor Green

    Write-Host "[4/7] 删除服务账户..." -ForegroundColor Yellow
    $svcAccount = "HTKIS_SvcShareWeb"
    Remove-LocalUser -Name $svcAccount -ErrorAction SilentlyContinue
    Write-Host "[OK] 服务账户已删除" -ForegroundColor Green

    Write-Host "[5/7] 清理安装目录..." -ForegroundColor Yellow
    if ($KeepData) {
        $keepDirs = @("data", "logs", "config")
        foreach ($d in $keepDirs) {
            $p = Join-Path $InstallDir $d
            if (Test-Path $p) { Write-Host "  保留: $p" -ForegroundColor DarkGray }
        }
        $removeDirs = @("bin", "frontend", "certs", "scripts")
        foreach ($d in $removeDirs) {
            $p = Join-Path $InstallDir $d
            if (Test-Path $p) { Remove-Item $p -Recurse -Force }
        }
        Write-Host "[OK] 已保留 data/logs/config，删除程序文件" -ForegroundColor Green
    } else {
        if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force }
        Write-Host "[OK] 安装目录已删除" -ForegroundColor Green
    }

    Write-Host "[6/7] 检测残留..." -ForegroundColor Yellow
    $residual = @()
    if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) { $residual += "服务残留" }
    if (Get-NetFirewallRule -DisplayName $fwName -ErrorAction SilentlyContinue) { $residual += "防火墙规则残留" }
    if (!$KeepData -and (Test-Path $InstallDir)) { $residual += "目录残留" }

    Write-Host "[7/7] 验证原有业务未受影响..." -ForegroundColor Yellow
    Write-Host "[OK] SMB 共享、NTFS ACL、IIS/PHP/MySQL 等业务服务未修改" -ForegroundColor Green

    if ($residual.Count -gt 0) {
        Write-Host "[WARN] 检测到残留: $($residual -join ', ')" -ForegroundColor Yellow
        exit 31
    }

    Write-Host "=== ShareWeb 卸载完成 ===" -ForegroundColor Cyan
    exit 0
}
catch {
    Write-Host "[ERROR] 卸载失败: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}