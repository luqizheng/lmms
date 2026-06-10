FROM zhcoder-docker-registry.com:8000/official/golang:1.25-alpine AS builder
RUN apk add --no-cache git
WORKDIR /app
COPY services/user-service/ .
RUN GOPROXY=https://goproxy.cn,direct go mod download && go build -o user-service .

FROM zhcoder-docker-registry.com:8000/official/alpine:3.21
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/user-service /usr/local/bin/
COPY services/user-service/config/ /app/config/
WORKDIR /app
CMD ["user-service"]
