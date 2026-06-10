import { $t } from "@/plugins/i18n";
import { account } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/account-settings",
  component: Layout,
  redirect: "/account-settings/profile",
  name: "AccountSettings",
  meta: {
    title: $t("menus.accountSettings"),
    icon: "ep/user",
    rank: account,
    showLink: false,
    hidden: true
  },
  children: [
    {
      path: "/account-settings/profile",
      name: "AccountProfile",
      component: () => import("@/views/account-settings/components/Profile.vue"),
      meta: {
        title: $t("menus.accountProfile"),
        showLink: false,
        hidden: true
      }
    },
    {
      path: "/account-settings/security",
      name: "AccountSecurity",
      component: () =>
        import("@/views/account-settings/components/SecurityLog.vue"),
      meta: {
        title: $t("menus.accountSecurity"),
        showLink: false,
        hidden: true
      }
    },
    {
      path: "/register",
      name: "Register",
      component: () => import("@/views/login/register.vue"),
      meta: {
        title: "注册管理员",
        showLink: false,
        hidden: true
      }
    }
  ]
} satisfies RouteConfigsTable;
