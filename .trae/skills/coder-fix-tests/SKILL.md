---
name: coder-fix-tests
description: 测试失败后读取 latest-fix-prompt.md，渲染 AI 修复提示词，指导 AI 仅改 tests/ 下代码。用于「修测试」「fix-prompt」「失败修复」等意图。
---

# coder-fix-tests

## 何时用

`test-orchestrator.ps1 report` 检测到失败，或用户说「修测试」「fix-prompt」时。

## 边界

**仅修改各服务下的测试代码**：
- Go: `services/<service>/*_test.go`
- Rust: `services/<service>/tests/*.rs`

**禁止**修改 `services/`、`libs/`、`sdk/` 下非 tests 目录的业务源码。

## 步骤

### 1. 确认有失败

```powershell
cd d:\projects\coder-micro-services
Get-Content reports\latest-fix-prompt.md
```

### 2. 把提示词贴给 AI

文件含三段：
- `## 失败用例`：测试名 + 服务 + 类型
- `## 错误详情`：每条 case 的 error 输出（截前 20 行）
- `## 相关文件`：候选修改路径

### 3. AI 修改后

```powershell
.\scripts\test-orchestrator.ps1 reset-db
.\scripts\test-orchestrator.ps1 test-integration
```

### 4. 验证无回归

```powershell
.\scripts\test-orchestrator.ps1 test-all
```

## 重生成 fix-prompt

```powershell
.\scripts\test-orchestrator.ps1 fix-prompt
```

## 模板位置

`scripts\prompts\fix-tests.template.md` —— 改模板会改所有后续生成内容。
