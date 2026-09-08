---
title: Email Providers
group: feature
updated_by: claude
last_updated: 2026-08-15
---
# Email Providers

> Last updated: 2026-08-15 | Updated by: claude

## Overview

A site can configure any number of named third-party email accounts — Mailgun, generic SMTP, SendGrid, Postmark — instead of the app being locked to a single install-wide Mailgun account. Each **form** (see the **Form Designer** doc) independently picks which configured provider its notify/confirmation emails send through, on that form's own Mail Settings tab. There's no single "default" provider per site — different forms on the same site may legitimately want different accounts (e.g. a client's own account vs. the agency's shared one). A form with no provider selected falls back to the install-wide Mailgun account set once in `.env`, same as the original single-account model.

This doc covers the provider system itself. See the **Sites & Multisite** doc for where it's configured (Site Settings → Email Settings), and the **Form Designer** doc for how a form picks one.

## How It Works

### Data model (`core/src/models/email_provider.rs`)

- `email_providers` table (migration `0058_create_email_providers.sql`): `id UUID PK`, `site_id UUID` (FK → `sites`, cascade delete), `provider_type TEXT` (`mailgun` | `smtp` | `sendgrid` | `postmark`), `label TEXT` (admin-chosen name), `config_encrypted TEXT`, `verified BOOLEAN`, `created_at`, `updated_at`.
- `forms.email_provider_id` (migration `0059_add_email_provider_to_forms.sql`): nullable FK → `email_providers`, `ON DELETE SET NULL` — deleting a provider a form was using silently reverts that form to the install-wide fallback rather than erroring or orphaning the form.
- Credentials vary by provider type, so rather than a wide sparse column set they're serialized to one JSON blob (`ProviderConfig` enum, `#[serde(tag = "provider_type")]`) and encrypted as a single opaque string (`config_encrypted`) via the same `crypto::encrypt`/`decrypt` (AES-256-GCM, keyed off `SECRET_KEY`) the original per-site Mailgun key used.

### Sending (`core/src/mail.rs`)

- `send_via()` dispatches on the `ProviderConfig` variant: Mailgun and SendGrid/Postmark are plain `reqwest` HTTP calls (multipart for Mailgun, JSON for the other two); SMTP goes through `lettre`'s async transport (STARTTLS / implicit TLS / none, per the provider's saved `tls_mode`).
- `resolve_provider()` — given a form's `email_provider_id` (or `None`), loads and decrypts that `email_providers` row, or falls back to `AppConfig.mailgun_api_key`/`mailgun_domain` from `.env` when unset.
- `send_for_site()` is the existing entry point (used by Form Designer's notify/confirm sends and by password recovery) — unchanged in shape, just now resolves through a provider instead of being hardcoded to Mailgun. Every send attempt is still recorded to `mail_log` regardless of which provider handled it.
- `send_test_email()` — a separate, `mail_log`-free path used only by the "Test" button in Email Settings; success marks the provider `verified = true`.

### Verification gate

Only providers with `verified = true` appear in a form's "Send via" dropdown — a provider with typo'd or revoked credentials can't accidentally be selected without ever having sent a real message through it. Editing a provider's credentials resets `verified` back to `false` (the new values haven't been proven to work yet), so it drops out of every form's dropdown until re-tested — existing forms keep their `email_provider_id` pointing at it, they just silently fall back to the install-wide account until it's re-verified.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | /admin/sites/{id}/email-providers | `admin_email_providers::create` | Add a new provider |
| POST | /admin/sites/{id}/email-providers/{provider_id} | `admin_email_providers::update` | Full-overwrite update of a provider's label/credentials; resets `verified` |
| POST | /admin/sites/{id}/email-providers/{provider_id}/test | `admin_email_providers::test` | Send a test email; marks `verified = true` on success |
| POST | /admin/sites/{id}/email-providers/{provider_id}/delete | `admin_email_providers::delete` | Delete a provider (forms using it fall back via `ON DELETE SET NULL`) |

## Database Schema

- `email_providers` (migration `0058_create_email_providers.sql`): `id UUID PK`, `site_id UUID` (FK → `sites`), `provider_type TEXT`, `label TEXT`, `config_encrypted TEXT`, `verified BOOLEAN DEFAULT FALSE`, `created_at`, `updated_at`.
- `forms.email_provider_id` (migration `0059_add_email_provider_to_forms.sql`): nullable `UUID` FK → `email_providers(id) ON DELETE SET NULL`.

## Security Notes

- Every provider's credentials are encrypted (AES-256-GCM) before being written to `config_encrypted`, keyed off `SECRET_KEY` — never stored or displayed in plaintext once saved, and never sent back to the browser (the Edit form is always blank, a full overwrite rather than a prefill).
- If `SECRET_KEY` is ever rotated, previously-saved provider credentials can no longer be decrypted — sends through that provider fail and are logged (not silently swallowed), and the form does **not** automatically fall back to the install-wide account the way an unresolvable provider row does; re-enter credentials via Edit after a rotation.
- The `POST .../test` endpoint sends to the requesting admin's own account email, not an arbitrary address — it can't be used to relay email to a third party.

