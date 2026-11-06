# 部署拓扑与跨主机开发指南（deploy topologies）

> 2026-11-06 三组独立部署定型后的权威文档。覆盖三种部署模式、跨主机寻址契约、
> 「先部署 infra(+obs) 到测试服务器、本地只开发 services 组」的准备工作（场景 0）、
> 以及 lmms 的开发→发布链路（场景 1）。

## 三种部署模式

| 模式 | 拓扑 | 用法 | 适用 |
|------|------|------|------|
| **A 同机三组** | 一台机，三个 compose 项目 `-p superpowers-{infra,obs,svc}` | `./infrastructure/scripts/start-prod.sh`（可点名组） | 生产单机、联调 |
| **B 合体单项目** | 一台机，入口 `docker-compose.yml` include 三块（模板包形态） | 模板包 `start-prod.sh` | 模板包/airgap 消费方 |
| **C 分置多机** | 三组各占一台服务器（或任意子集组合） | 每台机只点自己的组并加 `--remote` | 生产分机、隔离部署 |

模式 A/B 的跨组互访走共享 `superpowers` 网络容器名 DNS；模式 C 没有跨主机网络，
一切跨组流量改走 **宿主端口发布 + IP 指向**，由 `*.remote.yml` 片与 env 完成。

## 模式 C：分置多机

### 每台机的操作（目录即部署单元）

2026-11-06 起各组物理自包含（`infrastructure/{infra,observability,services}/`，
compose + config 分区 + start/stop.sh + .env.example）——**整目录 copy 到目标机
即可部署**：

```bash
# ── 分发（任选 scp / rsync / git 同步） ─────────────────────────────
scp -r infrastructure/infra/            infra-host:~/lmms/
scp -r infrastructure/services/         svc-host:~/lmms/
scp -r infrastructure/observability/    obs-host:~/lmms/

# ── infra 机 ────────────────────────────────────────────────────────
cd ~/lmms && cp .env.example .env        # 改中间件口令等
./start.sh --remote                      # remote 片发布宿主端口供他机访问

# ── services 机（env 按「跨主机寻址契约」填他机 IP） ─────────────────
cd ~/lmms && cp .env.example .env        # CONSUL_ADDR/SERVICE_ADDRESS/REDIS_HOST/…
./start.sh --remote

# ── obs 机（需 CONSUL_HOST_IP 指回 infra 机） ───────────────────────
cd ~/lmms && cp .env.example .env        # CONSUL_HOST_IP=<infra 机 IP>
./start.sh --remote
```

- 组目录内 `start.sh` 支持 `--remote/--dev/--staging` 选 overlay 片（默认 prod），
  其余 args 透传 compose；`stop.sh` 对应 down（`--volumes` 语义同前）。
- env 向上回退：仓内运行自动命中 `infrastructure/.env` 单一事实源，拷出后用
  本地 `.env`——两种形态同一份脚本。
- 同机组合（如 infra+obs 同机、services 异机）：同机的组不带 `--remote`
  （走共享网络容器名，无需 `CONSUL_HOST_IP`）。
- 仓内一键编排（模式 A 同机全量/分组）仍走根编排器：
  `./scripts/start-prod.sh infra --remote` 等，`--remote` 只作用于本次点名的组。
- `services/` 的拷出形态=镜像引用部署（仓内 build context 指仓根，拷出不可
  构建）；跨目录 config 挂载由发布管线 remap 成目录内 `./config/<slug>`。

### 跨主机寻址契约（services 机的 env 覆盖块，写进 .env.prod）

```bash
# infra 机中间件（端口由 infra.remote.yml 发布）
CONSUL_ADDR=http://<infra-ip>:8500        # 服务自注册 + consul-template
SERVICE_ADDRESS=<services 机 LAN IP>      # consul 健康检查回拨地址（必填！空值=容器
                                          # hostname，只有 infra 机自己能解析到）
REDIS_HOST=<infra-ip>
RABBITMQ_HOST=<infra-ip>
DATABASE_URL=postgres://<user>:<pass>@<infra-ip>:5432/<db>   # PG 形态
OTEL_EXPORTER_OTLP_ENDPOINT=http://<obs-ip>:4317             # obs 缺席时留默认，
                                                             # 导出失败不阻塞业务
```

机制说明（为什么不用改配置文件）：

- services→infra 中间件寻址本来就是 env（`REDIS_HOST`/`RABBITMQ_HOST`/`DATABASE_URL`），
  指 IP 即可；
- `CONSUL_ADDR`/`SERVICE_ADDRESS`/`OTEL_EXPORTER_OTLP_ENDPOINT` 已参数化
  （`docker-compose-services.yml` 的 `x-consul-env` anchor 与各服务 env）；
