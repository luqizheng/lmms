<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from "vue";
import { $t } from "@/plugins/i18n";

defineOptions({
  name: "DashboardIndex"
});

interface ServiceStatus {
  name: string;
  slug: string;
  probePath: string;
  port: number;
  status: "ok" | "down" | "unknown";
  latencyMs: number | null;
  lastCheckedAt: number | null;
}

const SERVICES: Array<Omit<ServiceStatus, "status" | "latencyMs" | "lastCheckedAt">> = [
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
  SERVICES.map(s => ({ ...s, status: "unknown", latencyMs: null, lastCheckedAt: null }))
);

const POLL_MS = 5000;
let timer: ReturnType<typeof setInterval> | null = null;

async function probeOne(s: ServiceStatus): Promise<ServiceStatus> {
  const t0 = performance.now();
  try {
    const resp = await fetch(`${s.probePath}/health`, {
      method: "GET",
      cache: "no-store"
    });
    const latencyMs = Math.round(performance.now() - t0);
    return {
      ...s,
      status: resp.ok ? "ok" : "down",
      latencyMs,
      lastCheckedAt: Date.now()
    };
  } catch {
    return {
      ...s,
      status: "down",
      latencyMs: null,
      lastCheckedAt: Date.now()
    };
  }
}

async function refreshAll() {
  const results = await Promise.all(statuses.value.map(probeOne));
  statuses.value = results;
}

const upCount = computed(() => statuses.value.filter(s => s.status === "ok").length);
const downCount = computed(() => statuses.value.filter(s => s.status === "down").length);
const lastRefreshAt = ref<number | null>(null);

function fmtTime(ts: number | null) {
  if (ts === null) return "—";
  const d = new Date(ts);
  return d.toLocaleTimeString();
}

function statusTag(status: ServiceStatus["status"]): {
  type: "success" | "danger" | "info";
  label: string;
} {
  switch (status) {
    case "ok":
      return { type: "success", label: $t("menus.dashboardStatusOk") };
    case "down":
      return { type: "danger", label: $t("menus.dashboardStatusDown") };
    default:
      return { type: "info", label: $t("menus.dashboardStatusUnknown") };
  }
}

onMounted(async () => {
  await refreshAll();
  lastRefreshAt.value = Date.now();
  timer = setInterval(async () => {
    await refreshAll();
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
      <el-col :xs="24" :sm="8">
        <el-card shadow="never">
          <div class="text-center">
            <div class="text-3xl font-bold text-green-500">{{ upCount }}</div>
            <div class="text-sm text-gray-500">
              {{ $t("menus.dashboardUpCount") }} / {{ statuses.length }}
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="8">
        <el-card shadow="never">
          <div class="text-center">
            <div class="text-3xl font-bold text-red-500">{{ downCount }}</div>
            <div class="text-sm text-gray-500">{{ $t("menus.dashboardDownCount") }}</div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="8">
        <el-card shadow="never">
          <div class="text-center">
            <div class="text-sm text-gray-500">
              {{ $t("menus.dashboardLastCheck") }}
            </div>
            <div class="text-lg">{{ fmtTime(lastRefreshAt) }}</div>
            <el-button
              type="primary"
              size="small"
              class="mt-2"
              @click="refreshAll"
            >
              {{ $t("menus.dashboardRefresh") }}
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
          <div class="flex items-center justify-between">
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
          <div class="mt-3 flex items-end justify-between text-xs text-gray-500">
            <span>
              {{ $t("menus.dashboardLastCheck") }}: {{ fmtTime(s.lastCheckedAt) }}
            </span>
            <span v-if="s.latencyMs !== null">
              {{ s.latencyMs }} ms
            </span>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>