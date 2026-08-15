# Transactional Email (Multi-Provider)

Synaptic Signals sends transactional email — currently Form Designer's
"notify on new submission" and "email the submitter a confirmation" — through
whichever provider a form is pointed at. There is no SMTP server this app
runs itself: every provider is either a plain HTTP API call (Mailgun,
SendGrid, Postmark) or a relay this app connects out to as a client (generic
SMTP via `lettre`). No socket to listen on, no IP reputation to manage, no
bounce/complaint handling to build — the provider owns all of that.

A site can configure **any number** of named provider accounts — Mailgun,
SMTP, SendGrid, Postmark — and each **form** independently picks which one
(if any) to send through. There's no single "default" provider per site,
since different forms may legitimately want different accounts (e.g. sales
vs. support, or a client's own account vs. the agency's). A form with none
selected falls back to the install-wide Mailgun account set once in `.env`.
Nothing is sent anywhere until some account is configured — a form with no
provider selected and no install-wide account configured just skips sending
and logs a warning.

---

## Install-wide fallback setup

Set these in `.env` (or `synaptic.toml`) on the server:

```env
MAILGUN_API_KEY=key-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
MAILGUN_DOMAIN=mg.example.com
```

This is the only provider type available install-wide (there's no
install-wide SMTP/SendGrid/Postmark config) — it's what every form on every
site uses by default until that form is given its own provider.

---

## Configuring a provider (Site Settings → Email Settings)

```
/admin/sites/{id}/settings → Email Settings tab
```

Two panels, side by side (the same list-on-the-left / add-form-on-the-right
layout used by Tags/Categories):

- **Email Providers** (left) — every provider configured for this site, each
  showing its label, type, and a Verified/Unverified badge, with Test / Edit
  / Delete actions.
- **Add Provider** (right) — pick a type, enter its credentials, save. The
  fields shown change based on the selected type:

| Provider | Fields |
|----------|--------|
| Mailgun | domain, sending key |
| SMTP | host, port, username, password, TLS mode (STARTTLS / Implicit TLS / None) |
| SendGrid | API key, from address (must be a verified sender/domain in SendGrid) |
| Postmark | server API token, message stream (defaults to `outbound`), from address (must be a verified sender signature) |

**Verifying a provider:** click the mail-icon "Test" button to send a real
test email (to the admin's own account email) through that provider. Success
flips the provider to **Verified** — only verified providers appear in a
form's "Send via" dropdown, so a form can't accidentally point at
credentials that have never actually been proven to work.

**Editing a provider:** the pencil-icon "Edit" button reveals an inline form
to update that provider's label/credentials. Credentials are never sent back
to the browser once saved, so editing is a full overwrite — re-enter every
field, not just the one you're changing. Saving an edit resets the provider
to **Unverified**, since the new credentials haven't been proven to work yet
— test again after editing.

**Deleting a provider** that a form is currently using doesn't error or
orphan anything: the form's `email_provider_id` foreign key is
`ON DELETE SET NULL`, so it silently reverts that form to the install-wide
fallback.

---

## Picking a provider per form (Form Designer → Mail Settings)

```
/admin/form-designer/{id} → Mail Settings tab → Send via
```

A dropdown lists **"Install-wide default account"** plus every *verified*
provider configured on the form's site. This is independent of the form's
"Notify on new submission" and "Email the submitter a confirmation" settings
on the same tab — the provider choice is just which account those two
features send through.

---

## Security: credentials are encrypted at rest

A provider's credentials — API key, SMTP password, whatever the type needs
— are serialized to JSON and encrypted as one blob (AES-256-GCM) before
being stored, keyed off `SECRET_KEY` — the same value already required in
production for session/cookie signing, so there's no separate secret to
provision or rotate. The install-wide Mailgun key in `.env` is not
encrypted; that's unchanged from how every other `.env` secret already
works.

If `SECRET_KEY` changes, previously-saved provider credentials can no longer
be decrypted — sends through that provider fail and are logged, and forms
using it don't automatically fall back to anything. Re-enter the provider's
credentials (via Edit) after a `SECRET_KEY` rotation, then re-test.

---

## Testing with a Mailgun sandbox domain

Mailgun gives every account a free sandbox domain
(`sandboxXXXXXXXX.mailgun.org`) that works immediately with no DNS setup —
useful for confirming the pipeline works before verifying a real domain. Two
things to know:

- Sandbox domains can only send to email addresses added to that domain's
  **Authorized Recipients** list in the Mailgun dashboard.
- Mail from a sandbox domain has no sending reputation yet and will very
  likely land in spam — that's expected, not a bug. It goes away once
  you're sending from a verified domain with DNS history.

To go to production for a domain, verify it in Mailgun (adds SPF/DKIM TXT
records — no MX records needed unless you also want Mailgun to *receive*
mail for that domain). SendGrid and Postmark have their own equivalent
sender/domain verification steps in their own dashboards.

---

## Checking which provider was used

`core/src/mail.rs` logs which path a send took, at `info` level:

```
using provider '{label}' ({provider_type}) for site {id}
using install-wide mailgun account for site {id} (no provider selected)
```

Useful when debugging whether a form's chosen provider actually took effect
versus silently falling back to the install-wide account.
