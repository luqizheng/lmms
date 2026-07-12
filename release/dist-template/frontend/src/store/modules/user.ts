import { defineStore } from "pinia";
import {
  type userType,
  store,
  router,
  resetRouter,
  routerArrays,
  storageLocal
} from "../utils";
import {
  login as loginApi,
  logout as logoutApi,
  refreshToken as refreshTokenApi,
  type LoginResult
} from "@/api/auth";
import { changeIamPassword } from "@/api/iam";
import { useMultiTagsStoreHook } from "./multiTags";
import { type DataInfo, setToken, removeToken, userKey } from "@/utils/auth";

export const useUserStore = defineStore("pure-user", {
  state: (): userType => ({
    // 头像
    avatar: storageLocal().getItem<DataInfo<number>>(userKey)?.avatar ?? "",
    // 用户名
    username: storageLocal().getItem<DataInfo<number>>(userKey)?.username ?? "",
    // 昵称
    nickname: storageLocal().getItem<DataInfo<number>>(userKey)?.nickname ?? "",
    // 页面级别权限
    roles: storageLocal().getItem<DataInfo<number>>(userKey)?.roles ?? [],
    // 按钮级别权限
    permissions:
      storageLocal().getItem<DataInfo<number>>(userKey)?.permissions ?? [],
    // 前端生成的验证码（按实际需求替换）
    verifyCode: "",
    // 判断登录页面显示哪个组件（0：登录（默认）、1：手机登录、2：二维码登录、3：注册、4：忘记密码）
    currentPage: 0,
    // 是否勾选了登录页的免登录
    isRemembered: false,
    // 登录页的免登录存储几天，默认7天
    loginDay: 7,
    // 当前租户 id（从 JWT `tid` claim 解出，登录时持久化）
    tenantId: storageLocal().getItem<DataInfo<number>>(userKey)?.tenantId ?? 0
  }),
  actions: {
    /** 存储头像 */
    SET_AVATAR(avatar: string) {
      this.avatar = avatar;
    },
    /** 存储用户名 */
    SET_USERNAME(username: string) {
      this.username = username;
    },
    /** 存储昵称 */
    SET_NICKNAME(nickname: string) {
      this.nickname = nickname;
    },
    /** 存储角色 */
    SET_ROLES(roles: Array<string>) {
      this.roles = roles;
    },
    /** 存储按钮级别权限 */
    SET_PERMS(permissions: Array<string>) {
      this.permissions = permissions;
    },
    /** 存储前端生成的验证码 */
    SET_VERIFYCODE(verifyCode: string) {
      this.verifyCode = verifyCode;
    },
    /** 存储登录页面显示哪个组件 */
    SET_CURRENTPAGE(value: number) {
      this.currentPage = value;
    },
    /** 存储是否勾选了登录页的免登录 */
    SET_ISREMEMBERED(bool: boolean) {
      this.isRemembered = bool;
    },
    /** 设置登录页的免登录存储几天 */
    SET_LOGINDAY(value: number) {
      this.loginDay = Number(value);
    },
    /** 存储当前租户 id（来自 JWT `tid` claim） */
    SET_TENANT_ID(tenantId: number) {
      this.tenantId = tenantId;
    },
    /** 登入 */
    async loginByUsername(data) {
      return new Promise<LoginResult>((resolve, reject) => {
        loginApi({ username: data.username, password: data.password })
          .then(res => {
            // 适配后端 snake_case 返回结构
            const expires = new Date(Date.now() + (res.expires_in ?? 0) * 1000);
            setToken({
              accessToken: res.access_token,
              refreshToken: res.refresh_token,
              expires,
              avatar: res.user?.avatar ?? "",
              username: res.user?.username,
              nickname: res.user?.nickname,
              roles:
                res.user?.roles && res.user.roles.length > 0
                  ? res.user.roles
                  : ["common"],
              permissions:
                res.user?.permissions && res.user.permissions.length > 0
                  ? res.user.permissions
                  : ["*:*:*"]
            } as any);
            resolve(res);
          })
          .catch(error => {
            reject(error);
          });
      });
    },
    /** 登出（先清本地再调后端，避免被 http 拦截器阻塞） */
    async logOut() {
      this.username = "";
      this.roles = [];
      this.permissions = [];
      removeToken();
      useMultiTagsStoreHook().handleTags("equal", [...routerArrays]);
      resetRouter();
      router.push("/login");
      // 后端 logout 失败不影响前端清理
      logoutApi().catch(() => {});
    },
    /** 修改当前用户密码 */
    async changePassword(oldPassword: string, newPassword: string) {
      await changeIamPassword({
        old_password: oldPassword,
        new_password: newPassword
      });
    },
    /** 刷新`token` */
    async handRefreshToken(data: { refreshToken: string }) {
      return new Promise<LoginResult>((resolve, reject) => {
        refreshTokenApi({ refresh_token: data.refreshToken })
          .then(res => {
            const expires = new Date(Date.now() + (res.expires_in ?? 0) * 1000);
            setToken({
              accessToken: res.access_token,
              refreshToken: res.refresh_token,
              expires
            } as any);
            resolve(res);
          })
          .catch(error => {
            reject(error);
          });
      });
    }
  }
});

export function useUserStoreHook() {
  return useUserStore(store);
}
