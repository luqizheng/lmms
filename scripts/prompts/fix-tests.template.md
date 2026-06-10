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
