<script setup lang="ts">
import { ref, reactive, onMounted } from "vue";
import { useRouter } from "vue-router";
import { message } from "@/utils/message";
import { useUserStoreHook } from "@/store/modules/user";
import { register, getSetupStatus } from "@/api/auth";
import Lock from "~icons/ri/lock-fill";
import User from "~icons/ri/user-3-fill";
import { avatar } from "./utils/static";

defineOptions({ name: "Register" });

const router = useRouter();
const loading = ref(false);

const form = reactive({
  username: "",
  password: "",
  confirm: "",
  email: ""
});

onMounted(async () => {
  try {
    const r = await getSetupStatus();
    if (!r.setup_required) {
      message("系统已就绪，请直接登录", { type: "info" });
      router.push("/login");
    }
  } catch (_e) {
    message("无法获取系统状态", { type: "error" });
  }
});

const onRegister = async () => {
  if (!form.username || !form.password) {
    message("用户名和密码必填", { type: "error" });
    return;
  }
  if (form.password !== form.confirm) {
    message("两次密码不一致", { type: "error" });
    return;
  }
  loading.value = true;
  try {
    await register({
      username: form.username,
      password: form.password,
      nickname: form.username,
      email: form.email || undefined
    });
    await useUserStoreHook().loginByUsername({
      username: form.username,
      password: form.password
    });
    message("注册成功", { type: "success" });
    router.push("/welcome");
  } catch (e: any) {
    const msg =
      e?.response?.data?.error ??
      e?.response?.data?.message ??
      e?.message ??
      "注册失败";
    message(msg, { type: "error" });
  } finally {
    loading.value = false;
  }
};
</script>

<template>
  <div class="select-none">
    <div class="login-container">
      <div class="login-form">
        <avatar class="avatar" />
        <h2 class="outline-hidden">注册管理员</h2>
        <el-form :model="form" size="large" @submit.prevent>
          <el-form-item>
            <el-input
              v-model="form.username"
              clearable
              placeholder="用户名（3-32 字符）"
            >
              <template #prefix><User /></template>
            </el-input>
          </el-form-item>
          <el-form-item>
            <el-input
              v-model="form.password"
              show-password
              clearable
              placeholder="密码（≥ 6 位）"
            >
              <template #prefix><Lock /></template>
            </el-input>
          </el-form-item>
          <el-form-item>
            <el-input
              v-model="form.confirm"
              show-password
              clearable
              placeholder="确认密码"
            >
              <template #prefix><Lock /></template>
            </el-input>
          </el-form-item>
          <el-form-item>
            <el-input
              v-model="form.email"
              clearable
              placeholder="邮箱（可选）"
            />
          </el-form-item>
          <el-form-item>
            <el-button
              class="w-full"
              type="primary"
              :loading="loading"
              @click="onRegister"
            >
              注册并登录
            </el-button>
          </el-form-item>
          <el-form-item>
            <router-link to="/login" class="text-primary">返回登录</router-link>
          </el-form-item>
        </el-form>
      </div>
    </div>
  </div>
</template>

<style scoped>
@import url("@/style/login.css");
</style>
