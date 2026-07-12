# syntax=docker/dockerfile:1.7
# multi-stage build for PM Platform (Spring Boot 3.x, JDK 21 runtime).
# Mirrors scripts/build-java-sdk-docker.js for cache friendliness.

FROM zhcoder-docker-registry.com:8000/builder/java-sdk:0.2.0 AS builder
WORKDIR /build

# Copy ONLY what mvn needs to resolve dependencies first (cache layer)
COPY apps/pm/pom.xml apps/pm/pom.xml
COPY apps/pm/pm-app-bootstrap/pom.xml apps/pm/pm-app-bootstrap/pom.xml
COPY apps/pm/pm-app-api/pom.xml apps/pm/pm-app-api/pom.xml
COPY apps/pm/pm-app-domain/pom.xml apps/pm/pm-app-domain/pom.xml
COPY apps/pm/pm-app-infrastructure/pom.xml apps/pm/pm-app-infrastructure/pom.xml
COPY apps/pm/pm-app-llm/pom.xml apps/pm/pm-app-llm/pom.xml
COPY sdk/java/pom.xml sdk/java/pom.xml
COPY apps/pm/pm-app-bootstrap/src apps/pm/pm-app-bootstrap/src
COPY apps/pm/pm-app-api/src apps/pm/pm-app-api/src
COPY apps/pm/pm-app-domain/src apps/pm/pm-app-domain/src
COPY apps/pm/pm-app-infrastructure/src apps/pm/pm-app-infrastructure/src
COPY apps/pm/pm-app-llm/src apps/pm/pm-app-llm/src
COPY sdk/java/common sdk/java/common
COPY sdk/java/audit-service-sdk sdk/java/audit-service-sdk
COPY sdk/java/file-service-sdk sdk/java/file-service-sdk
COPY sdk/java/notification-service-sdk sdk/java/notification-service-sdk
COPY sdk/java/config-service-sdk sdk/java/config-service-sdk
COPY sdk/java/kudi-http-client sdk/java/kudi-http-client
COPY sdk/java/token-server-user-sdk sdk/java/token-server-user-sdk
COPY sdk/java/token-server-client-sdk sdk/java/token-server-client-sdk
COPY sdk/java/user-service-user-sdk sdk/java/user-service-user-sdk
COPY sdk/java/user-service-client-sdk sdk/java/user-service-client-sdk

RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -Pjava17 -pl apps/pm/pm-app-bootstrap -am clean package -DskipTests

FROM eclipse-temurin:21-jre-alpine
RUN apk add --no-cache tzdata curl
WORKDIR /app
COPY --from=builder /build/apps/pm/pm-app-bootstrap/target/pm-app-bootstrap.jar app.jar

EXPOSE 8088
ENV JAVA_OPTS="-XX:+UseContainerSupport -XX:MaxRAMPercentage=75.0"

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -fs http://localhost:8088/pm-api/v1/health || exit 1

ENTRYPOINT ["sh", "-c", "exec java $JAVA_OPTS -jar app.jar --spring.profiles.active=${SPRING_PROFILES_ACTIVE:-dev}"]
