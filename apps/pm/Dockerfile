# syntax=docker/dockerfile:1.7
# Multi-stage build for PM Platform (Spring Boot 3.x, JDK 21).
# Builder: maven:3.9.9-eclipse-temurin-21 (JDK 21 + Maven 3.9.9 pre-installed).
# Runtime: eclipse-temurin:21-jre-alpine (Alpine, ~80MB base).
#
# Build strategy:
#   1. First resolve+install local SDK modules (sdk/java) into the local m2 repo
#      so that PM modules can depend on them.
#   2. Then copy pom.xml for all PM modules and resolve dependencies (cached layer).
#   3. Finally copy sources and package. This gives us best possible layer caching.

# ---------------------------------------------------------------------------
# Stage 1: build local Kudi Java SDK into local m2
# ---------------------------------------------------------------------------
FROM maven:3.9.9-eclipse-temurin-21 AS sdk-builder
WORKDIR /build

# Copy SDK parent + children poms (for dep resolution layer)
COPY sdk/java/pom.xml sdk/java/pom.xml
COPY sdk/java/common/pom.xml                 sdk/java/common/pom.xml
COPY sdk/java/kudi-http-client/pom.xml      sdk/java/kudi-http-client/pom.xml
COPY sdk/java/user-service-user-sdk/pom.xml sdk/java/user-service-user-sdk/pom.xml
COPY sdk/java/user-service-client-sdk/pom.xml sdk/java/user-service-client-sdk/pom.xml
COPY sdk/java/token-server-user-sdk/pom.xml sdk/java/token-server-user-sdk/pom.xml
COPY sdk/java/token-server-client-sdk/pom.xml sdk/java/token-server-client-sdk/pom.xml
COPY sdk/java/audit-service-sdk/pom.xml     sdk/java/audit-service-sdk/pom.xml
COPY sdk/java/file-service-sdk/pom.xml      sdk/java/file-service-sdk/pom.xml
COPY sdk/java/notification-service-sdk/pom.xml sdk/java/notification-service-sdk/pom.xml
COPY sdk/java/config-service-sdk/pom.xml    sdk/java/config-service-sdk/pom.xml

# Copy SDK sources
COPY sdk/java/common                 sdk/java/common
COPY sdk/java/kudi-http-client      sdk/java/kudi-http-client
COPY sdk/java/user-service-user-sdk sdk/java/user-service-user-sdk
COPY sdk/java/user-service-client-sdk sdk/java/user-service-client-sdk
COPY sdk/java/token-server-user-sdk sdk/java/token-server-user-sdk
COPY sdk/java/token-server-client-sdk sdk/java/token-server-client-sdk
COPY sdk/java/audit-service-sdk     sdk/java/audit-service-sdk
COPY sdk/java/file-service-sdk      sdk/java/file-service-sdk
COPY sdk/java/notification-service-sdk sdk/java/notification-service-sdk
COPY sdk/java/config-service-sdk    sdk/java/config-service-sdk

# Build SDK; snapshot populated m2 to /m2-out (cache mounts do not persist in image fs).
RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -f sdk/java/pom.xml -Pjava21 -DskipTests install \
    && mkdir -p /m2-out \
    && cp -a /root/.m2/. /m2-out/

# ---------------------------------------------------------------------------
# Stage 2: build PM application
# ---------------------------------------------------------------------------
FROM maven:3.9.9-eclipse-temurin-21 AS builder
WORKDIR /build

COPY --from=sdk-builder /m2-out /root/.m2

# Copy PM parent + child poms only (dependency resolution cache layer)
COPY apps/pm/pom.xml                              apps/pm/pom.xml
COPY apps/pm/pm-app-bootstrap/pom.xml             apps/pm/pm-app-bootstrap/pom.xml
COPY apps/pm/pm-app-api/pom.xml                   apps/pm/pm-app-api/pom.xml
COPY apps/pm/pm-app-domain/pom.xml                apps/pm/pm-app-domain/pom.xml
COPY apps/pm/pm-app-infrastructure/pom.xml        apps/pm/pm-app-infrastructure/pom.xml
COPY apps/pm/pm-app-llm/pom.xml                   apps/pm/pm-app-llm/pom.xml

# Pre-resolve PM dependencies so this layer is cached unless poms change
RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -f apps/pm/pom.xml -Pjava21 dependency:go-offline -DexcludeArtifactIds=pm-app-bootstrap,pm-app-api,pm-app-domain,pm-app-infrastructure,pm-app-llm || true

# Copy PM sources
COPY apps/pm/pm-app-bootstrap/src apps/pm/pm-app-bootstrap/src
COPY apps/pm/pm-app-api/src       apps/pm/pm-app-api/src
COPY apps/pm/pm-app-domain/src    apps/pm/pm-app-domain/src
COPY apps/pm/pm-app-infrastructure/src apps/pm/pm-app-infrastructure/src
COPY apps/pm/pm-app-llm/src       apps/pm/pm-app-llm/src

# Package. -am ensures all sibling modules are built; -Pjava21 matches JDK 21 runtime.
RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -f apps/pm/pom.xml -Pjava21 -pl pm-app-bootstrap -am clean package -DskipTests

# ---------------------------------------------------------------------------
# Stage 3: runtime (Alpine JRE 21)
# ---------------------------------------------------------------------------
FROM eclipse-temurin:21-jre-alpine
RUN apk add --no-cache tzdata curl
WORKDIR /app
COPY --from=builder /build/apps/pm/pm-app-bootstrap/target/pm-app-bootstrap.jar app.jar

EXPOSE 8088
ENV JAVA_OPTS="-XX:+UseContainerSupport -XX:MaxRAMPercentage=75.0"

HEALTHCHECK --interval=30s --timeout=5s --start-period=45s --retries=3 \
    CMD curl -fs http://localhost:8088/pm-api/v1/health || exit 1

ENTRYPOINT ["sh", "-c", "exec java $JAVA_OPTS -jar app.jar --spring.profiles.active=${SPRING_PROFILES_ACTIVE:-dev}"]
