# user-service-user-sdk

终端用户访问 `iam-service`（用户/角色/权限管理）的 SDK，走 v1 路由 `/v1/iam/...`。

## 安装

```xml
<dependency>
  <groupId>com.kudi.coder</groupId>
  <artifactId>user-service-user-sdk</artifactId>
  <version>0.2.0</version>
</dependency>
```

## 用法

```java
UserServiceUserClient c = UserServiceUserClient.login(
    "https://gw.example.com", "alice", "secret");
User u = c.getById("u-1");
```

## API

- `getById(String id)` / `getByIdAsync`
- `list()` / `listAsync`
- `update(String id, Map patch)`
- `delete(String id)`
- `listRoles()` / `listPermissions()`
