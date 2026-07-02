<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { ElMessageBox } from "element-plus";
import { message } from "@/utils/message";
import {
  getRetentionPolicy,
  getRetentionStats,
  triggerRetentionCleanup,
  updateRetentionPolicy,
  getDataStage,
  type RetentionPolicy,
  type RetentionStats
} from "@/api/audit";

defineOptions({
  name: "MonitorRetention"
});

const loading = ref(false);
const saving = ref(false);
const cleaning = ref(false);
const stats = ref<RetentionStats>({
  hot_count: 0,
  warm_count: 0,
  cold_count: 0,
  expired_count: 0,
  total_size_bytes: 0
});
const policy = reactive<RetentionPolicy>({
  hot_days: 90,
  warm_days: 365,
  cold_days: 2555,
  archive_enabled: true,
  purge_enabled: true
});

function formatBytes(n: number) {
  if (!n) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(2)} ${units[i]}`;
}

async function load() {
  loading.value = true;
  try {
    const [s, p] = await Promise.all([
      getRetentionStats(),
      getRetentionPolicy()
    ]);
    stats.value = s;
    Object.assign(policy, p);
  } catch (e: any) {
    message(`加载失败：${e?.message || e}`, { type: "error" });
  } finally {
    loading.value = false;
  }
}

async function onSave() {
  saving.value = true;
  try {
    await updateRetentionPolicy({ ...policy });
    message("保存成功", { type: "success" });
  } catch (e: any) {
    message(`保存失败：${e?.message || e}`, { type: "error" });
  } finally {
    saving.value = false;
  }
}

async function onCleanup() {
  try {
    await ElMessageBox.confirm(
      "立即触发清理将根据当前策略删除过期数据，确认继续？",
      "系统提示",
      { type: "warning" }
    );
  } catch {
    return;
  }
  cleaning.value = true;
  try {
    const res = await triggerRetentionCleanup();
    message(
      `清理完成，删除 ${res.deleted_count} 条${res.success ? "" : "（部分失败）"}`,
      { type: res.success ? "success" : "warning" }
    );
    load();
  } catch (e: any) {
    message(`清理失败：${e?.message || e}`, { type: "error" });
  } finally {
    cleaning.value = false;
  }
}

onMounted(load);

// ============== Stage 探针 ==============
const probeInput = ref<string>(
  new Date(Date.now() - 100 * 24 * 3600 * 1000)
    .toISOString()
    .replace(/\.\d+Z$/, "Z")
);
const probeResult = ref<{ stage: string; policy: RetentionPolicy } | null>(
  null
);
const probeLoading = ref(false);

async function onProbe() {
  if (!probeInput.value) {
    message("请输入 RFC3339 时间戳", { type: "warning" });
    return;
  }
  probeLoading.value = true;
  try {
    const res = await getDataStage(probeInput.value);
    probeResult.value = { stage: res.stage, policy: res.policy };
  } catch (e: any) {
    message(`查询失败：${e?.message || e}`, { type: "error" });
    probeResult.value = null;
  } finally {
    probeLoading.value = false;
  }
}

function stageTagType(
  stage: string
): "success" | "warning" | "info" | "danger" {
  switch (stage) {
    case "Hot":
      return "success";
    case "Warm":
      return "warning";
    case "Cold":
      return "info";
    case "Expired":
      return "danger";
    default:
      return "info";
  }
}
</script>

<template>
  <div v-loading="loading" class="main p-4">
    <el-row :gutter="16">
      <el-col :xs="12" :sm="8" :md="5">
        <el-card shadow="never">
          <div class="text-sm text-gray-500">热数据</div>
          <div class="text-2xl font-bold">{{ stats.hot_count }}</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="5">
        <el-card shadow="never">
          <div class="text-sm text-gray-500">温数据</div>
          <div class="text-2xl font-bold">{{ stats.warm_count }}</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="5">
        <el-card shadow="never">
          <div class="text-sm text-gray-500">冷数据</div>
          <div class="text-2xl font-bold">{{ stats.cold_count }}</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="5">
        <el-card shadow="never">
          <div class="text-sm text-gray-500">过期数据</div>
          <div class="text-2xl font-bold text-red-500">
            {{ stats.expired_count }}
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="24" :md="4">
        <el-card shadow="never">
          <div class="text-sm text-gray-500">总大小</div>
          <div class="text-2xl font-bold">
            {{ formatBytes(stats.total_size_bytes) }}
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-card class="mt-4" shadow="never">
      <template #header>
        <div class="flex-bc">
          <span>保留策略</span>
          <div>
            <el-button :loading="cleaning" type="warning" @click="onCleanup">
              立即清理
            </el-button>
            <el-button :loading="saving" type="primary" @click="onSave">
              保存策略
            </el-button>
          </div>
        </div>
      </template>
      <el-form :model="policy" label-width="160px" class="max-w-3xl">
        <el-form-item label="热数据保留(天)">
          <el-input-number
            v-model="policy.hot_days"
            :min="1"
            :max="3650"
            controls-position="right"
          />
        </el-form-item>
        <el-form-item label="温数据保留(天)">
          <el-input-number
            v-model="policy.warm_days"
            :min="1"
            :max="3650"
            controls-position="right"
          />
        </el-form-item>
        <el-form-item label="冷数据保留(天)">
          <el-input-number
            v-model="policy.cold_days"
            :min="1"
            :max="36500"
            controls-position="right"
          />
        </el-form-item>
        <el-form-item label="启用归档">
          <el-switch v-model="policy.archive_enabled" />
        </el-form-item>
        <el-form-item label="启用清理">
          <el-switch v-model="policy.purge_enabled" />
        </el-form-item>
      </el-form>
    </el-card>

    <el-card class="mt-4" shadow="never">
      <template #header>
        <span>Stage 探针（输入 created_at 看落点）</span>
      </template>
      <el-form label-width="160px" class="max-w-3xl">
        <el-form-item label="RFC3339 时间">
          <el-input v-model="probeInput" placeholder="2024-01-01T00:00:00Z" />
        </el-form-item>
        <el-form-item>
          <el-button :loading="probeLoading" type="primary" @click="onProbe"
            >查询</el-button
          >
        </el-form-item>
        <el-form-item v-if="probeResult" label="结果">
          <el-tag :type="stageTagType(probeResult.stage)" size="large">
            {{ probeResult.stage }}
          </el-tag>
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>
