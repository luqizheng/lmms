<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  listIamUsers,
  listIamRoles,
  listIamSessions
} from "@/api/iam";
import {
  listOperationLogs,
  type OperationLog
} from "@/api/audit";

defineOptions({
  name: "DashboardIndex"
});

const { t } = useI18n();

interface ServiceStatus {
  name: string;
  slug: string;
  probePath: string;
  port: number;
  status: "ok" | "down" | "unknown";
  latencyMs: number | null;
  lastCheckedAt: number | null;
  latencyHistory: number[];
  uptimeUp: number;
  uptimeTotal: number;
}

const LATENCY_HISTORY_LEN = 20;

const SERVICES: Array<Omit<ServiceStatus, "status" | "latencyMs" | "lastCheckedAt" | "latencyHistory" | "uptimeUp" | "uptimeTotal">> = [
  { name: "API Gateway", slug: "api-gateway", probePath: "/probe/api-gateway", port: 3000 },
  { name: "Token Service", slug: "token-service", probePath: "/probe/token-service", port: 8082 },
  { name: "IAM Service", slug: "iam-service", probePath: "/probe/iam-service", port: 9081 },
  { name: "Notification", slug: "notification-service", probePath: "/probe/notification-service", port: 8083 },
  { name: "Audit Service", slug: "audit-service", probePath: "/probe/audit-service", port: 8084 },
  { name: "File Service", slug: "file-service", probePath: "/probe/file-service", port: 8086 },
  { name: "Config Service", slug: "config-service", probePath: "/probe/config-service", port: 8085 },
  { name: "Prue Backend", slug: "prue-backend", probePath: "/probe/prue-backend", port: 8087 }
];

const statuses = ref<ServiceStatus[]>(
  SERVICES.map(s => ({
    ...s,
    status: "unknown",
    latencyMs: null,
    lastCheckedAt: null,
    latencyHistory: [],
    uptimeUp: 0,
    uptimeTotal: 0
  }))
);

const POLL_MS = 5000;
let timer: ReturnType<typeof setInterval> | null = null;

async function probeOne(s: ServiceStatus): Promise<ServiceStatus> {
  const t0 = performance.now();
  let ok = false;
  let latency: number | null = null;
  try {
    const resp = await fetch(`${s.probePath}/health`, {
      method: "GET",
      cache: "no-store"
    });
    latency = Math.round(performance.now() - t0);
    ok = resp.ok;
  } catch {
    ok = false;
  }
  const history = [...s.latencyHistory, latency ?? -1].slice(-LATENCY_HISTORY_LEN);
  return {
    ...s,
    status: ok ? "ok" : "down",
    latencyMs: latency,
    lastCheckedAt: Date.now(),
    latencyHistory: history,
    uptimeUp: s.uptimeUp + (ok ? 1 : 0),
    uptimeTotal: s.uptimeTotal + 1
  };
}

async function refreshAll() {
  const results = await Promise.all(statuses.value.map(probeOne));
  statuses.value = results;
}

const upCount = computed(() => statuses.value.filter(s => s.status === "ok").length);
const downCount = computed(() => statuses.value.filter(s => s.status === "down").length);
const lastRefreshAt = ref<number | null>(null);

function fmtTime(ts: number | null | string): string {
  if (ts === null || ts === undefined) return "—";
  const d = new Date(ts);
  return d.toLocaleTimeString();
}

function statusTag(status: ServiceStatus["status"]): {
  type: "success" | "danger" | "info";
  label: string;
} {
  switch (status) {
    case "ok":
      return { type: "success", label: t("menus.dashboardStatusOk") };
    case "down":
      return { type: "danger", label: t("menus.dashboardStatusDown") };
    default:
      return { type: "info", label: t("menus.dashboardStatusUnknown") };
  }
}

function uptimePct(s: ServiceStatus): string {
  if (s.uptimeTotal === 0) return "—";
  return ((s.uptimeUp / s.uptimeTotal) * 100).toFixed(1) + "%";
}

