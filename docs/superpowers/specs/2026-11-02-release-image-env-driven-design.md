# 发布脚本镜像引用 env 化改造 — 设计规格

- **日期**：2026-11-02
- **状态**：设计已定稿，待实施
- **触发**：用户反馈「这个基础框架使用起来有点复杂」，要求检查 `pnpm run release:build`

---

## 1. 目标与非目标

### 背景与动机

当前镜像引用**散落在三处**（compose 源码、`release.js` 硬编码数组、`transform-compose.js` 的 `IMAGE_MAP`），
换一次镜像源要改多处，且国内无法访问 docker hub 时缺少统一的切换入口。

用户诉求（原始表述）：

> 「主要国内无法访问docker hub。所以我想把redis等放入.env中，如果要从不同的镜像获取image，
> 我就直接改 .env，所以 redis 在 docker-compose.yaml 中就是 ${redis} 就可以了。」

> 「1. docker-compose.yml 中的lmms服务，使用环境变量${kudi_repos}，而环境变量默认就是
> zhcoder-docker-registry.com:8000/micro-server。它存在发布.env 文件中
> 2. 其余的官方 image，全部采用官方的 image名字，不在加前序了。」

**核心洞察**：`${BASE_REGISTRY}` 这类「前缀变量」不好用——国内换源得改前缀字符串，且 redis / mysql
无法各自独立换源。改成「整名变量」（`${redis}` = `redis:7-alpine`）后，每个镜像可独立换源，
且只改 `.env` 一处。

### 目标

1. **镜像引用单一真相源 = `.env`**：compose 中所有 `image:` 均为 `${VAR}` 形式，值由 `.env` 提供。
2. **国内换源只改 `.env` 一处**，无需触碰 compose 或打包脚本。
3. 平台镜像用 `${kudi_repos}`，官方镜像用各自的整名变量。
4. **源码与产物统一**：`infrastructure/*.yml` 与发布产物中的 compose 采用同一套写法。

### 用户已裁决的决策

| 决策 | 内容 | 理由 |
| --- | --- | --- |
| **A：官方镜像默认值** | `.env` 里填**裸官方名**（`redis=redis:7-alpine`） | air-gap 打包时自行 pull 裸名；online 场景国内用户自行改一行换源 |
| **B：改造范围** | **源码与产物统一**改 | 单一真相源，无两套写法 |

### 非目标

| 项 | 结论 | 理由 |
| --- | --- | --- |
| `services/*/Dockerfile` 的 `FROM ${BASE_REGISTRY}/xxx` | **本次不改** | 构建期 base image 与 compose 运行时 image 是两套东西；且 dist 包**不含服务源码**，Dockerfile 不进产物，改它无法解决本问题 |
| `.gitlab-ci.yml` 的 `image:` 字段 | **本次不改** | CI runner 自身的 image，与 compose 运行时 image 无关；且 CI 在内网跑，内网 registry 可达 |
| `release/compose-template/` 产物 | **本次不重新生成** | 未被 git 跟踪（`git ls-files` 为空）→ 是产物；由 `packages/cli/lib/compose-template.js` 生成，下次发布时自然带上新 compose |
| `packages/cli` 的 compose-template 生成逻辑 | **本次不改** | 它只是复制 compose 文件，不硬编码镜像名 |

---

## 2. 变量清单与命名约定

### 2.1 硬约束：所有新变量必须带 `:-` 默认值兜底

**实测结论**（docker compose v5.6.0，最小复现）：

| 变量类型 | 缺失时的行为 | 安全性 |
| --- | --- | --- |
| 整名变量 `image: ${redis}` | warning + **硬报错** `service "redis" has neither an image nor a build context specified: invalid compose project` | 安全，被拦住 |
| 拼接型 `image: ${kudi_repos}/api-gateway:${IMAGE_TAG:-1.6.0}` | **仅 warning，静默产出 `image: /api-gateway:1.6.0`** | ⚠ **隐患**，错误推迟到 pull 时才暴露 |

结论：**所有新增变量在 compose 中必须写 `:-` 默认值**：

```yaml
image: ${redis:-redis:7-alpine}
image: ${kudi_repos:-zhcoder-docker-registry.com:8000/micro-server}/api-gateway:${IMAGE_TAG:-1.6.0}
```

这与用户「改 `.env` 换源」的意图**不冲突**：compose 插值优先级是
`.env` 显式值 > 进程环境变量 > `:-` 默认值。`:-` 只在变量**完全未定义**时兜底，
`.env` 里显式赋值永远优先，用户的选择不会被覆盖。

