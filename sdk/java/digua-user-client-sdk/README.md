# digua-user-client-sdk

服务之间或受信任后台访问 `iam-service`（用户/角色/权限管理）的 SDK（`client_credentials` 流程），走 v1 路由 `/v1/iam/...`。

## 安装

```xml
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>digua-user-client-sdk</artifactId>
  <version>0.2.0</version>
</dependency>
```

## 用法

```java
UserServiceClientClient cc = UserServiceClientClient.create(
    "https://gw.example.com", "api_key_xxx", "secret_yyy", "auth-code");
List<User> all = cc.list();
```
