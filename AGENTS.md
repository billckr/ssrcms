# SynapCMS Agent Guide

Use this file as the starting point for work in this repository. Consult the
more detailed documents under `docs/` when a task touches those areas.

## Local Application Workflow

- Manage the development application with `./app.sh`.
- Use `./app.sh status`, `./app.sh start`, `./app.sh stop`, and
  `./app.sh restart` for normal process management.
- Use `./app.sh logs` when startup or request handling needs investigation.
- `./app.sh restart` does not compile Rust. Use `./app.sh rebuild` when Rust
  source changes must be included in the running application.
- Theme template and CSS changes do not require a Rust rebuild, but manually
  edited theme templates may require `./app.sh restart` because Tera caches
  templates in memory.
- The local application listens on port 3000 and is normally reached through
  Caddy at `https://localhost`.

## Active Model and Provider

- A session's model and provider come from the profile selected with
  `codex -p <profile>`. Profiles live in `~/.codex/<profile>.config.toml`
  (for example `~/.codex/deepseek-flash.config.toml`), not in the repo.
- Do not ask the model to name its own provider. The harness injects the same
  Codex system prompt for every backend, so self-description proves nothing.
- Verify from the session rollout and the request logs instead:
  - `~/.codex/sessions/<y>/<m>/<d>/rollout-*.jsonl` records `model_provider`
    on the `session_meta` line and `model` on each `turn_context` line. Match
    the file to the session by id or modification time.
  - `~/.codex/logs_2.sqlite` records every sampling request. Filter by
    `thread_id` and look for the request line naming `model=`, `wire_api=`, the
    provider URL, and the response status. A non-OpenAI provider also shows up
    in provider-specific response headers (DeepSeek sends `x-ds-trace-id`).

```
sqlite3 "file:$HOME/.codex/logs_2.sqlite?mode=ro&immutable=1" \
  "select datetime(ts,'unixepoch'), feedback_log_body from logs
   where feedback_log_body like '%api.deepseek.com/responses%'
   order by ts desc limit 1;"
```

- Expected healthy result: `model=<profile model>` on the sampling spans and
  `Request completed method=POST url=https://api.deepseek.com/responses
  status=200 OK` in the same line.
- `secret-tool` and outbound network access both fail inside the Codex sandbox,
  so direct `curl` probes against the provider must run in the user's own shell
  or with an approved escalation.
- `codex doctor` reports provider reachability and WebSocket failures when run
  inside the sandbox even though the real session connects fine. Treat only
  those two rows as unreliable in that context.
- `approvals_reviewer = "auto_review"` cannot work with a non-OpenAI provider.
  The reviewer samples with the internal alias `codex-auto-review`, which is
  sent verbatim to the custom `base_url` and rejected with HTTP 400 (`The
  supported API model names are deepseek-flash, deepseek-v4-pro, but you passed
  codex-auto-review`). Every escalation is then auto-rejected. Use a
  non-sampling reviewer mode while a DeepSeek profile is active.
- Codex has no built-in metadata for custom model names and logs `Unknown model
  deepseek-flash is used. This will use fallback model metadata.` Treat the
  context window, token limits, and compaction thresholds as approximate.

## Theme Resolution

- Bundled themes live under `themes/global/<theme>/`.
- A site may have an overriding copy under
  `sites/<site-id>/themes/<theme>/`. The site-specific copy takes precedence
  for that site.
- Before changing a live theme, determine the path actually used by the
  request. Application logs report messages such as `loaded theme ... from
  <path>` and are the most reliable check.
- If a feature is intended for both the bundled theme and a site using an
  override, apply the change to both copies while preserving unrelated,
  intentional site customizations. Do not blindly replace an entire
  site-specific theme with the global version.
- After changing a theme, verify both the rendered page and its served
  stylesheet. Browser caching may require a hard refresh.

## AI and Translation Work

- Before reviewing or changing AI-related code, read
  `docs/ai-integration-standards.md` and `documentation/translations.md`.
- This requirement applies whenever a task mentions or touches AI translation,
  AI providers, Anthropic, OpenAI-compatible APIs, models, prompts, model
  discovery, provider testing, generated content, or AI telemetry/logging.
- Treat `docs/ai-integration-standards.md` as the repository-wide design and
  safety contract for current and future AI features. Update it when an
  architectural decision changes; do not bypass it with feature-local code.

## Verification

- Run tests appropriate to the changed area. For core Rust changes, a useful
  baseline is `cargo test -p synaptic-core --lib`.
- Run `git diff --check` after editing files.
- When changing live behavior, restart or rebuild as appropriate and verify
  the actual localhost route rather than relying only on source inspection.
- A logged-out request cannot verify controls that render only for authenticated
  users; call out that limitation or perform a logged-in check when possible.

## Repository Safety

- Preserve existing uncommitted changes and keep unrelated edits out of the
  current task.
- Do not modify database content, site settings, or documentation stored in the
  database unless the user explicitly requests it.
- Prefer focused edits over broad synchronization, especially under `sites/`,
  where files may contain deliberate per-site customizations.

## Project Documentation

- Development commands and architecture: `docs/development-guide.md`
- Theme behavior and authoring: `docs/theme-authoring-guide.md`
- Deployment operations: `docs/deployment-guide.md`
- Logging operations, formats, privacy, and retention:
  `documentation/logging.md`
- AI architecture, safety, and observability standards:
  `docs/ai-integration-standards.md`
- AI post/page translation behavior and operations:
  `documentation/translations.md`
- Authentication review and implementation notes:
  `AUTH_SECURITY_REVIEW.md` and `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md`