### 2.2 变量清单

**平台镜像仓库前缀**（1 个）：

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `kudi_repos` | `zhcoder-docker-registry.com:8000/micro-server` | lmms 平台镜像仓库前缀 |

**官方镜像**（11 个，每个独立可换源）：

| 变量 | 默认值 | compose 中的 service |
| --- | --- | --- |
| `consul` | `hashicorp/consul:2.0.1` | consul |
| `registrator` | `gliderlabs/registrator:master` | registrator |
| `redis` | `redis:7-alpine` | redis |
| `mysql` | `mysql:8.0` | mysql |
| `rabbitmq` | `rabbitmq:3.13-management-alpine` | rabbitmq |
| `prometheus` | `prom/prometheus:v2.51.0` | prometheus |
| `alertmanager` | `quay.io/prometheus/alertmanager:v0.27.0` | alertmanager |
| `grafana` | `grafana/grafana:10.4.0` | grafana |
| `consul_template` | `hashicorp/consul-template:0.32.0` | consul-template |
| `otel_collector` | `otel/opentelemetry-collector-contrib:0.96.0` | otel-collector |
| `jaeger` | `jaegertracing/all-in-one:1.55` | jaeger |

### 2.3 命名论证

**采用小写无后缀形式**（`redis`、`mysql`、`consul_template`），理由：

1. **用户明确要求** `${redis}` 形式 —— 直观、一眼可读。
2. **与 compose service name 对齐**：变量名 = service 名，改动时容易交叉定位。
3. **与既有大写连接参数变量天然分层**：`REDIS_HOST` / `REDIS_PORT` / `REDIS_PASSWORD` 是
   **连接参数**（语义不同），`redis` 是**镜像引用**。虽同源，但大小写 + 语义双重区分。

**已检查的冲突**：

| 潜在冲突 | 结论 |
| --- | --- |
| `redis` vs `REDIS_HOST` / `REDIS_PORT` / `REDIS_PASSWORD` / `REDIS_DATABASES` | ✅ 大小写不同，无冲突 |
| `mysql` vs `MYSQL_HOST` / `MYSQL_PORT` / `MYSQL_USER` 等 | ✅ 大小写不同，无冲突 |
| `mysql` / `redis` 变量名与 compose service 名同名 | ✅ compose 的 service 名空间与变量名空间独立，无冲突 |
| `consul` vs `CONSUL_HOST` / `CONSUL_HTTP_PORT` | ✅ 无冲突 |
| `consul_template` / `otel_collector` 用下划线 | ✅ 避开 YAML 与 shell 中连字符易被误读的问题；compose 变量名允许下划线 |

**备选对比**（记录否决理由）：

| 备选 | 否决理由 |
| --- | --- |
| `REDIS_IMAGE` / `MYSQL_IMAGE` 后缀式 | 用户明确要求 `${redis}`；且与 `REDIS_HOST` 同前缀更易混 |
| 保留单一 `BASE_REGISTRY` 前缀变量 | 无法让 redis / mysql 独立换源——这是用户换源诉求的核心 |
| `KUDI_IMG_REDIS` 全前缀式 | 冗长，且 `kudi_repos` 已确立前缀风格，不宜混用两套命名法 |

### 2.4 alertmanager 特例处理

现状：compose 与 `release.js` 都写死 `swr.cn-north-4.myhuaweicloud.com/ddn-k8s/quay.io/prometheus/alertmanager:v0.27.0`
（注释说明：内网 registry 不同步该镜像，故意走公网代理）。

改造后统一为**官方名** `quay.io/prometheus/alertmanager:v0.27.0`，理由：

1. 符合用户「其余的官方 image，全部采用官方的 image名字」的要求。
2. 与其他 10 个官方镜像的命名规则一致。
3. 国内拉取问题由 `.env` 换源解决（可改 `alertmanager=swr.cn-north-4.myhuaweicloud.com/ddn-k8s/quay.io/prometheus/alertmanager:v0.27.0`）。

⚠ **注意**：swr 代理地址必须**同时**从 `release.js` 的镜像清单移除，否则 tar 内镜像名
（swr 全路径）与 compose 展开值（`quay.io/...`）不一致 → 离线 `docker load` 后 compose 找不到镜像。
这一条是**必须同步改**，见 §5。

---

## 3. compose 文件改法（逐文件）

### 3.1 `infrastructure/docker-compose-infra.yml`

