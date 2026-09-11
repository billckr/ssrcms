---
title: AI Post Translation
group: feature
updated_by: codex
last_updated: 2026-09-10
---
# AI Post Translation

> Last updated: 2026-09-10 | Updated by: codex

## Overview

SynapCMS can translate a saved post or page with an administrator-configured AI provider. A
translation contains localized copies of the source title, excerpt, and content. It does not
replace the source post. Visitors open it through a locale-prefixed URL such as
`/es/my-post` or `/ru/my-post`.

The integration is implemented directly in SynapCMS with `reqwest`; it does not use a third-party
AI SDK. The two supported wire protocols are:

- Anthropic's Messages API.
- OpenAI-compatible `POST /chat/completions`, including OpenAI and compatible local or hosted
  servers such as Ollama, LM Studio, and vLLM.

Translation is currently intended for conventional posts and pages whose body is stored as one
HTML or Markdown string. Builder/page-composition content is not translated.

## Administrator Workflow

### 0. Installation-wide switch (super admin)

A super admin can turn AI Translation off for every site at once from **System Settings →
General → Features** (`/admin/settings`), independent of any per-site provider configuration.
It's on by default — existing installs are unaffected until someone flips it.

Turning it off is non-destructive: providers, their encrypted credentials, verification status,
and existing translations are left untouched in the database and reappear exactly as they were
when the switch is turned back on. It only gates *access*:

- The AI Translation tab and panel are omitted from every site's Settings page (not just hidden
  with CSS — the markup isn't rendered at all).
- The post editor's Translations sidebar section is omitted the same way.
- Every AI-provider and translate route (create/update/delete a provider, discover models, test,
  translate post prose, or translate an embedded resource) checks the switch itself, first thing,
  and returns 403 if it's off — this is
  enforced independently of the UI, so a site manager who knows the URL shape can't bypass it by
  posting to the routes directly while the tab is hidden. See `ai_translation_enabled` in
  `core/src/handlers/admin/ai_providers.rs` and the equivalent check in
  `core/src/handlers/admin/posts.rs::translate_post_action`.

The setting itself lives in the installation-wide `app_settings` table (`AppSettings::ai_translation_enabled`
in `core/src/app_state.rs`) and is hot-reloadable — no restart needed after saving.

### 1. Configure a provider

Open **Sites**, choose the site, then open **Settings → AI Translation**. Add either:

- **Anthropic:** API key, then use the refresh button to load models available to that key.
- **OpenAI-Compatible:** base URL and optional API key, then load the endpoint's models. Include the API version
  in the base URL when the service requires it, for example `https://api.openai.com/v1`.

Choose a model from the returned list. Economy-labelled models are sorted first, but those labels
are name-based guidance rather than live pricing; confirm current pricing with the provider. If a
compatible server does not implement model discovery, choose **Enter a custom model ID**.

Loading models sends the entered connection details to SynapCMS, which performs the provider
request server-side. Discovery does not create or update a database provider; credentials are
stored only when the form is submitted.

Provider credentials are write-only in the UI. When editing a provider, blank credential fields
retain the stored encrypted values, so changing only the model does not require re-entering the
API key. The refresh button reloads the model list using the saved credential unless a replacement
key has been entered.

