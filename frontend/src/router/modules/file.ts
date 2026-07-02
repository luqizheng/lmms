export default {
  path: "/file",
  redirect: "/file/index",
  meta: {
    title: "文件中心",
    icon: "ep/folder",
    rank: 8,
    showLink: true
  },
  children: [
    {
      path: "/file/index",
      name: "FileList",
      component: () => import("@/views/file/index.vue"),
      meta: {
        title: "文件列表",
        showLink: true,
        auths: ["FILE_READ"]
      }
    },
    {
      path: "/file/upload",
      name: "FileUpload",
      component: () => import("@/views/file/upload.vue"),
      meta: {
        title: "上传",
        showLink: false,
        auths: ["FILE_UPLOAD"]
      }
    },
    {
      path: "/file/presign",
      name: "FilePresign",
      component: () => import("@/views/file/presign.vue"),
      meta: {
        title: "预签名上传",
        showLink: true,
        auths: ["FILE_UPLOAD"]
      }
    }
  ]
};
