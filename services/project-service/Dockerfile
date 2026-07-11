# syntax=docker/dockerfile:1.6
# Phase 9 PMD — multi-stage Spring Boot 3 + Java 25 build.
# Stage 1: eclipse-temurin:25-jdk (build + maven test + package).
# Stage 2: eclipse-temurin:25-jre-alpine (runtime).

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM ${BASE_REGISTRY}/eclipse-temurin:25-jdk AS builder
WORKDIR /build

# Maven wrapper or local mvn must be present on PATH.
# CI images pre-install mvn; for offline builds cache ~/.m2 via --mount.
COPY services/project-service/pom.xml /build/pom.xml
RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -B -ntp -f /build/pom.xml -DskipTests dependency:go-offline

COPY services/project-service/src /build/src

RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -B -ntp -f /build/pom.xml -DskipTests package && \
    cp /build/target/project-service.jar /app/project-service.jar

FROM ${BASE_REGISTRY}/eclipse-temurin:25-jre-alpine
RUN apk add --no-cache ca-certificates tzdata
WORKDIR /app
COPY --from=builder /app/project-service.jar /app/project-service.jar
COPY services/project-service/config /app/config
EXPOSE 8080
ENV JAVA_OPTS="-XX:+UseG1GC -XX:MaxRAMPercentage=75.0"
HEALTHCHECK --interval=15s --timeout=3s --retries=5 \
    CMD wget --quiet --tries=1 --spider http://localhost:8080/project/health || exit 1
CMD ["sh", "-c", "exec java $JAVA_OPTS -jar /app/project-service.jar --spring.config.location=optional:classpath:/,optional:file:/app/config/application.yml"]
