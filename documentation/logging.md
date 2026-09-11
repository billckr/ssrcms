---
title: Logging
group: system
updated_by: codex
last_updated: 2026-09-10
---
# Logging

> Last updated: 2026-09-10 | Updated by: codex

## Overview

SynapCMS has several logs with different purposes. Use the application log for server behavior and
unexpected failures, the Activity Log for durable administrative history, the AI translation log
for model-call diagnostics, the mail log for delivery attempts, and Caddy access logs for the HTTP
request seen by the public reverse proxy. Production application output goes to the systemd journal
instead of `logs/synapcms.log`.

Logs are an operator and developer diagnostic surface. They are not a complete business audit
history: ordinary content edits, settings changes, and administrative actions are not guaranteed to
produce durable audit records.

## Log Streams

| Log | Location | Format | Primary use |
|---|---|---|---|
| Application (development) | `logs/synapcms.log` | Text by default; optional JSON | Startup, database, templates, plugins, authentication, mail, background work, and unexpected handler failures |
| Application (production) | systemd journal for `synapcms` | Text by default; optional JSON | The same application events plus failures before or around service startup |
| Activity | PostgreSQL `audit_log`; **Admin > Activity Log** | Database rows; CSV export available | Durable record of selected staff sign-ins and important administrative changes |
| AI translation | `logs/ai-translation.jsonl` by default | JSON Lines | Correlated translation attempts, provider/model failures, response parsing, duration, and persistence outcome |
| Mail delivery | PostgreSQL `mail_log`; a form's **Analytics > Delivery Results** | Database rows | Outbound email attempts, delivery outcome, provider message ID, and failure text |
| Caddy access | `/var/log/caddy/{hostname}.log` when using generated Caddy config | JSON Lines | Request path, method, status, client/network details, response size, and proxy behavior |

Paths configured as relative values are resolved from the server process's working directory. The
generated systemd service uses the installation directory as `WorkingDirectory`, so the default AI
log lives under that installation directory in production.

## Application Log

The application uses Rust's `tracing` ecosystem. Events normally include a timestamp, severity,
target module, message, and any structured fields supplied by the caller.

At the default `info` level, the log covers:

- Startup, database migrations, site/theme/plugin loading, scheduler startup, and listening state.
- Important completed operations and state transitions that have an explicit event.
- Recoverable warnings such as blocked security checks, invalid sessions, missing optional data,
  filesystem cleanup problems, and provider verification failures.
- Unexpected errors from database writes, templates, mail delivery, filesystem operations, plugin
  management, and request handlers.

Generic per-request spans from the HTTP tracing layer become useful at more verbose filters such as
`debug`. Caddy's access log is the better first source for a complete request/status history.

### Development commands

```bash
# Follow new application output
./app.sh logs

# Inspect recent output without following it
tail -n 100 logs/synapcms.log

# Find warnings and errors in the normal text format
rg -n " WARN| ERROR" logs/synapcms.log

# Search for a route, record UUID, hostname, or module
rg -n "post-id-or-route-fragment" logs/synapcms.log
```

`synap app logs` is equivalent to `./app.sh logs` in a development checkout.

### Production commands

The systemd unit sends standard output and standard error to journald:

```bash
# Follow the service
synap app logs

# Equivalent direct journal command
sudo journalctl -u synapcms -n 100 -f

# One recent time window
sudo journalctl -u synapcms --since "1 hour ago"

# Warning and higher for the current boot
sudo journalctl -u synapcms -b -p warning
```

Use `systemctl status synapcms` first when the process will not remain running. Startup failures
that happen before tracing initializes may appear only in the journal or terminal.

## Activity Log

The Activity Log is durable product history stored in PostgreSQL, not a duplicate of the
application log. Open **Admin > Activity Log** (`/admin/activity-log`) to search, sort, paginate,
filter by site, export CSV, or clear entries. Global administrators can see all sites. Other
authorized administrators are limited to sites they manage; a clear operation follows the same
scope and writes a new `activity_log.cleared` entry after deletion.

Recorded events currently include staff login success/failure and selected high-impact actions
such as creating or deleting a site or user, suspending/reactivating a user, personal-data erasure,
site-role changes, content/taxonomy/media deletion, applying an application update, and clearing
test data. It is intentionally selective: it does not record every post edit, settings change, or
page view.

Each row records the time, actor identity and role, an action name (normally dot-namespaced), target
type/ID/label, site scope, and optional structured details. If writing this history fails, the
requested action is not rolled back; the write failure is emitted to the application log as a
warning.

