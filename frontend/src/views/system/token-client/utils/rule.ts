import { reactive } from "vue";
import type { FormRules } from "element-plus";

export const formRules = reactive(<FormRules>{
  name: [{ required: true, message: "客户端名称为必填项", trigger: "blur" }],
  access_token_ttl_sec: [
    { required: true, message: "Access Token TTL 必填", trigger: "blur" }
  ],
  refresh_token_ttl_sec: [
    { required: true, message: "Refresh Token TTL 必填", trigger: "blur" }
  ],
  status: [{ required: true, message: "状态为必填项", trigger: "change" }]
});

export const GRANT_TYPE_OPTIONS = [
  { label: "authorization_code", value: "authorization_code" },
  { label: "refresh_token", value: "refresh_token" },
  { label: "client_credentials", value: "client_credentials" },
  { label: "password", value: "password" }
];

export const STATUS_OPTIONS = [
  { label: "启用", value: "active" },
  { label: "禁用", value: "disabled" }
];
