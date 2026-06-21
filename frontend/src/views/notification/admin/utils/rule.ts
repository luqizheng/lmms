import { reactive } from "vue";
import type { FormRules } from "element-plus";

export const formRules = reactive<FormRules>({
  title: [{ required: true, message: "标题为必填", trigger: "blur" }],
  body: [{ required: true, message: "正文为必填", trigger: "blur" }],
  publish_at: [{ required: true, message: "发布时间为必填", trigger: "change" }]
});
