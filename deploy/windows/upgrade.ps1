#Requires -RunAsAdministrator
<#
.SYNOPSIS
  ShareWeb 升级主流程（保留数据/配置/审计日志）
  退出码：0=成功；40=备份失败；41=迁移失败；42=健康检查失败（自动回滚）
#>
param(
    [Parameter(Mandatory=$true)][string]$PackagePath,
    [string]$ServiceName = "HTKIS-ShareWeb",
    [string]$InstallDir = "C:\HTKIS\ShareWeb",
    [int]$Port = 18888
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path $MyInvocation.MyCommand.Path -Parent
$timestamp = (Get-Date).ToString("yyyyMMdd-HHmmss")
$backupDir = Join-Path $InstallDir "data\backup\$timestamp"

try {
    Write-Host "=== ShareWeb 升级开始 ===" -ForegroundColor Cyan

    Write-Host "[1/7] 备份当前版本..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $backupDir -Force | Out-Null
    Copy-Item (Join-Path $InstallDir "bin") $backupDir -Recurse -Force
    Copy-Item (Join-Path $InstallDir "frontend") $backupDir -Recurse -Force
    $dbPath = Join-Path $InstallDir "data\shareweb.db"
    if (Test-Path $dbPath) { Copy-Item $dbPath $backupDir -Force }
    Write-Host "[OK] 备份至: $backupDir" -ForegroundColor Green

    Write-Host "[2/7] 停止服务..." -ForegroundColor Yellow
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    Write-Host "[OK] 服务已停止" -ForegroundColor Green

    Write-Host "[3/7] 替换程序文件..." -ForegroundColor Yellow
    $tempExtract = Join-Path $env:TEMP "shareweb-upgrade-$(Get-Random)"
    Expand-Archive -Path $PackagePath -DestinationPath $tempExtract -Force
    Copy-Item (Join-Path $tempExtract "bin\*") (Join-Path $InstallDir "bin") -Recurse -Force
    Copy-Item (Join-Path $tempExtract "frontend\*") (Join-Path $InstallDir "frontend") -Recurse -Force
    Remove-Item $tempExtract -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "[OK] 程序文件已替换" -ForegroundColor Green

    Write-Host "[4/7] 启动服务..." -ForegroundColor Yellow
    Start-Service -Name $ServiceName
    Start-Sleep -Seconds 3
    Write-Host "[OK] 服务已启动" -ForegroundColor Green

    Write-Host "[5/7] 执行数据库迁移..." -ForegroundColor Yellow
    $exePath = Join-Path $InstallDir "bin\shareweb.exe"
    $configPath = Join-Path $InstallDir "config\config.toml"
    & $exePath --migrate $configPath
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[FAIL] 迁移失败，自动回滚..." -ForegroundColor Red
        & powershell -File (Join-Path $scriptDir "rollback.ps1") -Timestamp $timestamp -ServiceName $ServiceName -InstallDir $InstallDir
        exit 41
    }
    Write-Host "[OK] 迁移完成" -ForegroundColor Green

    Write-Host "[6/7] 健康检查..." -ForegroundColor Yellow
    try {
        $response = Invoke-RestMethod -Uri "http://localhost:$Port/api/health" -TimeoutSec 10 -ErrorAction Stop
        Write-Host "[OK] 健康检查通过" -ForegroundColor Green
    } catch {
        Write-Host "[FAIL] 健康检查失败，自动回滚..." -ForegroundColor Red
        & powershell -File (Join-Path $scriptDir "rollback.ps1") -Timestamp $timestamp -ServiceName $ServiceName -InstallDir $InstallDir
        exit 42
    }

    Write-Host "[7/7] 升级完成" -ForegroundColor Green
    Write-Host "=== ShareWeb 升级成功 ===" -ForegroundColor Cyan
    exit 0
}
catch {
    Write-Host "[ERROR] 升级失败: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "尝试回滚..." -ForegroundColor Yellow
    & powershell -File (Join-Path $scriptDir "rollback.ps1") -Timestamp $timestamp -ServiceName $ServiceName -InstallDir $InstallDir
    exit 40
}