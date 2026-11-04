<#
.SYNOPSIS
  ShareWeb Windows x64 Release 构建脚本
  退出码：0=成功；70=编译失败；71=前端构建失败
#>
param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$OutputDir = "dist\windows"
)

$ErrorActionPreference = "Stop"

Write-Host "=== ShareWeb Windows 构建开始 ===" -ForegroundColor Cyan

Write-Host "[1/4] 检查工具链..." -ForegroundColor Yellow
$rustVer = rustc --version 2>&1
if ($LASTEXITCODE -ne 0) { Write-Host "[FAIL] Rust 未安装" -ForegroundColor Red; exit 70 }
Write-Host "[OK] $rustVer" -ForegroundColor Green

$installedTargets = rustup target list --installed 2>&1
if ($installedTargets -notmatch $Target) {
    Write-Host "安装 target: $Target" -ForegroundColor Yellow
    rustup target add $Target
}

$nodeVer = node --version 2>&1
if ($LASTEXITCODE -ne 0) { Write-Host "[FAIL] Node.js 未安装" -ForegroundColor Red; exit 71 }
Write-Host "[OK] Node $nodeVer" -ForegroundColor Green

Write-Host "[2/4] 构建前端..." -ForegroundColor Yellow
Push-Location frontend
npm ci
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "[FAIL] npm ci 失败" -ForegroundColor Red; exit 71 }
npm run build
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "[FAIL] 前端构建失败" -ForegroundColor Red; exit 71 }
Pop-Location
Write-Host "[OK] 前端构建完成" -ForegroundColor Green

Write-Host "[3/4] 构建后端 (Release)..." -ForegroundColor Yellow
cargo build --release --target $Target
if ($LASTEXITCODE -ne 0) { Write-Host "[FAIL] Rust 编译失败" -ForegroundColor Red; exit 70 }
Write-Host "[OK] 后端编译完成" -ForegroundColor Green

Write-Host "[4/4] 收集产物..." -ForegroundColor Yellow
$exe = "target\$Target\release\shareweb.exe"
if (!(Test-Path $exe)) { $exe = "target\$Target\release\web-file-manager.exe" }
if (!(Test-Path $exe)) { Write-Host "[FAIL] EXE 未找到" -ForegroundColor Red; exit 70 }

if (Test-Path $OutputDir) { Remove-Item $OutputDir -Recurse -Force }
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $OutputDir "bin") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $OutputDir "frontend") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $OutputDir "migrations") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $OutputDir "deploy\windows") -Force | Out-Null

Copy-Item $exe (Join-Path $OutputDir "bin") -Force
Copy-Item "frontend\dist" (Join-Path $OutputDir "frontend\dist") -Recurse -Force
Copy-Item "migrations\*" (Join-Path $OutputDir "migrations") -Recurse -Force
Copy-Item "config.windows.toml" $OutputDir -Force
Copy-Item "deploy\windows\*.ps1" (Join-Path $OutputDir "deploy\windows") -Recurse -Force
Copy-Item "deploy\windows\README.md" (Join-Path $OutputDir "deploy\windows") -Force

$hash = (Get-FileHash (Join-Path $OutputDir "bin\$($exe | Split-Path -Leaf)") -Algorithm SHA256).Hash
"$hash  $($exe | Split-Path -Leaf)" | Set-Content (Join-Path $OutputDir "SHA256SUMS")

Write-Host "[OK] 产物已收集至 $OutputDir" -ForegroundColor Green
Write-Host "=== 构建完成 ===" -ForegroundColor Cyan
exit 0