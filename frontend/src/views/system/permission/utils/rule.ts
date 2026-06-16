import { reactive } from "vue";
import type { FormRules } from "element-plus";

export const formRules = reactive(<FormRules>{
  code: [{ required: true, message: "权限码必填", trigger: "blur" }],
  name: [{ required: true, message: "名称必填", trigger: "blur" }],
  category: [{ required: true, message: "类别必填", trigger: "change" }]
});

export const CATEGORY_OPTIONS = [
  { label: "菜单", value: 0 },
  { label: "按钮", value: 1 },
  { label: "API", value: 2 }
];

export const STATUS_OPTIONS = [
  { label: "启用", value: "active" },
  { label: "禁用", value: "disabled" }
];
