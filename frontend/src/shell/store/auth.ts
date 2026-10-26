import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { login as loginApi, logout as logoutApi } from "../api";
import {
  clearSession,
  getStoredUser,
  saveSession,
  type StoredUser
} from "../api/token-storage";

export const useAuthStore = defineStore("prism-auth", () => {
  const user = ref<StoredUser | null>(getStoredUser());

  const authenticated = computed(() => Boolean(user.value));
  const roles = computed(() => user.value?.roles ?? []);
  const permissions = computed(() => user.value?.permissions ?? []);

  async function signIn(
    username: string,
    password: string
  ): Promise<StoredUser> {
    const result = await loginApi({ username, password });
    saveSession({
      accessToken: result.access_token,
      refreshToken: result.refresh_token,
      expiresIn: result.expires_in,
      user: result.user
    });
    user.value = result.user;
    return result.user;
  }

  async function signOut(): Promise<void> {
    try {
      await logoutApi();
    } finally {
      clearSession();
      user.value = null;
    }
  }

  /** 退出登录（不请求后端）—— token 失效时使用 */
  function reset() {
    clearSession();
    user.value = null;
  }

  function hasPermission(code: string): boolean {
    return permissions.value.includes(code);
  }

  function hasRole(code: string): boolean {
    return roles.value.includes(code);
  }

  return {
    user,
    authenticated,
    roles,
    permissions,
    signIn,
    signOut,
    reset,
    hasPermission,
    hasRole
  };
});
