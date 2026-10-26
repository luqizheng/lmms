import "vue-router";

declare module "vue-router" {
  interface RouteMeta {
    /** 无需登录即可访问 */
    public?: boolean;
  }
}