function avgLatency(s: ServiceStatus): number | null {
  const valid = s.latencyHistory.filter(v => v >= 0);
  if (valid.length === 0) return null;
  return Math.round(valid.reduce((a, b) => a + b, 0) / valid.length);
}

function sparklinePath(history: number[], width = 80, height = 24): string {
  const valid = history.filter(v => v >= 0);
  if (valid.length < 2) return "";
  const max = Math.max(...valid, 1);
  const min = Math.min(...valid, 0);
  const range = max - min || 1;
  const stepX = width / (LATENCY_HISTORY_LEN - 1);
  return valid
    .map((v, i) => {
      const x = i * stepX;
      const y = height - ((v - min) / range) * height;
      return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
}

function sparklineColor(s: ServiceStatus): string {
  const avg = avgLatency(s);
  if (avg === null) return "#909399";
  if (avg < 50) return "#67c23a";
  if (avg < 200) return "#e6a23c";
  return "#f56c6c";
}

// =========== Stat cards（从真实 API 取 count） ===========
const userCount = ref(0);
const roleCount = ref(0);
const sessionCount = ref(0);
const recentOps = ref<OperationLog[]>([]);
const statsLoading = ref(false);

async function refreshStats() {
  statsLoading.value = true;
  try {
    const [users, roles, sessions, ops] = await Promise.all([
      listIamUsers({ page: 1, size: 1 }).catch(() => ({ items: [], total: 0 })),
      listIamRoles({ page: 1, size: 1 }).catch(() => ({ items: [], total: 0 })),
      listIamSessions().catch(() => []),
      listOperationLogs({ page: 1, size: 5 }).catch(() => ({ items: [] }))
    ]);
    userCount.value = users.total || 0;
    roleCount.value = roles.total || 0;
    sessionCount.value = Array.isArray(sessions) ? sessions.length : 0;
    recentOps.value = ops.items || [];
  } finally {
    statsLoading.value = false;
  }
}

onMounted(async () => {
  await Promise.all([refreshAll(), refreshStats()]);
  lastRefreshAt.value = Date.now();
  timer = setInterval(async () => {
    await Promise.all([refreshAll(), refreshStats()]);
    lastRefreshAt.value = Date.now();
  }, POLL_MS);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <div class="dashboard p-4">
    <el-row :gutter="16" class="mb-4">
      <el-col :xs="12" :sm="6">
        <el-card shadow="never" v-loading="statsLoading">
          <div class="text-center">
            <div class="text-3xl font-bold">{{ userCount }}</div>
            <div class="text-sm text-gray-500">
              {{ t("menus.dashboardUserCount") }}
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card shadow="never" v-loading="statsLoading">
          <div class="text-center">
            <div class="text-3xl font-bold">{{ roleCount }}</div>
            <div class="text-sm text-gray-500">
              {{ t("menus.dashboardRoleCount") }}
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card shadow="never" v-loading="statsLoading">
          <div class="text-center">
            <div class="text-3xl font-bold">{{ sessionCount }}</div>
            <div class="text-sm text-gray-500">
              {{ t("menus.dashboardSessionCount") }}
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card shadow="never">
          <div class="text-center">
            <div class="text-sm text-gray-500">
              {{ t("menus.dashboardOperationCount") }}
            </div>
            <div class="text-3xl font-bold text-blue-500">
              {{ recentOps.length > 0 ? "✓" : "—" }}
            </div>
            <el-button
              type="primary"
              size="small"
              class="mt-2"
              @click="refreshAll"
            >
              {{ t("menus.dashboardRefresh") }}
            </el-button>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="16" class="mb-4">
      <el-col :xs="24" :sm="8">
        <el-card shadow="never">
          <div class="text-center">
            <div class="text-3xl font-bold text-green-500">{{ upCount }}</div>
            <div class="text-sm text-gray-500">
              {{ t("menus.dashboardUpCount") }} / {{ statuses.length }}
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="8">
        <el-card shadow="never">
          <div class="text-center">
            <div class="text-3xl font-bold text-red-500">{{ downCount }}</div>
            <div class="text-sm text-gray-500">
              {{ t("menus.dashboardDownCount") }}
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="8">
        <el-card shadow="never">
          <div class="text-center">
            <div class="text-sm text-gray-500">
              {{ t("menus.dashboardLastCheck") }}
            </div>
            <div class="text-lg">{{ fmtTime(lastRefreshAt) }}</div>
            <el-button
              type="primary"
              size="small"
              class="mt-2"
              @click="refreshAll"
            >
              {{ t("menus.dashboardRefresh") }}
            </el-button>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="16">
      <el-col
        v-for="s in statuses"
        :key="s.slug"
        :xs="24"
        :sm="12"
        :md="8"
        :lg="6"
        class="mb-4"
      >
        <el-card shadow="hover">
          <div class="flex items-center justify-between mb-2">
            <div>
              <div class="text-base font-semibold">{{ s.name }}</div>
              <div class="text-xs text-gray-400">
                :{{ s.port }} · {{ s.slug }}
              </div>
            </div>
            <el-tag
              :type="statusTag(s.status).type"
              size="small"
              effect="dark"
            >
              {{ statusTag(s.status).label }}
            </el-tag>
          </div>

          <div class="grid grid-cols-2 gap-3 text-xs mt-2">
            <div>
              <div class="text-gray-400">
                {{ t("menus.dashboardAvgLatency") }}
              </div>
              <div class="font-mono text-base">
                {{ avgLatency(s) !== null ? avgLatency(s) + " ms" : "—" }}
              </div>
            </div>
            <div>
              <div class="text-gray-400">
                {{ t("menus.dashboardUptime") }}
              </div>
              <div class="font-mono text-base">{{ uptimePct(s) }}</div>
            </div>
          </div>

          <div
            v-if="s.latencyHistory.length >= 1"
            class="mt-2"
            :title="
              t('menus.dashboardLatencyTrend', { n: s.latencyHistory.length })
            "
          >
            <svg
              :width="80"
              :height="24"
              :viewBox="`0 0 80 24`"
              preserveAspectRatio="none"
              style="width: 100%; height: 24px"
            >
              <path
                :d="sparklinePath(s.latencyHistory)"
                fill="none"
                :stroke="sparklineColor(s)"
                stroke-width="1.5"
              />
            </svg>
          </div>

          <div class="mt-2 flex items-end justify-between text-xs text-gray-500">
            <span>
              {{ t("menus.dashboardLastCheck") }}: {{ fmtTime(s.lastCheckedAt) }}
            </span>
            <span v-if="s.latencyMs !== null">
              {{ s.latencyMs }} ms
            </span>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-card shadow="never" class="mt-2">
      <template #header>
        <div class="flex items-center justify-between">
          <span class="font-semibold">{{ t("menus.dashboardRecentActivity") }}</span>
          <el-button
            link
            type="primary"
            size="small"
            @click="refreshStats"
          >
            {{ t("menus.dashboardRefresh") }}
          </el-button>
        </div>
      </template>
      <div
        v-if="recentOps.length === 0"
        class="text-center text-gray-400 py-6 text-sm"
      >
        {{ t("menus.dashboardRecentActivityEmpty") }}
      </div>
      <el-table v-else :data="recentOps" size="small" stripe>
        <el-table-column
          :label="t('menus.dashboardColUser')"
          prop="user_id"
          width="160"
        />
        <el-table-column
          :label="t('menus.dashboardColAction')"
          prop="action"
          width="120"
        />
        <el-table-column
          :label="t('menus.dashboardColResource')"
          prop="resource"
        />
        <el-table-column
          :label="t('menus.dashboardColTime')"
          width="180"
        >
          <template #default="{ row }">
            {{ fmtTime(row.operating_time || row.created_at || row.occurred_at) }}
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>