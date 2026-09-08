---
title: Form Designer
group: feature
updated_by: claude
last_updated: 2026-08-16
---
# Form Designer

> Last updated: 2026-08-16 | Updated by: claude

## Overview

Form Designer lets a non-developer build a reusable form — an ordered list of fields plus a few behavior settings — in the admin panel, then insert it into any post or page from a picker in the content editor, instead of hand-writing `<form>` HTML per theme page. It's the *definition* side of the forms system: `models::form_def` owns a form's shape (fields/settings); the pre-existing `models::form_submission` (see the **Forms** doc) owns the data visitors actually send in. The two are linked by the form's `slug` string (a submission's `form_name` matches a `forms.slug`) **and**, since 2026-08-16, also by a real `form_submissions.form_id` foreign key populated at submit time — see **Database Schema** below and the **Forms** doc for why the slug link is still the authoritative one.

The nav item for this page is labeled **Form Builder** (renamed from "Forms" 2026-08-15, to avoid confusion with the separate **Forms**/Analytics nav item) and sits directly above Page Builder.

Field types cover the common cases plus two visual-only elements for structuring longer forms:

| Type | Renders as | Notes |
|------|-----------|-------|
| `text`, `email`, `number`, `phone`, `date` | `<input type="...">` | phone maps to `type="tel"`; email additionally gets a `pattern` attribute (added 2026-08-04) requiring a dot in the domain, since HTML5's native `type="email"` validation alone accepts a domain-less address like `a@b` |
| `textarea` | `<textarea>` | |
| `select` | `<select>` | options declared as value/label pairs |
| `radio` | radio button group | options declared as value/label pairs |
| `checkbox` | single checkbox | |
| `toggle` | checkbox styled as a switch | options textarea holds exactly two lines — off state label/value, then on state |
| `separator` | `<hr>`, with an optional section title above it | visual only — no `name`, nothing submitted |
| `note` | a tinted callout box (info text/instructions) | visual only — no `name`, nothing submitted |

## How It Works

### Data model (`core/src/models/form_def.rs`)

- `forms` table (migration `0052_create_forms.sql`; `email_provider_id` added by `0059_add_email_provider_to_forms.sql`; `total_submissions` added by `0061_forms_total_submissions.sql`): `id, site_id, name, slug, fields JSONB, settings JSONB, email_provider_id UUID NULL, total_submissions BIGINT, created_at, updated_at`, `UNIQUE (site_id, slug)`.
- `FormField { label, name, field_type, required, options: Vec<(String, String)> }` — `options` is only meaningful for select/radio (arbitrary value/label pairs) and toggle (exactly the off/on pair).
- `FormSettings { success_message, button_label, include_honeypot, notify_email, confirm_submitter, confirm_subject, confirm_body, no_mail }` — deliberately has no button-color field; the submit button is styled from the active theme's own CSS (`.themed-form button`/`.btn`), not chosen per-form. Which **provider** these emails send through is `email_provider_id`, a real column (not inside `settings`, so it can carry a real FK) — see **Mail Settings tab** below. `no_mail` (added 2026-08-16) is a hard override: when set, the form still saves submissions normally but `notify_email`/`confirm_submitter` are both skipped entirely at submit time, regardless of their own values.
- The slug is generated once from the name on creation (with `-2`, `-3`, ... suffixing on collision, same convention as post/page slugs) and is **immutable after creation** — both `form_submissions.form_name` and post/page embeds reference it, so renaming it later would silently orphan both.

### Editor (`/admin/form-designer`)

- List page: search + pagination (in-memory, 20/page — same pattern as `/admin/sites`, since a site's form count is small). Each row's name is plain text (no longer a link, as of 2026-08-16); the row action is a single **Edit** icon linking to the editor — the analytics-icon-on-the-list-row pattern was removed in favor of reaching per-form analytics from inside the editor itself (see below).
- Edit page (`/admin/form-designer/{id}`, shared with the create page at `/new`): a **Fields** card on the left, and a **Form Settings** card on the right with three tabs (**General Settings**, **Mail Settings**, **Preview**) — all-panels-in-DOM, JS-toggled, same `.page-tabs`/`.form-tab-panel` pattern used elsewhere in admin. An **Analytics** icon button (bar-chart-2) sits next to Save on the General Settings tab, linking to `/admin/analytics?tab=forms&form={id}` — the Forms tab pre-filtered to just this one form (see the **Forms** doc).
  - **General Settings**: form name, success message, button label, honeypot toggle. Its own **Save** icon-pill (also holds Analytics and Delete) submits the whole editor form.
  - **Mail Settings**: **Don't send any email for this form** toggle (`no_mail`, greys out the rest of the tab when on) at the top; then **Send via** (the provider picker — see below), **Notify on new submission**, the **Email the submitter a confirmation** toggle, and — moved here from General Settings on 2026-08-16 — the confirmation email's subject/body fields. Has its own **Save** icon-pill at the bottom of the tab, styled identically to General Settings'; both buttons submit the same underlying `<form>` (all tabs' inputs are always present in the DOM, just hidden via `display:none` on the inactive panel), so either one saves everything, not just that tab's fields.
  - **Preview**: a live, disabled-input mockup of the public form.
