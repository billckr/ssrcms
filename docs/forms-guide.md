# Forms Guide

Synaptic Signals has a built-in form submission system that requires no plugins or configuration. Any HTML form in a theme template can POST to the public endpoint, and submissions are stored per-site, viewable and exportable from the admin panel.

---

## How It Works

1. A theme template renders an HTML `<form>` that POSTs to `/form/{name}`.
2. The server strips any field whose name starts with `_` (honeypot fields), then checks:
   - Is every remaining field blank? → discard silently, redirect back.
   - Is the form administratively blocked? → redirect back with `?blocked=1`.
3. Otherwise, all remaining fields are stored as a JSONB object in `form_submissions`, tagged with site, form name, submitter IP, and timestamp.
4. The server reads the `Referer` header and redirects back to the originating page with `?submitted=1` (Post/Redirect/Get pattern — prevents resubmission on browser refresh).
5. The template checks `request.query.submitted` and swaps the form for a success message.

---

## The Public Endpoint

```
POST /form/{name}
```

- `{name}` is a free-form identifier you choose (e.g. `contact`, `quote-request`, `newsletter`).
- The body must be `application/x-www-form-urlencoded` — standard HTML form encoding.
- All submitted fields are stored as-is **except** fields whose name begins with `_` (stripped before storage).
- Completely blank submissions (all values empty after stripping `_` fields) are silently discarded.
- If the form name is blocked in the admin, the submission is silently rejected and the browser is redirected to `?blocked=1`.

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

### Handling a blocked form

If an admin blocks a form, any new submission redirects back to the page with `?blocked=1`. You can catch this in the template to show a helpful message rather than leaving the form silently unresponsive:

```html
{% if request.query.submitted %}
  <p>Thanks! Your message has been sent.</p>
{% elif request.query.blocked %}
  <p>This form is temporarily unavailable. Please try again later.</p>
{% else %}
  <form method="POST" action="/form/contact">
    …
  </form>
{% endif %}
```

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

Any field whose `name` starts with an underscore (`_`) is stripped before storage. Use this for a hidden honeypot field that real users never fill in but automated bots commonly do:

```html
<!-- Visually hidden from real users; bots fill it in -->
<div style="position:absolute;width:1px;height:1px;overflow:hidden;opacity:0;"
     aria-hidden="true" tabindex="-1">
  <label for="_hp">Leave this blank</label>
  <input type="text" id="_hp" name="_honeypot" tabindex="-1" autocomplete="off">
</div>
```

**Important:** the honeypot field is stripped on the server, but the rest of the submission is still stored. This means bot submissions that fill every field will appear in the admin, just without the honeypot column. The honeypot is a passive signal — it does not reject the submission on its own.

To actively block a form from accepting new submissions (spam wave, decommissioned form, etc.) use the **Block** button in Admin → Forms. Blocked forms silently redirect submitters with `?blocked=1` and nothing is written to the database.

---

## Per-Page Template Override

Each page in Synaptic Signals can specify a custom Tera template instead of the default `page.html`. This is how the contact and newsletter pages work — a dedicated template is selected in the page editor.

### Setting a template

1. Admin → Pages → New (or edit an existing page)
2. In the right sidebar, the **Template** dropdown lists every `.html` file in the active theme's `templates/` directory (excluding `base.html`)
3. Select your form template and publish

The dropdown is only shown for **pages**, not posts.

### Template naming

The dropdown value is the filename relative to `templates/`, without the `.html` extension:

| File path | Dropdown value |
|-----------|----------------|
| `templates/contact-page.html` | `contact-page` |
| `templates/newsletter.html` | `newsletter` |
| `templates/forms/quote.html` | `forms/quote` |
| `templates/landing.html` | `landing` |

The template scanner walks the theme's `templates/` directory recursively, so subdirectories work naturally.

### Template resolution

When rendering a page, the server checks `posts.template`:

```
post.template = "contact-page"  →  render  templates/contact-page.html
post.template = "newsletter"    →  render  templates/newsletter.html
post.template = NULL / ""       →  render  templates/page.html  (default)
```

---

## Multiple Forms on a Site

Each form is identified by the `{name}` segment in the POST URL. You can have any number of forms on a site simply by using different names:

| Form | POST URL |
|------|----------|
| Contact | `/form/contact` |
| Newsletter signup | `/form/newsletter` |
| Quote request | `/form/quote-request` |
| Support ticket | `/form/support` |
| Event RSVP | `/form/rsvp-summer-event` |

