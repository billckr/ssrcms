# Transactional Email (Mailgun)

Synaptic Signals sends transactional email — currently just Form Designer's
"notify on new submission" — through Mailgun's HTTP API. There is no SMTP
server anywhere in this app: no socket to configure, no IP reputation to
manage, no bounce/complaint handling to build. Mailgun owns all of that.

Each site can use either the install-wide Mailgun account (set once in
`.env`) or its own Mailgun account (set per-site in the admin). Nothing is
sent anywhere until an account is configured — a site with neither just
skips sending and logs a warning.

---

## Install-wide setup

Set these in `.env` (or `synaptic.toml`) on the server:

```env
MAILGUN_API_KEY=key-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
MAILGUN_DOMAIN=mg.example.com
```

`MAILGUN_BASE_URL` defaults to `https://api.mailgun.net/v3` (US region) and
only needs to be set if the Mailgun account is EU-region
(`https://api.eu.mailgun.net/v3`).

The `From` address for install-wide sends uses `SMTP_FROM_NAME` /
`SMTP_FROM_EMAIL` if set (the same fields used for SMTP, reused here rather
than duplicated), falling back to `noreply@{domain}` if unset.

Every site on the install uses this account unless it has its own — see
below.

---

## Per-site override

A site can be given its own Mailgun account from its Settings page:

```
/admin/sites/{id}/settings → Email (Mailgun) card
```

Enter that site's Mailgun **domain** and **API key**, then save. This is
useful for an agency where a client has (or wants) their own Mailgun
account — their sending reputation, their bill, isolated from every other
site on the install.

Rules:

- Both fields must be filled together, or both left blank — a domain with
  no key can't send anything, and a key with no domain has nowhere to send
  to. The form rejects a half-filled submission both client-side and
  server-side.
- Leaving the API key field blank on an *edit* keeps the key already saved
  (it's never echoed back to the browser — see **Security** below) — only
  the domain gets updated. This is the only case where "blank" means "don't
  touch" rather than "clear."
- Clearing the domain removes both the domain and the saved key, reverting
  the site to the install-wide account.
- The `From` address for a site's own account is always `noreply@{that
  site's domain}` — not the install-wide `SMTP_FROM_EMAIL` — since some
  Mailgun domains (sandbox domains in particular) reject a `From` address
  outside the sending domain.

---

## Form Designer: notify on new submission

Each form built in the admin's Form Designer
(`/admin/form-designer/{id}` → **Email** card) can have a notification
email address. When set, every new submission to that form sends a plain-text
email to that address — one field per line, plus the form's name in the
subject — using whichever Mailgun account applies to that site (its own, or
the install-wide fallback).

The send happens in the background, after the submission is already stored
and the visitor is redirected — a slow or failed Mailgun call never delays
or blocks the actual form submission. A failure is logged server-side, not
shown to the visitor.

Leaving the field blank disables notifications for that form entirely —
submissions are still stored and viewable in the admin either way.

---

## Security: API keys are encrypted at rest

A site's own Mailgun API key is encrypted (AES-256-GCM) before being stored
in the database, keyed off `SECRET_KEY` — the same value already required in
production for session/cookie signing, so there's no separate secret to
provision or rotate. The install-wide key in `.env` is not encrypted; that's
unchanged from how `SMTP_PASSWORD` and every other `.env` secret already
work.

If `SECRET_KEY` changes, previously-saved per-site keys can no longer be
decrypted and will silently fall back to the install-wide account (or no
account, if none is configured) — re-enter them after a `SECRET_KEY`
rotation.

---

## Testing with a sandbox domain

Mailgun gives every account a free sandbox domain
(`sandboxXXXXXXXX.mailgun.org`) that works immediately with no DNS setup —
useful for confirming the whole pipeline works before verifying a real
domain. Two things to know:

- Sandbox domains can only send to email addresses added to that domain's
  **Authorized Recipients** list in the Mailgun dashboard.
- Mail from a sandbox domain has no sending reputation yet and will very
  likely land in spam — that's expected, not a bug. It goes away once
  you're sending from a verified domain with DNS history.

To go to production for a domain, verify it in Mailgun (adds SPF/DKIM TXT
records — no MX records needed unless you also want Mailgun to *receive*
mail for that domain).

---

## Checking which account was used

`mail.rs` logs which path a send took, at `info` level:

```
using site-specific mailgun account for site {id} (domain {domain})
using install-wide mailgun account for site {id} (no site-specific account set)
```

Useful when debugging whether a site's per-site override actually took
effect versus silently falling back.