- Fields are edited entirely client-side and serialized to one hidden JSON field (`fields_json`) on submit — no per-field DB rows, matching the JSONB-column approach `form_submissions` already uses.
- Save stays disabled until something actually differs from what loaded (`snapshot()`/`checkDirty()` in the page's script — compares a JSON snapshot of every field + setting, including `email_provider_id` and `no_mail`, against the one captured on load), mirroring the dirty-check pattern the theme customizer's per-card Save buttons use.
- Separator/note rows skip the usual "both label and name are empty, discard this row" guard on submit — a titleless separator is a normal, common case, not an accidentally-added blank row.

### Inserting a form into a post or page

The Quill editor on `/admin/posts/*` and `/admin/pages/*` (both post types share one editor, `admin/src/pages/posts.rs`) registers a custom embed format:

- `FormEmbedBlot` (`admin/src/pages/posts.rs`) — a Quill `BlockEmbed`, `blotName: 'form-embed'`, serializes as the tag `<ss-form data-slug="..." data-label="...">` (mirrors the pre-existing `AudioBlot` pattern used for inline `<audio>` embeds). It renders as an inert placeholder chip in the editor (styled via CSS `::before` reading `data-label`, so the node itself stays empty) and is **not** editable text — an author can't mistype it into invalid state.
- A toolbar button (clipboard icon, next to the audio button) opens a dropdown of the site's saved forms — sourced from `PostEdit.saved_forms: Vec<(slug, name)>`, populated by `fetch_saved_forms()` in `core/src/handlers/admin/posts.rs` at every post/page editor entry point — and calls `quill.insertEmbed(range.index, 'form-embed', {slug, label}, 'user')` at the cursor.
- **The HTML sanitizer must allowlist the embed or it silently vanishes on save.** `core/src/models/post::sanitize_content()` runs on every save as defense-in-depth against unsafe HTML; `ss-form` and its `data-slug`/`data-label` attributes are explicitly added to ammonia's allowlist (same mechanism already used for `audio`/`source`). This was a real bug hit during development — the embed round-tripped fine until save, which stripped the then-unlisted tag.

### Render-time expansion

- `form_def::expand_embeds(pool, site_id, content)` scans for `<ss-form data-slug="...">` and replaces each with the real rendered `<form>` for that definition (`FormDef::render_html()`). It's a plain substring check (`content.contains("<ss-form")`) before touching the database, so posts/pages without an embed — the overwhelming majority — cost nothing extra.
- Called from `build_post_context()` in `core/src/handlers/home.rs`, which runs on every single post/page render (gated only on the post having a `site_id`, which every real post/page does) — so classic Tera-rendered posts and pages both pick up embeds automatically, with no theme-template changes required.
- `render_html()`'s markup deliberately matches the hand-written-form convention themes already used for contact/newsletter/subscribe pages (`.themed-form`, `.form-field`, `.form-required`, `.form-checkbox-label`, `.honeypot-field`, `.form-success`, and the theme's generic `.btn`) instead of inventing a parallel class scheme — a theme that already styles those (Leisure, Symantic Signals) renders a fully-styled embedded form with zero new CSS. **The Default theme does not have this "shared form styles" CSS block yet** — a form inserted into a page using the Default theme currently renders unstyled; the CSS would need to be added there the same way it was to Leisure and Symantic Signals if that theme goes into real use.
- The honeypot field (when enabled) is a real, visually-hidden `<div class="honeypot-field">` + `<input name="_honeypot">`; the underlying `/form/{slug}` submit handler already strips any field name starting with `_` before storage (see the **Forms** doc), so no changes were needed there.
- Success message: `form::submit` redirects with `?submitted={slug}` (URL-encoded form slug, not a generic `?submitted=1`) so a page with more than one embedded form shows the right one's success message. `render_html()` emits a small inline script per form that checks `location.search` for that exact slug and swaps the form for `.form-success` — automatic, unlike hand-written theme pages which do this server-side themselves via `{% if request.query.submitted %}`.

