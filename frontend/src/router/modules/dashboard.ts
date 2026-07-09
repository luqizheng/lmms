import { $t } from "@/plugins/i18n";
import { dashboard } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/dashboard",
  component: Layout,
  redirect: "/dashboard/index",
  name: "Dashboard",
  meta: {
    title: $t("menus.pureDashboard"),
    icon: "ep/dashboard",
    rank: dashboard
  },
  children: [
    {
      path: "/dashboard/index",
      name: "DashboardIndex",
      component: () => import("@/views/dashboard/index.vue"),
      meta: {
        title: $t("menus.pureDashboard"),
        icon: "ep/dashboard",
        keepAlive: true
      }
    }
  ]
} satisfies RouteConfigsTable;