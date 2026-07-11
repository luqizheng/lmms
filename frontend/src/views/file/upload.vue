<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import type { UploadFile, UploadRawFile } from "element-plus";
import { useI18n } from "vue-i18n";
import { uploadFile } from "@/api/file";
import {
  checkFileSize,
  checkFileExtension,
  FILE_MAX_SIZE_MB
} from "@/api/file/limits";
import { hasPerms } from "@/utils/auth";

defineOptions({ name: "FileUpload" });

const { t } = useI18n();

interface FileResult {
  name: string;
  status: "ok" | "fail";
  message: string;
}

const fileList = ref<UploadFile[]>([]);
const submitting = ref(false);
const isTemporary = ref(false);
const expiresIn = ref<number | undefined>(undefined);
const results = ref<FileResult[]>([]);
const previewVisible = ref(false);
const previewUrl = ref<string>("");
const previewTitle = ref<string>("");

function isImage(name: string): boolean {
  return /\.(png|jpe?g|gif|webp|bmp|svg)$/i.test(name);
}

function onBefore(raw: UploadRawFile): boolean {
  if (!checkFileSize(raw.size)) {
    ElMessage.error(
      t("menus.fileTooLarge", { name: raw.name, limit: FILE_MAX_SIZE_MB })
    );
    return false;
  }
  if (!checkFileExtension(raw.name)) {
    ElMessage.error(t("menus.fileExtensionBlocked", { name: raw.name }));
    return false;
  }
  return true;
}

function onPreview(file: UploadFile) {
  if (file.raw && isImage(file.name)) {
    previewUrl.value = URL.createObjectURL(file.raw);
    previewTitle.value = file.name;
    previewVisible.value = true;
  }
}

function closePreview() {
  if (previewUrl.value) URL.revokeObjectURL(previewUrl.value);
  previewUrl.value = "";
  previewVisible.value = false;
}

async function onSubmit() {
  if (!fileList.value.length) {
    ElMessage.warning(t("menus.fileUploadSelectFirst"));
    return;
  }
  submitting.value = true;
  results.value = [];
  let ok = 0;
  let fail = 0;
  try {
    for (const f of fileList.value) {
      if (!f.raw) continue;
      try {
        await uploadFile(f.raw, {
          content_type: f.raw.type || "application/octet-stream",
          is_temporary: isTemporary.value,
          expires_in_seconds: expiresIn.value
        });
        ok++;
        results.value.push({ name: f.name, status: "ok", message: "" });
      } catch (e) {
        fail++;
        const msg = e instanceof Error ? e.message : String(e);
        results.value.push({ name: f.name, status: "fail", message: msg });
      }
    }
    ElMessage.success(t("menus.fileUploadResultSummary", { ok, fail }));
    if (fail === 0) fileList.value = [];
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <div class="main p-6">
    <h3 class="mb-3">{{ t("menus.fileUploadTitle") }}</h3>
    <el-alert
      :title="t('menus.fileUploadBackendNotice')"
      type="warning"
      show-icon
      :closable="false"
      class="mb-4"
    />
    <el-form label-width="120px">
      <el-form-item :label="t('menus.fileLabel')">
        <el-upload
          v-model:file-list="fileList"
          :auto-upload="false"
          :before-upload="onBefore"
          :on-preview="onPreview"
          drag
          multiple
        >
          <el-icon style="font-size: 48px"
            ><IconifyIconOffline icon="ep:upload-filled"
          /></el-icon>
          <div class="el-upload__text">{{ t("menus.fileDragOrClick") }}</div>
        </el-upload>
      </el-form-item>
      <el-form-item :label="t('menus.fileLabelTemporary')">
        <el-switch v-model="isTemporary" />
      </el-form-item>
      <el-form-item :label="t('menus.fileLabelExpires')">
        <el-input-number
          v-model="expiresIn"
          :min="60"
          :max="86400"
          :placeholder="t('menus.fileExpiresOptional')"
        />
      </el-form-item>
      <el-form-item>
        <el-button
          v-if="hasPerms(['FILE_UPLOAD'])"
          type="primary"
          :loading="submitting"
          @click="onSubmit"
          >{{ t("menus.fileUploadSubmit") }}</el-button
        >
      </el-form-item>
    </el-form>

    <div v-if="results.length" class="mt-4">
      <el-divider>{{ t("menus.fileUploadTitle") }}</el-divider>
      <ul class="upload-results">
        <li
          v-for="r in results"
          :key="r.name + r.status"
          :class="['upload-result', r.status]"
        >
          <el-tag :type="r.status === 'ok' ? 'success' : 'danger'" size="small">
            {{ r.status === "ok" ? "✓" : "✗" }}
          </el-tag>
          <span class="ml-2">{{ r.name }}</span>
          <span v-if="r.message" class="ml-2 text-gray-400">{{
            r.message
          }}</span>
        </li>
      </ul>
    </div>

    <el-image-viewer
      v-if="previewVisible"
      :url-list="[previewUrl]"
      :title="previewTitle"
      :hide-on-click-modal="true"
      @close="closePreview"
    />
  </div>
</template>

<style lang="scss" scoped>
.upload-results {
  list-style: none;
  margin: 0;
  padding: 0;
}

.upload-result {
  padding: 6px 8px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  &.fail {
    background: var(--el-color-danger-light-9);
  }
  &.ok {
    background: var(--el-color-success-light-9);
  }
}
</style>
