# Forms Guide

Synaptic Signals has a built-in form submission system that requires no plugins or configuration. Any HTML form in a theme template can POST to the public endpoint, and submissions are stored per-site, viewable and exportable from the admin panel.

---

## How It Works

1. A theme template renders an HTML `<form>` that POSTs to `/form/{name}`.
2. The server stores every field as a JSONB object in the `form_submissions` table, tagged with the site, form name, submitter IP, and timestamp.
3. The server reads the `Referer` header and redirects back to the originating page with `?submitted=1` appended (Post/Redirect/Get pattern — prevents resubmission on browser refresh).
4. The template checks `request.query.submitted` and swaps the form for a success message.

---

## The Public Endpoint

```
POST /form/{name}
```

- `{name}` is a free-form identifier you choose (e.g. `contact`, `quote-request`, `newsletter`).
- The body must be `application/x-www-form-urlencoded` — standard HTML form encoding.
- All submitted fields are stored as-is except fields whose name begins with `_` (see Honeypot below).
- Completely blank submissions (all values empty after stripping) are silently discarded.

---

## Writing a Form Template

Forms work in any Tera template — a dedicated page template, a partial included in another template, or injected via a hook.

### Minimal example

```html
<form method="POST" action="/form/contact">
  <input type="text"  name="name"    required placeholder="Your name">
  <input type="email" name="email"   required placeholder="Email">
  <textarea           name="message" required></textarea>
  <button type="submit">Send</button>
</form>
```

### Showing a success message

After submission, the browser is redirected back to the page with `?submitted=1` in the URL. Use `request.query.submitted` in Tera to conditionally show the form or a confirmation:

```html
{% if request.query.submitted %}
  <p>Thanks! We'll be in touch.</p>
{% else %}
  <form method="POST" action="/form/contact">
    …
  </form>
{% endif %}
```

`request.query` is a `HashMap<String, String>` containing every key from the current URL's query string. It is available on every public page template.

### Double-submit prevention

Disable the submit button on first click so rapid double-clicks don't fire two requests:

```html
<button type="submit" id="submit-btn">Send</button>

<script>
  document.querySelector('form').addEventListener('submit', function () {
    var btn = document.getElementById('submit-btn');
    btn.disabled = true;
    btn.textContent = 'Sending…';
  });
</script>
```

Combined with the server-side PRG redirect, this covers all normal resubmission scenarios.

---

## Honeypot Spam Protection

Any field whose `name` starts with an underscore (`_`) is stripped before storage. Use this for a hidden honeypot field that real users never fill in, but bots usually do:

```html
<!-- Visually hidden from real users -->
<div style="position:absolute;width:1px;height:1px;overflow:hidden;opacity:0;" aria-hidden="true">
  <input type="text" name="_honeypot" tabindex="-1" autocomplete="off">
</div>
```

If a bot fills it in, the value is still stripped and the submission is stored. You can optionally check for it server-side in a future version, or simply ignore filled honeypot entries during review.

---

## Per-Page Template Override

Each page in Synaptic Signals can specify a custom Tera template instead of the default `page.html`. This is how the contact form demo works — a dedicated `contact-page.html` template is selected in the page editor.

### Setting a template

1. Admin → Pages → New (or edit an existing page)
2. In the right sidebar, the **Template** dropdown lists every `.html` file in the active theme's `templates/` directory (excluding `base.html`)
3. Select your form template and publish

The dropdown is only shown for **pages**, not posts.

### Template naming

The dropdown value is the filename relative to `templates/`, without the `.html` extension. Examples:

| File path | Dropdown value |
|-----------|----------------|
| `templates/contact-page.html` | `contact-page` |
| `templates/forms/quote.html` | `forms/quote` |
| `templates/landing.html` | `landing` |

The `scan_templates` function walks the theme's `templates/` directory recursively, so subdirectories work naturally.

### Template resolution

When rendering a page, the server checks `posts.template`:

```
post.template = "contact-page"  →  render  templates/contact-page.html
post.template = NULL / ""       →  render  templates/page.html  (default)
```

---

## Multiple Forms on a Site

Each form is identified by the `{name}` segment in the POST URL. You can have any number of forms on a site simply by using different names:

| Form | POST URL |
|------|----------|
| Contact | `/form/contact` |
| Quote request | `/form/quote-request` |
| Newsletter signup | `/form/newsletter` |
| Support ticket | `/form/support` |

Each name gets its own entry under Admin → Forms, with independent submission counts, CSV exports, and delete actions.

---

## Admin UI

Located at `/admin/forms` — visible only to **site_admin** and **super_admin** roles (`can_manage_forms` capability).

### Forms list (`/admin/forms`)

Shows all form names that have received at least one submission, with:
- Total submission count
- Unread count (submissions not yet viewed)
- Last submitted timestamp
- Links to view submissions and export CSV

### Submission detail (`/admin/forms/{name}`)

Shows a table where each row is one submission. Columns are derived dynamically from the JSONB keys present in the submissions — no schema definition required. New fields appear automatically if you add inputs to the form later.

- **Viewing** the detail page marks all submissions as read (clears the "new" badge)
- **Delete** removes a single submission
- **Delete All** purges the entire form's history

### CSV export (`/admin/forms/{name}/export`)

Downloads a `.csv` file with one row per submission. Columns match the dynamic JSONB key set plus `submitted_at` and `ip_address`. Field values containing commas, quotes, or newlines are properly RFC 4180–escaped.

---

## Data Storage

Table: `form_submissions`

| Column | Type | Notes |
|--------|------|-------|
| `id` | `UUID` | Primary key |
| `site_id` | `UUID` | FK → `sites.id` (cascades on delete) |
| `form_name` | `TEXT` | The `{name}` from the POST URL |
| `data` | `JSONB` | All submitted fields (after `_` stripping) |
| `ip_address` | `TEXT` | Best-effort: `X-Real-IP` → `X-Forwarded-For` → NULL |
| `read_at` | `TIMESTAMPTZ` | NULL until admin views the detail page |
| `submitted_at` | `TIMESTAMPTZ` | Set by the DB default (`NOW()`) |

An index on `(site_id, form_name, submitted_at DESC)` keeps list and export queries fast even with large volumes.

---

## The Claude Theme Example

[`themes/global/claude/templates/contact-page.html`](../themes/global/claude/templates/contact-page.html) is a complete reference implementation. It includes:

- `{% extends "base.html" %}` — inherits the theme's layout
- PRG success state via `{% if request.query.submitted %}`
- Four fields: name (required), email (required), subject, message (required)
- Honeypot field `_honeypot`
- Double-submit JS guard
- Self-contained CSS using theme CSS custom properties (`--color-primary`, `--color-border`, etc.) with sensible fallbacks

To use it on a new site:
1. Ensure the claude theme (or a theme containing `contact-page.html`) is active
2. Create a page with any slug (e.g. `contact`)
3. Set the template to `contact-page` in the page editor sidebar
4. Publish — form is live, submissions appear under Admin → Forms → contact

---

## Security Notes

- **User content is never rendered as a template string.** All submitted values are stored as JSONB and read back via `s.data.get(col)` in Rust. They are HTML-escaped before insertion into any admin HTML. This prevents template injection and XSS.
- **Site isolation is enforced.** Every query includes `site_id = $1` — one site can never read or delete another site's submissions.
- **Admin access is capability-gated.** All `/admin/forms/*` routes check `can_manage_forms` and return 403 for editor and author roles.
- **IP logging is best-effort.** The `X-Real-IP` header set by Caddy is trusted. If running without a reverse proxy, the field will be NULL.