```diff
   consul:
-    image: ${BASE_REGISTRY}hashicorp/consul:2.0.1
+    image: ${consul:-hashicorp/consul:2.0.1}

   registrator:
-    image: ${BASE_REGISTRY}gliderlabs/registrator:master
+    image: ${registrator:-gliderlabs/registrator:master}

   redis:
-    image: ${BASE_REGISTRY}redis:7-alpine
+    image: ${redis:-redis:7-alpine}

   mysql:
-    image: ${BASE_REGISTRY}mysql:8.0
+    image: ${mysql:-mysql:8.0}

   rabbitmq:
-    image: ${BASE_REGISTRY}rabbitmq:3.13-management-alpine
+    image: ${rabbitmq:-rabbitmq:3.13-management-alpine}
```

### 3.2 `infrastructure/docker-compose-observability.yml`

```diff
   prometheus:
-    image: ${BASE_REGISTRY}prom/prometheus:v2.51.0
+    image: ${prometheus:-prom/prometheus:v2.51.0}

   alertmanager:
-    # 有意走公网镜像代理（quay.io/prometheus 是 alertmanager 官方发布渠道），
-    # 与同文件其他组件走 ${BASE_REGISTRY} 不同：内网 registry 不同步该镜像，
-    # airgap tar 有网环境 pull/save、离线 load，勿改回内网 official/ 路径。
-    image: swr.cn-north-4.myhuaweicloud.com/ddn-k8s/quay.io/prometheus/alertmanager:v0.27.0
+    image: ${alertmanager:-quay.io/prometheus/alertmanager:v0.27.0}

   grafana:
-    image: ${BASE_REGISTRY}grafana/grafana:10.4.0
+    image: ${grafana:-grafana/grafana:10.4.0}

   consul-template:
-    image: ${BASE_REGISTRY}hashicorp/consul-template:0.32.0
+    image: ${consul_template:-hashicorp/consul-template:0.32.0}

   otel-collector:
-    image: ${BASE_REGISTRY}otel/opentelemetry-collector-contrib:0.96.0
+    image: ${otel_collector:-otel/opentelemetry-collector-contrib:0.96.0}

   jaeger:
-    image: ${BASE_REGISTRY}jaegertracing/all-in-one:1.55
+    image: ${jaeger:-jaegertracing/all-in-one:1.55}
```

### 3.3 `infrastructure/docker-compose-services.yml`

该文件**源码态只有 `build:` 块，没有 `image:` 行**（开发态本地构建）。
**源码不改**——`image:` 由 `transform-compose.js` 在打包时注入（见 §4）。

### 3.4 `infrastructure/docker-compose.yml`

`mysql-replica` 服务同时有 `build:` 块和 `image: coder/mysql-replica:8.0`（本地 tag）。
`transform-compose.js` 的 `skipCoexistingImageTag()` 会丢弃这个共存 tag 并改写为平台镜像引用，
**源码同样不改**。

---

## 4. `transform-compose.js` 的改造

### 4.1 输出形式

现状（registry 被烘焙进 image 行）：

```
image: zhcoder-docker-registry.com:8000/micro-server/api-gateway:${IMAGE_TAG:-v1.0.0}
```

改造后（引用变量，不再烘焙）：

```
image: ${kudi_repos:-zhcoder-docker-registry.com:8000/micro-server}/api-gateway:${IMAGE_TAG:-1.6.0}
```

### 4.2 `IMAGE_MAP` 的调整

前缀 `micro-server/` 从 `IMAGE_MAP` 移到 `kudi_repos` 的默认值里，因此 `IMAGE_MAP` 改为**只存服务名后缀**：

```js
const IMAGE_MAP = {
  'config-service':               'config-service',
  'file-service':                 'file-service',
  'api-gateway':                  'api-gateway',
  'token-service':                'token-service',
  'iam-service':                  'iam-service',
  'iam-outbox-publisher':         'iam-service',
  'notification-service':         'notification-service',
  'notification-outbox-publisher':'notification-service',
  'audit-service':                'audit-service',
  'prue_backend':                 'prue-admin-backend',   // 特殊映射保留
  'mysql-replica':                'mysql-replica',
};
```

注意 `prue_backend` → `prue-admin-backend` 与两个 outbox 复用主服务镜像的特殊映射**必须保留**。

### 4.3 tag 默认值必须不带 `v`

**既有缺陷**：CI 推送镜像时用 `export VERSION="${CI_COMMIT_TAG#v}"`（剥掉 `v`），
registry 里实际是 `micro-server/api-gateway:1.5.0`；而 `release.js` 拉的是
`.../api-gateway:v1.5.0`（带 `v`）→ **pull 失败**，且被 `ignoreError: true` 静默吞掉。

