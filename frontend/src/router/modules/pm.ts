import { $t } from "@/plugins/i18n";
import { pm } from "@/router/enums";
import { RouteConfigsTable } from "@/router/types";

const Layout = () => import("@/layout/index.vue");

export default {
  path: "/pm",
  component: Layout,
  redirect: "/pm/projects/list",
  name: "Pm",
  meta: {
    title: $t("menus.pm"),
    icon: "ep/folder",
    rank: pm
  },
  children: [
    {
      path: "/pm/projects/list",
      name: "PmProjectList",
      component: () => import("@/views/pm/projects/list.vue"),
      meta: {
        title: $t("menus.pmProjectList"),
        icon: "ep/discover",
        keepAlive: true
      }
    },
    {
      path: "/pm/projects/detail/:id",
      name: "PmProjectDetail",
      component: () => import("@/views/pm/projects/detail.vue"),
      meta: {
        title: $t("menus.pmProjectDetail"),
        showLink: false,
        keepAlive: false
      }
    },
    {
      path: "/pm/requirements/detail/:id",
      name: "PmRequirementDetail",
      component: () => import("@/views/pm/requirements/detail.vue"),
      meta: {
        title: $t("menus.pmRequirementDetail"),
        showLink: false,
        keepAlive: false
      }
    },
    {
      path: "/pm/kb/spaces",
      name: "PmKbSpaces",
      component: () => import("@/views/pm/kb/spaces-list.vue"),
      meta: {
        title: $t("menus.pmKbSpaces"),
        icon: "ep/document",
        keepAlive: true
      }
    },
    {
      path: "/pm/kb/spaces/:spaceId/documents",
      name: "PmKbDocuments",
      component: () => import("@/views/pm/kb/documents.vue"),
      meta: {
        title: "文档列表",
        showLink: false,
        keepAlive: false
      }
    },
    {
      path: "/pm/kb/documents/:id",
      name: "PmKbDocumentDetail",
      component: () => import("@/views/pm/kb/document-detail.vue"),
      meta: {
        title: "文档详情",
        showLink: false,
        keepAlive: false
      }
    },
    {
      path: "/pm/projects/:projectId/tasks",
      name: "PmTasks",
      component: () => import("@/views/pm/tasks/list.vue"),
      meta: {
        title: "任务",
        showLink: false,
        keepAlive: false
      }
    }
  ]
} satisfies RouteConfigsTable;
