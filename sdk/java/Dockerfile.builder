# Java SDK builder image.
#
# 配套 scripts/build-java-sdk-docker.js — 在容器内编译 / 测试 / 打包 sdk/java/，
# 主机无需安装 JDK / Maven。
#
# 单 JDK 21 镜像；pom 的 `-Pjava8 / -Pjava17 / -Pjava21` profile 只切换
# maven.compiler.source/target 属性，JDK 21 可以向下编译到 1.8 字节码，
# 故单 toolchain 即可。多 JDK 运行时矩阵是 CI 关注点，不在本镜像范围。
#
# 行为与 Rust builder (zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl)
# 对齐：源码与 ~/.m2 缓存通过 volume mount 进来，**不**烤进镜像。
#
# 构建：
#   docker build -f sdk/java/Dockerfile.builder \
#     -t zhcoder-docker-registry.com:8000/builder/java-sdk:0.2.0 sdk/java
#
# 用法见 scripts/build-java-sdk-docker.js。

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/maven:3.9-eclipse-temurin-21

USER root
WORKDIR /workspace

# 镜像本体仅 maven + JDK21，无任何额外层；预热 ~/.m2/repository 由
# build script 在首次 run 时按需做，避免烤入项目特定依赖。

CMD ["mvn", "-version"]
