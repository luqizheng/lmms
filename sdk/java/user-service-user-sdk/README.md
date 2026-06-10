# user-service-user-sdk

终端用户访问 user-service 的 SDK。

## 安装

```xml
<dependency>
  <groupId>com.kudi.coder</groupId>
  <artifactId>user-service-user-sdk</artifactId>
  <version>0.1.0</version>
</dependency>
```

## 用法

```java
UserServiceUserClient c = UserServiceUserClient.login(
    "https://gw.kudi.repos", "alice", "secret");
User u = c.getById("u-1");
```

## API

- `getById(String id)` / `getByIdAsync`
- `list()` / `listAsync`
- `update(String id, Map patch)`
- `delete(String id)`
- `listRoles()` / `listPermissions()`
