<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { createPresign } from "@/api/file";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";

defineOptions({ name: "FilePresign" });
const { t } = useI18n();
const form = ref({
  file_id: "",
  operation: "get" as "get" | "put",
  expires_in_seconds: 3600
});
const loading = ref(false);
const presigned = ref<{ url: string; expires_at: string } | null>(null);

async function onSubmit() {
  if (!form.value.file_id) {
    message(t("menus.presignInputFileIdFirst"), { type: "warning" });
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
    message(t("menus.presignGenerated"), { type: "success" });
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.presignGenerateFailFallback")), {
      type: "error"
    });
  } finally {
    loading.value = false;
  }
}

function copyUrl() {
  if (!presigned.value) return;
  navigator.clipboard.writeText(presigned.value.url);
  message(t("menus.presignCopied"), { type: "success" });
}
</script>

<template>
  <div class="main p-6">
    <h3 class="mb-4">{{ t("menus.presignTitle") }}</h3>
    <el-alert
      :title="t('menus.presignNotice')"
      type="info"
      show-icon
      :closable="false"
      class="mb-4"
    />
    <el-form :model="form" label-width="160px" class="w-150">
      <el-form-item :label="t('menus.presignFileId')">
        <el-input
          v-model="form.file_id"
          :placeholder="t('menus.presignFileIdPlaceholder')"
        />
      </el-form-item>
      <el-form-item :label="t('menus.presignOperation')">
        <el-radio-group v-model="form.operation">
          <el-radio value="get">{{ t("menus.presignOperationGet") }}</el-radio>
          <el-radio value="put">{{ t("menus.presignOperationPut") }}</el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item :label="t('menus.presignExpires')">
        <el-input-number
          v-model="form.expires_in_seconds"
          :min="60"
          :max="86400"
        />
      </el-form-item>
      <el-form-item>
        <el-button
          v-if="hasPerms(['FILE_UPLOAD'])"
          type="primary"
          :loading="loading"
          @click="onSubmit"
          >{{ t("menus.presignGenerateBtn") }}</el-button
        >
      </el-form-item>
    </el-form>
    <el-card v-if="presigned" class="mt-4">
      <template #header>{{ t("menus.presignResultHeader") }}</template>
      <p class="break-all">
        {{ t("menus.presignUrlLabel") }}: <code>{{ presigned.url }}</code>
      </p>
      <p>{{ t("menus.presignExpiresLabel") }}: {{ presigned.expires_at }}</p>
      <el-button @click="copyUrl">{{ t("menus.presignCopyUrl") }}</el-button>
    </el-card>
  </div>
</template>
