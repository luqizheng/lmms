# 双角色镜像的健康检查按角色分流

> 日期：2026-10-28
> 关联：`docs/superpowers/specs/2026-10-28-update-shell-skill-gap-fixes-design.md`
> 范围：`services/iam-service/Dockerfile`、`services/notification-service/Dockerfile`、
> `infrastructure/docker/healthcheck.sh`、`infrastructure/docker-compose-services.yml`
> 风险：低（只改健康探针，不动服务逻辑；探针误判最坏后果是容器被标记 unhealthy）

---

## 1. 现状

### 1.1 具体行为 / bug 复现 / 遗漏事实

`iam-service` 与 `notification-service` 两张镜像里**各装了两个二进制**：服务端
（`iam_service` / `notification_service`，监听 HTTP 端口）与 outbox publisher
（`iam_outbox_publisher` / `ntf_outbox_publisher`，无监听端口的 worker），
compose 用 `command:` 切换角色。

但 Docker 的 `HEALTHCHECK` 是**镜像级**的，两张镜像里都写死了服务端端口：

```dockerfile
HEALTHCHECK --interval=15s --timeout=3s --retries=5 --start-period=20s \
  CMD wget --spider -q http://127.0.0.1:8081/health || exit 1   # iam
```

publisher 角色没有 HTTP 端口可探，于是**恒 unhealthy**：

```
$ docker ps
infrastructure-iam-outbox-publisher-1          Up 33 seconds (unhealthy)
infrastructure-notification-outbox-publisher-1  Up 32 seconds (unhealthy)
```

功能其实正常（日志里有 `connected to RabbitMQ`、`published outbox batch`），
但状态是红的，排查时极具误导性——看起来像 publisher 挂了，其实只是探针问错了对象。

### 1.2 根因

一张镜像服务两个角色，而健康检查只有一个、且是镜像级的静态配置：
`services/*/Dockerfile` 的 `HEALTHCHECK` 与 `infrastructure/docker-compose-services.yml`
里 publisher 的 `command:` 之间没有任何联动机制。

### 1.3 生产影响 / 业务后果

- publisher 容器恒 unhealthy，掩盖真实故障：真挂了和探针问错了，长得一模一样。
- 依赖 `depends_on: {service_healthy}` 的编排无法把 publisher 当作可用信号。
- 运维按 `docker ps` 判断状态会误判。

---

## 2. 修复方案 / 设计方案

### 2.1 方案选择

| 候选 | 内容 | 结论 |
|---|---|---|
| A | `healthcheck: disable: true` | ✗ 状态变"无健康信号"，红字消失但失去任何存活判据 |
| B | compose 里给 publisher 探 `pgrep` 进程存活 | ✗ PID 1 活着容器就活着，等于没探 |
| C | **按角色分流：服务端探 /health，publisher 探轮询指标文件新鲜度** | ✓ **选中** |

选 C 的理由：publisher 每轮轮询会**原子写一次 Prometheus 指标文件**
（`METRICS_FILE_PATH`，默认 `/var/run/prometheus/{iam,ntf}_outbox.prom`），
文件 mtime 新鲜 = 轮询循环还活着；卡死则停更。这是 worker 角色**真实可用**的信号，
不是"进程还在"这种恒真判断。

### 2.2 改动清单

| 文件 | 改动 | 原因 |
|------|------|------|
| `infrastructure/docker/healthcheck.sh` | 新增 | 角色感知探针：按 PID 1 的 cmdline 分流 |
| `services/iam-service/Dockerfile` | +7 -2 | COPY 探针脚本、`ENV HEALTHCHECK_PORT=8081`、HEALTHCHECK 改调脚本 |
| `services/notification-service/Dockerfile` | +7 -2 | 同上（端口 8083） |
| `infrastructure/docker-compose-services.yml` | +30 | 给两个 publisher 显式 healthcheck + 显式 `METRICS_FILE_PATH` |

探针逻辑：

```
PID 1 cmdline 含 outbox_publisher ?
  ├─ 是 → 指标文件存在 且 mtime 距今 ≤ 60s  → healthy
  │        （轮询间隔默认 1s，留 60s 余量；停更即视为循环卡死）
  └─ 否 → wget http://127.0.0.1:$HEALTHCHECK_PORT/health → healthy
```

**两层防护**：镜像内的角色感知脚本让镜像**自身**正确（不依赖 compose，k8s /
直接 `docker run` 也成立）；compose 里给 publisher 显式探针，让**已发布的旧镜像**
（当前还是写死端口的老 HEALTHCHECK）在不重构建的前提下也立刻正确。

### 2.3 数据流 / 调用路径

```
compose: command: ["iam_outbox_publisher"]
   └→ 覆盖 CMD，但继承镜像 HEALTHCHECK ──→ 老镜像：探 8081 → 无端口 → unhealthy
新镜像 HEALTHCHECK → healthcheck.sh
   └→ 读 /proc/1/cmdline 判角色
        ├→ worker  → stat -c %Y /var/run/prometheus/iam_outbox.prom → 新鲜度
        └→ server  → wget /health
```

---

## 3. 测试计划

- [x] 脚本三分支实测（在运行中的容器内验证，不需重构建镜像）：
      worker 角色 → 退出码 0；server 角色（`HEALTHCHECK_PORT=8081`）→ 退出码 0；
      worker 但指标文件被改成 2020 年（模拟循环卡死）→ 退出码 1
- [x] `docker compose config` 校验上下游两份 compose 的语法与插值
- [x] 下游实跑：重建两个 publisher → `Up (healthy)`，日志仍有
      `connected to RabbitMQ` / `published outbox batch`
- [x] 上游门禁：`check-service-standard.js` / `check-spec.js` / `contracts-gen.js --check` 全过

---

## 4. 回滚方案

### 4.1 回滚触发条件

- 探针误判：publisher 被标记 unhealthy 但实际在正常发消息；
- 指标文件路径在某些部署形态下不存在，导致 publisher 恒 unhealthy。

### 4.2 回滚步骤

1. 移除 compose 里 publisher 的 `healthcheck` 段 → 回到镜像内置 HEALTHCHECK 行为；
2. 镜像侧回滚：把 Dockerfile 的 `HEALTHCHECK` 改回 `wget --spider -q http://127.0.0.1:<port>/health`，
   并删除 `COPY infrastructure/docker/healthcheck.sh` 与 `ENV HEALTHCHECK_PORT`。
3. 无 schema / config 变更，无数据面影响。

### 4.3 回滚耗时估计 + 数据损失面

分钟级；数据损失面为零（只影响容器状态标记与编排信号）。

### 4.4 不能回滚的情况

无。

---

## 5. 已知限制 / 后续工作

- 镜像侧改动要等 CI 重建发布镜像才生效；在此之前 compose 层的显式 healthcheck 已经覆盖
  compose 部署场景。
- `METRICS_FILE_PATH` 目前只在容器内，未挂卷到宿主机，Prometheus 的 textfile collector
  抓不到。要支持宿主侧抓取需给 publisher 加 volume，属独立议题。
- 60s 的新鲜度阈值写死在脚本里，未随 `OUTBOX_POLL_INTERVAL_MS` 缩放；轮询间隔被调到
  大于 6s 时需要同步放宽。

---

## 6. 关联 ADR / spec

- `2026-10-28-update-shell-skill-gap-fixes-design.md` —— 同批次修复（check-v-perms 扫描根）
- `services/iam-outbox-publisher/src/metrics.rs` —— 指标文件写入机制（原子写、每轮轮询一次）
