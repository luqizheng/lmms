# MySQL replica image — extends official mysql:8.0 with a custom entrypoint
# that wires up GTID-based replication to a primary on startup.
#
# Build context: this directory (infrastructure/mysql-replica/)
# Run as part of docker-compose: services.mysql-replica.image is built from here.

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/mysql:8.0

# Replicate binlog settings from docker-compose-infra.yml mysql service.
# These must match the primary's settings or GTID auto-positioning will fail.
CMD ["mysqld"]
ENTRYPOINT ["/usr/local/bin/replica-entrypoint.sh"]

# Copy our entrypoint wrapper
COPY replica-entrypoint.sh /usr/local/bin/replica-entrypoint.sh
RUN chmod +x /usr/local/bin/replica-entrypoint.sh