改造时统一：

```js
// 版本号归一：镜像 tag 不带 v（CI 用 ${CI_COMMIT_TAG#v} 剥 v 后推送）
const IMAGE_TAG_VALUE = VERSION.replace(/^v/, '');
```

`transform-compose.js` 接收的 `<version>` 参数也应传入**已归一的 tag**（不带 v），
使 compose 里的 `${IMAGE_TAG:-<tag>}` 默认值与 registry 中真实存在的 tag 一致。

### 4.4 向后兼容

**结论**：`<registry>` 参数**保留但不再用于拼路径**，仅打日志提示已废弃。

理由：
1. 避免旧调用方式（`<compose> <registry> <version>`）直接崩掉。
2. 参数位置不变 → `release.js` / `release-deploy.js` 的调用点不需要大改。
3. 未来若需支持「打包时指定不同仓库」，可让 `<registry>` 覆盖 `kudi_repos` 的默认值
   （即产出 `${kudi_repos:-<registry>/micro-server}/...`），扩展性保留。

---

## 5. 两个打包器的一致化

### 5.1 现状问题

| 打包器 | 镜像清单来源 | 问题 |
| --- | --- | --- |
| `release.js` | **硬编码 `IMAGES` 数组**（17 条） | 与 compose 脱节；tag 带 `v`（§4.3）；alertmanager 用 swr 全路径 |
| `release-deploy.js` | **扫 compose 的 `image:` 行** | `if (img.includes('${')) continue;` → **改造后会扫出空列表** |

改造后 compose 全是变量形式，两条路径**都会失效或不一致** → 必须引入共享模块。

### 5.2 共享镜像解析模块

新建 `release/dist-template/scripts/image-resolver.js`，职责：

```
scanComposeImages(composeDir, envSource) → { images: string[], unresolved: string[] }
```

- **扫描**：`infrastructure/*.yml` 中所有 `image:` 行。
- **变量插值**：按 compose 优先级解析 —— 进程环境变量 > `--env-file` 指定文件 > compose 内 `:-` 默认值。
- **tag 处理**：识别 `${IMAGE_TAG:-x}` 形式，取默认值或环境变量值。
- **未解析项收集**：变量既无环境值也无 `:-` 默认值时，记入 `unresolved` 并**报错**（不静默跳过）。

三处统一调用：

| 调用方 | 用途 |
| --- | --- |
| `release.js` | 阶段：docker pull + save 的镜像清单（**替换硬编码 IMAGES 数组**） |
| `release-deploy.js` | 阶段三：扫 compose → docker save |
| `load-images.js --mode=pull` | 扫 compose → docker pull（**替换现有正则扫描 + VAR_INTERPOLATION 白名单**） |

### 5.3 tag 去 `v` 缺陷修正

`release.js` 与 `release-deploy.js` 的镜像 tag 统一使用 `VERSION.replace(/^v/, '')`，
与 CI 推送规则对齐。

### 5.4 打包完整性闸门（必须新增）

当前 `docker save` 失败只打印 `✗ save failed` 后继续，导致产出**缺镜像的包**且不报错。
改造后必须：

1. `docker pull` 失败 → **中断打包**（`process.exit(1)`）。
2. `docker save` 失败 → **中断打包**。
3. 打包结束前校验：`实际 tar 数 == 期望镜像数`，不一致 → `process.exit(1)`。

### 5.5 打包时 env 源

优先级（从高到低）：

1. `--env-file <path>` 命令行参数
2. 进程环境变量
3. `release/dist-template/.env.example`（默认基准）

---

## 6. `load-images.js` 的改造

### 6.1 `--mode=pull`

现有两个缺陷：

- **tag 正则** `/:v?\d+\.\d+\.\d+(?:[-.][\w.-]+)*$/` 只匹配**字面数字 tag**，匹配不了 `${IMAGE_TAG:-1.6.0}`。
- **`VAR_INTERPOLATION` 白名单**只有 `REGISTRY` / `BASE_REGISTRY`，其余 `${VAR}` 引用被
  `if (interpolated.includes('${')) continue;` 直接跳过 → 扫出的列表**漏掉全部平台镜像**。

改造：删除这套正则 + 白名单逻辑，改用 §5.2 的共享模块。

### 6.2 `--mode=load`（air-gap 默认路径）

