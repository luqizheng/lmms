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
  # consul-template only renders the file; the nginx process lives in a
  # separate sidecar container that consumes the shared `nginx_rendered`
  # named volume. When the sidecar is added, change this to send SIGHUP
  # to the nginx container (e.g. `docker kill -s HUP observability-nginx`)
  # or use a shared signal file. For now we just noop so the renderer
  # doesn't spam errors about missing `nginx`.
  command     = "sh -c ':'"
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