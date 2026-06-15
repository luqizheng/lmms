---
name: coder-base-images-sync
description: 同步项目中的基础 Docker 镜像到 infrastructure/base-images.json，供 test-orchestrator 预拉取使用。
---

# coder-base-images-sync

## 何时使用

- 修改了 `services/*/Dockerfile*`
- 修改了 `infrastructure/docker-compose*.yml`
- 需要为某个镜像配置 mirror（如 `docker.xunyuan.cn`）

## 命令

```bash
node scripts/base-images-sync.js              # 扫描并写入
node scripts/base-images-sync.js --check      # 仅 diff，不写
node scripts/base-images-sync.js --set-mirror redis:7-alpine docker.xunyuan.cn
```

## 输出

`infrastructure/base-images.json`：

```json
{
  "version": 1,
  "generatedAt": "ISO-8601",
  "images": [{ "ref": "...", "source": "path:line", "mirror": "..." }]
}
```

## 行为

- `mirror` 字段拉取成功后会自动 `docker tag <mirror>/<ref> <ref>`，本地使用仍是原 tag
- mirror 拉取失败自动回源拉取
- 不删除已不在源码中出现的镜像（v1 策略）

## 不做的事

- ❌ 不修改业务源码
- ❌ 不验证镜像在 registry 中存在
- ❌ 不删 JSON 中已有但源码已不引用的镜像