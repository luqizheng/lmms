FROM zhcoder-docker-registry.com:8000/official/rust:1.96 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY libs libs
COPY services services
COPY sdk sdk
RUN cargo build --release -p user-service

FROM zhcoder-docker-registry.com:8000/official/alpine:3.21
RUN apk add --no-cache ca-certificates gcompat
WORKDIR /app
COPY --from=builder /app/target/release/user-service /usr/local/bin/user-service
COPY services/user-service/config /app/config
EXPOSE 8080
CMD ["user-service"]
