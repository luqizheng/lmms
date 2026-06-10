# 在真实 PowerShell（非 sandbox）跑一次，让 ring 0.17.14 在 v143 下 build 出 .a 缓存
# TraeIDE sandbox 隔离 vcvarsall 环境变量，无法自动跑；此脚本必须用户手动跑

& "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
Set-Location D:\projects\coder-micro-services
cargo build -p common 2>&1 | Select-Object -Last 30
