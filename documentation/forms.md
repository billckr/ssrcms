---
title: Forms
group: feature
updated_by: claude
last_updated: 2026-08-16
---
# Forms

> Last updated: 2026-08-16 | Updated by: claude

## Overview

SynapCMS lets themes embed plain HTML `<form>` elements that POST to `/form/{name}` (`{name}` is always a form's *slug*, whether it was hand-written into a theme template or created via Form Designer — see that doc). There is no required schema — any field names a theme author (or Form Designer) chooses are accepted and stored as JSONB, so a form's fields can change without a migration. Submissions are scoped per site and viewable/exportable from the admin. Admins can also disable ("block") a named form so it silently stops accepting submissions (e.g. to stop spam) without touching the theme template or Form Designer definition.

Where to look at collected data has moved around a couple of times; as of 2026-08-16 the canonical place is the **Submissions tab** on a form's own Analytics page (`/admin/analytics/form/{id}?tab=submissions`), reached via the Analytics icon on `/admin/analytics?tab=forms` or from inside the Form Designer editor. See **Admin views** below for how the older `/admin/form-data-analytics/{slug}` URL still fits in.

## How It Works

- **Public submission** (`core/src/handlers/form.rs`, `submit`): accepts any `Form<HashMap<String,String>>` posted to `/form/{name}`. Field names starting with `_` (e.g. a honeypot `_hp`) are stripped before storage so they never persist — a basic anti-spam trick. If all remaining fields are blank, nothing is stored. Before storing, it checks `form_submission::is_blocked` for the current site + form name; if blocked, it redirects back to the referring page with `?blocked=1` and never writes a row.
- **IP capture**: best-effort — prefers the `X-Real-IP` header, falls back to `X-Forwarded-For` (first entry), then falls back to the raw TCP peer address via `ConnectInfo`. In production Caddy sets the proxy headers; in local dev the peer address is used.
- **Email notifications**: `submit` looks up the form's definition by slug (`form_def::get_by_slug`) to read its mail settings. Unless `settings.no_mail` is set (added 2026-08-16 — see the **Form Designer** doc), a `notify_email` address sends a plain-text admin notification, and `confirm_submitter` sends the submitter a templated confirmation — both `tokio::spawn`ed *after* the submission is already stored, so a slow or failed provider call never blocks the visitor's redirect, and both route through `core::mail::send_for_site` using whichever provider the form's `email_provider_id` selects (or the install-wide fallback). Failures are logged server-side only, never surfaced to the visitor. Full setup guide: `docs/mailgun-email-guide.md` in the repo.
- **Redirect UX**: on success, redirects back to the `Referer` (query string stripped) with `?submitted={slug}` appended (the form's own slug, not a generic `?submitted=1`, so a page with multiple embedded forms shows the right one's success message).
- **Lifetime counter**: after the submission row is inserted, `form_submission::create` also runs a best-effort `UPDATE forms SET total_submissions = total_submissions + 1 WHERE id = $1` when the submission resolved to a real `form_id`. This never decrements, even when a submission is later deleted — see the **Form Designer** doc's Database Schema section for why, and where it's displayed.
- **Storage** (`core/src/models/form_submission.rs`): `create` inserts one row per submission with the full field map as `data: JSONB`, plus (since 2026-08-16) a `form_id` FK — see **Database Schema**. Helper queries: `list_forms` (distinct form names per site with submission/unread counts, ordered by most recent), `count_for_form`/`list_submissions` (paginated, newest first — these still key off `form_name`, not `form_id`, since orphaned submissions with a NULL `form_id` still need to be listable by their old slug), `delete` / `delete_all`, `count_unread`, and `mark_all_read` (called automatically when an admin opens a form's submissions).
- **Blocking** lives in the same model file, not a separate `form_block` model: `is_blocked`, `block`, `unblock`, and `blocked_names` all query the `form_blocks` table directly.
- **Admin views**:
  - `/admin/analytics?tab=forms` (`core/src/handlers/admin/analytics.rs::list`) is the main list — one row per distinct form name submitted on the site, with submission/unread counts, last-submitted time, and blocked state. A deleted form definition still shows up here (flagged, not hidden — see **Database Schema**), just without Edit/Analytics actions. Supports `?form={id}` to pre-filter to a single form (shown with a "Filtered to X — Clear" chip) — this is what the Form Designer editor's Analytics icon links to.
  - `/admin/analytics/form/{id}` (`admin_analytics::form_detail`) is a form's dedicated page, gated on having a real definition (an orphaned, definition-deleted form has no `{id}` to reach this by). Three tabs: **Stats** (total lifetime submissions, plus a Delivered/Failed email chart), **Delivery Results** (`mail_log` history, sortable/searchable), and **Submissions** (added 2026-08-16 — the actual collected data, replacing the old standalone page below). Its search/export/delete-all controls sit inline with the tab bar, same layout convention as Delivery Results' search.
  - `/admin/form-data-analytics/{slug}` (`admin_forms::view_form`) still exists but as of 2026-08-16 redirects to the Submissions tab above whenever the slug still resolves to a live form definition — it only renders directly for an **orphaned** form (definition deleted), which has nowhere else to go. `collect_columns` (shared with the Submissions tab and CSV export) inspects every submission's JSONB keys, prioritizes `name`, `email`, `subject`, `message` first, then adds any remaining keys alphabetically.
  - `export_csv` reuses the same column derivation to produce an RFC-4180-escaped CSV download (`form-{slug}.csv`). `toggle_block` flips the blocked state for a form. Deleting a single submission or all submissions for a form redirects back to the Submissions tab when a live definition exists, or the standalone page otherwise (same fallback logic as the view route).
- All admin form handlers require `admin.caps.can_manage_forms` and a resolved `site_id` from the `AdminUser` (returns 403/400 otherwise).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | /form/{name} | form::submit | Public form submission endpoint (used by any theme `<form action="/form/{name}">`, `{name}` being the form's slug) |
| GET | /admin/analytics?tab=forms | admin_analytics::list | List all forms for the current site with counts; `?form={id}` pre-filters to one form |
| GET | /admin/analytics/form/{id} | admin_analytics::form_detail | Stats / Delivery Results / Submissions tabs for one form (`?tab=stats\|results\|submissions`) |
| GET | /admin/form-data-analytics/{name} | admin_forms::view_form | Redirects to the Submissions tab above if the slug resolves to a live form; otherwise renders submissions directly (orphaned forms only) |
| POST | /admin/form-data-analytics/{name}/{id}/delete | admin_forms::delete_submission | Delete one submission |
| POST | /admin/form-data-analytics/{name}/delete-all | admin_forms::delete_all | Delete all submissions for a form |
| GET | /admin/form-data-analytics/{name}/export | admin_forms::export_csv | Download all submissions as CSV |
| POST | /admin/form-data-analytics/{name}/toggle-block | admin_forms::toggle_block | Block or unblock a form from accepting new submissions |

## Database Schema

- `form_submissions` (migration `0020_create_form_submissions`; `form_id` added by `0060_form_submissions_form_id.sql`): `id UUID PK`, `site_id UUID` (FK → `sites`, cascade delete), `form_name TEXT` (the slug — still the authoritative lookup key everywhere, since it's immutable and matches even for orphaned/pre-FK data), `data JSONB` (default `{}`), `ip_address TEXT`, `read_at TIMESTAMPTZ` (nullable — null means unread), `submitted_at TIMESTAMPTZ` (default now), `form_id UUID NULL` (FK → `forms.id`, `ON DELETE SET NULL`). Indexed on `(site_id, form_name, submitted_at DESC)` and on `form_id` (partial, where not null).
- `form_id` exists for exact joins/filtering (e.g. the `?form={id}` list filter) alongside `form_name`, not as a replacement for it — deleting a form definition sets existing rows' `form_id` back to NULL via the FK, but `form_name` still matches and the data stays fully viewable/exportable, just under the "orphaned" fallback path described above. New rows get `form_id` populated automatically whenever `form::submit` can resolve the slug to a live definition at submit time.
- `form_blocks` (migration `0021_create_form_blocks`): `site_id UUID` (FK → `sites`, cascade delete), `form_name TEXT`, `blocked_at TIMESTAMPTZ` (default now), composite PK `(site_id, form_name)`. Presence of a row means the form is blocked.

## Security Notes

- Underscore-prefixed field names are stripped server-side, closing off internal/honeypot fields from ever being stored or overwriting real columns.
- Blocking is enforced before any DB write and fails silently from the visitor's perspective (redirect only, no error page), avoiding tipping off spammers.
- All `/admin/form-data-analytics/*` and `/admin/analytics/*` form-related routes require `can_manage_forms` capability and a resolved site context; missing either returns 403 or 400 before touching the database.
- CSV export values are escaped per RFC 4180 to prevent malformed downloads from embedded commas/quotes/newlines (not a CSV-injection/formula-injection sanitizer — values are not prefixed to neutralize leading `=`/`+`/`-`/`@`).
- Submissions are strictly scoped by `site_id` in every query, so one site's admins cannot see or delete another site's form data.
- The notification/confirmation email sends (above) are fire-and-forget and failure-tolerant — a broken or unconfigured provider never prevents a submission from being stored or the visitor's redirect from completing, and `no_mail` (see the **Form Designer** doc) gives an explicit, auditable way to guarantee a form never emails anyone at all.
