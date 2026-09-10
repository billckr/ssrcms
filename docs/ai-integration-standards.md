# AI Integration Standards

These rules apply to AI translation today and to future AI-assisted features in SynapCMS. They are
intended to keep provider-specific code at the edge and make behavior, security, and operations
consistent as capabilities grow.

## Architecture

1. Feature handlers own authorization, tenant/site scoping, feature flags, user-facing responses,
   and persistence. They do not construct provider wire payloads.
2. A feature service owns prompts, domain input/output types, validation, and orchestration. Domain
   code depends on a small provider-neutral interface.
3. Provider adapters own authentication headers, URLs, request/response envelopes, timeouts, and
   conversion into provider-neutral results and error categories.
4. Stored provider configuration remains a tagged, encrypted type. Credentials must never be
   returned to a browser, formatted with `Debug`, or included in telemetry.
5. Adding a provider should require a new adapter and configuration variant, not branches spread
   through handlers or templates. OpenAI-compatible endpoints remain one adapter unless a service
   has materially different semantics.

The current translation implementation follows this split across the admin handlers,
`core/src/translate.rs`, and `core/src/models/ai_provider.rs`. If the number of AI features or
provider adapters grows, move `translate.rs` into an `ai/` module with `service`, `telemetry`, and
`providers/*` modules before adding more cross-cutting branches.

## Requests and responses

- Every outbound call has a finite timeout and a bounded output size.
- Prompts specify an explicit output contract. Model output is parsed into a typed domain result and
  validated before any write occurs.
- Do not silently retry non-idempotent operations. Any retry policy must be bounded, classify
  retryable failures, preserve one correlation ID, and avoid duplicate persistence.
- Treat all model output as untrusted. Existing HTML sanitization and rendering rules still apply;
  provider selection by an administrator does not make generated markup executable code.
- Provider errors shown in the UI should be useful but bounded. Never expose credentials, request
  headers, internal paths, or unrestricted upstream bodies.

## Telemetry

- Emit a start event and exactly one terminal event for every user-triggered AI operation.
- Use the tracing target `ai_translation` until a feature-neutral `ai` audit stream is introduced.
- Terminal events carry `outcome`, `stage`, `duration_ms`, provider type, model, tenant/site ID,
  actor ID, and relevant domain IDs. A generated correlation ID connects the event pair.
- Never log API keys, authorization headers, complete prompts, source content, or successful model
  output. Error response excerpts must be whitespace-normalized and capped.
- The dedicated audit stream is JSON Lines so operators can use `jq`, log shippers, or retention
  tooling without scraping prose. Access and retention should be treated like content data because
  error excerpts can contain fragments of a post.

## Testing and change review

- Unit-test prompt rules, response parsing, validation, error bounding, and provider-neutral
  dispatch. Use fake local HTTP servers for adapter behavior; normal tests must not call paid APIs.
- Route tests cover authorization, the global feature switch, site isolation, and the rule that a
  failed provider call or invalid response never writes content.
- A new provider or AI feature must document its data flow, timeout/output limits, supported model
  assumptions, observability fields, and deletion/retention behavior.
- Database schema should store durable product state, not raw provider traffic. Use audit logs for
  operational diagnostics unless a product requirement explicitly calls for queryable history.
