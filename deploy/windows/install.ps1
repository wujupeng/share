#Requires -RunAsAdministrator
<#
.SYNOPSIS
  ShareWeb 独立安装主流程
  退出码：0=成功；20=预检失败；21=服务注册失败；22=DB初始化失败；23=防火墙配置失败
#>
param(
    [Parameter(Mandatory=$true)][string]$PackagePath,
    [int]$Port = 18888,
    [string]$ServiceName = "HTKIS-ShareWeb",
    [string]$InstallDir = "C:\HTKIS\ShareWeb",
    [string]$CertPath = ""
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path $MyInvocation.MyCommand.Path -Parent

try {
    Write-Host "=== ShareWeb 安装开始 ===" -ForegroundColor Cyan

    Write-Host "[1/10] 执行预检..." -ForegroundColor Yellow
    $preflightResult = & powershell -File (Join-Path $scriptDir "preflight.ps1") -Port $Port -ServiceName $ServiceName -InstallDir $InstallDir
    $preflightExit = $LASTEXITCODE
    if ($preflightExit -ne 0) {
        Write-Host "[FAIL] 预检失败 (退出码=$preflightExit)" -ForegroundColor Red
        exit 20
    }
    $baselineLine = $preflightResult | Where-Object { $_ -match "^BASELINE_PATH=" }
    $baselinePath = if ($baselineLine) { $baselineLine.Split('=')[1] } else { "" }
    Write-Host "[OK] 预检通过" -ForegroundColor Green

    Write-Host "[2/10] 创建目录结构..." -ForegroundColor Yellow
    $dirs = @("bin","frontend","config","data","data\upload_tmp","data\logs","logs","certs","scripts")
    foreach ($d in $dirs) {
        $p = Join-Path $InstallDir $d
        if (!(Test-Path $p)) { New-Item -ItemType Directory -Path $p -Force | Out-Null }
    }
    Write-Host "[OK] 目录结构已创建" -ForegroundColor Green

    Write-Host "[3/10] 解压安装包..." -ForegroundColor Yellow
    if (!(Test-Path $PackagePath)) { throw "安装包不存在: $PackagePath" }
    $tempExtract = Join-Path $env:TEMP "shareweb-install-$(Get-Random)"
    Expand-Archive -Path $PackagePath -DestinationPath $tempExtract -Force
    Copy-Item (Join-Path $tempExtract "bin\*") (Join-Path $InstallDir "bin") -Recurse -Force
    Copy-Item (Join-Path $tempExtract "frontend\*") (Join-Path $InstallDir "frontend") -Recurse -Force
    Copy-Item (Join-Path $tempExtract "migrations\*") (Join-Path $InstallDir "data") -Recurse -Force -ErrorAction SilentlyContinue
    if (Test-Path (Join-Path $tempExtract "config.windows.toml")) {
        Copy-Item (Join-Path $tempExtract "config.windows.toml") (Join-Path $InstallDir "config\config.toml") -Force
    }
    if ($CertPath -and (Test-Path $CertPath)) {
        Copy-Item $CertPath (Join-Path $InstallDir "certs") -Recurse -Force
    }
    Remove-Item $tempExtract -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "[OK] 文件已复制" -ForegroundColor Green

    Write-Host "[4/10] 渲染配置..." -ForegroundColor Yellow
    $configPath = Join-Path $InstallDir "config\config.toml"
    $jwtSecret = -join ((48..122) | Get-Random -Count 48 | ForEach-Object {[char]$_})
    $content = Get-Content $configPath -Encoding UTF8
    $content = $content -replace 'change-me-in-production', $jwtSecret
    $content = $content -replace 'C:\\HTKIS\\ShareWeb', $InstallDir
    $content | Set-Content $configPath -Encoding UTF8
    Write-Host "[OK] 配置已渲染" -ForegroundColor Green

    Write-Host "[5/10] 创建服务账户..." -ForegroundColor Yellow
    $svcAccount = "HTKIS_SvcShareWeb"
    $existingUser = Get-LocalUser -Name $svcAccount -ErrorAction SilentlyContinue
    if (!$existingUser) {
        $password = -join ((48..122) | Get-Random -Count 32 | ForEach-Object {[char]$_}) | ConvertTo-SecureString -AsPlainText -Force
        New-LocalUser -Name $svcAccount -Password $password -Description "ShareWeb 服务账户" -PasswordNeverExpires | Out-Null
    }
    Write-Host "[OK] 服务账户就绪" -ForegroundColor Green

    Write-Host "[6/10] 初始化数据库..." -ForegroundColor Yellow
    $exePath = Join-Path $InstallDir "bin\shareweb.exe"
    & $exePath --migrate $configPath
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[FAIL] 数据库初始化失败" -ForegroundColor Red
        exit 22
    }
    Write-Host "[OK] 数据库已初始化" -ForegroundColor Green

    Write-Host "[7/10] 注册 Windows 服务..." -ForegroundColor Yellow
    $existingSvc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($existingSvc) { Remove-Service -Name $ServiceName -ErrorAction SilentlyContinue }
    $binaryPath = "`"$exePath`" --service `"$configPath`""
    sc.exe create $ServiceName binPath= $binaryPath start= auto | Out-Null
    sc.exe description $ServiceName "HTKIS ShareWeb 企业文件共享管理系统" | Out-Null
    sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/10000/restart/30000 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[FAIL] 服务注册失败" -ForegroundColor Red
        exit 21
    }
    Write-Host "[OK] 服务已注册" -ForegroundColor Green

    Write-Host "[8/10] 配置防火墙..." -ForegroundColor Yellow
    $fwName = "HTKIS-ShareWeb-$Port"
    $existingFw = Get-NetFirewallRule -DisplayName $fwName -ErrorAction SilentlyContinue
    if (!$existingFw) {
        New-NetFirewallRule -DisplayName $fwName -Direction Inbound -Protocol TCP -LocalPort $Port -Action Allow -Profile Domain,Private | Out-Null
    }
    Write-Host "[OK] 防火墙规则已添加" -ForegroundColor Green

    Write-Host "[9/10] 启动服务..." -ForegroundColor Yellow
    Start-Service -Name $ServiceName
    Start-Sleep -Seconds 3
    $svc = Get-Service -Name $ServiceName
    if ($svc.Status -ne 'Running') {
        throw "服务启动后状态异常: $($svc.Status)"
    }
    Write-Host "[OK] 服务已启动 (Running)" -ForegroundColor Green

    Write-Host "[10/10] 差异验证..." -ForegroundColor Yellow
    & powershell -File (Join-Path $scriptDir "verify.ps1") -BaselinePath $baselinePath -InstallDir $InstallDir -Port $Port -ServiceName $ServiceName
    $verifyExit = $LASTEXITCODE
    if ($verifyExit -ne 0) {
        Write-Host "[WARN] 差异验证发现变更，请检查报告" -ForegroundColor Yellow
    } else {
        Write-Host "[OK] 零侵入验证通过" -ForegroundColor Green
    }

    Write-Host "=== ShareWeb 安装完成 ===" -ForegroundColor Cyan
    Write-Host "访问地址: http://localhost:$Port" -ForegroundColor Green
    exit 0
}
catch {
    Write-Host "[ERROR] 安装失败: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "正在回滚..." -ForegroundColor Yellow
    if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        sc.exe delete $ServiceName | Out-Null
    }
    exit 1
}