Each name gets its own entry under Admin → Forms, with independent submission counts, CSV exports, block/unblock, and delete actions.

---

## Admin UI

Located at `/admin/forms` — visible only to users with the `can_manage_forms` capability (`site_admin` and `super_admin` roles).

### Forms list (`/admin/forms`)

Shows all form names that have received at least one submission, sorted by most recently submitted. Columns:

- **Form Name** — links to the submission detail view
- **Submissions** — total submission count
- **Last Submitted** — timestamp of the most recent submission
- **Actions** — View, CSV export, Block/Unblock

#### Filtering by form name

A **dropdown** next to the New Form button lets you filter the list to a single form (e.g. show only `newsletter` or only `contact`). Selecting a name reloads the page with `?filter=<name>`. Choose **(All forms)** to clear the filter. The dropdown is hidden when there are no submissions yet.

#### Blocking a form

The **Block** button adds the form name to the `form_blocks` table. New submissions to a blocked form are silently rejected (redirect with `?blocked=1`). Existing submissions are preserved. Click **Unblock** to resume accepting submissions.

A **Blocked** badge appears on the form row and the form name turns muted when blocked.

### Submission detail (`/admin/forms/{name}`)

Shows a table of up to **500 submissions**, newest first. Columns are derived dynamically from the JSONB keys present in the stored submissions — no schema definition required. New fields appear automatically if you add inputs to the form later.

- **Viewing** the detail page marks all submissions for that form as read (clears the unread count in the sidebar badge)
- **Delete** removes a single submission permanently
- **Delete All** purges the entire form's submission history for this site (confirmation required)

### CSV export (`/admin/forms/{name}/export`)

Downloads a `.csv` file (up to 10,000 rows) with one row per submission. Columns match the dynamic JSONB key set, plus `submitted_at` and `ip_address` appended at the right. Field values containing commas, quotes, or newlines are RFC 4180–escaped.

---

## Built-in Template Examples

Both bundled themes ship two ready-made form templates. Create a page, set the template in the sidebar, and publish — the form is immediately live.

### Contact form

**Files:**
- `themes/global/default/templates/contact-page.html`
- `themes/global/claude/templates/contact-page.html` (orange-accented, matches claude theme)

**Fields:** name (required), email (required), subject, message (required)

**POST URL:** `/form/contact`

**To use:**
1. Admin → Pages → New Page
2. Title: `Contact` (or anything), Slug: `contact`
3. Template: `contact-page`
4. Status: Published → Save
5. Visit `http://yoursite.com/contact` — form is live
6. Submissions appear at Admin → Forms → contact

### Newsletter signup

**Files:**
- `themes/global/default/templates/newsletter.html`
- `themes/global/claude/templates/newsletter.html`

**Fields:** email (required), terms_accepted checkbox (required)

**POST URL:** `/form/newsletter`

**Captured automatically:** submission date (`submitted_at` column) and IP address (`ip_address` column) — no hidden fields needed.

**To use:**
1. Admin → Pages → New Page
2. Title: `Newsletter`, Slug: `newsletter`
3. Template: `newsletter`
4. Status: Published → Save
5. Visit `http://yoursite.com/newsletter`
6. Submissions appear at Admin → Forms → newsletter

---

## Creating a Custom Form Template

Any `.html` file placed in `themes/<theme>/templates/` is automatically listed in the Template dropdown. Here is a complete starter template you can copy and adapt:

```html
{% extends "base.html" %}

{% block title %}{{ page.title }} — {{ site.name }}{% endblock title %}

{% block content %}
<article class="single-page">
  <h1>{{ page.title }}</h1>

  {% if page.content %}
  <div class="page-content">{{ page.content | safe }}</div>
  {% endif %}

  {% if request.query.submitted %}
  <p class="form-success">Thanks! Your submission has been received.</p>

  {% elif request.query.blocked %}
  <p class="form-error">This form is temporarily unavailable.</p>

  {% else %}
  <form method="POST" action="/form/my-form-name">

    <!-- Honeypot (hidden from real users) -->
    <div style="position:absolute;width:1px;height:1px;overflow:hidden;opacity:0;"
         aria-hidden="true" tabindex="-1">
      <label for="_hp">Leave this blank</label>
      <input type="text" id="_hp" name="_honeypot" tabindex="-1" autocomplete="off">
    </div>

    <label for="f-name">Name <span aria-hidden="true">*</span></label>
    <input type="text" id="f-name" name="name" required autocomplete="name">

    <label for="f-email">Email <span aria-hidden="true">*</span></label>
    <input type="email" id="f-email" name="email" required autocomplete="email">

    <label for="f-message">Message <span aria-hidden="true">*</span></label>
    <textarea id="f-message" name="message" rows="5" required></textarea>

    <button type="submit" id="form-submit">Send</button>
  </form>

  <script>
    document.querySelector('form').addEventListener('submit', function () {
      var btn = document.getElementById('form-submit');
      btn.disabled = true;
      btn.textContent = 'Sending…';
    });
  </script>
  {% endif %}
</article>
{% endblock content %}
```