### Mail Settings tab: no-mail override, notify + confirm + provider picker (updated 2026-08-16)

Everything about whether/how a form emails anyone now lives on the **Mail Settings** tab, in this order:

1. **Don't send any email for this form** (`no_mail`, added 2026-08-16) — a hard off switch. When on, the form behaves exactly as if it had no email settings at all: submissions still save to the database and are visible/exportable in the admin as normal, but `form::submit` skips both the notify and confirm sends entirely, even if `notify_email` is filled in or `confirm_submitter` is checked. It exists as an explicit, discoverable "definitely no mail" setting rather than relying on an admin remembering to blank out both fields separately.
2. **Send via** — which configured email provider (or the install-wide fallback) both sends below go through; see next paragraph.
3. **Notify on new submission** (`notify_email: Option<String>`) — when set, every submission triggers a background email with the submitted fields as plain-text `key: value` lines, subject `New submission: {form name}`.
4. **Email the submitter a confirmation** (`confirm_submitter: bool`) — auto-replies to whichever value the form's first `email`-type field collected, using the `confirm_subject`/`confirm_body` templates (moved to this tab 2026-08-16 — previously on General Settings; `{{field_name}}` placeholders get filled from the submission).

Both sends go through **whichever email provider the form's `email_provider_id` points at** (the **Send via** dropdown — lists "Install-wide default account" plus every *verified* provider configured on the site) — or the install-wide Mailgun fallback in `.env` if none is selected. See the **Email Providers** doc for the full provider system, and the **Sites & Multisite** doc for where providers are configured.

The send happens in the background, after the submission is already stored and the visitor is redirected — a slow or failed provider call never delays or blocks the actual form submission. A failure is logged server-side, not shown to the visitor.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /admin/form-designer | form_designer::list | List forms for the site (search via `?search=`, `?page=`, `?partial=1` for the live-search AJAX fragment) |
| POST | /admin/form-designer | form_designer::create | Create a form |
| GET | /admin/form-designer/new | form_designer::new_form | New-form editor |
| GET | /admin/form-designer/{id} | form_designer::edit_form | Edit-form editor |
| POST | /admin/form-designer/{id} | form_designer::update | Save changes to a form (fields, settings — including `no_mail` — and `email_provider_id`) |
| POST | /admin/form-designer/{id}/delete | form_designer::delete | Delete a form definition (does not delete its submissions) |

## Database Schema

- `forms` (migration `0052_create_forms.sql`; `email_provider_id` added by `0059_add_email_provider_to_forms.sql`; `total_submissions` added by `0061_forms_total_submissions.sql`): `id UUID PK`, `site_id UUID` (FK → `sites`, cascade delete), `name TEXT`, `slug TEXT`, `fields JSONB` (default `[]`), `settings JSONB` (default `{}`), `email_provider_id UUID NULL` (FK → `email_providers`, `ON DELETE SET NULL`), `total_submissions BIGINT NOT NULL DEFAULT 0`, `created_at`, `updated_at`. Unique on `(site_id, slug)`. `notify_email`/`confirm_submitter`/`confirm_subject`/`confirm_body`/`no_mail` live inside `settings` JSONB; `email_provider_id` and `total_submissions` are real columns — the former so it can carry a real foreign key, the latter so it can be atomically incremented.
- `total_submissions` is a **lifetime counter, incremented once per public submission and never decremented** (see `form_submission::create` in the **Forms** doc) — it deliberately diverges from the live submission count shown on the Submissions tab, so deleting old responses for cleanup doesn't erase the historical record of how much a form was actually used. Surfaced on the Stats tab of `/admin/analytics/form/{id}`.

## Security Notes

- The embed tag and its two attributes are the only theme-injectable HTML explicitly allowlisted beyond ammonia's defaults (alongside the pre-existing `audio`/`source`) — everything else in post content is still sanitized normally.
- Field definitions themselves are never rendered as Tera template source (per the project's cardinal template-injection rule) — `render_html()` builds plain escaped HTML strings in Rust, never passes user-supplied field labels/options through the template engine.
- Deleting a form definition does not touch `form_submissions` — existing collected data for that form name is untouched and still visible/exportable (see the **Forms** doc for where), it just has no matching `forms` row anymore (`form_id` on those rows is set NULL via the FK's `ON DELETE SET NULL`, and the slug-based lookup no longer resolves either).
