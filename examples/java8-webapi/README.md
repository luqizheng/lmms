# Java 8 Web API integration example

A downstream integration example for the **current LMMS Java SDK**. It uses Spring Boot 2.1.18.RELEASE and targets Java 8.

This example intentionally demonstrates only capabilities that exist in the current SDK modules:

- IAM user client: user lookup
- IAM client-credential client: role creation
- Token user client: JWKS, refresh/revoke, resource-token issuance
- Token client-credential client: JWKS
- Notification SDK: unread announcement count
- Generic HTTP client: exposed as a Spring bean for application-specific gateway calls

The former hand-written Audit SDK, Config polling implementation, File endpoints, and local task-management demo were removed. Those were not part of the current published Java SDK contract and must not be presented as SDK-supported features.

## Prerequisites

- JDK 8
- Maven 3.9+
- Node.js and pnpm only if you also start the LMMS Rust service stack
- A reachable LMMS API Gateway and valid IAM / client credentials for live calls

## Build from this repository

Build and install the SDK modules from source first. This avoids relying on a previously published artifact or a developer-specific internal Maven repository.

```bash
# From repository root
mvn -B -f sdk/java/pom.xml -Pjava8 clean install

# Build the downstream Spring Boot example
mvn -B -f examples/java8-webapi/pom.xml -Pjava8 clean verify
```

## Configure and run

Set credentials through environment variables. Do not commit credentials to application.yml.

Linux / macOS / Git Bash:

```bash
export KUDI_GATEWAY_URL=http://localhost:3000
export DEMO_USER=your-user
export DEMO_PASS=your-password
export DEMO_API_KEY=your-api-key
export DEMO_API_SECRET=your-api-secret
export DEMO_API_CODE=your-client-code

mvn -f examples/java8-webapi/pom.xml -Pjava8 -pl demo-webapp spring-boot:run
```

PowerShell:

```powershell
$env:KUDI_GATEWAY_URL = "http://localhost:3000"
$env:DEMO_USER = "your-user"
$env:DEMO_PASS = "your-password"
$env:DEMO_API_KEY = "your-api-key"
$env:DEMO_API_SECRET = "your-api-secret"
$env:DEMO_API_CODE = "your-client-code"

mvn -f examples/java8-webapi/pom.xml -Pjava8 -pl demo-webapp spring-boot:run
```

The demo listens on port 7000 by default.

## Endpoints

| Endpoint | Current SDK API exercised |
|---|---|
| `GET /demo/auth/me` | `UserServiceUserClient.getById("me")` |
| `GET /demo/auth/jwks` | User token client `jwks()` |
| `POST /demo/auth/refresh` | User token client `refresh(...)` |
| `POST /demo/auth/revoke` | User token client `revoke(...)` |
| `GET /demo/perm/internal` | User client-credential SDK `createRole(...)` |
| `GET /demo/perm/resource` | User token client `issueResource(...)` |
| `GET /demo/perm/client-jwks` | Client-credential token SDK `jwks()` |
| `GET /demo/notify/unread-count` | Notification SDK `unreadCount()` |

All endpoints return the example's `ApiResponse` envelope. The envelope is demo-webapp code, not an LMMS SDK response type.

## Smoke test against a running gateway

With demo-webapp running and credentials configured:

```bash
bash examples/java8-webapi/scripts/demo.sh
```

The script uses `curl --fail` so HTTP error responses fail the walkthrough instead of being reported as successful calls. Some endpoints call authenticated services and therefore require valid credentials and corresponding service permissions.

## CI

The repository workflow `.github/workflows/java8-example-integration.yml` builds the SDK from source, installs its modules into the job-local Maven repository, and then compiles/tests this downstream example with Java 8. It does not publish artifacts.

## Notes

- Maven coordinates are `io.github.luqizheng:<artifactId>:1.1.2`.
- Java source packages intentionally remain `io.digua.*`; Maven groupId migration does not rename Java packages.
- The example does not claim live Gateway compatibility from MockMvc tests. Live endpoint behavior must be checked against a running LMMS service stack using the smoke script.
- NotificationServiceClient accepts a bearer-token supplier. This sample supplies no bearer token by default; configure a real token supplier in `SdkConfig` if the target deployment requires authenticated notification calls.
