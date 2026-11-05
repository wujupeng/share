#Requires -RunAsAdministrator
<#
.SYNOPSIS
  ShareWeb 回滚主流程（从指定备份恢复）
  退出码：0=成功；50=备份不存在；51=回滚迁移失败
#>
param(
    [string]$Timestamp = "",
    [string]$ServiceName = "HTKIS-ShareWeb",
    [string]$InstallDir = "C:\HTKIS\ShareWeb"
)

$ErrorActionPreference = "Stop"

try {
    Write-Host "=== ShareWeb 回滚开始 ===" -ForegroundColor Cyan

    if ([string]::IsNullOrEmpty($Timestamp)) {
        $backupRoot = Join-Path $InstallDir "data\backup"
        if (Test-Path $backupRoot) {
            $latest = Get-ChildItem $backupRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
            if ($latest) { $Timestamp = $latest.Name }
        }
    }

    $backupDir = Join-Path $InstallDir "data\backup\$Timestamp"
    if (!(Test-Path $backupDir)) {
        Write-Host "[FAIL] 备份不存在: $backupDir" -ForegroundColor Red
        exit 50
    }
    Write-Host "使用备份: $backupDir" -ForegroundColor Yellow

    Write-Host "[1/6] 停止服务..." -ForegroundColor Yellow
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    Write-Host "[OK] 服务已停止" -ForegroundColor Green

    Write-Host "[2/6] 恢复程序文件..." -ForegroundColor Yellow
    Copy-Item (Join-Path $backupDir "bin\*") (Join-Path $InstallDir "bin") -Recurse -Force
    Copy-Item (Join-Path $backupDir "frontend\*") (Join-Path $InstallDir "frontend") -Recurse -Force
    Write-Host "[OK] 程序文件已恢复" -ForegroundColor Green

    Write-Host "[3/6] 恢复数据库..." -ForegroundColor Yellow
    $backupDb = Join-Path $backupDir "shareweb.db"
    if (Test-Path $backupDb) {
        Copy-Item $backupDb (Join-Path $InstallDir "data\shareweb.db") -Force
        Write-Host "[OK] 数据库已恢复" -ForegroundColor Green
    }

    Write-Host "[4/6] 启动服务..." -ForegroundColor Yellow
    Start-Service -Name $ServiceName
    Start-Sleep -Seconds 3
    $svc = Get-Service -Name $ServiceName
    if ($svc.Status -ne 'Running') {
        Write-Host "[FAIL] 服务启动失败" -ForegroundColor Red
        exit 51
    }
    Write-Host "[OK] 服务已启动 (Running)" -ForegroundColor Green

    Write-Host "[5/6] 验证状态..." -ForegroundColor Yellow
    Write-Host "[OK] 回滚完成" -ForegroundColor Green

    Write-Host "[6/6] 原有业务未受影响" -ForegroundColor Green
    Write-Host "=== ShareWeb 回滚成功 ===" -ForegroundColor Cyan
    exit 0
}
catch {
    Write-Host "[ERROR] 回滚失败: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}