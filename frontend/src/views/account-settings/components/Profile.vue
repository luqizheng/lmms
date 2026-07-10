<script setup lang="ts">
import { formUpload } from "@/api/mock";
import { message } from "@/utils/message";
import { onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { type UserInfo, getMine } from "@/api/user";
import type { FormInstance, FormRules } from "element-plus";
import ReCropperPreview from "@/components/ReCropperPreview";
import { createFormData, deviceDetection } from "@pureadmin/utils";
import uploadLine from "~icons/ri/upload-line";

defineOptions({
  name: "Profile"
});

const { t } = useI18n();

const imgSrc = ref("");
const cropperBlob = ref();
const cropRef = ref();
const uploadRef = ref();
const isShow = ref(false);
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

function queryEmail(queryString, callback) {
  const emailList = [
    { value: "@qq.com" },
    { value: "@126.com" },
    { value: "@163.com" }
  ];
  let results = [];
  let queryList = [];
  emailList.map(item =>
    queryList.push({ value: queryString.split("@")[0] + item.value })
  );
  results = queryString
    ? queryList.filter(
        item =>
          item.value.toLowerCase().indexOf(queryString.toLowerCase()) === 0
      )
    : queryList;
  callback(results);
}

const onChange = uploadFile => {
  const reader = new FileReader();
  reader.onload = e => {
    imgSrc.value = e.target.result as string;
    isShow.value = true;
  };
  reader.readAsDataURL(uploadFile.raw);
};

const handleClose = () => {
  cropRef.value.hidePopover();
  uploadRef.value.clearFiles();
  isShow.value = false;
};

const onCropper = ({ blob }) => (cropperBlob.value = blob);

const handleSubmitImage = () => {
  const formData = createFormData({
    files: new File([cropperBlob.value], "avatar")
  });
  formUpload(formData)
    .then(({ code }) => {
      if (code === 0) {
        message(t("menus.profileUpdateAvatarSuccess"), { type: "success" });
        handleClose();
      } else {
        message(t("menus.profileUpdateAvatarFail"));
      }
    })
    .catch(error => {
      message(t("menus.profileUpdateAvatarError", { error }), {
        type: "error"
      });
    });
};

const onSubmit = async (formEl: FormInstance) => {
  await formEl.validate((valid, fields) => {
    if (valid) {
      console.log(userInfos);
      message(t("menus.profileUpdateInfoSuccess"), { type: "success" });
    } else {
      console.log("error submit!", fields);
    }
  });
};

onMounted(async () => {
  const { code, data } = await getMine();
  if (code === 0) {
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
      :closeOnClickModal="false"
      :before-close="handleClose"
      :fullscreen="deviceDetection()"
    >
      <ReCropperPreview ref="cropRef" :imgSrc="imgSrc" @cropper="onCropper" />
      <template #footer>
        <div class="dialog-footer">
          <el-button bg text @click="handleClose">
            {{ t("menus.profileCancel") }}
          </el-button>
          <el-button bg text type="primary" @click="handleSubmitImage">
            {{ t("menus.profileConfirm") }}
          </el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>