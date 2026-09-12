---
title: Site Settings
group: feature
---
# Site Settings

Site Settings controls how one site is presented, when visitors can reach it, and which outside
services it can use. Open **Sites**, find the site you want to manage, and select its **Site
Settings** gear. Changes apply only to that site.

The page is divided into five tabs. **AI Translation** is shown only when that feature is enabled
for the installation.

- [General](#general)
- [Maintenance](#maintenance)
- [Email Settings](#email-settings)
- [AI Translation](#ai-translation)
- [Import Content](#import-content)

Most Save buttons stay disabled until you change something. Settings are saved separately in each
tab, so save your work before moving on.

<a id="general"></a>
## General

Use General for the site's identity, publishing defaults, public registration, and post URLs.

- **Site Name** — the public name used by the theme, including places such as the browser tab,
  header, and footer.
- **Site Description** — a short description or tagline. Where it appears depends on the active
  theme.
- **Language** — the base language code for the site, such as `en`, `en-US`, or `fr`. Themes use it
  to identify the language of normal, untranslated pages and feeds. This does not translate
  existing content; translated languages are managed in the AI Translation tab.
- **Posts Per Page** — how many posts are shown on each page of paginated public listings. Enter a
  number from 1 to 100.
- **Date Format** — controls how dates are printed by themes. It uses chrono-style tokens; for
  example, `%B %-d, %Y` produces `January 1, 2026`. Take care when editing the `%` tokens because
  unsupported or misplaced tokens can produce an unexpected date.
- **Administration Email** — receives notices for this site. Leave it blank to use the site
  owner's account email shown as the placeholder.
- **Anyone can register** — turns the public `/subscribe` form on or off. Every account created
  there is a Subscriber; enabling it does not grant access to the staff admin area.

### Permalinks

Permalinks control the public URL pattern for posts. Pages keep their own slug or hierarchical
path and are not affected. If you are moving an existing site, matching its old URL structure
helps previously shared and indexed links continue to work.

Choose **Post name**, **Month and name**, **Day and name**, or **Category** to fill the structure
automatically. Choose **Custom Structure** to write your own pattern. Available tokens are:

- `%postname%` — the post slug. This is required and must be the final token.
- `%year%`, `%monthnum%`, and `%day%` — parts of the publication date.
- `%category%` — the first assigned category slug. Posts without a category use `uncategorized`.
- `%post_id%` — the post's ID.

Changing a permalink structure changes post URLs, so existing external links may stop working.
Decide on the structure before launch when possible, and use the same structure as the source site
before a WordPress import.

### Site ID

The Support card contains the site's internal ID. Select the eye button to reveal it, then select
the ID to copy it. It is useful when contacting support or distinguishing sites in logs; it does
not change any setting.

<a id="maintenance"></a>
## Maintenance

Maintenance mode replaces the public site with a maintenance page while you work. It affects only
this site, takes effect immediately, and does not require a restart. The `/admin` area remains
available so staff can sign in and turn it off.

1. Write the visitor-facing **Message**. It can be up to 250 characters. A blank message is
   replaced with the standard maintenance message when saved.
2. Select **Enable maintenance mode**.
3. Select Save and confirm the prompt.

Clear the checkbox and save to reopen the site. Saving a new message while the checkbox remains
unchanged updates the stored message without changing whether maintenance mode is active.

<a id="email-settings"></a>
## Email Settings

Email providers deliver messages created by forms, such as staff notifications and confirmations
to visitors. You can add several providers to one site and choose a different one for each form in
**Designer → Form → Mail Settings**. A form with no provider selected uses the installation-wide
fallback account, if one has been configured.

### Add a provider

Give the provider a unique, descriptive **Label**, such as `Support SMTP`, then select its type and
enter the connection details:

- **Mailgun** — enter the Mailgun domain and its domain-specific Sending key. Do not use the
  account-wide Private API key.
- **SMTP** — enter the server host and port, optional username and password, and the TLS mode.
  `STARTTLS` is commonly used on port 587, while `Implicit TLS` is commonly used on port 465; use
  the values supplied by your email service. Choose `None` only for a trusted server that
  explicitly requires an unencrypted connection.
- **SendGrid** — enter an API key and a From address that SendGrid has verified as a sender or as
  part of a verified domain.
- **Postmark** — enter the Server API token, message stream (normally `outbound`), and a From
  address with a verified Postmark sender signature.

Saving adds the provider as **Unverified**. Select its mail icon to send a real test message to
your own account email. A successful test marks it **Verified**; only verified providers can be
selected by forms.

### Manage providers

- **Edit** — changes the label or connection details. Email credentials are never filled back into
  the page after saving, so re-enter every provider field when editing. Saving makes the provider
  Unverified until it passes another test.
- **Send Test Email** — checks the saved connection and sends to the signed-in administrator's
  account email. If it fails, confirm the credentials, sender verification, host, port, and TLS
  mode with the provider.
- **Delete** — permanently removes the provider. Forms that used it revert to the installation-wide
  fallback account rather than keeping a broken reference.

Provider secrets are stored encrypted and are not displayed again. Keep a separate secure copy of
API keys, tokens, and passwords.

<a id="ai-translation"></a>
## AI Translation

AI Translation adds provider accounts and languages that staff can use to create localized copies
of posts, pages, forms, and polls. It does not translate the whole site automatically, and it does
not replace the original content. Always have generated translations reviewed by someone who
understands the target language.

If this tab is missing, AI Translation has been disabled in installation-wide System Settings.

### Enable languages

Choose a language from **Add a language**, select the plus button, and repeat for every language
you need. Remove a language with the `x` on its chip, then Save the list.

Each enabled language reserves its code as the first part of public translated URLs. For example,
Spanish uses paths beginning `/es/`. Before enabling a language, check that the site does not
already have a top-level post or page whose slug is exactly that code, or that existing URL will
be shadowed. Enabling a language makes it available to editors; it does not generate translations.

### Add a provider

Enter a unique **Label**, select the provider type, and supply its connection details:

- **Anthropic** — enter an Anthropic API key.
- **DeepSeek** — enter a DeepSeek API key. Its service URL is already configured by SynapCMS.
- **OpenAI-Compatible** — enter a Base URL including its version path, such as
  `https://api.openai.com/v1`, and an API key if the service requires one. This choice also supports
  local services such as Ollama and LM Studio, but is available only to a Super Admin. See
  [Local AI Models](/help#doc-local-ai-models) for local setup.

Select **Connect and load models**, then choose a model. If the service cannot list models, choose
**Enter a custom model ID** and copy the exact model ID from the provider. Economy, Balanced, and
Premium labels are general guidance based on model families, not current price quotes.

Saving performs a small test request. A successful provider is marked Verified and becomes
available in translation controls. A failed provider is still saved so you can correct it, then
use **Test Provider** to try again. Verification confirms the connection, credentials, model ID,
and required response format; it cannot guarantee the quality or capacity of a full translation.

### Manage AI providers

- **Edit** — changes the label, model, or connection. Blank credential fields keep their saved
  encrypted values. Saving automatically tests and re-verifies the updated provider.
- **Test Provider** — repeats the small verification request without changing the configuration.
- **Delete** — removes the provider but keeps translations already generated with it. You will
  need another verified provider to create or refresh translations.

API requests may cost money and can send the content being translated to the selected service.
Review the provider's pricing and data-handling terms. A local provider keeps content on your own
infrastructure but may be slower and requires server access to set up.

<a id="import-content"></a>
## Import Content

Import Content moves a WordPress site's standard content into SynapCMS. In WordPress, go to
**Tools → Export → All content** and download the `.xml` WXR export. Then:

1. Select that file under **WordPress export file (.xml)**.
2. If the old site is offline or its media cannot be downloaded, zip its
   `wp-content/uploads/` directory and select it under **Media files zip**. This file is optional.
3. Select Import and leave the progress window open while files and content are processed.
4. Review the completion summary. If new author accounts were created, download the credentials
   CSV and store it safely before closing the window.

The import includes standard posts and pages, authors, categories, tags, featured and inline
images, publication status and dates, page hierarchy, and most custom fields. Media is placed in
the Media Library. It does not import WordPress passwords, comments, custom post types, widgets,
plugin forms, or theme layout. Shortcodes and WordPress block markers may remain as plain content
and should be reviewed after import.

New authors receive Author access with a generated password and cannot publish directly until an
administrator grants that permission. Authors without an email address in the export cannot be
matched or created; their content is assigned to the administrator running the import.

### Re-importing the same export

Re-importing the same WordPress export updates content that was imported previously instead of
creating duplicates. This is useful when adding a media zip after an earlier run. The imported
post's existing slug is preserved and an already-resolved featured image is not cleared, but the
export overwrites its title, body, excerpt, status, and publication date. Back up or copy any
changes made in SynapCMS before re-importing.

After an import, review a sample of published and draft content, image links, categories and tags,
page hierarchy, author access, and public post URLs. Import each WordPress multisite subsite into
its corresponding SynapCMS site separately.
