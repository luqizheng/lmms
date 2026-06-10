# Spec-Validator.ps1
# 校验 tests/specs/ 下 YAML 文件的必填字段
# 不依赖外部模块；用手写解析识别顶层 key

$Required = @("name", "service", "type", "description", "steps")
$AllowedServices = @("auth-service", "user-service", "api-gateway", "token-server")
$AllowedTypes = @("unit", "integration")
$NamePattern = "^[a-z][a-z0-9_]*$"

function Get-YamlValue {
    param([string[]]$Lines, [string]$Key)
    $re = "^\s*$([regex]::Escape($Key))\s*:\s*(.+?)\s*$"
    foreach ($line in $Lines) {
        if ($line -match $re) { return $Matches[1].Trim().Trim('"').Trim("'") }
    }
    return $null
}

function Test-KeyPresent {
    param([string[]]$Lines, [string]$Key)
    $re = "^\s*$([regex]::Escape($Key))\s*:"
    foreach ($line in $Lines) {
        if ($line -match $re) { return $true }
    }
    return $false
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
        $present = Test-KeyPresent -Lines $lines -Key $k
        if (-not $present) {
            $errors += "$Path : missing required field '$k'"
        } elseif ($k -in @("name", "service", "type", "description")) {
            $v = Get-YamlValue $lines $k
            if ([string]::IsNullOrWhiteSpace($v)) {
                $errors += "$Path : '$k' must have inline value"
            }
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
    return $errors
}

function Invoke-SpecValidation {
    $root = "d:\projects\coder-micro-services\tests\specs"
    $all = @()
    if (Test-Path $root) {
        $all = Get-ChildItem -Path $root -Recurse -Filter "*.yaml" -File
    }
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
