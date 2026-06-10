#!/usr/bin/env bash
# 通过 openapi-generator 生成 Java/.NET 客户端骨架。
# 实际业务逻辑、TokenCache、异常由手工代码补足（见 sdk/java/* 和 sdk/dotnet/*）。

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OPENAPI_DIR="$ROOT/openapi"
OUT_DIR="$ROOT/.generated"

JAVA_PKG_BASE="com.kudi.coder.generated"
DOTNET_NS_BASE="KudiCoder.Generated"

USER_SPEC="$OPENAPI_DIR/user-service.json"
TOKEN_SPEC="$OPENAPI_DIR/token-server.json"

GENERATOR_JAR="${OPENAPI_GENERATOR_JAR:-/usr/local/bin/openapi-generator-cli.jar}"

if [[ ! -f "$GENERATOR_JAR" ]]; then
  echo "openapi-generator-cli.jar not found at $GENERATOR_JAR" >&2
  echo "Download via: npm i -g @openapitools/openapi-generator-cli" >&2
  exit 1
fi

run() {
  local spec="$1" lang="$2" out="$3" extra="$4"
  echo "==> generating $lang for $(basename "$spec") -> $out"
  java -jar "$GENERATOR_JAR" generate \
    -i "$spec" \
    -g "$lang" \
    -o "$out" \
    --additional-properties="$extra" \
    --skip-validate-spec \
    --strict-spec=false
}

run "$USER_SPEC" "java" "$OUT_DIR/java/user-service" "library=okhttp-gson,dateLibrary=java8,sourceFolder=src/main/java,apiPackage=${JAVA_PKG_BASE}.user.api,modelPackage=${JAVA_PKG_BASE}.user.model,invokerPackage=${JAVA_PKG_BASE}.user.invoker,artifactId=user-service-generated"
run "$USER_SPEC" "csharp" "$OUT_DIR/dotnet/UserService" "library=httpclient,targetFramework=net6.0,netStandardLibrary=false,namespace=${DOTNET_NS_BASE}.UserService,clientPackageName=KudiCoder.UserService.Generated"

run "$TOKEN_SPEC" "java" "$OUT_DIR/java/token-server" "library=okhttp-gson,dateLibrary=java8,sourceFolder=src/main/java,apiPackage=${JAVA_PKG_BASE}.token.api,modelPackage=${JAVA_PKG_BASE}.token.model,invokerPackage=${JAVA_PKG_BASE}.token.invoker,artifactId=token-server-generated"
run "$TOKEN_SPEC" "csharp" "$OUT_DIR/dotnet/TokenServer" "library=httpclient,targetFramework=net6.0,netStandardLibrary=false,namespace=${DOTNET_NS_BASE}.TokenServer,clientPackageName=KudiCoder.TokenServer.Generated"

echo "==> done. outputs at $OUT_DIR"
