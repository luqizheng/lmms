# Base Images Sync 设计

**日期**：2026-06-15
**作者**：System Architect
**目标**：将硬编码在 `Docker-Helper.js` 的 `PRE_PULL_IMAGES` 替换为源码驱动的镜像清单，并支持 mirror 拉取。

---

## 1. 背景

`scripts/lib/Docker-Helper.js` 中：

```js
const PRE_PULL_IMAGES = [
  'alpine:3.21',
  'rust:1.96-slim',
  'mysql:8.0',
  'redis:7-alpine',
  'consul:1.15',
];
```

问题：
1. 列表与源码（`Dockerfile` / `docker-compose.yml`）漂移
2. 缺少 `rust:1.96-slim-musl`、`debian:bookworm-slim`、`node:20-alpine`、`nginx:stable-alpine` 等构建镜像
3. 无法为单个镜像指定 mirror；mirror 拉取后无法自动 retag

---

## 2. 目标

- 单一真相源：`infrastructure/base-images.json`
- Skill `coder-base-images-sync` 扫描 `FROM` 与 compose `image:`，写入 JSON
- `prePull` 读取 JSON，支持 mirror + 自动 retag
- 镜像失败回源

---

## 3. 文件清单

| 路径 | 用途 | 类型 |
|---|---|---|
| `infrastructure/base-images.json` | 镜像清单 | 新增 |
| `scripts/base-images-sync.js` | 扫描脚本入口 | 新增 |
| `scripts/lib/Base-Images-Scanner.js` | FROM/compose 解析 | 新增 |
| `scripts/lib/Base-Images-Writer.js` | JSON 写入 + 合并保留 mirror | 新增 |
| `.trae/skills/coder-base-images-sync/SKILL.md` | Skill 说明 | 新增 |
| `scripts/lib/Docker-Helper.js` | 修改 `prePull` 读取 JSON | 修改 |
| `.trae/rules/project_rules.md` | 新增路径 → skill 映射 | 修改 |
| `AGENTS.md` | Skill 索引新增一行 | 修改 |

---

## 4. JSON 结构

```json
{
  "version": 1,
  "generatedAt": "2026-06-15T10:00:00Z",
  "images": [
    {
      "ref": "alpine:3.21",
      "source": "services/api-gateway/Dockerfile:15",
      "mirror": null
    },
    {
      "ref": "mysql:8.0",
      "source": "infrastructure/docker-compose.yml:32",
      "mirror": "docker.xunyuan.cn"
    }
  ]
}
```

字段说明：
- `ref`：原始镜像引用（拉取成功 tag 后的最终名）
- `source`：来源定位（`path:line`），仅供溯源
- `mirror`：可选，mirror registry 域名（不含协议/路径前缀）
- 若 `mirror` 留空或不存在则直接从默认 registry 拉取

---

## 5. 扫描逻辑

### 5.1 Dockerfile `FROM`

正则：`/^FROM\s+(\S+)/gm`

去重 key：`<image-ref>` 大小写敏感

匹配示例：
- `FROM alpine:3.21` → `alpine:3.21`
- `FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder` → `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`

### 5.2 docker-compose.yml `image:`

正则：`/^\s*image:\s*(\S+)/gm`

仅扫 `infrastructure/docker-compose*.yml`，**不包含业务服务镜像**（如 `zhcoder-docker-registry.com:8000/micro-server/*:latest`），因为它们由 CI 构建产出，不需预拉。

### 5.3 合并策略

读旧 JSON → 以 `ref` 为 key 合并：
- 旧 JSON 存在的 `mirror` 字段保留（避免每次 sync 重填）
- 扫描新出现的镜像 `mirror` 默认 `null`
- 扫描不再出现的镜像：保留 30 天警告期，超期删除

实际落地：v1 仅做新增/更新，不删除（避免误删）；删除逻辑 v2 引入。

---

## 6. prePull 流程

```
读取 base-images.json
for img in images:
  ref = img.ref
  if img.mirror:
    mirrorRef = `${img.mirror}/${ref}`   // 例如 docker.xunyuan.cn/redis:7-alpine
    log.cyan(`[pull] ${mirrorRef} (mirror of ${ref})`)
    if spawnSync('docker', ['pull', mirrorRef]).status === 0:
      spawnSync('docker', ['tag', mirrorRef, ref])
      log.green(`[pull] tagged ${mirrorRef} -> ${ref}`)
      continue
    log.yellow(`[pull] mirror failed, fallback to ${ref}`)
  log.cyan(`[pull] ${ref}`)
  res = spawnSync('docker', ['pull', ref], { stdio: 'inherit' })
  if res.status !== 0:
    log.yellow(`[pull] WARNING: failed to pull ${ref}`)
```

错误处理：
- mirror 失败：警告 + 回源，不中断
- 原 tag 失败：警告，继续下一个镜像（保持原有容错语义）

---

## 7. Skill 接口

### 7.1 文件结构

```
.trae/skills/coder-base-images-sync/
  SKILL.md
```

### 7.2 触发关键词

`base images`、`sync images`、`image manifest`、`镜像清单`

### 7.3 命令

```bash
node scripts/base-images-sync.js
```

可选：
- `--check`：只 diff，不写文件
- `--set-mirror <ref> <mirror>`：手动设置镜像的 mirror（如 `redis:7-alpine docker.xunyuan.cn`）

### 7.4 SKILL.md 内容要点

- 何时使用：修改了 Dockerfile / docker-compose.yml 后
- 跑什么命令：`node scripts/base-images-sync.js`
- 输出什么：`infrastructure/base-images.json`
- 手动改 mirror：`--set-mirror`
- 不做的事：不改业务源码；不删 JSON 中已有镜像；不验证镜像在 registry 中存在

---

## 8. 与现有规则的集成

### 8.1 project_rules.md 路径映射

新增：

```
| `services/*/Dockerfile*`                | `coder-base-images-sync`      | 改完保存  |
| `infrastructure/docker-compose*.yml`   | `coder-base-images-sync`      | 改完保存  |
```

### 8.2 AGENTS.md Skill 索引

新增一行：

```
| `coder-base-images-sync` | base images、镜像清单 | node scripts/base-images-sync.js |
```

---

## 9. 测试

`scripts/lib/Base-Images-Scanner.js` 提供 `--check` 与可注入的 fs 抽象，便于纯单元测试。

测试要点（`scripts/lib/__tests__/base-images-scanner.test.js`）：
- 解析 `FROM alpine:3.21`
- 解析 `FROM zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl AS builder`
- 多 stage build 去重
- compose `image:` 解析
- 合并旧 JSON 保留 `mirror`

不测试项：
- 真实 `docker pull`（依赖本地 Docker + 网络）

---

## 10. 风险与约束

| 风险 | 缓解 |
|---|---|
| scanner 漏解析（多行 FROM、`$VAR` 插值） | v1 只支持字面量；多行/变量留 v2 |
| mirror 拉取成功但 retag 失败 | 仍能 `docker run mirror/image:tag`，retag 仅是优化 |
| 删镜像误删业务镜像 | v1 不删；v2 引入白名单 |
| Node.js 内置模块限制 | 仅用 `node:fs` / `node:path` / `node:child_process`，与现有脚本一致 |

---

## 11. 不做的事（YAGNI）

- ❌ 镜像版本漂移检测（业务不依赖）
- ❌ 自动从 registry 验证镜像存在
- ❌ 多 mirror fallback 链（v1 仅 1 个 mirror）
- ❌ 删除 JSON 中已不存在的镜像
- ❌ 解析 `${VAR}` 镜像引用
- ❌ CI 自动跑 sync（本地工具即可）