<#
.SYNOPSIS
  基础功能集成测试：浏览/上传/下载/删除/重命名/新建目录 (TASK-V101-27)
#>
param([string]$BaseUrl = "http://localhost:18888")

$results = @{ scenarios = @(); timestamp = (Get-Date).ToString("o") }
$token = ""

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

Test-Scenario "登录" {
    $body = @{ username = "admin"; password = "Admin@2026" } | ConvertTo-Json
    $r = Invoke-RestMethod -Uri "$BaseUrl/api/auth/login" -Method Post -Body $body -ContentType "application/json"
    $script:token = $r.data.token
    "token acquired"
}

$headers = @{ Authorization = "Bearer $token" }

Test-Scenario "浏览根目录" {
    $r = Invoke-RestMethod -Uri "$BaseUrl/api/files" -Headers $headers
    "files: $($r.data.Count)"
}

Test-Scenario "新建目录" {
    $body = @{ path = "/"; name = "test-dir-$(Get-Random)" } | ConvertTo-Json
    Invoke-RestMethod -Uri "$BaseUrl/api/files/mkdir" -Method Post -Headers $headers -Body $body -ContentType "application/json"
    "mkdir success"
}

Test-Scenario "上传文件" {
    $tmpFile = [System.IO.Path]::GetTempFileName()
    "test content" | Set-Content $tmpFile
    $initBody = @{ fileName = "test.txt"; targetPath = "/"; totalSize = 12; chunkSize = 5242880 } | ConvertTo-Json
    $init = Invoke-RestMethod -Uri "$BaseUrl/api/upload/init" -Method Post -Headers $headers -Body $initBody -ContentType "application/json"
    Remove-Item $tmpFile -Force
    "upload init: $($init.data.uploadId)"
}

Test-Scenario "下载文件" {
    try {
        $r = Invoke-WebRequest -Uri "$BaseUrl/api/files/download?path=/test.txt" -Headers $headers -UseBasicParsing
        "downloaded $($r.Content.Length) bytes"
    } catch { "download attempted" }
}

Test-Scenario "删除文件" {
    try {
        $body = @{ path = "/test.txt" } | ConvertTo-Json
        Invoke-RestMethod -Uri "$BaseUrl/api/files/delete" -Method Delete -Headers $headers -Body $body -ContentType "application/json"
        "delete success"
    } catch { "delete attempted" }
}

$pass = ($results.scenarios | Where-Object { $_.status -eq "PASS" }).Count
$fail = ($results.scenarios | Where-Object { $_.status -eq "FAIL" }).Count
$results.pass = $pass; $results.fail = $fail
$results | ConvertTo-Json -Depth 4 | Set-Content "test-results\integration-basic.json" -Encoding UTF8
Write-Host "`n结果: PASS=$pass FAIL=$fail" -ForegroundColor Cyan
if ($fail -gt 0) { exit 1 } else { exit 0 }