<script setup lang="ts">
import { ref } from "vue";
import { listResources } from "@/api/token";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { ElTag } from "element-plus";

defineOptions({ name: "TokenResourceProbe" });
const loading = ref(false);
const resources = ref<string[]>([]);

async function onProbe() {
  loading.value = true;
  try {
    const res = await listResources();
    resources.value = res || [];
    message(`共 ${resources.value.length} 个资源`, { type: "success" });
  } catch (e: any) {
    message(translateError(e?.code ?? "查询失败"), { type: "error" });
    resources.value = [];
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div>
    <el-button :loading="loading" @click="onProbe">资源探针</el-button>
    <div v-if="resources.length" class="mt-3 flex flex-wrap gap-2">
      <el-tag v-for="r in resources" :key="r" type="info" effect="plain">
        {{ r }}
      </el-tag>
    </div>
    <el-empty
      v-else
      description="点击「资源探针」拉取后端可用资源标识"
      :image-size="60"
    />
  </div>
</template>
