# syntax=docker/dockerfile:1.6
# Phase 9 PMD — Spring Boot 3 / Java 25 multi-stage build.
# Builder installs maven via apt because the internal registry does not yet
# mirror a maven:*-eclipse-temurin-25 image (mirror apps/pm/Dockerfile once it
# does). maven-settings.xml mirrors Central -> Aliyun and routes
# com.kudi.coder:* to the on-prem Reposilite at mvn.kudi.repos:18181.

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official

FROM ${BASE_REGISTRY}/eclipse-temurin:25-jdk AS builder
WORKDIR /build

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && \
    apt-get install -y --no-install-recommends maven && \
    rm -rf /var/lib/apt/lists/*

RUN mkdir -p /root/.m2 /app
COPY services/project-service/maven-settings.xml /root/.m2/settings.xml

COPY services/project-service/pom.xml /build/pom.xml
RUN --mount=type=cache,id=m2-project-service,target=/root/.m2 \
    mvn -B -ntp -f /build/pom.xml -Dmaven.test.skip=true dependency:go-offline

COPY services/project-service/src /build/src

RUN --mount=type=cache,id=m2-project-service,target=/root/.m2 \
    mvn -B -ntp -f /build/pom.xml -Dmaven.test.skip=true package && \
    cp /build/target/project-service.jar /app/project-service.jar

FROM ${BASE_REGISTRY}/eclipse-temurin:25-jre-alpine
RUN apk add --no-cache ca-certificates tzdata wget
WORKDIR /app
COPY --from=builder /app/project-service.jar /app/project-service.jar
COPY services/project-service/config /app/config
EXPOSE 8080
ENV JAVA_OPTS="-XX:+UseG1GC -XX:MaxRAMPercentage=75.0"
HEALTHCHECK --interval=15s --timeout=3s --retries=5 \
    CMD wget --quiet --tries=1 --spider http://localhost:8080/project/health || exit 1
CMD ["sh", "-c", "exec java $JAVA_OPTS -jar /app/project-service.jar --spring.config.location=optional:classpath:/,optional:file:/app/config/application.yml"]
