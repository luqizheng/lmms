# 测试编排与 AI 协同修复 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立 PowerShell 编排工具链，实现 Docker 启动 → DB 重置 → 单元/集成测试 → 报告 → AI 提示词生成 → 修复后重跑的完整链路。

**Architecture:** 单一 PS1 入口 + lib 拆分（Docker/Test/Report/Validator）+ 模板驱动提示词。复用现有 `infrastructure/docker-compose.test.yaml`。

**Tech Stack:** PowerShell 5.1、Docker Compose v2、cargo test (`--message-format=json`)、go test (`-json`)、PowerShell-Yaml（可选）、psyaml 内置解析（无需外部模块）。

**参考文档：** `docs/superpowers/specs/2026-06-08-test-orchestrator-design.md`

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `scripts/test-orchestrator.ps1` | 入口；解析子命令、参数；串联调用 |
| `scripts/lib/Docker-Helper.ps1` | compose up/down/restart、healthcheck 等待、exec 封装 |
| `scripts/lib/Test-Runner.ps1` | 调用 cargo/go；输出 JSON；解析统一 Run 模型 |
| `scripts/lib/Report-Builder.ps1` | 解析 JSON 日志 → summary.md + fix-prompt.md |
| `scripts/lib/Spec-Validator.ps1` | YAML 字段校验（不依赖外部模块） |
| `scripts/lib/Prompt-Renderer.ps1` | 模板占位符替换 |
| `scripts/prompts/fix-tests.template.md` | 修复提示词模板 |
| `scripts/prompts/spec-to-code.template.md` | 生成代码提示词模板 |
| `tests/specs/.gitkeep` | 跟踪 specs 目录 |
| `tests/specs/auth-service/login.yaml` | 示例规格（演示用） |
| `reports/.gitkeep` | 跟踪 reports 目录 |
| `README.md` | 在 scripts/ 下加一段使用说明（可选） |

约束：不创建 `tests/generated/` 目录，生成代码直接放回各服务 `tests/`。

---

## Task 1: 创建目录与跟踪文件

**Files:**
- Create: `scripts/lib/.gitkeep`
- Create: `scripts/prompts/.gitkeep`
- Create: `tests/specs/.gitkeep`
- Create: `reports/.gitkeep`

- [ ] **Step 1: 创建目录**

Run:
```powershell
$root = "d:\projects\coder-micro-services"
New-Item -ItemType Directory -Force -Path "$root\scripts\lib"
New-Item -ItemType Directory -Force -Path "$root\scripts\prompts"
New-Item -ItemType Directory -Force -Path "$root\tests\specs"
New-Item -ItemType Directory -Force -Path "$root\reports"
```

Expected: 无错误，4 个目录存在。

- [ ] **Step 2: 添加 .gitkeep 占位**

写入空文件到四个目录（让 git 跟踪）：
```powershell
$dirs = @("scripts\lib", "scripts\prompts", "tests\specs", "reports")
foreach ($d in $dirs) {
    $path = Join-Path "d:\projects\coder-micro-services" $d
    "" | Out-File -FilePath (Join-Path $path ".gitkeep") -Encoding utf8
}
```

Expected: 4 个 `.gitkeep` 文件已创建。

- [ ] **Step 3: 提交**

```bash
git add scripts/lib/.gitkeep scripts/prompts/.gitkeep tests/specs/.gitkeep reports/.gitkeep
git commit -m "chore: scaffold scripts/lib, scripts/prompts, tests/specs, reports dirs"
```

---

## Task 2: 编写提示词模板

**Files:**
- Create: `scripts/prompts/fix-tests.template.md`
- Create: `scripts/prompts/spec-to-code.template.md`

- [ ] **Step 1: 写 fix-tests 模板**

