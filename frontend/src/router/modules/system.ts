import { $t } from "@/plugins/i18n";
import { system } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/system",
  component: Layout,
  redirect: "/system/user",
  name: "System",
  meta: {
    title: $t("menus.system"),
    icon: "ep/set-up",
    rank: system
  },
  children: [
    {
      path: "/system/user",
      name: "SystemUser",
      component: () => import("@/views/system/user/index.vue"),
      meta: {
        title: $t("menus.systemUser"),
        icon: "ep/user",
        keepAlive: true
      }
    },
    {
      path: "/system/role",
      name: "SystemRole",
      component: () => import("@/views/system/role/index.vue"),
      meta: {
        title: $t("menus.systemRole"),
        icon: "ep/user-filled",
        keepAlive: true
      }
    },
    {
      path: "/system/permission",
      name: "SystemPermission",
      component: () => import("@/views/system/permission/index.vue"),
      meta: {
        title: $t("menus.systemPermission"),
        icon: "ep/lock",
        keepAlive: true
      }
    },
    {
      path: "/system/token-client",
      name: "SystemTokenClient",
      component: () => import("@/views/system/token-client/index.vue"),
      meta: {
        title: $t("menus.systemTokenClient"),
        icon: "ep/key",
        keepAlive: true
      }
    }
  ]
} satisfies RouteConfigsTable;