Saving a provider (create or edit) automatically runs the same check as the Test button: SynapCMS
sends a small model request and requires the same structured JSON shape required by translation. On
success the provider is marked verified immediately and becomes available in post editors with no
extra click. On failure the provider is still saved (so entered fields aren't lost), but stays
unverified, and the flash message reports the provider/parser error — fix the issue and save again,
or use the provider's Test button directly to retry the same check without resubmitting the whole
form. Verification establishes connectivity, authentication, model-name validity, and basic
structured-output compatibility; it does not guarantee that a much larger translation request will
fit the model's limits.

### 2. Enable languages

In the same AI Translation tab, select the languages the site should expose and save the list.
Only enabled languages appear in the post editor.

Enabling a locale reserves its code as the first public URL segment for the entire site. For
example, enabling Spanish reserves `/es/...`. Check for a top-level post or page whose slug is
exactly `es` before enabling it, because that path will be interpreted as the locale namespace.

### 3. Translate a post or page

Open an existing post or page and find **Translations** in the editor sidebar. Choose a language
and a verified provider. The controls have deliberately separate scopes:

- **Translate post prose** (globe) creates a missing post translation or refreshes one marked
  **Source changed**. It is disabled when that locale's post translation is current, and the route
  also refuses a redundant current-post request.
- **Translate embedded items only** (layers) translates only missing or stale forms/polls. It is
  available after the post itself has a translation and never calls the post translator or writes
  translated post prose.

Each translated locale lists its embedded forms and polls as **Current**, **Missing**, or **Source
changed**, and links each item to its authoritative Designer screen. The text under the controls
explains which scope currently needs work.

For a post that is already translated, adding, removing, or moving only a form/poll does **not**
require retranslating the post. Save the source post, then use **Translate embedded items only** or
translate the reusable form/poll from its own Designer screen. Embed identity and placement come
mechanically from the source post at render time, while the existing translated prose is reused
unchanged. This is the standard way to avoid spending tokens on prose that has already been
translated.

If the edit changes the title, excerpt, or prose as well, the post translation is marked stale and
should be translated again. Refresh missing/stale embedded resources with the separate layers
action afterward; the two scopes never spend each other's tokens.

The request uses the last saved version of the post. If the editor contains unsaved changes,
SynapCMS asks the administrator to save them first. While either model request is running, its
button is disabled and spins, and a small status line describes the active request so a slow
provider response doesn't look like nothing is happening. The request is synchronous and can
legitimately take several seconds.

On success, the editor reloads with a message such as `Translated into Spanish.` and lists the
translation with a public View link. A stale post translation is replaced; a current one is not
sent again. Use the trash button next to a translation to delete only that localized copy; the
source post is unaffected.

AI output should be reviewed by someone who understands the target language. Provider success
means the response was structurally usable, not that terminology, tone, facts, or formatting are
perfect.

## Provider Requests and Responses

The implementation lives in `core/src/translate.rs`.

### Prompt construction

SynapCMS sends the source title, optional excerpt, and complete content in one prompt. The prompt
names the target language, asks for a faithful translation, and instructs the model to preserve
HTML tags and attributes or Markdown syntax according to the post's `content_format`.

The required model response is one JSON object:

```json
{
  "title": "Translated title",
  "excerpt": "Translated excerpt or null",
  "content": "Translated HTML or Markdown"
}
```

The parser accepts a plain JSON object and defensively removes one surrounding `json` or bare
Markdown code fence. Commentary, partial JSON, a different field shape, or truncated output causes
the translation to fail without writing a database row.

Before the post body is sent, exact `<ss-form>` and `<ss-poll>` markers are replaced with opaque,
numbered tokens. The response must contain each token exactly once and in the original order. The
source marker bytes are then restored. This prevents a model from translating a slug, changing an
embed type, duplicating a resource, or silently removing it.

Forms and polls use separate structured calls. Only visitor-facing labels and messages are
translated. Form field names and option values and poll option keys are validated byte-for-byte;
they remain the identifiers used by submissions, votes, email placeholders, exports, and
analytics. Confirmation-email `{{field_name}}` placeholders and the poll result `{count}`
placeholder must also remain exact. Any changed identifier, count, or required placeholder rejects
the response.

### Anthropic

`send_via_anthropic` sends `POST https://api.anthropic.com/v1/messages` with `x-api-key` and
`anthropic-version: 2023-06-01`. It requests at most 8,192 output tokens and reads the first text
content block from the response.

Model discovery sends authenticated `GET https://api.anthropic.com/v1/models?limit=1000`. Claude
families containing `haiku`, `sonnet`, or `opus` are labelled Economy, Balanced, or Premium
respectively. These labels express the usual relative family positioning and are not a pricing
quote.

### OpenAI-compatible

Model discovery sends authenticated `GET {base_url}/models` and accepts the standard response
shape containing a `data` array of model IDs. Some nominally compatible services omit or customize
this endpoint; the custom-model option exists for those services. IDs with `nano` or `mini` name
segments are labelled Economy and IDs with a `pro` segment are labelled Premium. Unknown names
remain unlabelled rather than guessing their cost.

`send_via_openai_compatible` appends `/chat/completions` to the configured base URL and optionally
adds a Bearer token. Translation calls request `response_format: {"type":"json_object"}`; the
provider health check uses the same JSON mode with a much smaller response. The adapter reads
`choices[0].message.content`.

Some nominally OpenAI-compatible servers do not implement `response_format`. If such a server
rejects the request, use a version/configuration that supports JSON-object responses or update the
adapter deliberately. The provider test checks the JSON response shape, but it cannot predict
context limits or formatting quality on a full post.

Both adapters use a 90-second HTTP timeout. Provider errors are returned to the editor and written
to the application log. API keys are never deliberately logged.

## Storage and Data Lifecycle

Migration `migrations/0003_ai_translation.sql` creates the provider and post-translation tables.
Migration `migrations/0004_embedded_content_translations.sql` adds localized presentation for
reusable forms and polls.

### `ai_providers`

Each provider belongs to one site. Its type and label are stored normally; its provider-specific
configuration is serialized and encrypted into `config_encrypted` using AES-256-GCM and the
installation's `SECRET_KEY`. The same cryptographic helpers are used for email-provider secrets.

Changing `SECRET_KEY` without migrating encrypted values makes existing provider configurations
undecryptable. Credentials are never prefilled back into edit forms. Admin summaries and
placeholders reveal at most the final four characters of a non-empty credential, regardless of its
delimiter format.

### `post_translations`

There is one row per `(post_id, locale)` containing:

- Localized `title`, nullable `excerpt`, and `content`.
- `source_updated_at`, copied from the source post when translation begins.
- `generated_at`, plus normal creation/update timestamps.

The unique `(post_id, locale)` constraint makes retranslation an upsert. Deleting a source post
also deletes its translations through the foreign key. Deleting an AI provider does not delete
translations it previously generated.

If the source title, excerpt, content format, or prose is saved after translation, the editor marks
the localized copy **Source changed**. It remains public until an administrator retranslates or
deletes it; translations are not refreshed automatically. Metadata-only and form/poll-marker-only
saves advance translations that were already current and do not create a false stale state. A
translation already stale from a prose edit is never made current by a later unrelated save.

### `form_translations` and `poll_translations`

There is one row per reusable resource and locale. Each JSONB payload contains only translated
presentation text; it does not clone a form or poll. `source_updated_at` supports stale badges and
lets the embedded-item action skip a component that has not changed. Foreign keys cascade when the
source form or poll is deleted.

Saved forms expose a **Translations** tab and saved polls expose a **Translations** section. Site
administrators can translate or refresh a component directly and delete one localized payload.
Deleting it does not affect submissions or votes; localized pages fall back to the source-language
component until it is translated again.

### Enabled locales

Enabled locale codes are stored per site in the existing `site_settings` table under the
`enabled_locales` key. `core/src/utils/locales.rs` contains the hand-maintained list of locale codes
offered by the UI. SynapCMS does not fetch or update that list from an external registry.

## Public Routing and Rendering

Locale-aware resolution is implemented in `core/src/handlers/page.rs`, with post rendering in
`core/src/handlers/post.rs` and shared SEO context in `core/src/handlers/home.rs`.

When the first path segment is enabled for the current site, the fallback router removes that
segment for normal post/page lookup and carries the locale separately. Examples include:

- `/es/my-post`
- `/ru/about`
- `/fr/company/team`

If a translation row exists, its three localized fields overlay the source record for rendering.
If no translation exists, SynapCMS deliberately renders the original content at the locale URL
instead of returning 404. This silent fallback keeps links working, but it can make an untranslated
page appear to be translated unless the theme communicates language state clearly.

Embed expansion happens after that localized body overlay. Before expansion, translated-content
markers are removed and the exact current source markers are inserted at corresponding structural
HTML boundaries. Therefore adding a form/poll to an already translated post does not modify or
regenerate its translated prose. If source and translated document structures cannot be reconciled,
SynapCMS retains the stored translated body and records a warning rather than guessing a new inline
position.

At render time, a matching form or poll translation is merged onto a clone of the source definition
by stable key. If no valid component translation exists, the source definition still renders and
remains functional. Form submissions
carry the locale in a reserved hidden field so confirmation subject/body text can use the same
localized payload without storing the reserved field in submission data. Poll result links carry
the locale so result labels are localized while totals still use the original poll ID and keys.

The original-language URL is used as `canonical_url`. `hreflang_links` contains the original URL
plus only locales that have an actual translation row. Enabled languages with no translation are
not advertised as alternates.

Homepage content, archives, search results, taxonomy names, menus, and navigation labels are not
currently translated. Locale prefixes select localized post/page fields; they do not create a
fully localized site shell.

## Theme Integration

The renderer provides two top-level Tera values to post and page templates:

- `canonical_url`: the source-language canonical URL.
- `hreflang_links`: entries containing `locale` and `url`.
- `current_locale`: the locale currently being rendered, or the site's base language on the
  original URL.

The bundled Flow Light theme renders canonical/alternate tags and an accessible dropdown language
switcher in `base.html`, `single.html`, and `page.html`. The closed control displays only the current
locale, so its width does not grow as translations are added. Other themes must add equivalent
markup if they should show a visitor-facing switcher. Translation URLs still resolve when a theme
has no switcher.

Site-specific themes under `sites/<site-id>/themes/<theme>/` override bundled themes. Therefore, a
site-specific Flow Light copy also needs the translation template changes; editing only
`themes/global/flow-light/` will not change a site already using its own override. Application logs
show the path actually loaded for each request.

Template loops over `hreflang_links` should be guarded because pages such as home, archive, and
search do not necessarily add these values to their Tera context.

## Permissions and Security

- The installation-wide switch (see Administrator Workflow §0) can only be changed by a super
  admin viewing their own default/home site (`can_manage_settings`), same restriction as the rest
  of System Settings. Every AI-provider and translate route rejects requests while it's off,
  regardless of the caller's own role — a site manager cannot re-enable it for just their site.
- Provider and enabled-language management uses the site's normal site-manager authorization.
- Translating a post requires content-management permission; translating a page requires
  page-management permission.
- Provider lookups are restricted to the source post's site.
- Only verified providers are offered by the editor UI.
- Admin/account responses are protected by the existing no-store and authentication middleware.
- Translation content is trusted output from the administrator-selected provider. It is parsed as
  JSON but is not passed through `post::sanitize_content` again before being stored, and themes may
  render it as trusted HTML just like saved source content. A compromised or untrusted provider
  could therefore inject markup. Administrators should review generated links, attributes,
  embeds, and formatting and should configure only providers they trust.

Do not paste decrypted credentials or the `config_encrypted` value into tickets or logs. The
encrypted value is installation-specific and still sensitive.

## Known Limitations and Caveats

- Builder/page-composition posts are unsupported.
- Translation is manual and per locale; there is no site-wide batch translation or background
  queue.
- Requests hold the admin HTTP request open for up to 90 seconds.
- Long posts can exceed provider context or output-token limits. The provider test uses a tiny
  prompt and cannot detect that condition.
- Models can ignore the formatting instruction, translate code, alter URLs, or return invalid JSON.
- Existing translations are snapshots and never update automatically.
- Only enabled locales appear in the UI, and the available locale list is curated in source.
- A locale code can collide with an existing top-level slug.
- Missing translations silently fall back to source-language content.
- The canonical URL always points to the source-language page rather than self-canonicalizing each
  localized URL.
- Post/page fields and embedded form/poll presentation text can be localized. Menus, taxonomies,
  widgets, metadata from plugins, and other site chrome remain in their source language.
- A site-specific theme override can hide the switcher even though translated URLs work.

## Troubleshooting

### Provider verifies but is absent from the editor

Confirm that the provider belongs to the same site as the post, still shows **Verified**, and at
least one locale is enabled. Saving an edit to the provider re-runs verification automatically; if
that save's flash message reported a verification failure, the provider stayed unverified — fix the
reported issue and save again, or use the Test button to retry.

### Clicking Translate saves the post instead

This was caused by an early implementation rendering a translation form inside the editor form.
Nested forms are invalid HTML. Current code uses a dedicated JavaScript request. Rebuild the Rust
application after updating `admin/src/pages/posts.rs`, restart it, and reload the editor. A stale
binary will continue serving the old behavior even when the source file has changed.

### Translation fails although Provider Test succeeds

The test proves a small structured-output round trip. A real translation can still fail because of:

- Context or output-token limits.
- A model returning commentary or malformed/truncated JSON.
- An OpenAI-compatible endpoint rejecting `response_format`.
- Rate limits, exhausted credit, provider overload, or a request timeout.
- HTML/Markdown content that produces a much larger response than expected.

The editor should display the provider or parser error. Check the dedicated structured audit log:

```bash
./app.sh translation-logs
```

For a focused historical list of failures:

```bash
jq -c 'select(.fields.outcome == "failure")' logs/ai-translation.jsonl
```

Every attempt has correlated start and terminal events with site/post/provider/model identifiers,
operation scope, duration, outcome, and failure stage. Provider HTTP errors and malformed model output include a
single-line response excerpt capped at 500 characters. Prompts, credentials, and successful
translated content are not logged.

See the **Logging** document for the central log-location reference, additional parsing recipes,
retention guidance, and the recommended cross-stream troubleshooting workflow.

### Local OpenAI-compatible provider cannot be reached

The base URL is resolved by the SynapCMS server process, not by the administrator's browser.
`localhost` therefore means the machine/container running SynapCMS. In containerized deployments,
use a hostname or network address reachable from that container. Include the required version path
but do not include `/chat/completions`, because SynapCMS appends it.

### Model list cannot be loaded

Confirm the key and base URL, then inspect the error shown beside the model control. Model discovery
uses the same saved credential as translation but calls `/models` instead of `/chat/completions`.
If the service can translate but does not expose the standard model-list endpoint, select **Enter a
custom model ID** and paste the service's exact model identifier. A successful discovery response
only proves that a model ID is visible to the credential; use Test after saving to prove that the
selected model supports SynapCMS's translation request.

### Translation reports success but is not visible publicly

Open the post editor and confirm the locale appears in its translation list. Then check:

1. The locale is still enabled for that site.
2. The requested URL begins with the exact locale code shown by the editor.
3. The post is published and otherwise publicly accessible.
4. The active theme or site-specific override contains language-switcher markup if the missing
   item is the switcher rather than the translated content itself.
5. The logs identify which global or site-specific theme path was loaded.

If the locale URL returns source-language content, the route is working but no matching
`post_translations` row was found; this is the intentional fallback behavior.

### Translation is marked Source changed

The source post's `updated_at` is newer than the translation's `source_updated_at`. Review the
source edits and run Translate again to replace that locale's snapshot.

### Formatting is damaged

Compare the saved translation with the source and inspect the model response error/log if one was
reported. Retranslate with a model that follows structured-output and markup-preservation
instructions more reliably. There is no automated semantic or DOM-equivalence validator in this
version.

## Routes

| Method | Path | Purpose |
|---|---|---|
| POST | `/admin/sites/{id}/ai-providers` | Create a provider and verify it |
| POST | `/admin/sites/{id}/ai-providers/models` | Discover models with unsaved Add Provider credentials |
| POST | `/admin/sites/{id}/ai-providers/{provider_id}` | Update provider configuration and re-verify |
| POST | `/admin/sites/{id}/ai-providers/{provider_id}/models` | Discover models while retaining blank saved credentials |
| POST | `/admin/sites/{id}/ai-providers/{provider_id}/test` | Test and verify a provider |
| POST | `/admin/sites/{id}/ai-providers/{provider_id}/delete` | Delete a provider |
| POST | `/admin/sites/{id}/enabled-locales` | Save enabled locales |
| POST | `/admin/posts/{id}/translate` | Generate or refresh stale post prose; refuses a current translation |
| POST | `/admin/posts/{id}/translate-embeds` | Translate only missing/stale embedded forms and polls for an existing locale |
| POST | `/admin/posts/{id}/translations/{locale}/delete` | Delete one translation |
| GET | `/{locale}/{slug}` and nested page paths | Render localized content or source fallback |

## Implementation Map

| File | Responsibility |
|---|---|
| `core/src/translate.rs` | Prompt construction, provider HTTP adapters, response parsing |
| `core/src/models/ai_provider.rs` | Encrypted provider configuration and verification state |
| `core/src/models/post_translation.rs` | Translation reads, listing, upsert, deletion, staleness |
| `core/src/models/embedded_translation.rs` | Locale-specific reusable form/poll presentation payloads |
| `core/src/embedded_content.rs` | Mechanical source-marker reconciliation into translated prose |
| `core/src/models/site_locale.rs` | Per-site enabled locale setting |
| `core/src/utils/locales.rs` | Curated locale codes and display names |
| `core/src/handlers/admin/ai_providers.rs` | Provider create/edit/test/delete handlers |
| `core/src/handlers/admin/posts.rs` | Translation generation and deletion handlers |
| `admin/src/pages/sites.rs` | AI Translation settings UI |
| `admin/src/pages/posts.rs` | Post-editor translation controls and status list |
| `core/src/handlers/page.rs` | Locale-prefix recognition and page resolution |
| `core/src/handlers/post.rs` | Translation overlay during post rendering |
| `core/src/handlers/home.rs` | Canonical and `hreflang` context construction |
| `migrations/0003_ai_translation.sql` | Provider and translation tables |
| `migrations/0004_embedded_content_translations.sql` | Form/poll translation tables |
| `core/src/app_state.rs` | `AppSettings::ai_translation_enabled` installation-wide switch |
| `core/src/handlers/admin/settings.rs` | Features tab save handler for the installation-wide switch |

Cross-feature provider boundaries, safety rules, and telemetry conventions are defined in
`docs/ai-integration-standards.md`.

## Testing and Verification

Unit coverage includes prompt construction, JSON/fence parsing, provider encryption/decryption,
secret masking, and locale-list hygiene. Ignored HTTP integration tests in `core/tests/routes.rs`
cover translated rendering, source fallback, and non-enabled locale behavior without spending API
credits.

Useful checks after changing the implementation:

```bash
cargo test -p synaptic-core --lib
git diff --check
./app.sh rebuild
curl -kfsS -o /dev/null -w '%{http_code}\n' https://localhost/<locale>/<slug>
```

Theme changes do not require Rust compilation, but Tera may cache manually edited templates; use
`./app.sh restart` when needed and verify the rendered route and served stylesheet. Rust/admin UI
changes require `./app.sh rebuild`.
