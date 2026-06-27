# Windows WSL 开发规则

## 1. 环境要求

### 1.1 强制规则

**Windows 下必须使用 WSL 进行开发**：
- 禁止在 Windows 原生环境下直接开发
- 必须使用 WSL2（Windows Subsystem for Linux 2）
- 推荐使用 Debian 或 Ubuntu 发行版

### 1.2 环境配置

- WSL 版本：WSL2（版本 2.x）
- 推荐发行版：
  - Debian（推荐）
  - Ubuntu 20.04/22.04 LTS
- 文件系统：项目代码应位于 WSL 文件系统内（如 ~/projects/）
- 禁止将项目放置在 Windows 文件系统（如 /mnt/c/）下，性能影响严重

## 2. 开发工具配置

### 2.1 VS Code 配置

- 安装 " WSL 扩展
