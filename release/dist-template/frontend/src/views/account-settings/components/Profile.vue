<script setup lang="ts">
import { message } from "@/utils/message";
import { onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { type UserInfo, getMine } from "@/api/user";
import { uploadFile } from "@/api/file";
import type { FormInstance, FormRules } from "element-plus";
import ReCropperPreview from "@/components/ReCropperPreview";
import { deviceDetection } from "@pureadmin/utils";
import uploadLine from "~icons/ri/upload-line";

defineOptions({
  name: "Profile"
});

const { t } = useI18n();

const imgSrc = ref("");
const cropperBlob = ref<Blob | undefined>();
const cropRef = ref();
const uploadRef = ref();
const isShow = ref(false);
const submittingAvatar = ref(false);
const userInfoFormRef = ref<FormInstance>();

const userInfos = reactive({
  avatar: "",
  nickname: "",
  email: "",
  phone: "",
  description: ""
});

const rules = reactive<FormRules<UserInfo>>({
  nickname: [
    {
      required: true,
      message: t("menus.profileNicknameRequired"),
      trigger: "blur"
    }
  ]
});

function queryEmail(
  queryString: string,
  callback: (results: unknown[]) => void
) {
  const emailList = [
    { value: "@qq.com" },
    { value: "@126.com" },
    { value: "@163.com" }
  ];
  const results: { value: string }[] = [];
  if (queryString) {
    const prefix = queryString.split("@")[0] || "";
    const q = queryString.toLowerCase();
    for (const item of emailList) {
      const candidate = `${prefix}${item.value}`;
      if (candidate.toLowerCase().indexOf(q) === 0) {
        results.push({ value: candidate });
      }
    }
  } else {
    const prefix = queryString.split("@")[0] || "";
    for (const item of emailList) {
      results.push({ value: `${prefix}${item.value}` });
    }
  }
  callback(results);
}

const onChange = (uploadFile: { raw?: File }) => {
  if (!uploadFile.raw) return;
  const reader = new FileReader();
  reader.onload = e => {
    imgSrc.value = e.target?.result as string;
    isShow.value = true;
  };
  reader.readAsDataURL(uploadFile.raw);
};

const handleClose = () => {
  cropRef.value?.hidePopover();
  uploadRef.value?.clearFiles();
  isShow.value = false;
};

const onCropper = ({ blob }: { blob: Blob }) => {
  cropperBlob.value = blob;
};

async function handleSubmitImage() {
  if (!cropperBlob.value || submittingAvatar.value) return;
  submittingAvatar.value = true;
  try {
    const file = new File([cropperBlob.value], "avatar.png", {
      type: "image/png"
    });
    const res = await uploadFile(file, {
      content_type: file.type,
      is_temporary: false
    });
    if (res?.id) {
      // 优先用后端返回的 url（相对路径，前端 axios baseURL /api 自动拼接）
      if (res.url) userInfos.avatar = res.url;
      message(t("menus.profileUpdateAvatarSuccess"), { type: "success" });
      handleClose();
    } else {
      message(t("menus.profileUpdateAvatarFail"));
    }
  } catch (error) {
    message(t("menus.profileUpdateAvatarError", { error }), {
      type: "error"
    });
  } finally {
    submittingAvatar.value = false;
  }
}

const onSubmit = async (formEl: FormInstance | undefined) => {
  if (!formEl) return;
  await formEl.validate((valid, fields) => {
    if (valid) {
      message(t("menus.profileUpdateInfoSuccess"), { type: "success" });
    } else if (fields) {
      message(
        Object.values(fields)
          .map(f => f[0]?.message)
          .filter(Boolean)
          .join("; "),
        { type: "error" }
      );
    }
  });
};

onMounted(async () => {
  const { code, data } = await getMine();
  if (code === 0 && data) {
    Object.assign(userInfos, data);
  }
});
</script>

<template>
  <div :class="['min-w-45', deviceDetection() ? 'max-w-full' : 'max-w-[70%]']">
    <h3 class="my-8!">{{ t("menus.pureProfile") }}</h3>
    <el-form
      ref="userInfoFormRef"
      label-position="top"
      :rules="rules"
      :model="userInfos"
    >
      <el-form-item :label="t('menus.profileAvatar')">
        <el-avatar :size="80" :src="userInfos.avatar" />
        <el-upload
          ref="uploadRef"
          accept="image/*"
          action="#"
          :limit="1"
          :auto-upload="false"
          :show-file-list="false"
          :on-change="onChange"
        >
          <el-button plain class="ml-4!">
            <IconifyIconOffline :icon="uploadLine" />
            <span class="ml-2">{{ t("menus.profileUpdateAvatarBtn") }}</span>
          </el-button>
        </el-upload>
      </el-form-item>
      <el-form-item :label="t('menus.profileNickname')" prop="nickname">
        <el-input
          v-model="userInfos.nickname"
          :placeholder="t('menus.profileNicknamePlaceholder')"
        />
      </el-form-item>
      <el-form-item :label="t('menus.profileEmail')" prop="email">
        <el-autocomplete
          v-model="userInfos.email"
          :fetch-suggestions="queryEmail"
          :trigger-on-focus="false"
          :placeholder="t('menus.profileEmailPlaceholder')"
          clearable
          class="w-full"
        />
      </el-form-item>
      <el-form-item :label="t('menus.profilePhone')">
        <el-input
          v-model="userInfos.phone"
          :placeholder="t('menus.profilePhonePlaceholder')"
          clearable
        />
      </el-form-item>
      <el-form-item :label="t('menus.profileDescription')">
        <el-input
          v-model="userInfos.description"
          :placeholder="t('menus.profileDescriptionPlaceholder')"
          type="textarea"
          :autosize="{ minRows: 6, maxRows: 8 }"
          maxlength="56"
          show-word-limit
        />
      </el-form-item>
      <el-button type="primary" @click="onSubmit(userInfoFormRef)">
        {{ t("menus.profileUpdateInfo") }}
      </el-button>
    </el-form>
    <el-dialog
      v-model="isShow"
      width="40%"
      :title="t('menus.profileEditAvatarTitle')"
      destroy-on-close
      :close-on-click-modal="false"
      :before-close="handleClose"
      :fullscreen="deviceDetection()"
    >
      <ReCropperPreview ref="cropRef" :imgSrc="imgSrc" @cropper="onCropper" />
      <template #footer>
        <div class="dialog-footer">
          <el-button bg text @click="handleClose">
            {{ t("menus.profileCancel") }}
          </el-button>
          <el-button
            bg
            text
            type="primary"
            :loading="submittingAvatar"
            @click="handleSubmitImage"
          >
            {{ t("menus.profileConfirm") }}
          </el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>
