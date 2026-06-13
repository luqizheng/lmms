# contracts/ 双端契约层

契约层是前后端一致的单一真相源。

## 文件结构

| 文件 | 用途 |
|---|---|
| error-codes.yaml | 全局错误码字典 |
| permissions.yaml | 权限码字典 |
| events.yaml | 领域事件名（预留） |
| http-conventions.md | HTTP 响应/分页/排序约定 |

## 代码生成

```bash
node scripts/contracts-gen.js
```

生成目标：
- `libs/common-error/src/codes.rs` — Rust 错误码常量
- `libs/authz/src/perms.rs` — Rust 权限码常量
- `frontend/src/contracts/*.ts` — TypeScript 类型（预留）

## 命名规则

### 错误码

格式：`<SERVICE>_<CATEGORY>_<NUMBER>`

- 服务前缀：`IAM` / `CFG` / `AUD` / `NTF` / `FILE` / `GW` / `SYS`
- 类别：`AUTH` / `VALID` / `DB` / `EXT` / `INTERNAL`

### 权限码

格式：`<SERVICE>_<RESOURCE>_<ACTION>`

- 动作后缀：`CREATE` / `READ` / `UPDATE` / `DELETE` / `EXPORT` / `IMPORT`

## CI 校验

```bash
node scripts/test-orchestrator.js validate-contracts
```

检查生成的代码是否与 YAML 源文件一致，防止漂移。

## 编辑流程

1. 编辑 YAML 文件
2. 运行 `node scripts/contracts-gen.js`
3. 运行 `cargo build -p common-error -p authz`
4. Commit YAML + 生成的 Rust 文件