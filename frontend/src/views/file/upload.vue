<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import type { UploadFile, UploadRawFile } from "element-plus";
import { uploadFile } from "@/api/file";
import {
  checkFileSize,
  checkFileExtension,
  FILE_MAX_SIZE_MB
} from "@/api/file/limits";
import { hasPerms } from "@/utils/auth";

defineOptions({ name: "FileUpload" });

const fileList = ref<UploadFile[]>([]);
const submitting = ref(false);
const isTemporary = ref(false);
const expiresIn = ref<number | undefined>(undefined);

function onBefore(raw: UploadRawFile): boolean {
  if (!checkFileSize(raw.size)) {
    ElMessage.error(`${raw.name} 超过 ${FILE_MAX_SIZE_MB}MB`);
    return false;
  }
  if (!checkFileExtension(raw.name)) {
    ElMessage.error(`${raw.name} 是禁止的可执行扩展名`);
    return false;
  }
  return true;
}

async function onSubmit() {
  if (!fileList.value.length) {
    ElMessage.warning("请先选文件");
    return;
  }
  submitting.value = true;
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
      } catch {
        fail++;
      }
    }
    ElMessage.success(`成功 ${ok}，失败 ${fail}`);
    fileList.value = [];
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <div class="main p-6">
    <h3 class="mb-3">上传文件</h3>
    <el-alert
      title="后端 /file/upload 当前仅接 metadata，不接受二进制内容。前端先发文件名+content-type 占位，待后端补 multipart 后自动生效。客户端已校验大小 ≤ 100MB + 黑名单扩展名。"
      type="warning"
      show-icon
      :closable="false"
      class="mb-4"
    />
    <el-form label-width="120px">
      <el-form-item label="文件">
        <el-upload
          v-model:file-list="fileList"
          :auto-upload="false"
          :before-upload="onBefore"
          drag
          multiple
        >
          <el-icon style="font-size: 48px"
            ><IconifyIconOffline icon="ep:upload-filled"
          /></el-icon>
          <div class="el-upload__text">拖入或点击选择文件</div>
        </el-upload>
      </el-form-item>
      <el-form-item label="临时文件">
        <el-switch v-model="isTemporary" />
      </el-form-item>
      <el-form-item label="过期秒数">
        <el-input-number
          v-model="expiresIn"
          :min="60"
          :max="86400"
          placeholder="可选"
        />
      </el-form-item>
      <el-form-item>
        <el-button
          v-if="hasPerms(['FILE_UPLOAD'])"
          type="primary"
          :loading="submitting"
          @click="onSubmit"
          >提交上传</el-button
        >
      </el-form-item>
    </el-form>
  </div>
</template>
