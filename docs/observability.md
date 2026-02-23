# Observability

Synaptic Signals ships with structured logging and a Prometheus-compatible
metrics endpoint out of the box.

---

## Logging

Logs are emitted via the `tracing` crate. The format and verbosity are
controlled by two config values.

| Config key  | Env var      | Default | Description |
|-------------|--------------|---------|-------------|
| `log_level` | `LOG_LEVEL`  | `info`  | [EnvFilter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) string, e.g. `debug`, `synaptic_core=debug,info` |
| `log_format`| `LOG_FORMAT` | `text`  | `text` for human-readable output; `json` for newline-delimited JSON |

### JSON log format

Set `LOG_FORMAT=json` (or `log_format = "json"` in `synaptic.toml`) to emit
structured JSON logs. Each line is a valid JSON object, compatible with:

- **Grafana Loki** — use `promtail` or the Loki Docker driver
- **Datadog** — point the Datadog Agent log collector at the log file
- **AWS CloudWatch** — use the CloudWatch agent with `log_format = json`
- **Elasticsearch** — use Filebeat or Logstash

Example JSON log line:

```json
{"timestamp":"2026-01-15T10:23:45.123Z","level":"INFO","fields":{"message":"Synaptic Signals CMS starting..."},"target":"synaptic_core"}
```

---

## Metrics

Synaptic Signals exposes a Prometheus metrics endpoint at `GET /metrics`.

### Accessing the endpoint

```bash
curl http://localhost:3000/metrics
```

If `METRICS_TOKEN` is set, include the bearer token:

```bash
curl -H "Authorization: Bearer <token>" http://localhost:3000/metrics
```

### Access control

The `/metrics` endpoint has two protection modes:

**Option A — Bearer token (application-level)**

Set `METRICS_TOKEN` in your config. Requests without a valid
`Authorization: Bearer <token>` header receive `401 Unauthorized`.

```bash
# Generate a token
openssl rand -hex 32
```

```
# .env
METRICS_TOKEN=your-secret-token-here

# synaptic.toml
metrics_token = "your-secret-token-here"
```

**Option B — Network-level restriction (recommended for production)**

Leave `METRICS_TOKEN` unset and restrict `/metrics` at the Caddy or
firewall level. Example Caddy snippet to block external access:

```caddy
@metrics path /metrics
respond @metrics 403
```

Or allow only internal scraping (e.g. Prometheus on the same host):

```caddy
@metrics {
    path /metrics
    not remote_ip 127.0.0.1
}
respond @metrics 403
```

### Available metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `synaptic_http_requests_total` | Counter | `method`, `status` | Total HTTP requests handled |
| `synaptic_http_request_duration_seconds` | Histogram | `method` | HTTP request latency in seconds |
| `synaptic_search_queries_total` | Counter | — | Full-text search queries executed |

### Scraping with Prometheus

Add a scrape job to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: synaptic_signals
    static_configs:
      - targets: ["localhost:3000"]
    # If using bearer token auth:
    # bearer_token: "your-secret-token-here"
    metrics_path: /metrics
```

### Grafana dashboard

The metrics are standard Prometheus format and work with any Grafana
Prometheus data source. Useful queries:

```promql
# Request rate (per second, 5m window)
rate(synaptic_http_requests_total[5m])

# Error rate
rate(synaptic_http_requests_total{status=~"5.."}[5m])

# p95 request latency
histogram_quantile(0.95, rate(synaptic_http_request_duration_seconds_bucket[5m]))

# Search query rate
rate(synaptic_search_queries_total[5m])
```
