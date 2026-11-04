<#
.SYNOPSIS
  首次启动管理员密码设置测试 (TASK-V101-29)
#>
param([string]$BaseUrl = "http://localhost:18888")

$results = @{ scenarios = @(); timestamp = (Get-Date).ToString("o") }

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

Test-Scenario "未初始化时业务接口返回403" {
    try {
        $r = Invoke-WebRequest -Uri "$BaseUrl/api/files" -UseBasicParsing -ErrorAction Stop
        throw "预期 403，实际 $($r.StatusCode)"
    } catch {
        if ($_.Exception.Response.StatusCode -ne 403) { throw "预期 403，实际 $($_.Exception.Message)" }
    }
    "403 SETUP_REQUIRED"
}

Test-Scenario "查询 setup 状态" {
    $r = Invoke-RestMethod -Uri "$BaseUrl/api/setup/status"
    if ($r.data.admin_initialized -ne $false) { throw "预期 admin_initialized=false" }
    "admin_initialized=false"
}

Test-Scenario "弱密码被拒绝" {
    try {
        $body = @{ username = "admin"; password = "123" } | ConvertTo-Json
        Invoke-RestMethod -Uri "$BaseUrl/api/setup/admin" -Method Post -Body $body -ContentType "application/json" -ErrorAction Stop
        throw "预期 400"
    } catch {
        if ($_.Exception.Response.StatusCode -ne 400) { throw "预期 400，实际 $($_.Exception.Message)" }
    }
    "400 WEAK_PASSWORD"
}

Test-Scenario "强密码设置成功" {
    $body = @{ username = "admin"; password = "Admin@2026" } | ConvertTo-Json
    $r = Invoke-RestMethod -Uri "$BaseUrl/api/setup/admin" -Method Post -Body $body -ContentType "application/json"
    "success"
}

Test-Scenario "重复设置返回409" {
    try {
        $body = @{ username = "admin"; password = "Admin@2026" } | ConvertTo-Json
        Invoke-RestMethod -Uri "$BaseUrl/api/setup/admin" -Method Post -Body $body -ContentType "application/json" -ErrorAction Stop
        throw "预期 409"
    } catch {
        if ($_.Exception.Response.StatusCode -ne 409) { throw "预期 409，实际 $($_.Exception.Message)" }
    }
    "409 SETUP_ALREADY_DONE"
}

Test-Scenario "初始化后业务接口可用" {
    $body = @{ username = "admin"; password = "Admin@2026" } | ConvertTo-Json
    $r = Invoke-RestMethod -Uri "$BaseUrl/api/auth/login" -Method Post -Body $body -ContentType "application/json"
    "login success"
}

$pass = ($results.scenarios | Where-Object { $_.status -eq "PASS" }).Count
$fail = ($results.scenarios | Where-Object { $_.status -eq "FAIL" }).Count
$results.pass = $pass; $results.fail = $fail
$results | ConvertTo-Json -Depth 4 | Set-Content "test-results\setup.json" -Encoding UTF8
Write-Host "`n结果: PASS=$pass FAIL=$fail" -ForegroundColor Cyan
if ($fail -gt 0) { exit 1 } else { exit 0 }