**一致性约束**：tar 内镜像名必须与 compose 展开后的名字**逐字一致**。

在决策 A（裸名）下：
- 打包时 pull 的名字 = `.env` 里的值 = compose 展开值 → 三者天然一致 ✅
- `docker load` 后本地镜像名与 compose 引用匹配 ✅

⚠ **用户可见后果（必须在文档中写明）**：
**离线端修改 `.env` 换源会导致 compose 找不到镜像**（tar 里是打包时的名字）。
换源只在**打包时**（有网环境）或 **online 部署**时有效。
air-gap 离线端必须保持 `.env` 与打包时一致的值。

---

## 7. 开发态一致性

### 7.1 `infrastructure/.env` 需新增

11 个官方镜像变量 + `kudi_repos`。

**默认值取内网源**（而非裸名）——开发机在内网，内网 registry 更稳：

```
kudi_repos=zhcoder-docker-registry.com:8000/micro-server
consul=zhcoder-docker-registry.com:8000/official/hashicorp/consul:2.0.1
redis=zhcoder-docker-registry.com:8000/official/redis:7-alpine
... (其余同理)
```

compose 内仍有 `:-` 兜底为裸名，即使 `.env` 缺失也不会产生非法镜像名。

### 7.2 与 `base-images-sync.js` 的共存

`scripts/base-images-sync.js` 已实现「单变量驱动全部镜像源」：读 `.docker-registry.json` 的
`baseRegistry`（或 `CODER_BASE_REGISTRY`），**幂等改写** `infrastructure/.env` 的 `BASE_REGISTRY=` 行；
`baseRegistry` 为空时写 `BASE_REGISTRY=`（让 compose 引用塌缩成裸名）。

**协调方案**：改造后 `BASE_REGISTRY` 不再被 compose 引用，该脚本的同步目标需要调整为新的镜像变量组。

- **本期**：脚本保持不动（`BASE_REGISTRY` 仍被 CI 与 `Dockerfile FROM` 使用）。
  compose 不再引用它 → 脚本同步的值对 compose 无影响，**无冲突**。
- **后续**（不在本期）：若要让「改一处 `.docker-registry.json` 同步所有镜像变量」，
  需扩展该脚本去批量改写 11 个变量行。

### 7.3 开发态加载链路

实测确认：`pnpm run dev`（`docker compose -f infrastructure/docker-compose.yml up -d`）
**不带** `--env-file`，compose 自动加载 compose 文件同目录的 `infrastructure/.env`。
改造后这条链路**仍然成立**（变量定义在 `infrastructure/.env` 即可）。

与 dist 包的差异（两种不同机制，改造后都要可用）：

| 环境 | compose 调用 | env 来源 |
| --- | --- | --- |
| 开发态 | `docker compose -f infrastructure/docker-compose.yml up -d` | 自动读 `infrastructure/.env` |
| dist 包 | `docker compose --env-file .env -f infrastructure/docker-compose.yml up -d` | 显式 `--env-file .env` |

---

## 8. 风险清单

| 风险 | 影响 | 缓解措施 |
| --- | --- | --- |
| air-gap 离线端改 `.env` 换源 | compose 找不到镜像（tar 内是打包时名字） | 在 dist 包 README / `make load-images` 提示中写明「离线端勿改镜像变量」；打包时以 `.env.example` 为基准生成 |
| 官方镜像裸名在国内打包机不可达 | air-gap 打包阶段 pull 失败 | 打包机需可达 docker hub / 镜像加速器；或用 `--env-file` 指定含国内源的 env 文件打包 |
| `alertmanager` 换成官方名后与 `release.js` 清单不同步 | 离线端 compose 找不到镜像 | §2.4 + §5.2：清单由扫 compose 自动得出，**从结构上消除不同步** |
| 变量拼写错误 | compose 仅 warning，整名变量硬报错但拼接型静默降级 | §2.1 强制 `:-` 兜底；`check-compose-images.js` 校验（§10） |
| `BASE_REGISTRY` 在 CI 中无尾斜杠，compose 约定有尾斜杠 | 拼接出 `.../officialmysql` 畸形引用 | 本次改造后 compose 不再引用 `BASE_REGISTRY` → 该隐患自然消除；CI 的 `BASE_REGISTRY` 仍供 CI job image 与 Dockerfile FROM 使用，不受影响 |
| `release/compose-template/` 旧产物与新 compose 不一致 | 该产物是 gitignore 的历史快照，不参与构建 | 不重新生成；下次发布自然更新 |
| 改造中间态产物不一致 | 分批提交会产生「新脚本 + 旧 compose」的产物 | §9 要求：compose 改动与脚本改动**同一提交**落地 |

