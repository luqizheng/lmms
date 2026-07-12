import { $t } from "@/plugins/i18n";
import { monitor } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/monitor",
  component: Layout,
  redirect: "/monitor/operation-log",
  name: "Monitor",
  meta: {
    title: $t("menus.pureSysMonitor"),
    icon: "ep/monitor",
    rank: monitor
  },
  children: [
    {
      path: "/monitor/operation-log",
      name: "MonitorOperationLog",
      component: () => import("@/views/monitor/operation-log/index.vue"),
      meta: {
        title: $t("menus.monitorOperationLog"),
        icon: "ep/document",
        keepAlive: true
      }
    },
    {
      path: "/monitor/login-log",
      name: "MonitorLoginLog",
      component: () => import("@/views/monitor/login-log/index.vue"),
      meta: {
        title: $t("menus.monitorLoginLog"),
        icon: "ep/key",
        keepAlive: true
      }
    },
    {
      path: "/monitor/retention",
      name: "MonitorRetention",
      component: () => import("@/views/monitor/retention/index.vue"),
      meta: {
        title: $t("menus.monitorRetention"),
        icon: "ep/timer",
        keepAlive: true
      }
    }
  ]
} satisfies RouteConfigsTable;
