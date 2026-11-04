<#
.SYNOPSIS
  ShareWeb 安装包打包脚本
#>
param(
    [string]$Version = "1.0.1",
    [string]$SourceDir = "dist\windows",
    [string]$OutputFile = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrEmpty($OutputFile)) {
    $OutputFile = "shareweb-v$Version-windows-x64.zip"
}

Write-Host "打包 $SourceDir -> $OutputFile" -ForegroundColor Cyan

if (Test-Path $OutputFile) { Remove-Item $OutputFile -Force }
Compress-Archive -Path (Join-Path $SourceDir "*") -DestinationPath $OutputFile -Force

$hash = (Get-FileHash $OutputFile -Algorithm SHA256).Hash
"$hash  $(Split-Path $OutputFile -Leaf)" | Set-Content "SHA256SUMS" -Append

Write-Host "[OK] 打包完成: $OutputFile (SHA-256: $hash)" -ForegroundColor Green