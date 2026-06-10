import { $t } from "@/plugins/i18n";
import { error } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/error",
  component: Layout,
  redirect: "/error/404",
  name: "Error",
  meta: {
    title: $t("menus.error"),
    icon: "ep/circle-close",
    rank: error,
    showLink: false,
    hidden: true
  },
  children: [
    {
      path: "/error/403",
      name: "Error403",
      component: () => import("@/views/error/403.vue"),
      meta: { title: "403", showLink: false, hidden: true }
    },
    {
      path: "/error/404",
      name: "Error404",
      component: () => import("@/views/error/404.vue"),
      meta: { title: "404", showLink: false, hidden: true }
    },
    {
      path: "/error/500",
      name: "Error500",
      component: () => import("@/views/error/500.vue"),
      meta: { title: "500", showLink: false, hidden: true }
    }
  ]
} satisfies RouteConfigsTable;