---

## 9. 实施计划

按依赖顺序。**步骤 3 与步骤 4 必须同一提交**（否则中间态产物不一致）。

| # | 步骤 | 文件 | 改什么 |
| --- | --- | --- | --- |
| 1 | 共享镜像解析模块 | `release/dist-template/scripts/image-resolver.js`（新建） | 扫 compose + 变量插值 + tag 归一；导出 `scanComposeImages()` |
| 2 | 官方镜像变量化 | `infrastructure/docker-compose-infra.yml`<br>`infrastructure/docker-compose-observability.yml` | 按 §3.1 / §3.2 替换 `image:` 行 |
| 3 | 平台镜像变量化 | `release/dist-template/scripts/transform-compose.js` | `IMAGE_MAP` 去前缀；输出 `${kudi_repos:-...}/<svc>:<no-v tag>`；`registry` 参数保留但废弃 |
| 4 | 打包器一致化 | `release/dist-template/scripts/release.js`<br>`release/dist-template/scripts/release-deploy.js` | 删除硬编码 `IMAGES` 数组；改调共享模块；tag 去 `v`；pull/save 失败即中断 + tar 数校验 |
| 5 | env 文件 | `release/dist-template/.env.example`<br>`infrastructure/.env` | 加 `kudi_repos` + 11 个官方镜像变量（见 §2.2 / §7.1） |
| 6 | load-images 改造 | `release/dist-template/bin/load-images.js` | `--mode=pull` 改用共享模块；删除旧正则 + `VAR_INTERPOLATION` 白名单 |
| 7 | 校验脚本 | `scripts/check-compose-images.js`（新建） | 校验 compose 变量展开无退化（§10） |
| 8 | 单测扩展 | `release/dist-template/scripts/release-transforms.test.js` | 新增 `image-resolver` 用例（§10） |

**同步提交约束**：步骤 2 + 3 + 4 必须在**同一个 commit**（否则「compose 已变量化、脚本还扫旧格式」
的中间态会让 air-gap 包静默产出空镜像列表）。

---

## 10. 验证方案

### 10.1 变量展开校验

```bash
# dist 包语义
docker compose --env-file .env -f infrastructure/docker-compose.yml config

# 开发态语义（自动读同目录 .env）
docker compose -f infrastructure/docker-compose.yml config
```

### 10.2 退化形态检查（新增 `scripts/check-compose-images.js`）

对 `docker compose config` 的输出检查以下退化形态，命中即 `exit 1`：

- `image:` 为空
- 镜像名以 `/` 开头（如 `/api-gateway:1.6.0`）—— 拼接型变量未定义的特征
- 镜像名含未解析的 `${`
- 出现意外的 `:latest`

### 10.3 现有测试

```bash
node --check release/dist-template/scripts/transform-compose.js
node --check release/dist-template/scripts/release.js
node --check release/dist-template/scripts/release-deploy.js
node --check release/dist-template/bin/load-images.js
node --test release/dist-template/scripts/release-transforms.test.js
```

### 10.4 新增测试用例（建议）

`image-resolver` 模块：

1. compose 中 `image: ${redis:-redis:7-alpine}` → 解析为 `redis:7-alpine`
2. compose 中 `image: ${kudi_repos:-X}/api-gateway:${IMAGE_TAG:-1.6.0}` → 解析为 `X/api-gateway:1.6.0`
3. 进程环境变量 `redis=other:7` 覆盖 `:-` 默认值 → `other:7`
4. `--env-file` 提供的值优先于 `:-` 默认值
5. 变量既无环境值也无 `:-` 默认值 → 记入 `unresolved` 并报错（不静默跳过）
6. tag 归一：`v1.5.0` → `1.5.0`

### 10.5 端到端验证

```bash
# 骨架打包（不拉镜像，快）
EXPORT_IMAGES=0 node release/dist-template/scripts/release.js v1.6.0
# 检查产物 compose 已变量化 + .env 变量齐全
grep -n "image:" release/dist-template/release/coder-micro-services-v1.6.0/infrastructure/docker-compose-infra.yml
grep -n "kudi_repos\|^redis=\|^mysql=" release/dist-template/release/coder-micro-services-v1.6.0/.env.example
```

完整 air-gap 打包（需拉 17+ 镜像）应在有网环境执行，并检查打包日志中
「实际 tar 数 == 期望镜像数」的校验通过。
