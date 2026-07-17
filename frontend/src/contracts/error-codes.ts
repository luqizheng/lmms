// Auto-generated from contracts/error-codes.yaml
// DO NOT EDIT MANUALLY

export type ErrorCode =
  | 'SYS_INTERNAL_0001'
  | 'SYS_DB_0001'
  | 'SYS_DB_0002'
  | 'SYS_HTTP_0001'
  | 'IAM_AUTH_1001'
  | 'IAM_AUTH_1002'
  | 'IAM_AUTH_1003'
  | 'IAM_AUTH_1004'
  | 'IAM_AUTH_1005'
  | 'IAM_AUTH_1006'
  | 'IAM_AUTH_1007'
  | 'IAM_AUTH_1008'
  | 'VALID_2001'
  | 'VALID_2002'
  | 'VALID_2003'
  | 'VALID_2004'
  | 'IAM_PERM_3001'
  | 'IAM_PERM_3002'
  | 'CFG_ITEM_1001'
  | 'CFG_NS_2001'
  | 'CFG_VALID_3001'
  | 'CFG_VALID_4001'
  | 'CFG_VALID_4002'
  | 'GW_ROUTE_5001'
  | 'GW_ROUTE_5002'
  | 'GW_RATE_5003'
  | 'AUD_LOG_6001'
  | 'AUD_LOG_6002'
  | 'AUD_LOG_6003'
  | 'FILE_ITEM_1001'
  | 'FILE_UPLOAD_2001'
  | 'FILE_UPLOAD_2002'
  | 'FILE_PRESIGN_3001'
  | 'FILE_READ_4001'
  | 'FILE_DELETE_4002'
;

export const ErrorMessages: Record<ErrorCode, string> = {
  'SYS_INTERNAL_0001': 'internal server error',
  'SYS_DB_0001': 'database error',
  'SYS_DB_0002': 'row not found',
  'SYS_HTTP_0001': 'http client error',
  'IAM_AUTH_1001': 'invalid credentials',
  'IAM_AUTH_1002': 'token expired',
  'IAM_AUTH_1003': 'token revoked',
  'IAM_AUTH_1004': 'invalid token',
  'IAM_AUTH_1005': 'missing token',
  'IAM_AUTH_1006': 'password too weak',
  'IAM_AUTH_1007': 'username already exists',
  'IAM_AUTH_1008': 'email already exists',
  'VALID_2001': 'invalid input',
  'VALID_2002': 'missing required field',
  'VALID_2003': 'invalid phone format',
  'VALID_2004': 'invalid email format',
  'IAM_PERM_3001': 'permission denied',
  'IAM_PERM_3002': 'role not found',
  'CFG_ITEM_1001': 'config item not found',
  'CFG_NS_2001': 'namespace not found',
  'CFG_VALID_3001': 'validation failed',
  'CFG_VALID_4001': 'config key not found',
  'CFG_VALID_4002': 'config parse error',
  'GW_ROUTE_5001': 'route not found',
  'GW_ROUTE_5002': 'service unavailable',
  'GW_RATE_5003': 'rate limit exceeded',
  'AUD_LOG_6001': 'audit log not found',
  'AUD_LOG_6002': 'export format not supported',
  'AUD_LOG_6003': 'export generation failed',
  'FILE_ITEM_1001': 'file not found',
  'FILE_UPLOAD_2001': 'file upload failed',
  'FILE_UPLOAD_2002': 'file too large',
  'FILE_PRESIGN_3001': 'presign url expired',
  'FILE_READ_4001': 'file read failed',
  'FILE_DELETE_4002': 'refine forbidden for caller',
};