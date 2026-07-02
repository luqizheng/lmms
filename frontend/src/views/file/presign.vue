<script setup lang="ts">
import { ref } from "vue";
import { createPresign } from "@/api/file";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";

defineOptions({ name: "FilePresign" });
const form = ref({
  file_id: "",
  operation: "get" as "get" | "put",
  expires_in_seconds: 3600
});
const loading = ref(false);
const presigned = ref<{ url: string; expires_at: string } | null>(null);

async function onSubmit() {
  if (!form.value.file_id) {
    message("请输入 file_id", { type: "warning" });
    return;
  }
  loading.value = true;
  try {
    const res = await createPresign({
      file_id: form.value.file_id,
      operation: form.value.operation,
      expires_in_seconds: form.value.expires_in_seconds
    });
    presigned.value = { url: res.url, expires_at: res.expires_at };
    message("已生成", { type: "success" });
  } catch (e: any) {
    message(translateError(e?.code ?? "生成失败"), { type: "error" });
  } finally {
    loading.value = false;
  }
}

function copyUrl() {
  if (!presigned.value) return;
  navigator.clipboard.writeText(presigned.value.url);
  message("URL 已复制", { type: "success" });
}
</script>

<template>
  <div class="main p-6">
    <h3 class="mb-4">预签名 URL</h3>
    <el-alert
      title="对已存在的 file_id 生成 GET/PUT 临时 URL。直传请用 /file/upload。"
      type="info"
      show-icon
      :closable="false"
      class="mb-4"
    />
    <el-form :model="form" label-width="160px" class="w-150">
      <el-form-item label="File ID">
        <el-input v-model="form.file_id" placeholder="file id" />
      </el-form-item>
      <el-form-item label="操作">
        <el-radio-group v-model="form.operation">
          <el-radio value="get">GET（下载）</el-radio>
          <el-radio value="put">PUT（覆盖）</el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item label="过期秒数">
        <el-input-number v-model="form.expires_in_seconds" :min="60" :max="86400" />
      </el-form-item>
      <el-form-item>
        <el-button v-if="hasPerms(['FILE_UPLOAD'])" type="primary" :loading="loading" @click="onSubmit">生成 URL</el-button>
      </el-form-item>
    </el-form>
    <el-card v-if="presigned" class="mt-4">
      <template #header>预签名结果</template>
      <p class="break-all">URL: <code>{{ presigned.url }}</code></p>
      <p>过期: {{ presigned.expires_at }}</p>
      <el-button @click="copyUrl">复制 URL</el-button>
    </el-card>
  </div>
</template>