# 计划：添加 Windows WSL 开发规则

## 概述
在项目中添加 Windows WSL 开发环境的规则文档，指导开发者在 Windows 环境下使用 WSL 进行开发。

## 当前状态分析
- 项目已有两个规则文件：
  - `.trae/rules/project_rules.md` - 项目级规则（提交、分支、代码风格等）
  - `.trae/rules/auth-rules.md` - 微服务身份认证规则
- 规则文件采用 Markdown 格式，结构清晰，包含编号章节和子章节
- 当前工作目录已配置为 WSL 环境（`\\wsl.localhost\Debian\home\kudi\projects\templates\low-memory-micro-services`）

## 建议变更

### 文件：`.trae/rules/wsl-development-rules.md`（新建）

**创建内容：**

```markdown
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
- 文件系统：项目代码应位于 WSL 文件系统内（如 `~/projects/`）
- 禁止将项目放置在 Windows 文件系统（如 `/mnt/c/`）下，性能影响严重

## 2. 开发工具配置

### 2.1 VS Code 配置

- 安装 "WSL" 扩展
- 使用 "Remote - WSL" 模式打开项目
- 配置路径：
  ```json
  {
    "terminal.integrated.defaultProfile.windows": "WSL",
    "terminal.integrated.profiles.windows": {
      "WSL": {
        "path": "wsl.exe",
        "args": ["-d", "Debian"]
      }
    }
  }
  ```

### 2.2 终端配置

- 默认终端：PowerShell Core 或 Windows Terminal
- 在 WSL 内使用 bash 或 zsh
- 路径访问：
  - Windows 访问 WSL：`\\wsl.localhost\Debian\home\kudi\projects\`
  - WSL 访问 Windows：`/mnt/c/Users/`

## 3. 文件系统规则

### 3.1 项目位置

- 项目必须位于 WSL 文件系统内
- 路径格式：`/home/<user>/projects/<project-name>`
- 禁止路径：`/mnt/c/Users/<user>/projects/`（性能问题）

### 3.2 文件权限

- 注意 Windows 和 Linux 文件权限差异
- 使用 `chmod` 和 `chown` 设置正确权限
- Git 配置：
  ```bash
  git config --global core.fileMode false
  ```

## 4. 常见问题处理

### 4.1 路径问题

- Windows 路径：`C:\Users\luqiz\...`
- WSL 路径：`/home/kudi/...`
- 跨系统访问：
  - 从 Windows 访问 WSL：`\\wsl.localhost\Debian\...`
  - 从 WSL 访问 Windows：`/mnt/c/Users/...`

### 4.2 性能优化

- 禁用 Windows Defender 实时扫描 WSL 目录
- 将项目克隆到 WSL 文件系统而非 Windows 文件系统
- 使用 `.wslconfig` 配置资源限制：
  ```ini
  [wsl2]
  memory=8GB
  processors=4
  swap=2GB
  ```

## 5. 命令执行规范

### 5.1 命令格式

- 在 WSL 内执行 Linux 命令
- 文件操作使用 Linux 命令（`ls`, `cat`, `rm` 等）
- 使用正斜杠 `/` 作为路径分隔符

### 5.2 Docker 使用

- 安装 Docker Desktop 或在 WSL 内安装 Docker Engine
- 使用 Docker Desktop 时启用 WSL2 backend
- 配置 Docker context：
  ```bash
  docker context use default
  ```

## 6. 违规处理

- 代码审查时发现非 WSL 环境应拒绝合并
- 环境问题优先检查是否在 WSL 内
- 提供迁移到 WSL 的指导文档

## 7. 参考文档

- [WSL 官方文档](https://docs.microsoft.com/en-us/windows/wsl/)
- [VS Code Remote - WSL](https://code.visualstudio.com/docs/remote/wsl)
- 项目环境准备文档：`docs/onboarding/02-环境准备.md`
```

### 文件：`project_rules.md`（更新）

**在 Section 5 Local Development Setup 中添加引用：**

在 "## 5. Local Development Setup" 章节末尾添加：

```markdown
4. **WSL Development (Windows users)**:
   - Windows 开发者**必须**使用 WSL2 环境
   - 详细规则见 `.trae/rules/wsl-development-rules.md`
```

### 文件：`routing-rules.md`（更新）

**在文件顶部添加环境依赖说明：**

```markdown
# 路由规则

> **环境要求**：Windows 用户必须使用 WSL2 进行开发，详见 `.trae/rules/wsl-development-rules.md`

任何一层偏离该链路即为违规，需同步修复三层……
```

## 假设与决策

### 假设
1. 项目已在 WSL 环境中运行（已验证）
2. 开发团队需要在规则中明确 WSL 使用要求
3. 规则文件将被 AI 助手和开发者参考

### 决策
1. **创建独立规则文件**：选择创建 `wsl-development-rules.md` 而非合并到现有文件
   - 理由：WSL 开发是一个独立的主题，便于维护和查找
   - 参考：auth-rules.md 也是独立的专题规则

2. **规则覆盖范围**：包括环境配置、工具使用、常见问题
   - 理由：提供完整的 WSL 开发指导，减少开发者踩坑

3. **在现有文件中添加引用**：在 project_rules.md 和 routing-rules.md 中引用新规则
   - 理由：确保规则被发现和遵循

## 验证步骤

1. **文件检查**
   ```bash
   ls -la .trae/rules/wsl-development-rules.md
   cat .trae/rules/wsl-development-rules.md
   ```

2. **引用检查**
   ```bash
   grep -n "wsl-development-rules" .trae/rules/project_rules.md
   grep -n "wsl-development-rules" .trae/rules/routing-rules.md
   ```

3. **格式验证**
   - 确认 Markdown 格式正确
   - 确认文件编码为 UTF-8
   - 确认章节编号连续

## 影响范围

- **直接影响**：Windows 平台开发者
- **间接影响**：所有使用 AI 助手的开发者（AI 会读取这些规则）
- **无影响**：Linux/macOS 开发者