写入 `scripts/prompts/fix-tests.template.md`：
```markdown
# 任务

修复以下失败的测试。**仅修改各服务下的测试代码**：
- Go: `services/<service>/*_test.go`
- Rust: `services/<service>/tests/*.rs`

**禁止**修改 `services/`、`libs/`、`sdk/` 下非 tests 目录的业务源码。

## 失败用例

{{FAILING_TESTS}}

## 错误详情

{{ERROR_DETAILS}}

## 相关文件

{{RELATED_FILES}}

## 完成后

运行：
```powershell
./scripts/test-orchestrator.ps1 reset-db
./scripts/test-orchestrator.ps1 test-integration
```
```

- [ ] **Step 2: 写 spec-to-code 模板**

写入 `scripts/prompts/spec-to-code.template.md`：
```markdown
# 任务

根据以下 YAML 测试规格，生成对应语言的测试代码：
- Go 服务 → `services/<service>/<name>_test.go`（与被测代码同包）
- Rust 服务 → `services/<service>/tests/<name>.rs`

**仅输出代码块**，不要解释。

## 规格

{{SPECS}}
```

- [ ] **Step 3: 提交**

```bash
git add scripts/prompts/
git commit -m "feat(scripts): add fix-tests and spec-to-code prompt templates"
```

---

## Task 3: 编写 Docker-Helper.ps1

**Files:**
- Create: `scripts/lib/Docker-Helper.ps1`

- [ ] **Step 1: 写入脚本**

写入 `scripts/lib/Docker-Helper.ps1`：
```powershell
# Docker-Helper.ps1
# 封装 docker compose 与 healthcheck 等待

$ErrorActionPreference = "Stop"
$ComposeFile = "d:\projects\coder-micro-services\infrastructure\docker-compose.test.yaml"

function Get-ComposeArgs {
    @("-f", $ComposeFile)
}

function Start-TestStack {
    Write-Host "[docker] starting test stack..." -ForegroundColor Cyan
    & docker compose @(Get-ComposeArgs) up -d --build 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) { throw "docker compose up failed" }
    Wait-AllHealthy
}

function Stop-TestStack {
    Write-Host "[docker] stopping test stack..." -ForegroundColor Cyan
    & docker compose @(Get-ComposeArgs) down -v 2>&1 | ForEach-Object { Write-Host $_ }
}

function Restart-Services {
    param([string[]]$Services)
    Write-Host "[docker] restarting: $($Services -join ', ')" -ForegroundColor Cyan
    & docker compose @(Get-ComposeArgs) restart @Services 2>&1 | ForEach-Object { Write-Host $_ }
    Wait-AllHealthy
}

function Reset-Database {
    Write-Host "[db] dropping and recreating superpowers..." -ForegroundColor Cyan
    & docker exec mysql mysql -uroot -prootpass -e "DROP DATABASE IF EXISTS superpowers; CREATE DATABASE superpowers CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"
    if ($LASTEXITCODE -ne 0) { throw "mysql reset failed" }

    Write-Host "[db] flushing redis..." -ForegroundColor Cyan
    & docker exec redis redis-cli FLUSHALL
    if ($LASTEXITCODE -ne 0) { throw "redis flush failed" }

    Restart-Services -Services @("auth-service-test", "user-service-test")
}

function Wait-AllHealthy {
    param([int]$TimeoutSec = 120)
    $services = @("mysql", "redis", "consul", "api-gateway-test", "token-server-test", "auth-service-test", "user-service-test")
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $allHealthy = $true
        foreach ($s in $services) {
            $status = & docker inspect --format='{{.State.Health.Status}}' $s 2>$null
            if ($status -ne "healthy") {
                $allHealthy = $false
                break
            }
        }
        if ($allHealthy) {
            Write-Host "[docker] all services healthy" -ForegroundColor Green
            return
        }
        Start-Sleep -Seconds 3
    }
    throw "healthcheck timeout after ${TimeoutSec}s"
}

Export-ModuleMember -Function Start-TestStack, Stop-TestStack, Reset-Database, Restart-Services, Wait-AllHealthy
```

注：脚本末尾 `Export-ModuleMember` 在 dot-source 模式下不生效；改用变量方式（见 Task 7）。但保留此文件以备模块化时使用。

- [ ] **Step 2: 语法检查**

Run:
```powershell
$null = [System.Management.Automation.Language.Parser]::ParseFile("d:\projects\coder-micro-services\scripts\lib\Docker-Helper.ps1", [ref]$null, [ref]$errs)
if ($errs) { $errs | ForEach-Object { Write-Host $_ } } else { Write-Host "OK" }
```

Expected: 输出 `OK`。

- [ ] **Step 3: 提交**

```bash
git add scripts/lib/Docker-Helper.ps1
git commit -m "feat(scripts): add Docker-Helper with start/stop/reset-db/wait"
```

---

## Task 4: 编写 Test-Runner.ps1

**Files:**
- Create: `scripts/lib/Test-Runner.ps1`

- [ ] **Step 1: 写入脚本**

写入 `scripts/lib/Test-Runner.ps1`：
```powershell
# Test-Runner.ps1
# 封装 cargo test 与 go test，输出统一 JSON

$Root = "d:\projects\coder-micro-services"

$ServiceMap = @{
    "api-gateway"  = @{ Lang = "rust"; Path = "services\api-gateway" }
    "token-server"  = @{ Lang = "rust"; Path = "services\token-server" }
    "auth-service"  = @{ Lang = "go";    Path = "services\auth-service" }
    "user-service"  = @{ Lang = "go";    Path = "services\user-service" }
}

function Invoke-ServiceTests {
    param(
        [Parameter(Mandatory)][string]$Service,
        [Parameter(Mandatory)][ValidateSet("unit", "integration")][string]$Type,
        [Parameter(Mandatory)][string]$OutputJson
    )
    $cfg = $ServiceMap[$Service]
    if (-not $cfg) { throw "unknown service: $Service" }
    $svcRoot = Join-Path $Root $cfg.Path
    Push-Location $svcRoot
    try {
        if ($cfg.Lang -eq "rust") {
            if ($Type -eq "unit") {
                & cargo test --lib --message-format=json 2>&1 | Tee-Object -FilePath $OutputJson | Out-Null
            } else {
                & cargo test --test '*' --message-format=json 2>&1 | Tee-Object -FilePath $OutputJson | Out-Null
            }
        } else {
            if ($Type -eq "unit") {
                & go test -short -json ./... 2>&1 | Tee-Object -FilePath $OutputJson | Out-Null
            } else {
                & go test -json ./... 2>&1 | Tee-Object -FilePath $OutputJson | Out-Null
            }
        }
    } finally {
        Pop-Location
    }
}

function Get-ServiceList { @("api-gateway", "token-server", "auth-service", "user-service") }

Export-ModuleMember -Function Invoke-ServiceTests, Get-ServiceList
```

- [ ] **Step 2: 语法检查**

```powershell
$errs = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile("d:\projects\coder-micro-services\scripts\lib\Test-Runner.ps1", [ref]$null, [ref]$errs)
if ($errs) { $errs | ForEach-Object { Write-Host $_ } } else { Write-Host "OK" }
```

Expected: `OK`。

- [ ] **Step 3: 提交**

```bash
git add scripts/lib/Test-Runner.ps1
git commit -m "feat(scripts): add Test-Runner for cargo/go with json output"
```

---

## Task 5: 编写 Report-Builder.ps1

**Files:**
- Create: `scripts/lib/Report-Builder.ps1`

- [ ] **Step 1: 写入脚本**

写入 `scripts/lib/Report-Builder.ps1`：
```powershell
# Report-Builder.ps1
# 解析 cargo/go 的 JSON 输出，生成 summary.md 与 fix-prompt.md

function Read-JsonLines {
    param([string]$Path)
    Get-Content $Path -Raw -Encoding utf8 |
        ForEach-Object { $_ -split "`n" } |
        Where-Object { $_.Trim() -ne "" } |
        ForEach-Object {
            try { $_ | ConvertFrom-Json -ErrorAction Stop } catch { $null }
        } |
        Where-Object { $_ -ne $null }
}

function Parse-CargoEvents {
    param([string]$JsonPath)
    $cases = @()
    Read-JsonLines $JsonPath | ForEach-Object {
        if ($_.type -eq "test" -and $_.event -in @("ok", "failed", "ignored")) {
            $cases += [pscustomobject]@{
                name     = $_.name
                status   = switch ($_.event) { "ok" { "pass" } "failed" { "fail" } "ignored" { "ignored" } }
                message  = $_.message
                exec_time_ms = [int]($_.exec_time * 1000)
            }
        }
    }
    ,$cases
}

function Parse-GoEvents {
    param([string]$JsonPath)
    $cases = @()
    Read-JsonLines $JsonPath | ForEach-Object {
        if ($_.Action -eq "pass" -or $_.Action -in @("fail", "skip")) {
            $status = switch ($_.Action) { "pass" { "pass" } "fail" { "fail" } "skip" { "ignored" } }
            $cases += [pscustomobject]@{
                name    = $_.Test
                status  = $status
                message = $_.Output
                exec_time_ms = [int](($_.Elapsed * 1000))
            }
        }
    }
    ,$cases
}

function Get-ParserForService {
    param([string]$ServicePath)
    if ($ServicePath -match "services[\\/]api-gateway|services[\\/]token-server") {
        return "cargo"
    }
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
        $parser = Get-ParserForService -ServicePath $f.Name
        if ($parser -eq "cargo") { $cases = Parse-CargoEvents $f.FullName } else { $cases = Parse-GoEvents $f.FullName }
        $pass = ($cases | Where-Object status -eq "pass").Count
        $fail = ($cases | Where-Object status -eq "fail").Count
        $ign  = ($cases | Where-Object status -eq "ignored").Count
        $type = if ($f.Name -match "unit") { "unit" } else { "integration" }
        $svc  = $f.BaseName -replace "\.(unit|integration)$", ""
        $summaryLines += "| $svc | $type | $pass | $fail | $ign |"
        $allFailing += $cases | Where-Object status -eq "fail" | ForEach-Object {
            [pscustomobject]@{ Service = $svc; Type = $type; Name = $_.name; Message = $_.message }
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
                "services/$($_.Service)/src/$($_.Name).rs（或 *_test.go）"
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

Export-ModuleMember -Function Build-Summary, Build-FixPrompt
```

- [ ] **Step 2: 语法检查**

```powershell
$errs = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile("d:\projects\coder-micro-services\scripts\lib\Report-Builder.ps1", [ref]$null, [ref]$errs)
if ($errs) { $errs | ForEach-Object { Write-Host $_ } } else { Write-Host "OK" }
```

Expected: `OK`。

- [ ] **Step 3: 提交**

```bash
git add scripts/lib/Report-Builder.ps1
git commit -m "feat(scripts): add Report-Builder for json logs to summary+fix-prompt"
```

---

## Task 6: 编写 Spec-Validator.ps1

**Files:**
- Create: `scripts/lib/Spec-Validator.ps1`

- [ ] **Step 1: 写入脚本**

写入 `scripts/lib/Spec-Validator.ps1`：
```powershell
# Spec-Validator.ps1
# 校验 tests/specs/ 下 YAML 文件的必填字段
# 不依赖外部模块；用手写解析识别顶层 key 与必需字段

$Required = @("name", "service", "type", "description", "steps")
$AllowedServices = @("auth-service", "user-service", "api-gateway", "token-server")
$AllowedTypes = @("unit", "integration")
$NamePattern = "^[a-z][a-z0-9_]*$"

function Get-YamlValue {
    param([string[]]$Lines, [string]$Key)
    $re = "^\s*$Key\s*:\s*(.+?)\s*$"
    foreach ($line in $Lines) {
        if ($line -match $re) { return $Matches[1].Trim().Trim('"').Trim("'") }
    }
    return $null
}

function Test-SpecFile {
    param([string]$Path)
    $errors = @()
    $content = Get-Content $Path -Raw -Encoding utf8
    $lines = $content -split "`n"

    $name = Get-YamlValue $lines "name"
    $service = Get-YamlValue $lines "service"
    $type = Get-YamlValue $lines "type"
    $description = Get-YamlValue $lines "description"

    foreach ($k in $Required) {
        $v = Get-YamlValue $lines $k
        if ([string]::IsNullOrWhiteSpace($v)) {
            $errors += "$Path : missing required field '$k'"
        }
    }
    if ($name -and $name -notmatch $NamePattern) {
        $errors += "$Path : name '$name' must match $NamePattern"
    }
    if ($service -and $AllowedServices -notcontains $service) {
        $errors += "$Path : service must be one of $($AllowedServices -join ', ')"
    }
    if ($type -and $AllowedTypes -notcontains $type) {
        $errors += "$Path : type must be 'unit' or 'integration'"
    }
    if (-not ($content -match "steps\s*:")) {
        $errors += "$Path : 'steps:' block not found"
    }
    return $errors
}

function Invoke-SpecValidation {
    $root = "d:\projects\coder-micro-services\tests\specs"
    $all = Get-ChildItem -Path $root -Recurse -Filter "*.yaml" -File
    $allErrors = @()
    foreach ($f in $all) {
        $allErrors += Test-SpecFile -Path $f.FullName
    }
    if ($allErrors.Count -gt 0) {
        $allErrors | ForEach-Object { Write-Host $_ -ForegroundColor Red }
        exit 1
    }
    Write-Host "[validate] $($all.Count) spec files OK" -ForegroundColor Green
}

Export-ModuleMember -Function Invoke-SpecValidation
```

- [ ] **Step 2: 语法检查**

```powershell
$errs = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile("d:\projects\coder-micro-services\scripts\lib\Spec-Validator.ps1", [ref]$null, [ref]$errs)
if ($errs) { $errs | ForEach-Object { Write-Host $_ } } else { Write-Host "OK" }
```

Expected: `OK`。

- [ ] **Step 3: 提交**

```bash
git add scripts/lib/Spec-Validator.ps1
git commit -m "feat(scripts): add Spec-Validator for YAML field checks"
```

---

## Task 7: 编写 Prompt-Renderer.ps1

**Files:**
- Create: `scripts/lib/Prompt-Renderer.ps1`

- [ ] **Step 1: 写入脚本**

写入 `scripts/lib/Prompt-Renderer.ps1`：
```powershell
# Prompt-Renderer.ps1
# 渲染 spec-to-code 提示词

function Get-Specs {
    $root = "d:\projects\coder-micro-services\tests\specs"
    if (-not (Test-Path $root)) { return @() }
    Get-ChildItem -Path $root -Recurse -Filter "*.yaml" -File
}

function Render-SpecToCodePrompt {
    $tmpl = Get-Content "d:\projects\coder-micro-services\scripts\prompts\spec-to-code.template.md" -Raw -Encoding utf8
    $specsText = ""
    foreach ($f in (Get-Specs)) {
        $specsText += "`n---`nFile: $($f.FullName)`n`n"
        $specsText += (Get-Content $f.FullName -Raw -Encoding utf8)
    }
    $rendered = $tmpl -replace "\{\{SPECS\}\}", $specsText
    $out = "d:\projects\coder-micro-services\reports\spec-to-code-prompt.md"
    $rendered | Out-File -FilePath $out -Encoding utf8
    Write-Host "[prompt] rendered to $out" -ForegroundColor Green
}

Export-ModuleMember -Function Render-SpecToCodePrompt
```

- [ ] **Step 2: 语法检查**

```powershell
$errs = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile("d:\projects\coder-micro-services\scripts\lib\Prompt-Renderer.ps1", [ref]$null, [ref]$errs)
if ($errs) { $errs | ForEach-Object { Write-Host $_ } } else { Write-Host "OK" }
```

Expected: `OK`。

- [ ] **Step 3: 提交**

```bash
git add scripts/lib/Prompt-Renderer.ps1
git commit -m "feat(scripts): add Prompt-Renderer for spec-to-code"
```

---

## Task 8: 编写入口 test-orchestrator.ps1

**Files:**
- Create: `scripts/test-orchestrator.ps1`

- [ ] **Step 1: 写入脚本**

写入 `scripts/test-orchestrator.ps1`：
```powershell
# test-orchestrator.ps1
# 入口：Docker 启动、DB 重置、测试运行、报告生成、AI 提示词
#
# 用法：
#   ./scripts/test-orchestrator.ps1 start
#   ./scripts/test-orchestrator.ps1 stop
#   ./scripts/test-orchestrator.ps1 reset-db
#   ./scripts/test-orchestrator.ps1 test-unit [-Service <name>]
#   ./scripts/test-orchestrator.ps1 test-integration [-Service <name>]
#   ./scripts/test-orchestrator.ps1 test-all [-Service <name>]
#   ./scripts/test-orchestrator.ps1 report
#   ./scripts/test-orchestrator.ps1 fix-prompt
#   ./scripts/test-orchestrator.ps1 generate-prompt
#   ./scripts/test-orchestrator.ps1 validate-specs
#   ./scripts/test-orchestrator.ps1 loop

param(
    [Parameter(Mandatory, Position = 0)][string]$Command,
    [string]$Service,
    [string]$Timestamp
)

$ErrorActionPreference = "Stop"
$Root = "d:\projects\coder-micro-services"
$LibDir = Join-Path $Root "scripts\lib"

# dot-source 所有 lib（避免 Export-ModuleMember 复杂性）
Get-ChildItem -Path $LibDir -Filter "*.ps1" -File | ForEach-Object {
    . $_.FullName
}

function Get-RunDir {
    param([string]$Ts = $Timestamp)
    if (-not $Ts) { $Ts = Get-Date -Format "yyyyMMdd-HHmmss" }
    Join-Path $Root "reports\run-$Ts"
}

function Initialize-RunDir {
    param([string]$Dir)
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    return $Dir
}

function Run-TestPhase {
    param(
        [Parameter(Mandatory)][ValidateSet("unit", "integration")][string]$Type,
        [string]$Svc
    )
    $runDir = Initialize-RunDir -Dir (Get-RunDir)
    $services = if ($Svc) { @($Svc) } else { Get-ServiceList }
    foreach ($s in $services) {
        $json = Join-Path $runDir "$s.$Type.json"
        $log = Join-Path $runDir "$s.$Type.log"
        Write-Host "[test] $s ($Type) -> $json" -ForegroundColor Cyan
        try {
            Invoke-ServiceTests -Service $s -Type $Type -OutputJson $json 2>&1 | Tee-Object -FilePath $log | Out-Null
        } catch {
            Write-Host "[test] $s ($Type) FAILED to run: $_" -ForegroundColor Red
        }
    }
    return $runDir
}

switch ($Command) {
    "start"          { Start-TestStack }
    "stop"           { Stop-TestStack }
    "reset-db"       { Reset-Database }
    "test-unit"      { Run-TestPhase -Type "unit" -Svc $Service }
    "test-integration" { Run-TestPhase -Type "integration" -Svc $Service }
    "test-all" {
        $runDir = Run-TestPhase -Type "unit" -Svc $Service
        Run-TestPhase -Type "integration" -Svc $Service | Out-Null
        Write-Host "[done] logs: $runDir" -ForegroundColor Green
    }
    "report" {
        $runDir = Get-RunDir
        if (-not (Test-Path $runDir)) { throw "no run dir: $runDir" }
        $result = Build-Summary -RunDir $runDir
        Write-Host "[report] $($result.SummaryPath)" -ForegroundColor Green
        if ($result.Failing.Count -gt 0) {
            $fix = Build-FixPrompt -RunDir $runDir -Failing $result.Failing
            Write-Host "[report] failures detected, fix-prompt: $fix" -ForegroundColor Yellow
            Write-Host "[report] latest: reports\latest-fix-prompt.md" -ForegroundColor Yellow
        }
    }
    "fix-prompt" {
        $runDir = Get-RunDir
        $result = Build-Summary -RunDir $runDir
        if ($result.Failing.Count -eq 0) {
            Write-Host "[fix-prompt] no failures in latest run" -ForegroundColor Green
        } else {
            $fix = Build-FixPrompt -RunDir $runDir -Failing $result.Failing
            Write-Host "[fix-prompt] $fix" -ForegroundColor Yellow
        }
    }
    "generate-prompt" { Render-SpecToCodePrompt }
    "validate-specs"  { Invoke-SpecValidation }
    "loop" {
        Start-TestStack
        Reset-Database
        $runDir = Run-TestPhase -Type "unit" -Svc $Service
        Run-TestPhase -Type "integration" -Svc $Service | Out-Null
        $result = Build-Summary -RunDir $runDir
        Write-Host "[report] $($result.SummaryPath)" -ForegroundColor Green
        if ($result.Failing.Count -gt 0) {
            $fix = Build-FixPrompt -RunDir $runDir -Failing $result.Failing
            Write-Host "[loop] failures detected." -ForegroundColor Yellow
            Write-Host "[loop] open: $fix" -ForegroundColor Yellow
            Write-Host "[loop] also: reports\latest-fix-prompt.md" -ForegroundColor Yellow
            Write-Host "[loop] (paste to AI; after AI edits, re-run: ./scripts/test-orchestrator.ps1 reset-db && ./scripts/test-orchestrator.ps1 test-integration)" -ForegroundColor Yellow
        } else {
            Write-Host "[loop] all tests passed" -ForegroundColor Green
        }
    }
    default {
        Write-Host "unknown command: $Command" -ForegroundColor Red
        Write-Host "valid: start, stop, reset-db, test-unit, test-integration, test-all, report, fix-prompt, generate-prompt, validate-specs, loop"
        exit 1
    }
}
```

- [ ] **Step 2: 语法检查**

```powershell
$errs = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile("d:\projects\coder-micro-services\scripts\test-orchestrator.ps1", [ref]$null, [ref]$errs)
if ($errs) { $errs | ForEach-Object { Write-Host $_ } } else { Write-Host "OK" }
```

Expected: `OK`。

- [ ] **Step 3: 烟雾测试（不启动 docker）**

```powershell
cd d:\projects\coder-micro-services
.\scripts\test-orchestrator.ps1 validate-specs
```

Expected: `[validate] 0 spec files OK`（空目录无错误）。

- [ ] **Step 4: 提交**

```bash
git add scripts/test-orchestrator.ps1
git commit -m "feat(scripts): add test-orchestrator entry point with all subcommands"
```

---

## Task 9: 添加示例 YAML 规格

**Files:**
- Create: `tests/specs/auth-service/login.yaml`

- [ ] **Step 1: 写示例规格**

写入 `tests/specs/auth-service/login.yaml`：
```yaml
name: login_with_valid_credentials
service: auth-service
type: integration
description: 有效凭证登录返回 access_token
tags:
  - smoke
  - auth
preconditions:
  - 数据库存在测试用户 test_user / Passw0rd!
steps:
  - method: POST
    path: /auth/login
    body:
      username: test_user
      password: Passw0rd!
expected:
  status: 200
  body_contains: access_token
data_cleanup:
  - DELETE FROM users WHERE username='test_user'
```

- [ ] **Step 2: 验证规格**

```powershell
cd d:\projects\coder-micro-services
.\scripts\test-orchestrator.ps1 validate-specs
```

Expected: `[validate] 1 spec files OK`。

- [ ] **Step 3: 故意写错一个，验证报错**

把 `service: auth-service` 改成 `service: bogus-service`，再跑 `validate-specs`。

Expected: 退出码 1，输出 `... service must be one of ...`。

改回 `auth-service`。

- [ ] **Step 4: 提交**

```bash
git add tests/specs/auth-service/login.yaml
git commit -m "test(specs): add sample login.yaml for auth-service"
```

---

## Task 10: 端到端手动验证

**Files:** 无（只运行命令）

- [ ] **Step 1: 启动 Docker 栈**

```powershell
cd d:\projects\coder-micro-services
.\scripts\test-orchestrator.ps1 start
```

Expected: 所有容器健康，输出 `[docker] all services healthy`。

- [ ] **Step 2: 重置 DB**

```powershell
.\scripts\test-orchestrator.ps1 reset-db
```

Expected: `[db] dropping...`、`[db] flushing redis...`、`[docker] restarting: auth-service-test, user-service-test`、`[docker] all services healthy`。

- [ ] **Step 3: 运行所有测试**

```powershell
.\scripts\test-orchestrator.ps1 test-all
```

Expected: 每个服务都有 `<service>.unit.json`、`<service>.unit.log`、`<service>.integration.json`、`<service>.integration.log`。部分服务可能失败（取决于现有测试状态）。

- [ ] **Step 4: 生成报告**

```powershell
.\scripts\test-orchestrator.ps1 report
```

Expected: `reports\run-{ts}\summary.md` 创建；若有任何失败，`reports\run-{ts}\fix-prompt.md` 与 `reports\latest-fix-prompt.md` 也创建。

- [ ] **Step 5: 验证提示词内容**

```powershell
Get-Content reports\latest-fix-prompt.md
```

Expected: 含 `## 失败用例` 与 `## 错误详情` 区块，模板占位符已被替换。

- [ ] **Step 6: 清理**

```powershell
.\scripts\test-orchestrator.ps1 stop
```

Expected: 容器全部停止。

- [ ] **Step 7: 提交（若无新文件则跳过）**

```bash
git status
# 若有 .gitkeep 或示例规格未提交，git add 并 commit
```

---

## 自审检查

- [ ] **Spec 覆盖**：设计文档 7 个组件 + 1 入口 + 2 模板 + 1 样例 → 9 个 Task 全部覆盖。
- [ ] **占位符扫描**：无 TBD / TODO；所有代码块完整。
- [ ] **类型一致**：`Invoke-ServiceTests`/`Build-Summary`/`Build-FixPrompt`/`Render-SpecToCodePrompt`/`Invoke-SpecValidation` 全部在 lib 与入口中名称一致。
- [ ] **路径一致**：报告目录 `reports\run-{ts}\` 在 Test-Runner、Report-Builder、入口三处一致。
- [ ] **模板一致**：`fix-tests.template.md` 的占位符 `{{FAILING_TESTS}}`/`{{ERROR_DETAILS}}`/`{{RELATED_FILES}}` 与 Report-Builder 渲染代码一致。