- prometheus 的 consul SD 与 consul-template 写死 `consul:8500` 容器名，配置文件是静态
  挂载、无 env 插值——obs.remote 片用 `extra_hosts: ["consul:${CONSUL_HOST_IP}"]`
  把名字映射到 infra 机 IP，零配置改动。

### 防火墙清单

| 机器 | 放行（入站） | 用途 |
|------|--------------|------|
| infra 机 | 8500 8502 6379 3306 5432 5672 15672 | services/obs 组跨主机访问中间件 + consul UI |
| services 机 | 3000 8082 8083 8084 8086 8087 9081 + 各服务 metrics 口 | api-gateway 对外、consul 健康检查回拨、prometheus 抓取（经 consul SD 拿到 `SERVICE_ADDRESS`） |
| obs 机 | 4317 4318 | services 机 OTLP 上报 |

### 已知边界（诚实声明）

- **日志**：promtail 只发现**本机** docker 容器。obs 机上的 Loki 收不到 services 机的
  容器日志。跨主机要聚合日志时，在 services 机临时跑一个 promtail sidecar
  （`LOKI_URL=http://<obs-ip>:3100`，docker socket 只读挂载）。trace（jaeger）与
  metrics（consul SD）不受影响，全链路跨主机可用。
- **obs 缺席**：不部署 observability 组时 services 组照常运行——OTLP 导出失败静默
  重试不阻塞业务（这也是「3 不是必须的」契约）。
- **上游单 IMAGE_TAG 缺陷**：上游 dist 包用单 `IMAGE_TAG` env 覆盖镜像版本，对 PG
  形态会丢 `-pg` 后缀（api-gateway 无 PG 变体）。ai-pm 侧已用双变量
  （`IMAGE_TAG_PG`/`IMAGE_TAG_GATEWAY`，见其 `.env-infra`）规避；上游 dist 侧待后续
  版本对齐，本仓 compose 默认值不受影响。

## 场景 0：先在测试服务器（192.168.2.10）部署 infra(+obs)，本地开发 services

> 不追求本地搞定全部：中间件与服务发现放服务器，本地只跑业务服务。

```bash
# ── 服务器侧（192.168.2.10，一次性） ─────────────────────────────
# 取仓（git clone / 同步 lmms 仓 infrastructure/），准备 .env + .env.prod
cd infrastructure
./scripts/start-prod.sh infra --remote     # 中间件端口发布到宿主
./scripts/start-prod.sh obs                # 可选：同机不带 --remote，走共享网络

# ── 本地开发机（一次性准备） ────────────────────────────────────
# infrastructure/.env 或 .env.prod 追加覆盖块（IP 换成服务器地址）：
#   CONSUL_ADDR=http://192.168.2.10:8500
#   SERVICE_ADDRESS=<本机 LAN IP，如 192.168.2.4>
#   REDIS_HOST=192.168.2.10
#   RABBITMQ_HOST=192.168.2.10
#   DATABASE_URL=postgres://postgres:<pass>@192.168.2.10:5432/coder
#   OTEL_EXPORTER_OTLP_ENDPOINT=http://192.168.2.10:4317   # obs 部署了才填

# 本地起 services 组（连服务器中间件）
cd infrastructure && ./scripts/start-prod.sh svc --remote
```

- 防火墙反向要求：服务器 consul（8500）要能回拨本机 `SERVICE_ADDRESS:SERVICE_PORT`
  → 本机开发防火墙放行服务端口；反之本机访问服务器中间件端口须放行（见上表）。
- 单元测试不受影响：`libs/digua-common-consul` 的契约是 `SERVICE_NAME` 未设 ⇒ 禁用
  注册——`cargo test` 不会尝试连任何 consul。
- 宿主机直跑进程（非容器）开发时同样成立：env 指服务器 IP 即可，
  `pnpm run dev:infra`（本地起 infra）不再是必需项。

## 场景 1：lmms 开发 → 发布链路

```
写 Rust / 单元测试（cargo test；测试内嵌 testcontainers，不依赖部署栈）
  → 发布 SDK（crate → kellnr；npm 包 → 私有 registry；下游按包升级接新）
  → 发布 docker image（build → push zhcoder-docker-registry.com:8000/micro-server，
     tag 规则 <ver> 与 <ver>-pg 双变体，api-gateway 仅 <ver>）
  → 发布 services 编排（release compose-template 模板包：新版本 .env-infra 镜像行
     + compose 默认值；下游整目录覆盖 lmms 命名文件即完成升级）
```

- 发布命令与门禁见根 README「发布」节与 `packages/cli`（`release compose-template
  --dialect pg`）；镜像构建配方见 `infrastructure/README.md`「构建镜像」。
- 下游消费侧（ai-pm）的升级动作与其「有意差异重放」清单，见
  `ai-pm/infrastructure/README.md`。
