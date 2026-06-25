# consul-template config — watches Consul for service changes
# and regenerates nginx.conf, then reloads nginx.

consul {
  address = "consul:8500"
  retry {
    enabled     = true
    attempts    = 5
    backoff     = "500ms"
    max_backoff = "10s"
  }
}

template {
  source      = "/etc/nginx/nginx.conf.template"
  destination = "/etc/nginx/nginx.conf"
  perms       = 0644
  command     = "nginx -s reload || true"
  wait {
    min = "2s"
    max = "10s"
  }
}

# Optional: log API request metrics to a Prometheus pushgateway endpoint
# template {
#   source = "/etc/nginx/reqlog.tpl"
#   destination = "/var/log/nginx/reqlog.json"
# }