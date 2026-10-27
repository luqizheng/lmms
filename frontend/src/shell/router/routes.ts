import type { RouteRecordRaw } from "vue-router";
import AppShell from "../components/AppShell.vue";
import LoginPage from "../views/login/LoginPage.vue";
import NotFound from "../views/NotFound.vue";

/** 静态路由（不需要 DB 菜单即可访问） */
export const staticRoutes: RouteRecordRaw[] = [
  {
    path: "/login",
    name: "login",
    component: LoginPage,
    meta: { public: true, title: "登录" }
  },
  {
    path: "/",
    name: "shell",
    component: AppShell,
    redirect: "/dashboard",
    children: [
      {
        path: "dashboard",
        name: "dashboard",
        component: () => import("../views/dashboard/DashboardView.vue"),
        meta: { title: "概览", icon: "layout-dashboard" }
      },
      {
        path: "profile",
        name: "profile",
        component: () => import("../views/profile/ProfileView.vue"),
        meta: { title: "个人设置", icon: "settings" }
      }
    ]
  },
  {
    path: "/:pathMatch(.*)*",
    name: "not-found",
    component: NotFound,
    meta: { title: "页面不存在" }
  }
];
