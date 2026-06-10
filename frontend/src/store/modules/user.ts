import { defineStore } from "pinia";
import {
  type userType,
  store,
  router,
  resetRouter,
  routerArrays,
  storageLocal
} from "../utils";
import { login, type LoginRequest } from "@/api/auth";
import { getCurrentUser, type User } from "@/api/user";
import { listPermissions } from "@/api/permission";
import { createAccessToken, refreshToken as refreshTokenApi } from "@/api/token";
import { useMultiTagsStoreHook } from "./multiTags";
import {
  type DataInfo,
  setToken,
  removeToken,
  userKey
} from "@/utils/auth";

const CLIENT_ID = "api-gateway";
const CLIENT_SECRET =
  (import.meta.env.VITE_CLIENT_SECRET as string | undefined) ?? "";

export const useUserStore = defineStore("pure-user", {
  state: (): userType => ({
    avatar: storageLocal().getItem<DataInfo<number>>(userKey)?.avatar ?? "",
    username: storageLocal().getItem<DataInfo<number>>(userKey)?.username ?? "",
    nickname: storageLocal().getItem<DataInfo<number>>(userKey)?.nickname ?? "",
    roles: storageLocal().getItem<DataInfo<number>>(userKey)?.roles ?? [],
    permissions:
      storageLocal().getItem<DataInfo<number>>(userKey)?.permissions ?? [],
    verifyCode: "",
    currentPage: 0,
    isRemembered: false,
    loginDay: 7
  }),
  actions: {
    SET_AVATAR(avatar: string) {
      this.avatar = avatar;
    },
    SET_USERNAME(username: string) {
      this.username = username;
    },
    SET_NICKNAME(nickname: string) {
      this.nickname = nickname;
    },
    SET_ROLES(roles: Array<string>) {
      this.roles = roles;
    },
    SET_PERMS(permissions: Array<string>) {
      this.permissions = permissions;
    },
    SET_VERIFYCODE(verifyCode: string) {
      this.verifyCode = verifyCode;
    },
    SET_CURRENTPAGE(value: number) {
      this.currentPage = value;
    },
    SET_ISREMEMBERED(bool: boolean) {
      this.isRemembered = bool;
    },
    SET_LOGINDAY(value: number) {
      this.loginDay = Number(value);
    },
    async loginByUsername(data: LoginRequest) {
      const codeRes = await login(data);
      const tokenRes = await createAccessToken({
        grant_type: "authorization_code",
        code: codeRes.data.code,
        client_id: CLIENT_ID,
        client_secret: CLIENT_SECRET
      });
      setToken(tokenRes.data);
      return tokenRes.data;
    },
    async setUserInfo() {
      const { data } = await getCurrentUser();
      this.avatar = data.avatar;
      this.username = data.username;
      this.nickname = data.nickname;
      this.roles = (data as User).roles ?? [];
      const permRes = await listPermissions();
      this.permissions = (permRes.data ?? []).map(p => p.code);
      return data;
    },
    logOut() {
      this.username = "";
      this.roles = [];
      this.permissions = [];
      removeToken();
      useMultiTagsStoreHook().handleTags("equal", [...routerArrays]);
      resetRouter();
      router.push("/login");
    },
    async handRefreshToken(data: { refreshToken: string }) {
      const res = await refreshTokenApi({
        refresh_token: data.refreshToken,
        client_id: CLIENT_ID,
        client_secret: CLIENT_SECRET
      });
      setToken(res.data);
      return res.data;
    }
  }
});

export function useUserStoreHook() {
  return useUserStore(store);
}