Replace `my-form-name` with any slug-safe identifier. The corresponding admin view will appear at `/admin/forms/my-form-name` automatically after the first submission.

---

## Data Storage

### `form_submissions` table

| Column | Type | Notes |
|--------|------|-------|
| `id` | `UUID` | Primary key, auto-generated |
| `site_id` | `UUID` | FK → `sites.id`; cascades on site delete |
| `form_name` | `TEXT` | The `{name}` from the POST URL |
| `data` | `JSONB` | All submitted fields (after `_` prefix stripping) |
| `ip_address` | `TEXT` | `X-Real-IP` → `X-Forwarded-For` (first value) → NULL |
| `read_at` | `TIMESTAMPTZ` | NULL until the admin detail page is viewed |
| `submitted_at` | `TIMESTAMPTZ` | Set by DB default (`NOW()`) |

An index on `(site_id, form_name, submitted_at DESC)` keeps list and export queries fast even at high submission volume.

### `form_blocks` table

| Column | Type | Notes |
|--------|------|-------|
| `site_id` | `UUID` | FK → `sites.id` |
| `form_name` | `TEXT` | Matches `form_submissions.form_name` |

A row here means that form is blocked. The check happens on every `POST /form/{name}` before any write. Unblocking deletes the row.

---

## Known Limitations

- **No server-side CAPTCHA.** The honeypot is the only built-in bot mitigation. For high-spam scenarios, a JavaScript CAPTCHA (e.g. Cloudflare Turnstile, hCaptcha) can be added to a template, but verification must be handled by a plugin or external service — there is no built-in hook for it yet.
- **No email notifications.** Submissions are stored in the database only. There is currently no mechanism to send an email alert to the site owner when a new submission arrives. An SMTP/notification plugin is planned.
- **No file uploads.** The form handler only accepts `application/x-www-form-urlencoded`. `multipart/form-data` (file input fields) is not supported.
- **Detail page shows 500 submissions maximum.** The admin submission detail view loads at most 500 rows. The CSV export raises this to 10,000 rows. Pagination for the detail view is not yet implemented.
- **Honeypot does not reject submissions.** Filled honeypot fields are stripped, but the rest of the submission is still stored. It is a detection aid, not a filter. Use the **Block** button to stop accepting submissions from a specific form entirely.
- **No field validation on the server.** Required field enforcement is HTML5 client-side only (`required` attribute). A determined submitter can bypass it with a direct POST. Add your own validation logic in a plugin or accept that the data may occasionally be incomplete.
- **Form name is permanent.** Once a form has received submissions, renaming it in the template (changing the `action` URL) creates a new form entry in the admin — the old submissions remain under the old name. There is no rename operation.
- **No spam filtering on blocked status change.** Blocking a form does not retroactively delete existing submissions; it only stops new ones.

---

## Security Notes

- **User content is never rendered as a template string.** All submitted values are stored as JSONB and read back in Rust via `s.data.get(col)`. They are HTML-escaped before insertion into any admin HTML. This prevents template injection and XSS.
- **Site isolation is enforced.** Every query includes `site_id = $1` — one site can never read, export, block, or delete another site's submissions.
- **Admin access is capability-gated.** All `/admin/forms/*` routes check `can_manage_forms` before responding; editor and author roles receive a 403.
- **IP logging is best-effort.** The `X-Real-IP` header (set by Caddy) is trusted. When running without a reverse proxy, the field will be NULL.
- **The honeypot is passive.** Bot submissions that fill all fields (including the honeypot) are still stored — just without the `_honeypot` column. The honeypot makes detection easy; the **Block** button is the actual rejection mechanism.
