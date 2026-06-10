# Test-Runner.ps1
# 封装 cargo test，输出统一 JSON

$Root = "d:\projects\coder-micro-services"

$ServiceMap = @{
    "api-gateway"              = @{ Lang = "rust"; Path = "services\api-gateway" }
    "token-service"            = @{ Lang = "rust"; Path = "services\token-service" }
    "credential-login-service" = @{ Lang = "rust"; Path = "services\credential-login-service" }
    "user-service"             = @{ Lang = "rust"; Path = "services\user-service" }
    "notification-service"     = @{ Lang = "rust"; Path = "services\notification-service" }
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
        $prevPref = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            # Don't use --message-format=json: it strips libtest's human-readable
            # "test name ... ok/failed/ignored" output that Report-Builder parses.
            if ($Type -eq "unit") {
                & cargo test --lib 2>&1 | Tee-Object -FilePath $OutputJson | Out-Null
            } else {
                & cargo test --test '*' 2>&1 | Tee-Object -FilePath $OutputJson | Out-Null
            }
        } catch {
            # capture but don't rethrow - tee'd output is the source of truth
            Write-Debug "test runner caught (ignored): $_"
        } finally {
            $ErrorActionPreference = $prevPref
        }
    } finally {
        Pop-Location
    }
}

function Get-ServiceList {
    @("api-gateway", "token-service", "credential-login-service", "user-service", "notification-service")
}
