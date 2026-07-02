export default {
  path: "/config",
  redirect: "/config/namespace",
  meta: {
    title: "配置中心",
    icon: "ep/set-up",
    rank: 9,
    showLink: true
  },
  children: [
    {
      path: "/config/namespace",
      name: "ConfigNamespace",
      component: () => import("@/views/config/namespace/index.vue"),
      meta: {
        title: "命名空间",
        showLink: true,
        auths: ["CFG_NAMESPACE_READ"]
      }
    },
    {
      path: "/config/item",
      name: "ConfigItem",
      component: () => import("@/views/config/item/index.vue"),
      meta: {
        title: "配置项",
        showLink: true,
        auths: ["CFG_CONFIG_READ"]
      }
    }
  ]
};