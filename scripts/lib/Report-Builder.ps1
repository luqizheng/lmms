# Report-Builder.ps1
# 解析 cargo/go 的 JSON 输出，生成 summary.md 与 fix-prompt.md

function Read-JsonLines {
    param([string]$Path)
    if (-not (Test-Path $Path)) { return @() }
    Get-Content $Path -Raw -Encoding utf8 |
        ForEach-Object { $_ -split "`n" } |
        Where-Object { $_.Trim() -ne "" } |
        ForEach-Object {
            try { $_ | ConvertFrom-Json -ErrorAction Stop } catch { $null }
        } |
        Where-Object { $_ -ne $null }
}

function Parse-CargoEvents {
    param([string]$JsonPath, [string]$LogPath = $null)
    $cases = @()
    # cargo --message-format=json emits only compiler events; libtest still writes
    # "test name ... ok/failed/ignored" to stdout. Parse that from .log.
    $textPath = if ($LogPath -and (Test-Path $LogPath)) { $LogPath } else { $JsonPath }
    if (Test-Path $textPath) {
        Get-Content $textPath -Encoding utf8 | ForEach-Object {
            $line = $_
            if ($line -match '^\s*test\s+([^\s\.]+?)(?:\s*\.\.\.\s*)?\s+(ok|FAILED|ignored|ok\.)$') {
                $name = $matches[1]
                $st   = $matches[2].TrimEnd('.')
                $status = switch ($st) { "ok" { "pass" } "FAILED" { "fail" } "ignored" { "ignored" } default { "pass" } }
                if (-not ($cases | Where-Object { $_.name -eq $name -and $_.status -eq $status })) {
                    $cases += [pscustomobject]@{
                        name = $name; status = $status; message = $null; exec_time_ms = 0
                    }
                }
            }
        }
    }
    ,$cases
}

function Parse-GoEvents {
    param([string]$JsonPath)
    $cases = @()
    Read-JsonLines $JsonPath | ForEach-Object {
        if ($_.Action -in @("pass", "fail", "skip")) {
            $status = switch ($_.Action) { "pass" { "pass" } "fail" { "fail" } "skip" { "ignored" } }
            $cases += [pscustomobject]@{
                name    = $_.Test
                status  = $status
                message = $_.Output
                exec_time_ms = if ($_.Elapsed) { [int]($_.Elapsed * 1000) } else { 0 }
            }
        }
    }
    ,$cases
}

function Get-ParserForService {
    param([string]$BaseName)
    $svc = $BaseName -replace "\.(unit|integration)$", ""
    if ($svc -in @("api-gateway", "token-server")) { return "cargo" }
    return "go"
}

function Build-Summary {
    param(
        [Parameter(Mandatory)][string]$RunDir
    )
    $summaryLines = @("# Test Run $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')", "", "## 总览", "")
    $summaryLines += "| 服务 | 类型 | 通过 | 失败 | 跳过 |"
    $summaryLines += "|------|------|------|------|------|"
    $allFailing = @()
    $jsonFiles = Get-ChildItem -Path $RunDir -Filter "*.json" -File
    foreach ($f in $jsonFiles) {
        $parser = Get-ParserForService -BaseName $f.BaseName
        $logPath = Join-Path $RunDir "$($f.BaseName).log"
        if ($parser -eq "cargo") { $cases = Parse-CargoEvents $f.FullName $logPath } else { $cases = Parse-GoEvents $f.FullName }
        $pass = @($cases | Where-Object status -eq "pass").Count
        $fail = @($cases | Where-Object status -eq "fail").Count
        $ign  = @($cases | Where-Object status -eq "ignored").Count
        $type = if ($f.BaseName -match "unit") { "unit" } else { "integration" }
        $svc  = $f.BaseName -replace "\.(unit|integration)$", ""
        $summaryLines += "| $svc | $type | $pass | $fail | $ign |"
        $cases | Where-Object status -eq "fail" | ForEach-Object {
            $allFailing += [pscustomobject]@{
                Service = $svc
                Type    = $type
                Name    = $_.name
                Message = $_.message
            }
        }
    }
    $summaryLines += ""
    if ($allFailing.Count -gt 0) {
        $summaryLines += "## 失败详情"
        $summaryLines += ""
        foreach ($fail in $allFailing) {
            $msg = if ($fail.Message) { ($fail.Message -split "`n" | Select-Object -First 20) -join "`n" } else { "(无消息)" }
            $summaryLines += "### $($fail.Service) [$($fail.Type)] :: $($fail.Name)"
            $summaryLines += "``````"
            $summaryLines += $msg
            $summaryLines += "``````"
            $summaryLines += ""
        }
    }
    $summaryPath = Join-Path $RunDir "summary.md"
    $summaryLines -join "`n" | Out-File -FilePath $summaryPath -Encoding utf8
    return @{ SummaryPath = $summaryPath; Failing = $allFailing }
}

function Build-FixPrompt {
    param(
        [Parameter(Mandatory)][string]$RunDir,
        [Parameter(Mandatory)][object[]]$Failing
    )
    $tmpl = Get-Content "d:\projects\coder-micro-services\scripts\prompts\fix-tests.template.md" -Raw -Encoding utf8
    $failingText = ($Failing | ForEach-Object { "- $($_.Service) [$($_.Type)] :: $($_.Name)" }) -join "`n"
    $errorText = ($Failing | ForEach-Object {
            $msg = if ($_.Message) { ($_.Message -split "`n" | Select-Object -First 20) -join "`n" } else { "(无消息)" }
            "### $($_.Service) [$($_.Type)] :: $($_.Name)`n``````$msg``````"
        }) -join "`n`n"
    $files = ($Failing | ForEach-Object {
            if ($_.Type -eq "integration") {
                if ($_.Service -in @("api-gateway", "token-server")) {
                    "services/$($_.Service)/tests/$($_.Name).rs"
                } else {
                    "services/$($_.Service)/$($_.Name)_test.go"
                }
            } else {
                if ($_.Service -in @("api-gateway", "token-server")) {
                    "services/$($_.Service)/src/$($_.Name).rs"
                } else {
                    "services/$($_.Service)/$($_.Name)_test.go"
                }
            }
        } | Select-Object -Unique) -join "`n"
    $rendered = $tmpl `
        -replace "\{\{FAILING_TESTS\}\}", $failingText `
        -replace "\{\{ERROR_DETAILS\}\}", $errorText `
        -replace "\{\{RELATED_FILES\}\}", $files
    $fixPath = Join-Path $RunDir "fix-prompt.md"
    $rendered | Out-File -FilePath $fixPath -Encoding utf8
    $latest = "d:\projects\coder-micro-services\reports\latest-fix-prompt.md"
    Copy-Item -Force $fixPath $latest
    return $fixPath
}