Use the Activity Log when the question is **who changed what and when**. Use the application log
when the question is **why did the operation fail or behave unexpectedly**.

## AI Translation Log

AI translation events are deliberately separated from the broad application stream and are always
written as one JSON object per line. The default path is `logs/ai-translation.jsonl`; change it with
`AI_LOG_PATH` or `ai_log_path` in `synaptic.toml`.

Every real translation has a `translation_attempt_started` event followed by exactly one
`translation_attempt_finished` event sharing the same `attempt_id`. Fields include site, post,
locale, provider type and ID, model, initiating administrator, character counts, duration, outcome,
and failure stage. A process startup also writes `audit_log_ready` so operators can confirm that the
file is writable.

```bash
# Follow translation events
./app.sh translation-logs

# Show real failures, excluding diagnostic records
jq -c 'select(.fields.outcome == "failure" and .fields.synthetic != true)' \
  logs/ai-translation.jsonl

# Show every event for one attempt
jq -c 'select(.fields.attempt_id == "ATTEMPT-UUID")' \
  logs/ai-translation.jsonl

# Show failures for one site
jq -c 'select(.fields.outcome == "failure" and .fields.site_id == "SITE-UUID")' \
  logs/ai-translation.jsonl

# Count terminal outcomes by model and outcome
jq -r 'select(.fields.event == "translation_attempt_finished") |
  [.fields.model, .fields.outcome] | @tsv' logs/ai-translation.jsonl |
  sort | uniq -c
```

To test parsing or alerting without changing provider settings, sending content, or spending
tokens:

```bash
./app.sh translation-log-test
```

The emitted pair uses nil domain IDs and `synthetic: true`. Exclude synthetic records from product
failure counts and alerts. The AI Translation document explains the meaning of provider, response,
and persistence failures in more detail.

## Mail Delivery Log

Every normal outbound email attempt is written to the PostgreSQL `mail_log` table after the
provider call. A row includes site, optional form, recipient email, subject, success/failure,
provider message ID when available, error text, and time. Provider **Test** sends deliberately do
not create mail-log rows.

For form mail, open **Admin > Analytics**, select the form, then choose **Delivery Results**. The
screen shows the most recent 50 sends associated with that form; its Stats tab summarizes delivered
and failed counts. There is not currently one admin screen for every site's non-form mail, although
those attempts are retained in `mail_log` and can be examined by an operator with database access.

The mail-log write is best-effort: a failure to save the row does not change the result of an
otherwise successful or failed provider call. A database-write failure is emitted to the
application log.

## Caddy Access Logs

Each site block generated by SynapCMS writes JSON access records to
`/var/log/caddy/{hostname}.log`. These records answer questions the application log may not:
whether a request reached Caddy, which host/path/method was requested, the returned status, and
whether failure happened before or after reverse proxying.

```bash
# Follow one site's requests
sudo tail -f /var/log/caddy/example.com.log

# Show server-error responses
sudo jq -c 'select(.status >= 500)' /var/log/caddy/example.com.log

# Inspect Caddy service failures and config/reload errors
sudo journalctl -u caddy -n 100 -f

# Validate the active Caddy configuration
sudo caddy validate --config /etc/caddy/Caddyfile
```

If Caddy cannot create its log, ensure `/var/log/caddy` exists and is owned by `caddy:caddy`. The
deployment and CLI documents describe the supported permission setup.

## Configuration and Levels

| Config key | Environment | Default | Meaning |
|---|---|---|---|
| `log_level` | `LOG_LEVEL` | `info` | `tracing-subscriber` filter for the general application stream |
| `log_format` | `LOG_FORMAT` | `text` | General stream format: `text` or newline-delimited `json` |
| `ai_log_path` | `AI_LOG_PATH` | `logs/ai-translation.jsonl` | Dedicated AI JSON-lines file |

Common application levels:

| Level | Use |
|---|---|
| `error` | Unexpected failure that prevented an operation from completing |
| `warn` | Recoverable problem, rejected security condition, or degraded optional behavior |
| `info` | Normal lifecycle and meaningful operational events |
| `debug` | Detailed request/component diagnosis; enable temporarily when needed |
| `trace` | Very verbose internals; use narrowly and briefly |

Examples:

