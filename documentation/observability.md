---
title: Observability & Metrics
group: feature
updated_by: codex
last_updated: 2026-09-10
---
# Observability & Metrics

> Last updated: 2026-09-10 | Updated by: codex

## Overview

SynapCMS exposes application metrics in Prometheus text format at `GET /metrics`. Metrics are collected using the `metrics` crate with a `metrics-exporter-prometheus` backend. An optional bearer token protects the endpoint. HTTP request counts/durations are tracked globally via middleware; search query counts are tracked per query.

For log locations, formats, commands, parsing examples, privacy rules, and retention guidance, see
the dedicated **Logging** document. That document is the central operational source for logging;
feature documents only explain errors specific to their own workflows.

## How It Works

### Metrics Handler (`core/src/handlers/metrics.rs`)

`GET /metrics` — reads `state.metrics_handle.render()` (a `PrometheusHandle`) and returns the Prometheus text exposition format (version 0.0.4) with `Content-Type: text/plain; version=0.0.4; charset=utf-8`.

If `state.metrics_token` is `Some(token)`, the request must include `Authorization: Bearer <token>`; the provided value is extracted with `headers.get(AUTHORIZATION).and_then(...).and_then(|v| v.strip_prefix("Bearer "))`. A missing or incorrect token returns `401 Unauthorized` with a plain-text body. If `metrics_token` is `None`, the endpoint is open.

### Metrics Collected

| Metric | Type | Labels | Source |
|--------|------|--------|--------|
| `synaptic_http_requests_total` | Counter | `method`, `status` | `track_http_metrics` middleware (router.rs) |
| `synaptic_http_request_duration_seconds` | Histogram | `method` | `track_http_metrics` middleware (router.rs) |
| `synaptic_search_queries_total` | Counter | (none) | `search::render_search` (core/src/handlers/search.rs) |

The `track_http_metrics` Tower middleware records these on every request by extracting the method before calling `next.run(req)`, then reading the response status after completion.

### AppState Integration

`AppState` carries both `metrics_handle: PrometheusHandle` and `metrics_token: Option<String>`, both read-only after startup. The `PrometheusHandle` is initialized when `AppState` is constructed and shared via `Arc`.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /metrics | `metrics::metrics` | Prometheus metrics endpoint |

## Configuration

`metrics_token` in `AppConfig` (optional). Set via `METRICS_TOKEN` environment variable or `synaptic.toml`. If unset, the endpoint is open — restrict access at the Caddy/network level in production.

## Security Notes

Token comparison is a plain string equality check (`provided != Some(token.as_str())`) — not constant-time. Since this is a low-sensitivity metrics-scraping token (not an auth credential for user data), this is an acceptable tradeoff but worth noting if the token model changes.
