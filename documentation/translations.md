---
title: AI Post Translation
group: feature
updated_by: codex
last_updated: 2026-09-09
---
# AI Post Translation

> Last updated: 2026-09-09 | Updated by: codex

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

### 1. Configure a provider

Open **Sites**, choose the site, then open **Settings → AI Translation**. Add either:

- **Anthropic:** API key and exact model name.
- **OpenAI-Compatible:** base URL, optional API key, and exact model name. Include the API version
  in the base URL when the service requires it, for example `https://api.openai.com/v1`.

Provider credentials are write-only in the UI. Editing a provider requires entering all its
fields again, and any edit resets its verification status.

Use the provider's globe/Test button after saving it. A successful test must complete an actual
model request and return `OK`. The provider is then marked verified and becomes available in post
editors. Verification establishes basic connectivity, authentication, and model-name validity; it
does not guarantee that a much larger translation request will fit the model's limits or produce
valid structured output.

### 2. Enable languages

In the same AI Translation tab, select the languages the site should expose and save the list.
Only enabled languages appear in the post editor.

Enabling a locale reserves its code as the first public URL segment for the entire site. For
example, enabling Spanish reserves `/es/...`. Check for a top-level post or page whose slug is
exactly `es` before enabling it, because that path will be interpreted as the locale namespace.

### 3. Translate a post or page

Open an existing post or page and find **Translations** in the editor sidebar. Choose a language
and a verified provider, then press the globe button.

The request uses the last saved version of the post. If the editor contains unsaved changes,
SynapCMS asks the administrator to save them first. While the model request is running, the globe
button is disabled. The request is synchronous and can legitimately take several seconds.

On success, the editor reloads with a message such as `Translated into Spanish.` and lists the
translation with a public View link. Translating the same post into the same locale again replaces
the previous translation. Use the trash button next to a translation to delete only that localized
copy; the source post is unaffected.

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

### Anthropic

`send_via_anthropic` sends `POST https://api.anthropic.com/v1/messages` with `x-api-key` and
`anthropic-version: 2023-06-01`. It requests at most 8,192 output tokens and reads the first text
content block from the response.

### OpenAI-compatible

`send_via_openai_compatible` appends `/chat/completions` to the configured base URL and optionally
adds a Bearer token. Translation calls request `response_format: {"type":"json_object"}`; the
plain-`OK` provider health check omits JSON mode. The adapter reads
`choices[0].message.content`.

Some nominally OpenAI-compatible servers do not implement `response_format`. If such a server
rejects the request, use a version/configuration that supports JSON-object responses or update the
adapter deliberately; the basic provider test may still pass because its prompt is smaller and
does not prove every translation behavior.

Both adapters use a 90-second HTTP timeout. Provider errors are returned to the editor and written
to the application log. API keys are never deliberately logged.

## Storage and Data Lifecycle

Migration `migrations/0003_ai_translation.sql` creates the two feature tables.

### `ai_providers`

Each provider belongs to one site. Its type and label are stored normally; its provider-specific
configuration is serialized and encrypted into `config_encrypted` using AES-256-GCM and the
installation's `SECRET_KEY`. The same cryptographic helpers are used for email-provider secrets.

Changing `SECRET_KEY` without migrating encrypted values makes existing provider configurations
undecryptable. Credentials are masked in summaries and never prefilled back into edit forms.

### `post_translations`

There is one row per `(post_id, locale)` containing:

- Localized `title`, nullable `excerpt`, and `content`.
- `source_updated_at`, copied from the source post when translation begins.
- `generated_at`, plus normal creation/update timestamps.

The unique `(post_id, locale)` constraint makes retranslation an upsert. Deleting a source post
also deletes its translations through the foreign key. Deleting an AI provider does not delete
translations it previously generated.

If the source post is saved after translation, the editor marks the localized copy **Source
changed**. It remains public until an administrator retranslates or deletes it; translations are
not refreshed automatically.

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

The bundled Flow Light theme renders canonical/alternate tags and a compact language switcher in
`base.html`, `single.html`, and `page.html`. Other themes must add equivalent markup if they should
show a visitor-facing switcher. Translation URLs still resolve when a theme has no switcher.

Site-specific themes under `sites/<site-id>/themes/<theme>/` override bundled themes. Therefore, a
site-specific Flow Light copy also needs the translation template changes; editing only
`themes/global/flow-light/` will not change a site already using its own override. Application logs
show the path actually loaded for each request.

Template loops over `hreflang_links` should be guarded because pages such as home, archive, and
search do not necessarily add these values to their Tera context.

## Permissions and Security

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
- Translation is manual and per post/page; there is no batch translation or background queue.
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
- Only post/page fields are localized; menus, taxonomies, widgets, metadata from plugins, and other
  site chrome remain in their source language.
- A site-specific theme override can hide the switcher even though translated URLs work.

## Troubleshooting

### Provider verifies but is absent from the editor

Confirm that the provider belongs to the same site as the post, still shows **Verified**, and at
least one locale is enabled. Editing provider settings resets verification, so run Test again.

### Clicking Translate saves the post instead

This was caused by an early implementation rendering a translation form inside the editor form.
Nested forms are invalid HTML. Current code uses a dedicated JavaScript request. Rebuild the Rust
application after updating `admin/src/pages/posts.rs`, restart it, and reload the editor. A stale
binary will continue serving the old behavior even when the source file has changed.

### Translation fails although Provider Test succeeds

The test proves only a small round trip. A real translation can still fail because of:

- Context or output-token limits.
- A model returning commentary or malformed/truncated JSON.
- An OpenAI-compatible endpoint rejecting `response_format`.
- Rate limits, exhausted credit, provider overload, or a request timeout.
- HTML/Markdown content that produces a much larger response than expected.

The editor should display the provider or parser error. Check the server log for the complete
server-side error:

```bash
./app.sh logs
```

For a focused historical search:

```bash
rg -n "translation failed|test call failed|OpenAI-compatible request|Anthropic request" logs/synapcms.log
```

Provider HTTP errors include the response status and provider body. JSON parser errors include the
returned model text; inspect it carefully because translated post content may be present in logs.

### Local OpenAI-compatible provider cannot be reached

The base URL is resolved by the SynapCMS server process, not by the administrator's browser.
`localhost` therefore means the machine/container running SynapCMS. In containerized deployments,
use a hostname or network address reachable from that container. Include the required version path
but do not include `/chat/completions`, because SynapCMS appends it.

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
| POST | `/admin/sites/{id}/ai-providers` | Create a provider |
| POST | `/admin/sites/{id}/ai-providers/{provider_id}` | Replace provider configuration and reset verification |
| POST | `/admin/sites/{id}/ai-providers/{provider_id}/test` | Test and verify a provider |
| POST | `/admin/sites/{id}/ai-providers/{provider_id}/delete` | Delete a provider |
| POST | `/admin/sites/{id}/enabled-locales` | Save enabled locales |
| POST | `/admin/posts/{id}/translate` | Generate or replace one translation |
| POST | `/admin/posts/{id}/translations/{locale}/delete` | Delete one translation |
| GET | `/{locale}/{slug}` and nested page paths | Render localized content or source fallback |

## Implementation Map

| File | Responsibility |
|---|---|
| `core/src/translate.rs` | Prompt construction, provider HTTP adapters, response parsing |
| `core/src/models/ai_provider.rs` | Encrypted provider configuration and verification state |
| `core/src/models/post_translation.rs` | Translation reads, listing, upsert, deletion, staleness |
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
