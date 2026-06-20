import { $t } from "@/plugins/i18n";
import { notification } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/notification",
  component: Layout,
  redirect: "/notification/inbox",
  name: "Notification",
  meta: {
    title: $t("menus.notification"),
    icon: "ep/bell",
    rank: notification
  },
  children: [
    {
      path: "/notification/inbox",
      name: "NotificationInbox",
      component: () => import("@/views/notification/inbox/index.vue"),
      meta: {
        title: $t("menus.notificationInbox"),
        icon: "ep/message",
        keepAlive: true
      }
    },
    {
      path: "/notification/admin",
      name: "NotificationAdmin",
      component: () => import("@/views/notification/admin/index.vue"),
      meta: {
        title: $t("menus.notificationAdmin"),
        icon: "ep/notification",
        roles: ["notification.admin"],
        keepAlive: true
      }
    },
    {
      path: "/notification/detail/:id",
      name: "NotificationDetail",
      component: () => import("@/views/notification/detail/index.vue"),
      meta: {
        title: $t("menus.notificationDetail"),
        showLink: false,
        keepAlive: false
      }
    }
  ]
} satisfies RouteConfigsTable;
