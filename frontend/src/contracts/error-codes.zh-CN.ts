// 中文（zh-CN）错误码翻译
// 镜像 libs/common-error/src/codes.rs message() 默认英文文案。
// 失效码（新增但未翻译）落在 ErrorMessagesFallback 中处理。
//
// 维护：增删 contracts/error-codes.yaml 时，本文件需同步增减。
// 生成：hand-written 是的，便于产品文案评审；如需 codegen，
//   改 contracts/codegen/gen-ts.js 增加 zh-CN 输出。

import type { ErrorCode } from "./error-codes";

export const ErrorMessagesZhCN: Record<ErrorCode, string> = {
  SYS_INTERNAL_0001: "服务器内部错误",
  SYS_DB_0001: "数据库错误",
  SYS_DB_0002: "记录未找到",
  SYS_HTTP_0001: "外部 HTTP 调用失败",
  IAM_AUTH_1001: "用户名或密码错误",
  IAM_AUTH_1002: "登录已过期，请重新登录",
  IAM_AUTH_1003: "登录已被撤销",
  IAM_AUTH_1004: "无效的令牌",
  IAM_AUTH_1005: "缺少令牌",
  IAM_AUTH_1006: "密码强度不足",
  IAM_AUTH_1007: "用户名已被占用",
  IAM_AUTH_1008: "邮箱已被占用",
  VALID_2001: "输入不合法",
  VALID_2002: "缺少必填项",
  VALID_2003: "手机号格式不正确",
  VALID_2004: "邮箱格式不正确",
  IAM_PERM_3001: "权限不足",
  IAM_PERM_3002: "角色不存在",
  CFG_ITEM_1001: "配置项不存在",
  CFG_NS_2001: "配置命名空间不存在",
  CFG_VALID_3001: "配置校验失败",
  CFG_VALID_4001: "配置键不存在",
  CFG_VALID_4002: "配置解析失败",
  GW_ROUTE_5001: "路由不存在",
  GW_ROUTE_5002: "服务暂不可用",
  GW_RATE_5003: "请求过于频繁，请稍后再试",
  AUD_LOG_6001: "审计日志不存在",
  AUD_LOG_6002: "不支持的导出格式",
  AUD_LOG_6003: "导出生成失败",
  FILE_ITEM_1001: "文件不存在",
  FILE_UPLOAD_2001: "文件上传失败",
  FILE_UPLOAD_2002: "文件大小超过限制",
  FILE_PRESIGN_3001: "预签名 URL 已过期",
  FILE_READ_4001: "文件读取失败",
  FILE_DELETE_4002: "文件删除失败"
};
