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
    $specs = Get-Specs
    foreach ($f in $specs) {
        $specsText += "`n---`nFile: $($f.FullName)`n`n"
        $specsText += (Get-Content $f.FullName -Raw -Encoding utf8)
    }
    if ($specs.Count -eq 0) {
        $specsText = "(无 YAML 规格)"
    }
    $rendered = $tmpl -replace "\{\{SPECS\}\}", $specsText
    $out = "d:\projects\coder-micro-services\reports\spec-to-code-prompt.md"
    $rendered | Out-File -FilePath $out -Encoding utf8
    Write-Host "[prompt] rendered to $out" -ForegroundColor Green
}
