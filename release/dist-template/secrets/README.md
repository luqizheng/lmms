# Secrets 目录

此目录存放运行时生成的敏感密钥文件，**禁止提交到 git**。

## 文件清单

| 文件 | 说明 | 用途 |
|------|------|------|
| `jwt_private.pem` | RSA-2048 私钥 | JWT RS256 签名（**chmod 600**）|
| `jwt_public.pem` | RSA 公钥 | JWT 验签 |
| `.env` | 环境变量（含所有密码）| 运行时加载，**不提交** |

## 生成方式

```bash
make secrets
# 或
bash bin/generate-secrets.sh
```

## 安全建议

- `jwt_private.pem` 权限必须是 `600`，仅 root 可读
- 正式环境建议将私钥存入 HSM/KMS
- 所有 secrets 建议定期轮换（90 天）
- `.env` 文件不要提交到版本控制系统
