# Windows WSL 开发规则（参考）

> 本文件中的"强制规则"已放宽为**推荐做法**。开发者可根据自身环境选用 Windows 原生 shell（PowerShell / CMD）、WSL2、macOS 或 Linux，不强制 WSL。

## 1. 环境要求

### 1.1 推荐规则

**Windows 下推荐使用 WSL 进行开发**（非强制）：
- Windows 原生环境（PowerShell / CMD）也可以使用
- 如使用 WSL，推荐 WSL2（Windows Subsystem for Linux 2）
- 推荐发行版：
  - Debian（推荐）
  - Ubuntu 20.04 / 22.04 LTS
- 文件系统：项目代码建议位于 WSL 文件系统内（如 ~/projects/）
- 如放置在 Windows 文件系统（如 /mnt/c/）下，性能可能受影响，请自行评估

## 2. 开发工具配置

### 2.1 VS Code 配置（可选）

- 如果使用 WSL，可安装 " WSL 扩展