```bash
# General debug logging
LOG_LEVEL=debug ./app.sh restart

# Debug only SynapCMS core while dependencies remain at info
LOG_LEVEL='synaptic_core=debug,info' ./app.sh restart

# JSON application output for a log shipper
LOG_FORMAT=json ./app.sh restart
```

Environment or TOML logging changes require a process restart. AI audit events are not suppressed
by `LOG_LEVEL`; their dedicated sink is always active so failures cannot disappear because the
general filter was tightened.

If a single `logs/synapcms.log` contains older text records and newer JSON records after changing
`LOG_FORMAT`, rotate or archive it before treating the whole file as JSON.

## Troubleshooting Workflow

1. Record the approximate time, hostname, route, site/post/provider ID, and visible error.
2. Reproduce once if the action is safe. Do not repeatedly retry paid provider calls or destructive
   operations.
3. Check the application stream for startup, handler, database, template, or filesystem errors.
4. For an administrative change, correlate the actor, action, target, and time in the Activity Log.
5. For translations, find the terminal event in the AI log and use its `attempt_id` to retrieve the
   matching start event.
6. For email, check the form's Delivery Results first, then correlate the time with the application
   log for provider or database details.
7. Check the site's Caddy access log to confirm the request and status seen at the public edge.
8. Check `journalctl -u synapcms` and `journalctl -u caddy` for service-level failure or restarts.
9. Temporarily narrow `LOG_LEVEL` to the component under investigation if the existing evidence is
   insufficient, reproduce once, then restore the normal filter.

Avoid searching only for the word `error`. Structured fields, a UUID, hostname, route fragment,
tracing target, or AI `attempt_id` usually gives a cleaner correlation.

## Privacy and Secret Handling

Logs are sensitive operational data:

- Never log passwords, API keys, bearer tokens, cookies, authorization headers, encrypted provider
  configuration, complete prompts, request bodies, or successful model output.
- AI error response excerpts are whitespace-normalized and capped at 500 characters, but they can
  still contain fragments of translated content. Protect the AI log like content data.
- Activity rows contain actor email addresses, target labels, and sometimes structured details.
- Mail history contains recipient email addresses, subject lines, and provider error text.
- Caddy records can contain client IP addresses, paths, query strings, user agents, and referrers.
- Application records can contain hostnames, user/content UUIDs, filesystem paths, and internal
  error details.
- Redact secrets and personal data before attaching logs to an issue or support request. Prefer the
  smallest time window and only the fields needed for diagnosis.

If a credential is ever written to a log, rotate the credential first. Removing a line from the
current file is not sufficient if the log has already been shipped, backed up, or copied.

## Retention and Rotation

SynapCMS does not currently rotate `logs/synapcms.log` or the AI JSON-lines file itself. Journald,
Caddy, a log shipper, or the host's log-rotation policy must enforce retention appropriate to the
deployment. The PostgreSQL `audit_log` and `mail_log` tables also have no automatic age-based
retention. Activity Log entries can be exported and cleared from the admin UI; mail rows are only
deleted by explicit workflows such as an administrator-approved personal-data erasure.

For development, archive or truncate old files only while the app is stopped. In production,
prefer journald retention settings for the application stream and the platform's supported Caddy
rotation. If external `logrotate` manages the AI file, use a policy compatible with a long-running
process that holds the file open (for example, `copytruncate`) or restart the service after
rotation. Confirm ownership still permits the service user to write.

Retention should be shortest for logs containing client network data or AI response excerpts. Test
rotation and disk-usage alerts before relying on them; an unbounded log volume can take the service
offline even when application behavior is otherwise healthy.

## Writing New Log Events

Code-level conventions live in `docs/admin-handler-patterns.md` and
`docs/ai-integration-standards.md`. In summary:

- Use structured fields for IDs, operation, outcome, and duration rather than embedding everything
  in prose.
- Use the lowest appropriate level; normal user validation is not an application error.
- Log a failed write or unexpected dependency failure before returning a generic message.
- Do not duplicate content-bearing AI events into the general stream.
- A new dedicated stream must document its path, schema, privacy classification, retention, and
  operator commands here.

## Related Documentation

- Observability & Metrics — Prometheus endpoint, access control, and available metrics.
- AI Post Translation — translation-specific behavior and failure meanings.
- Admin Panel — Activity Log permissions and navigation.
- Forms and Email Providers — delivery-history behavior and provider-specific failures.
- Deployment — production systemd and Caddy setup.
- CLI — `synap app`, Caddy permissions, and production operations.
- WordPress Import — feature-specific import diagnostics written to the application stream.
