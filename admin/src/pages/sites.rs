//! Admin sites management page.

pub struct SiteRow {
    pub id: String,
    pub hostname: String,
    /// Email of the site_admin who owns this site, if one is assigned.
    pub admin_email: Option<String>,
    /// Count of non-subscriber users (site_admin, editor, author).
    pub user_count: i64,
    /// Count of subscribers only.
    pub subscriber_count: i64,
    pub post_count: i64,
    pub page_count: i64,
    /// True for the first site created during CLI install — cannot be deleted.
    pub is_default: bool,
    /// True when the current user may edit settings / delete this site.
    pub can_manage: bool,
    /// True when a Caddy block exists for this hostname (SSL provisioned).
    pub ssl_active: bool,
    /// True when this site is the default_site_id of its non-super_admin owner.
    /// Shown as a blue "primary domain" badge in the super-admin system view only.
    pub is_primary_domain: bool,
    /// True when maintenance mode is currently on for this site.
    pub maintenance_mode: bool,
}

fn sites_pagination(page: i64, total_pages: i64, search_qs: &str, sort_qs: &str) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let qs = format!("{search_qs}{sort_qs}");
    let prev = if page > 1 {
        format!(r#"<a href="/admin/sites?page={}{qs}" class="page-btn">&laquo; Prev</a>"#, page - 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">&laquo; Prev</span>"#.to_string()
    };
    let next = if page < total_pages {
        format!(r#"<a href="/admin/sites?page={}{qs}" class="page-btn">Next &raquo;</a>"#, page + 1)
    } else {
        r#"<span class="page-btn page-btn-disabled">Next &raquo;</span>"#.to_string()
    };
    let start = (page - 3).max(1);
    let end = (page + 3).min(total_pages);
    let mut nums = String::new();
    for p in start..=end {
        if p == page {
            nums.push_str(&format!(r#"<span class="page-btn page-btn-active">{p}</span>"#));
        } else {
            nums.push_str(&format!(r#"<a href="/admin/sites?page={p}{qs}" class="page-btn">{p}</a>"#));
        }
    }
    format!(r#"<div class="pagination">{prev}{nums}{next}</div>"#)
}

/// Table + pagination only — swapped by the live-search JS, and reused for
/// the initial full-page render so both paths render identically.
pub fn sites_list_fragment(sites: &[SiteRow], page: i64, total_pages: i64, search: &str, sort: &str, dir: &str, ctx: &crate::PageContext) -> String {
    let search_qs = if search.is_empty() {
        String::new()
    } else {
        format!("&search={}", crate::html_escape(search))
    };
    let sort_qs = if sort.is_empty() { String::new() } else { format!("&sort={}&dir={}", sort, if dir == "desc" { "desc" } else { "asc" }) };
    let asc = dir != "desc";

    // Sortable column header: link toggles asc/desc for that column, preserving
    // the current search filter and resetting to page 1 (a new sort is a new view).
    let sort_th = |label: &str, key: &str| -> String {
        let is_active = sort == key;
        let next_dir = if is_active && asc { "desc" } else { "asc" };
        let arrow = if is_active { if asc { " \u{25B2}" } else { " \u{25BC}" } } else { "" };
        format!(
            r#"<th><a href="/admin/sites?sort={key}&dir={next_dir}{search_qs}" style="color:inherit;text-decoration:none;white-space:nowrap">{label}{arrow}</a></th>"#
        )
    };

    let rows = sites.iter().map(|s| {
        let manage_html = if s.can_manage {
            let delete_html = if s.is_default {
                String::new()
            } else {
                let confirm_msg = format!(
                    "Delete site '{}'? This permanently deletes the site, its content, and its settings. \
                     Any user account that exists only for this site is deleted too — users with roles on \
                     other sites keep their accounts and just lose their role here. This cannot be undone.",
                    s.hostname.replace('\'', "\\'")
                );
                format!(
                    r#"<form method="post" action="/admin/sites/{id}/delete" style="display:inline"
                          data-confirm="{confirm_msg}" onsubmit="return confirm(this.dataset.confirm)">
                      <button type="submit" class="icon-btn icon-danger" title="Delete site">
                        <img src="/admin/static/icons/trash.svg" alt="Delete">
                      </button>
                    </form>"#,
                    id = crate::html_escape(&s.id),
                    confirm_msg = crate::html_escape(&confirm_msg),
                )
            };
            format!(
                r#"<a href="/admin/sites/{id}/settings" class="icon-btn" title="Site Settings">
                  <img src="/admin/static/icons/edit.svg" alt="Site Settings">
                </a>
                {delete}"#,
                id = crate::html_escape(&s.id),
                delete = delete_html,
            )
        } else {
            String::new()
        };

        // SSL status/provisioning is only shown to roles that can manage sites
        // (super_admin, site_admin) — editors and authors have no use for it and
        // the provision-ssl route itself is already restricted to those roles.
        let ssl_badge = if !ctx.can_manage_sites {
            String::new()
        } else if s.ssl_active {
            r#"<span class="ssl-badge ssl-active" title="SSL is active for this site">
                 <img src="/admin/static/icons/lock.svg" alt="SSL active" style="width:18px;height:18px;vertical-align:middle;filter:invert(35%) sepia(80%) saturate(500%) hue-rotate(95deg)">
               </span>"#.to_string()
        } else {
            format!(
                r#"<form method="post" action="/admin/sites/{id}/provision-ssl" style="display:inline"
                        onsubmit="return confirm('Enable SSL for {hostname_js}?\n\nDNS for this domain must already point to this server — we\'ll check and let you know if it isn\'t ready yet.\nA certificate will be issued automatically.')">
                     <button type="submit" class="ssl-badge ssl-inactive" title="SSL not provisioned — Click to secure">
                       <img src="/admin/static/icons/lock.svg" alt="Provision SSL" style="width:18px;height:18px;vertical-align:middle;opacity:0.4">
                     </button>
                   </form>"#,
                id          = crate::html_escape(&s.id),
                hostname_js = crate::html_escape(&s.hostname),
            )
        };

        let maintenance_badge = if s.maintenance_mode {
            r#" <span class="ssl-badge" title="Maintenance mode is ON — visitors see a maintenance page">
                 <img src="/admin/static/icons/tool.svg" alt="Maintenance mode active" style="width:14px;height:14px;vertical-align:middle;filter:invert(60%) sepia(90%) saturate(600%) hue-rotate(2deg)">
               </span>"#
        } else {
            ""
        };

        let site_url = format!(
            "{scheme}://{hostname}",
            scheme = if s.ssl_active { "https" } else { "http" },
            hostname = s.hostname,
        );

        format!(
            r#"<tr>
              <td><a href="{site_url}" target="_blank" rel="noopener noreferrer">{hostname}</a>{default_badge} {maintenance_badge}</td>
              <td style="color:var(--muted);font-size:0.875rem">{admin_email}</td>
              <td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{user_count}</span></td>
              <td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{subscriber_count}</span></td>
              <td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{post_count}</span></td>
              <td><span style="display:inline-block;background:var(--tint);color:var(--text);border-radius:4px;padding:.15rem .5rem;font-size:.78rem;font-weight:500">{page_count}</span></td>
              <td class="actions">
                <div class="icon-pill-actionbuttons">
                  {switch_btn}
                  {ssl_badge}
                  {users_link}
                  {manage}
                </div>
              </td>
            </tr>"#,
            hostname         = crate::html_escape(&s.hostname),
            site_url         = crate::html_escape(&site_url),
            switch_btn       = if s.hostname == ctx.current_site {
                String::new()
            } else {
                format!(
                    r#"<form method="post" action="/admin/sites/switch" style="display:inline">
                  <input type="hidden" name="site_id" value="{id}">
                  <button type="submit" class="icon-btn" title="Switch to this site">
                    <img src="/admin/static/icons/log-in.svg" alt="Switch">
                  </button>
                </form>"#,
                    id = crate::html_escape(&s.id),
                )
            },
            default_badge    = if s.is_default {
                r#" <span class="badge-site-default" title="Default site — cannot be deleted">default</span>"#
            } else if s.is_primary_domain {
                r#" <span class="badge-primary-domain" title="Primary domain for this account">primary</span>"#
            } else {
                ""
            },
            ssl_badge        = ssl_badge,
            maintenance_badge = maintenance_badge,
            users_link       = if ctx.can_manage_users {
                format!(
                    r#"<a href="/admin/users?site={id}" class="icon-btn" title="View users for this site">
                  <img src="/admin/static/icons/users.svg" alt="Users">
                </a>"#,
                    id = crate::html_escape(&s.id),
                )
            } else {
                String::new()
            },
            admin_email      = s.admin_email.as_deref().map(|e| crate::html_escape(e)).unwrap_or_else(|| "<em>none</em>".to_string()),
            user_count       = s.user_count,
            subscriber_count = s.subscriber_count,
            post_count       = s.post_count,
            page_count       = s.page_count,
            manage           = manage_html,
        )
    }).collect::<Vec<_>>().join("\n");

    let rows = if rows.is_empty() {
        r#"<tr><td colspan="7" class="empty-state">No sites found.</td></tr>"#.to_string()
    } else {
        rows
    };

    format!(
        r#"<table class="data-table">
  <thead><tr>{site_th}{admin_th}{users_th}{subs_th}{posts_th}{pages_th}<th>Actions</th></tr></thead>
  <tbody>{rows}</tbody>
</table>
{pagination}"#,
        rows = rows,
        pagination = sites_pagination(page, total_pages, &search_qs, &sort_qs),
        site_th  = sort_th("Site", "hostname"),
        admin_th = sort_th("Admin", "admin"),
        users_th = sort_th("Users", "users"),
        subs_th  = sort_th("Subs", "subs"),
        posts_th = sort_th("Posts", "posts"),
        pages_th = sort_th("Pages", "pages"),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_list(
    sites: &[SiteRow],
    flash: Option<&str>,
    can_create: bool,
    page: i64,
    total_pages: i64,
    search: &str,
    sort: &str,
    dir: &str,
    ctx: &crate::PageContext,
) -> String {
    let search_toggle = crate::pill_search_toggle("site-search", "Search sites&hellip;", search);
    let action_pill = if can_create {
        format!(
            r#"<div class="icon-pill" style="align-self:flex-end;margin-top:0">{search_toggle}<a href="/admin/sites/new" class="icon-btn" title="New Site" aria-label="New Site"><img src="/admin/static/icons/file-plus.svg" alt=""></a></div>"#,
            search_toggle = search_toggle,
        )
    } else {
        format!(r#"<div class="icon-pill" style="align-self:flex-end;margin-top:0">{search_toggle}</div>"#, search_toggle = search_toggle)
    };

    let fragment = sites_list_fragment(sites, page, total_pages, search, sort, dir, ctx);
    let sort_qs = if sort.is_empty() { String::new() } else { format!("&sort={}&dir={}", sort, if dir == "desc" { "desc" } else { "asc" }) };
    let fetch_prefix = format!("/admin/sites?partial=1{}", sort_qs);
    let live_search = crate::live_search_script("site-search", "sites-list", &fetch_prefix);

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:flex-end;gap:.75rem;margin-bottom:1rem;flex-wrap:wrap">
  {action_pill}
</div>
<div id="sites-list">{fragment}</div>
{live_search}
{pill_search_init}"#,
        action_pill = action_pill,
        fragment = fragment,
        live_search = live_search,
        pill_search_init = crate::pill_search_init_script(),
    );

    crate::admin_page("Sites", "/admin/sites", flash, &content, ctx)
}

pub struct SiteSettingsData {
    pub id: String,
    pub hostname: String,
    pub site_name: String,
    pub site_description: String,
    pub language: String,
    pub posts_per_page: i64,
    pub date_format: String,
    pub maintenance_mode: bool,
    pub maintenance_message: String,
    pub providers: Vec<EmailProviderSummary>,
}

/// One configured email provider, as shown in the Email Settings tab's
/// provider list. Credentials themselves never come back to the browser.
pub struct EmailProviderSummary {
    pub id: String,
    pub label: String,
    /// "mailgun" | "smtp" | "sendgrid" | "postmark"
    pub provider_type: String,
    pub verified: bool,
    /// A short, non-sensitive identifying string (domain + masked key, etc.)
    /// so an admin can tell providers of the same type apart. `None` if the
    /// stored credentials couldn't be decrypted (e.g. a `SECRET_KEY` rotation).
    pub hint: Option<String>,
    /// Per-field placeholder text for this provider's Edit form (real value
    /// for non-secret fields, masked for secrets) — never a value attribute,
    /// so the edit form stays a full overwrite rather than a prefill.
    pub field_placeholders: std::collections::HashMap<String, String>,
}

fn provider_type_label(provider_type: &str) -> &'static str {
    match provider_type {
        "mailgun" => "Mailgun",
        "smtp" => "SMTP",
        "sendgrid" => "SendGrid",
        "postmark" => "Postmark",
        _ => "Unknown",
    }
}

/// A provider row's type badge — a small brand mark, shared 16x16
/// dimensions across all four provider types so the row stays visually
/// consistent regardless of which icon has more/less native whitespace.
/// Mailgun/SendGrid/Postmark are their own full-color brand icons (opted
/// out of the admin's dark-mode icon-invert rule via `.brand-icon`, since
/// that rule assumes monochrome feather icons); SMTP has no brand mark, so
/// it uses feather's at-sign glyph instead.
fn provider_type_badge_html(provider_type: &str) -> String {
    let (src, alt, brand) = match provider_type {
        "mailgun" => ("/admin/static/icons/mailgun.svg", "Mailgun", true),
        "sendgrid" => ("/admin/static/icons/sendgrid.svg", "SendGrid", true),
        "postmark" => ("/admin/static/icons/postmark.svg", "Postmark", true),
        "smtp" => ("/admin/static/icons/at-sign.svg", "SMTP", false),
        _ => return format!(
            r#"<span class="form-note" style="margin:0">{}</span>"#,
            provider_type_label(provider_type)
        ),
    };
    format!(
        r#"<img src="{src}" alt="{alt}" title="{alt}" class="{class}" style="height:16px;width:16px;vertical-align:middle">"#,
        src = src,
        alt = alt,
        class = if brand { "brand-icon" } else { "" },
    )
}

/// The credential fields for one provider type, used both by the Add form
/// (all four, toggled by the type `<select>`) and by a provider row's Edit
/// form (just the one matching its own type). `id_prefix` keeps element ids
/// unique when the same fields appear more than once on the page (one Edit
/// form per row, plus the Add form).
fn provider_fields_html(
    provider_type: &str,
    id_prefix: &str,
    placeholders: Option<&std::collections::HashMap<String, String>>,
) -> String {
    let ph = |field: &str, default: &str| -> String {
        crate::html_escape(
            placeholders
                .and_then(|m| m.get(field))
                .map(|s| s.as_str())
                .unwrap_or(default),
        )
    };
    match provider_type {
        "mailgun" => format!(
            r#"<div class="form-group">
  <label for="{p}mailgun_domain">Mailgun domain</label>
  <input type="text" id="{p}mailgun_domain" name="mailgun_domain" placeholder="{domain_ph}">
</div>
<div class="form-group">
  <label for="{p}mailgun_api_key">Sending key</label>
  <input type="password" id="{p}mailgun_api_key" name="mailgun_api_key" autocomplete="off" placeholder="{key_ph}">
  <small>Use the domain's Sending key (Domains &rarr; select domain &rarr; Sending Keys), not the account-wide Private API key.</small>
</div>"#,
            p = id_prefix,
            domain_ph = ph("mailgun_domain", "e.g. mg.example.com"),
            key_ph = ph("mailgun_api_key", "e.g. key-xxxxxxxx"),
        ),
        "smtp" => format!(
            r#"<div class="form-group">
  <label for="{p}smtp_host">Host</label>
  <input type="text" id="{p}smtp_host" name="smtp_host" placeholder="{host_ph}">
</div>
<div class="form-group">
  <label for="{p}smtp_port">Port</label>
  <input type="number" id="{p}smtp_port" name="smtp_port" placeholder="{port_ph}">
</div>
<div class="form-group">
  <label for="{p}smtp_tls_mode">TLS</label>
  <select id="{p}smtp_tls_mode" name="smtp_tls_mode">
    <option value="starttls">STARTTLS</option>
    <option value="implicit">Implicit TLS</option>
    <option value="none">None</option>
  </select>
</div>
<div class="form-group">
  <label for="{p}smtp_username">Username</label>
  <input type="text" id="{p}smtp_username" name="smtp_username" autocomplete="off" placeholder="{username_ph}">
</div>
<div class="form-group">
  <label for="{p}smtp_password">Password</label>
  <input type="password" id="{p}smtp_password" name="smtp_password" autocomplete="off" placeholder="{password_ph}">
</div>"#,
            p = id_prefix,
            host_ph = ph("smtp_host", "e.g. smtp.example.com"),
            port_ph = ph("smtp_port", "587"),
            username_ph = ph("smtp_username", ""),
            password_ph = ph("smtp_password", ""),
        ),
        "sendgrid" => format!(
            r#"<div class="form-group">
  <label for="{p}sendgrid_api_key">API key</label>
  <input type="password" id="{p}sendgrid_api_key" name="sendgrid_api_key" autocomplete="off" placeholder="{key_ph}">
</div>
<div class="form-group">
  <label for="{p}sendgrid_from_email">From address</label>
  <input type="text" id="{p}sendgrid_from_email" name="sendgrid_from_email" placeholder="{from_ph}">
  <small>Must be a verified sender or domain in your SendGrid account.</small>
</div>"#,
            p = id_prefix,
            key_ph = ph("sendgrid_api_key", ""),
            from_ph = ph("sendgrid_from_email", "e.g. noreply@example.com"),
        ),
        "postmark" => format!(
            r#"<div class="form-group">
  <label for="{p}postmark_server_token">Server API token</label>
  <input type="password" id="{p}postmark_server_token" name="postmark_server_token" autocomplete="off" placeholder="{token_ph}">
</div>
<div class="form-group">
  <label for="{p}postmark_message_stream">Message stream</label>
  <input type="text" id="{p}postmark_message_stream" name="postmark_message_stream" placeholder="{stream_ph}">
</div>
<div class="form-group">
  <label for="{p}postmark_from_email">From address</label>
  <input type="text" id="{p}postmark_from_email" name="postmark_from_email" placeholder="{from_ph}">
  <small>Must be a verified sender signature in your Postmark account.</small>
</div>"#,
            p = id_prefix,
            token_ph = ph("postmark_server_token", ""),
            stream_ph = ph("postmark_message_stream", "outbound"),
            from_ph = ph("postmark_from_email", "e.g. noreply@example.com"),
        ),
        _ => String::new(),
    }
}

pub fn render_settings(data: &SiteSettingsData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let providers_list_html = if data.providers.is_empty() {
        r#"<p class="form-note" style="margin:0">No email providers configured yet — add one below.</p>"#.to_string()
    } else {
        data.providers.iter().map(|p| {
            let status = if p.verified {
                r#"<span class="badge badge-published">Verified</span>"#.to_string()
            } else {
                r#"<span class="badge">Unverified</span>"#.to_string()
            };
            let edit_id = format!("edit-provider-{}", p.id);
            let field_prefix = format!("edit-{}-", p.id);
            format!(
                r#"<div class="card-boxed-section">
  <div style="display:flex;align-items:center;justify-content:space-between;gap:.75rem;flex-wrap:wrap">
    <div style="display:flex;align-items:center;gap:.6rem">
      <strong>{label}</strong>
      {type_badge}
      {status}
    </div>
    <div class="icon-pill-actionbuttons">
      <button type="button" class="icon-btn" title="Edit Provider" aria-label="Edit Provider"
              onclick="toggleProviderEdit('{edit_id}')">
        <img src="/admin/static/icons/edit.svg" alt="">
      </button>
      <form method="post" action="/admin/sites/{site_id}/email-providers/{id}/test" style="display:inline">
        <button type="submit" class="icon-btn" title="Send Test Email" aria-label="Send Test Email"><img src="/admin/static/icons/mail.svg" alt=""></button>
      </form>
      <form method="post" action="/admin/sites/{site_id}/email-providers/{id}/delete" style="display:inline" onsubmit="return confirm('Delete this email provider? Any forms using it will fall back to the install-wide account.')">
        <button type="submit" class="icon-btn icon-danger" title="Delete Provider" aria-label="Delete Provider"><img src="/admin/static/icons/trash.svg" alt=""></button>
      </form>
    </div>
  </div>
  <form method="post" action="/admin/sites/{site_id}/email-providers/{id}" id="{edit_id}" class="provider-edit-form" style="display:none;margin-top:.75rem;padding-top:.75rem;border-top:1px solid var(--border)">
    <input type="hidden" name="provider_type" value="{provider_type}">
    <div class="form-group">
      <label for="{field_prefix}label">Label</label>
      <input type="text" id="{field_prefix}label" name="label" required value="{label}">
    </div>
    {fields_html}
    <p class="field-hint">Re-enter every field — credentials aren't shown back once saved, and saving here replaces them all.</p>
    <div class="icon-pill">
      <button type="submit" class="icon-btn" title="Save Provider" aria-label="Save Provider"><img src="/admin/static/icons/save.svg" alt=""></button>
      <button type="button" class="icon-btn" title="Cancel" aria-label="Cancel"
              onclick="document.getElementById('{edit_id}').style.display='none'"><img src="/admin/static/icons/x.svg" alt=""></button>
    </div>
  </form>
</div>"#,
                label = crate::html_escape(&p.label),
                type_badge = provider_type_badge_html(&p.provider_type),
                status = status,
                site_id = crate::html_escape(&data.id),
                id = crate::html_escape(&p.id),
                edit_id = edit_id,
                provider_type = crate::html_escape(&p.provider_type),
                field_prefix = field_prefix,
                fields_html = provider_fields_html(&p.provider_type, &field_prefix, Some(&p.field_placeholders)),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div>
<style>
.settings-tab-panel {{ display: none; }}
.settings-tab-panel.active {{ display: block; }}
</style>
<div class="page-tabs" role="tablist" style="margin:0 0 1rem">
  <button type="button" class="page-tab active" role="tab" aria-selected="true" aria-controls="tab-general" data-tab="general">General</button>
  <button type="button" class="page-tab" role="tab" aria-selected="false" aria-controls="tab-maintenance" data-tab="maintenance">Maintenance</button>
  <button type="button" class="page-tab" role="tab" aria-selected="false" aria-controls="tab-email" data-tab="email">Email Settings</button>
</div>

<div id="tab-general" class="settings-tab-panel active" role="tabpanel">
<div style="max-width:720px">
<div class="card-boxed">
  <h2 class="card-boxed-header">Settings</h2>
  <div class="card-boxed-body">
  <form method="post" action="/admin/sites/{id}/site-config" class="edit-form" id="site-settings-form">
    <div class="card-boxed-section">
      <div class="form-group">
        <label for="site_name">Site Name</label>
        <input type="text" id="site_name" name="site_name" value="{site_name}" required>
        <small>The display name shown in the browser tab, header, and footer.</small>
      </div>
      <div class="form-group">
        <label for="site_description">Site Description</label>
        <textarea id="site_description" name="site_description" rows="3">{site_description}</textarea>
      </div>
      <div class="form-group">
        <label for="language">Language</label>
        <input type="text" id="language" name="language" value="{language}">
      </div>
      <div class="form-group">
        <label for="posts_per_page">Posts Per Page</label>
        <input type="number" id="posts_per_page" name="posts_per_page" value="{posts_per_page}" min="1" max="100">
      </div>
      <div class="form-group">
        <label for="date_format">Date Format</label>
        <input type="text" id="date_format" name="date_format" value="{date_format}">
        <small>Uses chrono format strings, e.g. "%B %-d, %Y" &rarr; January 1, 2026</small>
      </div>
    </div>
    <div class="icon-pill">
      <button type="submit" id="save-settings-btn" class="icon-btn" title="Save Settings" aria-label="Save Settings" disabled>
        <img src="/admin/static/icons/save.svg" alt="">
      </button>
    </div>
  </form>
  </div>
</div>
<script>
(function() {{
  var form   = document.getElementById('site-settings-form');
  var saveBtn = document.getElementById('save-settings-btn');
  var fields = Array.prototype.slice.call(form.querySelectorAll('input, textarea'));
  var initial = fields.map(function(f) {{ return f.value; }});

  function syncSaveBtn() {{
    var changed = fields.some(function(f, i) {{ return f.value !== initial[i]; }});
    saveBtn.disabled = !changed;
  }}

  fields.forEach(function(f) {{
    f.addEventListener('input', syncSaveBtn);
  }});
}})();
</script>

<div class="card-boxed">
  <h2 class="card-boxed-header">Support</h2>
  <div class="card-boxed-body">
    <p class="form-note" style="margin:0 0 .6rem">
      Include this Site ID when contacting support or troubleshooting an
      issue with this site.
    </p>
    <div style="display:flex;align-items:center;gap:.6rem">
      <div class="icon-pill" style="margin-top:0">
        <button type="button" class="icon-btn" id="site-id-toggle" title="Show Site ID" aria-label="Show Site ID" onclick="toggleSiteId()"><img src="/admin/static/icons/eye.svg" alt=""></button>
      </div>
      <code id="site-id-value" title="Click to copy" onclick="copySiteId()" style="display:none;cursor:pointer;font-size:.8rem;background:var(--tint);padding:.3rem .6rem;border-radius:4px">{id}</code>
    </div>
  </div>
</div>
<script>
function toggleSiteId() {{
  var val    = document.getElementById('site-id-value');
  var toggle = document.getElementById('site-id-toggle');
  var shown  = val.style.display !== 'none';
  val.style.display = shown ? 'none' : '';
  toggle.title = shown ? 'Show Site ID' : 'Hide Site ID';
  toggle.setAttribute('aria-label', toggle.title);
  toggle.querySelector('img').src = '/admin/static/icons/' + (shown ? 'eye.svg' : 'eye-off.svg');
}}
function copySiteId() {{
  var val = document.getElementById('site-id-value');
  var markCopied = function() {{
    // Stays green until the page reloads — re-clicking just re-copies.
    val.style.color = getComputedStyle(document.documentElement).getPropertyValue('--success');
  }};
  // navigator.clipboard requires a secure context (https, or localhost) —
  // on a plain http:// origin (common in dev, e.g. pong.com/bckr.local)
  // it's undefined or its promise silently rejects, so this always falls
  // back to the older execCommand technique rather than doing nothing.
  if (navigator.clipboard && window.isSecureContext) {{
    navigator.clipboard.writeText(val.textContent).then(markCopied, function() {{ legacyCopy(val, markCopied); }});
  }} else {{
    legacyCopy(val, markCopied);
  }}
}}
function legacyCopy(el, onDone) {{
  var ta = document.createElement('textarea');
  ta.value = el.textContent;
  ta.style.position = 'fixed';
  ta.style.opacity = '0';
  document.body.appendChild(ta);
  ta.focus();
  ta.select();
  try {{ document.execCommand('copy'); onDone(); }} catch (e) {{}}
  document.body.removeChild(ta);
}}
</script>
</div>
</div>

<div id="tab-maintenance" class="settings-tab-panel" role="tabpanel">
<div style="max-width:720px">
<div class="card-boxed">
  <h2 class="card-boxed-header">Maintenance Mode</h2>
  <div class="card-boxed-body">
  <form method="post" action="/admin/sites/{id}/maintenance" class="edit-form" id="maintenance-form">
    <div class="card-boxed-section">
      <div class="form-group">
        <label style="display:inline;font-weight:400">
          <input type="checkbox" id="maintenance_mode" name="maintenance_mode" style="display:inline;width:auto;height:auto"{maintenance_checked}>
          Enable maintenance mode
        </label>
        <p class="form-note" style="margin:.4rem 0 0">
          Shows a maintenance page to visitors of this site. Takes effect immediately &mdash; no
          restart needed. <code>/admin/*</code> always stays reachable so you can turn it back off.
        </p>
      </div>
    </div>
    <div class="card-boxed-section">
      <div class="form-group">
        <label for="maintenance_message">Message</label>
        <textarea id="maintenance_message" name="maintenance_message" rows="3" maxlength="250">{maintenance_message}</textarea>
        <small id="maintenance-message-count" style="color:var(--muted)">250/250</small>
      </div>
    </div>
    <div class="icon-pill">
      <button type="submit" id="save-maintenance-btn" class="icon-btn" title="Save Maintenance" aria-label="Save Maintenance" disabled>
        <img src="/admin/static/icons/save.svg" alt="">
      </button>
    </div>
  </form>
  </div>
</div>
<script>
(function() {{
  var form = document.getElementById('maintenance-form');
  var btn  = document.getElementById('save-maintenance-btn');
  var checkbox = document.getElementById('maintenance_mode');
  var initialChecked = checkbox.checked;
  var messageField = document.getElementById('maintenance_message');
  var countEl = document.getElementById('maintenance-message-count');
  function updateCount() {{
    var remaining = 250 - messageField.value.length;
    countEl.textContent = remaining + '/250';
    countEl.style.color = remaining <= 20 ? 'var(--danger)' : 'var(--muted)';
  }}
  messageField.addEventListener('input', updateCount);
  updateCount();
  function snapshot() {{
    return Array.from(new FormData(form).entries()).map(function (e) {{ return e[0] + '=' + e[1]; }}).join('&');
  }}
  var initialSnapshot = snapshot();
  function checkChanged() {{
    btn.disabled = snapshot() === initialSnapshot;
  }}
  form.addEventListener('input', checkChanged);
  form.addEventListener('change', checkChanged);
  form.addEventListener('submit', function(e) {{
    if (checkbox.checked !== initialChecked) {{
      var msg = checkbox.checked
        ? 'Enable maintenance mode? Visitors will see a maintenance page immediately.'
        : 'Disable maintenance mode? The site will be reachable by visitors again.';
      if (!confirm(msg)) {{
        e.preventDefault();
      }}
    }}
  }});
}})();
</script>
</div>
</div>

<div id="tab-email" class="settings-tab-panel" role="tabpanel">
<div class="two-col">
<div>
<div class="card-boxed">
  <h2 class="card-boxed-header">Email Providers</h2>
  <div class="card-boxed-body">
  <p class="form-note" style="margin:0 0 1rem">
    Configure as many third-party email accounts as you like, then pick which one each form
    should send through on that form's own Mail Settings tab. A form with none selected uses
    the install-wide account.
  </p>
  {providers_list_html}
  </div>
</div>
</div>
<script>
function toggleProviderEdit(id) {{
  var target = document.getElementById(id);
  var opening = target.style.display === 'none';
  document.querySelectorAll('.provider-edit-form').forEach(function(f) {{
    f.style.display = 'none';
  }});
  if (opening) target.style.display = 'block';
}}
</script>

<div>
<div class="card-boxed">
  <h2 class="card-boxed-header">Add Provider</h2>
  <div class="card-boxed-body">
  <form method="post" action="/admin/sites/{id}/email-providers" class="edit-form" id="add-provider-form">
    <div class="card-boxed-section">
      <div class="form-group">
        <label for="provider-label">Label</label>
        <input type="text" id="provider-label" name="label" required placeholder="e.g. Marketing Mailgun">
      </div>
      <div class="form-group">
        <label for="provider-type">Provider</label>
        <select id="provider-type" name="provider_type">
          <option value="mailgun">Mailgun</option>
          <option value="smtp">SMTP</option>
          <option value="sendgrid">SendGrid</option>
          <option value="postmark">Postmark</option>
        </select>
      </div>
    </div>
    <div class="card-boxed-section provider-fields" data-provider="mailgun">
      <div class="form-group">
        <label for="mailgun_domain">Mailgun domain</label>
        <input type="text" id="mailgun_domain" name="mailgun_domain" placeholder="e.g. mg.example.com">
      </div>
      <div class="form-group">
        <label for="mailgun_api_key">Sending key</label>
        <input type="password" id="mailgun_api_key" name="mailgun_api_key" autocomplete="off" placeholder="e.g. key-xxxxxxxx">
        <small>Use the domain's Sending key (Domains &rarr; select domain &rarr; Sending Keys), not the account-wide Private API key.</small>
      </div>
    </div>
    <div class="card-boxed-section provider-fields" data-provider="smtp" style="display:none">
      <div class="form-group">
        <label for="smtp_host">Host</label>
        <input type="text" id="smtp_host" name="smtp_host" placeholder="e.g. smtp.example.com">
      </div>
      <div class="form-group">
        <label for="smtp_port">Port</label>
        <input type="number" id="smtp_port" name="smtp_port" placeholder="587">
      </div>
      <div class="form-group">
        <label for="smtp_tls_mode">TLS</label>
        <select id="smtp_tls_mode" name="smtp_tls_mode">
          <option value="starttls">STARTTLS</option>
          <option value="implicit">Implicit TLS</option>
          <option value="none">None</option>
        </select>
      </div>
      <div class="form-group">
        <label for="smtp_username">Username</label>
        <input type="text" id="smtp_username" name="smtp_username" autocomplete="off">
      </div>
      <div class="form-group">
        <label for="smtp_password">Password</label>
        <input type="password" id="smtp_password" name="smtp_password" autocomplete="off">
      </div>
    </div>
    <div class="card-boxed-section provider-fields" data-provider="sendgrid" style="display:none">
      <div class="form-group">
        <label for="sendgrid_api_key">API key</label>
        <input type="password" id="sendgrid_api_key" name="sendgrid_api_key" autocomplete="off">
      </div>
      <div class="form-group">
        <label for="sendgrid_from_email">From address</label>
        <input type="text" id="sendgrid_from_email" name="sendgrid_from_email" placeholder="e.g. noreply@example.com">
        <small>Must be a verified sender or domain in your SendGrid account.</small>
      </div>
    </div>
    <div class="card-boxed-section provider-fields" data-provider="postmark" style="display:none">
      <div class="form-group">
        <label for="postmark_server_token">Server API token</label>
        <input type="password" id="postmark_server_token" name="postmark_server_token" autocomplete="off">
      </div>
      <div class="form-group">
        <label for="postmark_message_stream">Message stream</label>
        <input type="text" id="postmark_message_stream" name="postmark_message_stream" value="outbound">
      </div>
      <div class="form-group">
        <label for="postmark_from_email">From address</label>
        <input type="text" id="postmark_from_email" name="postmark_from_email" placeholder="e.g. noreply@example.com">
        <small>Must be a verified sender signature in your Postmark account.</small>
      </div>
    </div>
    <div class="icon-pill">
      <button type="submit" id="add-provider-btn" class="icon-btn" title="Add Provider" aria-label="Add Provider">
        <img src="/admin/static/icons/save.svg" alt="">
      </button>
    </div>
  </form>
  </div>
</div>
<script>
(function() {{
  var typeSelect = document.getElementById('provider-type');
  var groups = document.querySelectorAll('.provider-fields');
  function sync() {{
    groups.forEach(function(g) {{
      g.style.display = (g.dataset.provider === typeSelect.value) ? '' : 'none';
    }});
  }}
  typeSelect.addEventListener('change', sync);
  sync();
}})();
</script>
</div>
</div>
</div>
</div>
<script>
(function() {{
  var settingsTabs = document.querySelectorAll('.page-tab[data-tab]');
  var settingsPanels = document.querySelectorAll('.settings-tab-panel');
  function activate(btn) {{
    settingsTabs.forEach(function(b) {{
      var on = b === btn;
      b.classList.toggle('active', on);
      b.setAttribute('aria-selected', on ? 'true' : 'false');
    }});
    settingsPanels.forEach(function(panel) {{
      panel.classList.toggle('active', panel.id === 'tab-' + btn.dataset.tab);
    }});
  }}
  settingsTabs.forEach(function(btn) {{
    btn.addEventListener('click', function() {{ activate(btn); }});
  }});
  var wantedTab = new URLSearchParams(window.location.search).get('tab');
  if (wantedTab) {{
    var wantedBtn = document.querySelector('.page-tab[data-tab="' + wantedTab + '"]');
    if (wantedBtn) activate(wantedBtn);
  }}
}})();
</script>
</div>"#,
        id = crate::html_escape(&data.id),
        site_name = crate::html_escape(&data.site_name),
        site_description = crate::html_escape(&data.site_description),
        language = crate::html_escape(&data.language),
        posts_per_page = data.posts_per_page,
        date_format = crate::html_escape(&data.date_format),
        maintenance_checked = if data.maintenance_mode { " checked" } else { "" },
        maintenance_message = crate::html_escape(&data.maintenance_message),
        providers_list_html = providers_list_html,
    );

    crate::admin_page(&format!("Site Settings - {}", data.hostname), "/admin/sites", flash, &content, ctx)
}

/// An existing user selectable as the new site's admin.
pub struct UserOption {
    pub id: String,
    pub label: String,
}

pub struct NewSiteData {
    /// Preserved on validation failure so the admin doesn't have to retype it.
    pub hostname: String,
    /// "existing" | "new" — which Site Admin sub-form was active.
    pub user_assignment: String,
    pub existing_user_id: String,
    pub new_username: String,
    pub new_email: String,
    pub new_display_name: String,
    /// Assignable users: "You" (the acting admin) plus every site_admin-role
    /// user. Every site must have an owner, so this list is never empty.
    pub existing_users: Vec<UserOption>,
}

impl Default for NewSiteData {
    fn default() -> Self {
        Self {
            hostname: String::new(),
            user_assignment: "existing".to_string(),
            existing_user_id: String::new(),
            new_username: String::new(),
            new_email: String::new(),
            new_display_name: String::new(),
            existing_users: Vec::new(),
        }
    }
}

/// Site admins always own what they create — no user picker. Kept as a
/// separate, simpler render rather than branching deep inside render_new's
/// template, since the two forms genuinely have different fields and JS.
fn render_new_for_site_admin(data: &NewSiteData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = format!(
        r#"<div class="card-boxed" style="max-width:560px">
  <h2 class="card-boxed-header">New Site</h2>
  <div class="card-boxed-body">
  <form method="post" action="/admin/sites" class="edit-form" id="new-site-form" style="max-width:580px">
  <div class="card-boxed-section">
  <div class="form-group">
    <label for="hostname">Domain Name</label>
    <input type="text" id="hostname" name="hostname" required placeholder="example.com" autofocus
           value="{hostname}" oninput="hnUpdate()">
    <small>The domain this site will respond to</small>
  </div>
  <div class="form-note" style="margin-bottom:0">
    <p><strong>Domain requirements:</strong></p>
    <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
      <li id="hn-req-dot"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Contains at least one dot (e.g. example<strong>.com</strong>)</li>
      <li id="hn-req-tld"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>TLD is 2 or more letters (e.g. .com, .io, .co.uk)</li>
      <li id="hn-req-chars"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Letters, numbers, and hyphens only — no spaces or symbols</li>
      <li id="hn-req-hyphen"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>No label starts or ends with a hyphen</li>
    </ul>
  </div>
  </div>

  <div class="card-boxed-section">
    <p style="color:var(--muted);font-size:.875rem;margin:0">
      You'll be this site's owner and admin.
    </p>
  </div>
  </div>
  <div class="icon-pill">
    <button type="submit" form="new-site-form" id="create-btn" class="icon-btn" title="Create Site" aria-label="Create Site" disabled>
      <img src="/admin/static/icons/file-plus.svg" alt="">
    </button>
  </div>
  </form>
  </div>
</div>
<script>
(function() {{
  var hnReqs = [
    {{ id: 'hn-req-dot',    test: function(h) {{ return h.indexOf('.') !== -1; }} }},
    {{ id: 'hn-req-tld',    test: function(h) {{ var tld = h.split('.').pop(); return tld.length >= 2 && /^[a-z]+$/i.test(tld); }} }},
    {{ id: 'hn-req-chars',  test: function(h) {{ return /^[a-z0-9.\-]+$/i.test(h); }} }},
    {{ id: 'hn-req-hyphen', test: function(h) {{ return h.split('.').every(function(l) {{ return l.length > 0 && !l.startsWith('-') && !l.endsWith('-'); }}); }} }},
  ];

  window.hnUpdate = function() {{
    var val = document.getElementById('hostname').value.trim();
    var allPass = val.length > 0;
    hnReqs.forEach(function(req) {{
      var li  = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!val) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
        allPass = false;
      }} else if (req.test(val)) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
        allPass = false;
      }}
    }});
    document.getElementById('create-btn').disabled = !allPass;
  }};
}})();
</script>"#,
        hostname = crate::html_escape(&data.hostname),
    );

    crate::admin_page("New Site", "/admin/sites", flash, &content, ctx)
}

pub fn render_new(data: &NewSiteData, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    if !ctx.is_global_admin {
        return render_new_for_site_admin(data, flash, ctx);
    }
    let checked = |val: &str| if data.user_assignment == val { " checked" } else { "" };
    let existing_opts = data.existing_users.iter().map(|u| {
        let sel = if data.existing_user_id == u.id { " selected" } else { "" };
        format!(
            r#"<option value="{id}"{sel}>{label}</option>"#,
            id    = crate::html_escape(&u.id),
            label = crate::html_escape(&u.label),
            sel   = sel,
        )
    }).collect::<Vec<_>>().join("\n");

    let content = format!(
        r#"<div class="card-boxed" style="max-width:560px">
  <h2 class="card-boxed-header">New Site</h2>
  <div class="card-boxed-body">
  <form method="post" action="/admin/sites" class="edit-form" id="new-site-form" style="max-width:580px">
  <div class="card-boxed-section">
  <div class="form-group">
    <label for="hostname">Domain Name</label>
    <input type="text" id="hostname" name="hostname" required placeholder="example.com" autofocus
           value="{hostname}" oninput="hnUpdate()">
    <small>The domain this site will respond to</small>
  </div>
  <div class="form-note" style="margin-bottom:0">
    <p><strong>Domain requirements:</strong></p>
    <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
      <li id="hn-req-dot"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Contains at least one dot (e.g. example<strong>.com</strong>)</li>
      <li id="hn-req-tld"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>TLD is 2 or more letters (e.g. .com, .io, .co.uk)</li>
      <li id="hn-req-chars"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Letters, numbers, and hyphens only — no spaces or symbols</li>
      <li id="hn-req-hyphen"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>No label starts or ends with a hyphen</li>
    </ul>
  </div>
  </div>

  <div class="card-boxed-section">
  <div class="form-group">
    <label>Site Admin</label>
    <div style="display:flex;gap:1.5rem;margin:0.4rem 0 0.75rem;flex-wrap:wrap">
      <label class="radio-label">
        <input type="radio" name="user_assignment" value="existing"{existing_checked} onchange="toggleUserFields()"> Existing user
      </label>
      <label class="radio-label">
        <input type="radio" name="user_assignment" value="new"{new_checked} onchange="toggleUserFields()"> New user
      </label>
    </div>
    <div id="user-existing" style="display:none">
      <select name="existing_user_id" id="user-existing-select">
        <option value="" disabled selected>Select User</option>
        {existing_opts}
      </select>
      <small>The selected user will be the site admin.</small>
    </div>
    <div id="user-new" style="display:none">
      <div class="user-form-grid stacked">
        <div class="form-group">
          <label for="new_username">Username</label>
          <input type="text" id="new_username" name="new_username" value="{new_username}" autocomplete="off"
                 pattern="[a-z0-9][a-z0-9\-]{{3,13}}[a-z0-9]" minlength="5" maxlength="15"
                 title="5-15 characters: lowercase letters, numbers and hyphens only, cannot start or end with a hyphen">
        </div>
        <div class="form-group">
          <label for="new_display_name">Display Name</label>
          <input type="text" id="new_display_name" name="new_display_name" value="{new_display_name}" autocomplete="off">
        </div>
        <div class="form-group">
          <label for="new_email">Email</label>
          <input type="email" id="new_email" name="new_email" value="{new_email}" autocomplete="off">
          <small id="new-email-hint" style="color:#dc2626;display:none">Please enter a valid email address.</small>
        </div>
        <div class="form-group">
          <label for="new_password">Password</label>
          <input type="password" id="new_password" name="new_password" autocomplete="new-password">
        </div>
      </div>
      <div class="form-note" style="margin-bottom:.75rem">
        <p><strong>Username requirements:</strong></p>
        <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
          <li id="new-uname-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>5–15 characters</li>
          <li id="new-uname-req-chars"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>Lowercase letters, numbers, and hyphens only</li>
        </ul>
      </div>
      <div class="form-note" style="margin-bottom:1.25rem">
        <p><strong>Password requirements:</strong></p>
        <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
          <li id="new-pw-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>8–12 characters</li>
          <li id="new-pw-req-upper"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one uppercase letter</li>
          <li id="new-pw-req-num"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one number</li>
          <li id="new-pw-req-sym"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">·</span>At least one symbol: ! @ # $ % &amp;</li>
        </ul>
      </div>
      <small>A new account is created and assigned as this site's admin and owner.</small>
    </div>
  </div>
  </div>
  <div class="icon-pill">
    <button type="submit" form="new-site-form" id="create-btn" class="icon-btn" title="Create Site" aria-label="Create Site" disabled>
      <img src="/admin/static/icons/file-plus.svg" alt="">
    </button>
  </div>
  </form>
  </div>
</div>
<script>
(function() {{
  var hnReqs = [
    {{ id: 'hn-req-dot',    test: function(h) {{ return h.indexOf('.') !== -1; }} }},
    {{ id: 'hn-req-tld',    test: function(h) {{ var tld = h.split('.').pop(); return tld.length >= 2 && /^[a-z]+$/i.test(tld); }} }},
    {{ id: 'hn-req-chars',  test: function(h) {{ return /^[a-z0-9.\-]+$/i.test(h); }} }},
    {{ id: 'hn-req-hyphen', test: function(h) {{ return h.split('.').every(function(l) {{ return l.length > 0 && !l.startsWith('-') && !l.endsWith('-'); }}); }} }},
  ];
  var pwReqs = [
    {{ id: 'new-pw-req-len',   test: function(p) {{ return p.length >= 8 && p.length <= 12; }} }},
    {{ id: 'new-pw-req-upper', test: function(p) {{ return /[A-Z]/.test(p); }} }},
    {{ id: 'new-pw-req-num',   test: function(p) {{ return /[0-9]/.test(p); }} }},
    {{ id: 'new-pw-req-sym',   test: function(p) {{ return /[!@#$%&]/.test(p); }} }},
  ];
  var unameReqs = [
    {{ id: 'new-uname-req-len',    test: function(u) {{ return u.length >= 5 && u.length <= 15; }} }},
    {{ id: 'new-uname-req-chars',  test: function(u) {{ return /^[a-z0-9-]+$/.test(u); }} }},
  ];
  var slugPattern = /^[a-z0-9][a-z0-9\-]{{6,13}}[a-z0-9]$/;
  var usernameTouched = false;

  function isValidHostname(h) {{
    return /^(?:[a-z0-9](?:[a-z0-9\-]*[a-z0-9])?\.)+[a-z]{{2,}}$/i.test(h);
  }}
  function isValidPassword(p) {{
    return p.length >= 8 && p.length <= 12 && /[A-Z]/.test(p) && /[0-9]/.test(p) && /[!@#$%&]/.test(p);
  }}
  function toSlug(s) {{
    return s.toLowerCase().replace(/[^a-z0-9\s-]/g, '').trim()
      .replace(/[\s]+/g, '-').replace(/-{{2,}}/g, '-').replace(/^-|-$/g, '')
      .slice(0, 15).replace(/-$/, '');
  }}

  window.hnUpdate = function() {{
    var val = document.getElementById('hostname').value.trim();
    var allPass = val.length > 0;
    hnReqs.forEach(function(req) {{
      var li  = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!val) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
        allPass = false;
      }} else if (req.test(val)) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
        allPass = false;
      }}
    }});
    syncCreateBtn(allPass);
  }};

  function updateNewUserFeedback() {{
    var pw = document.getElementById('new_password').value;
    pwReqs.forEach(function(req) {{
      var li  = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!pw) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
      }} else if (req.test(pw)) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
      }}
    }});
    var uname = document.getElementById('new_username').value;
    unameReqs.forEach(function(req) {{
      var li  = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!uname) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
      }} else if (req.test(uname)) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
      }}
    }});
    var email = document.getElementById('new_email').value.trim();
    var emailHint = document.getElementById('new-email-hint');
    if (emailHint) emailHint.style.display = (email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) ? '' : 'none';
  }}

  function newUserComplete() {{
    var uname = document.getElementById('new_username').value.trim();
    var dname = document.getElementById('new_display_name').value.trim();
    var email = document.getElementById('new_email').value.trim();
    var pw    = document.getElementById('new_password').value;
    if (!uname || !dname || !email || !pw) return false;
    if (!slugPattern.test(uname)) return false;
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) return false;
    if (!isValidPassword(pw)) return false;
    return true;
  }}

  function syncCreateBtn(hostnameOk) {{
    var assign = document.querySelector('input[name="user_assignment"]:checked').value;
    var userOk = false;
    if (assign === 'existing') {{
      userOk = !!document.getElementById('user-existing-select').value;
    }} else if (assign === 'new') {{
      updateNewUserFeedback();
      userOk = newUserComplete();
    }}
    document.getElementById('create-btn').disabled = !(hostnameOk && userOk);
  }}

  window.toggleUserFields = function() {{
    var val = document.querySelector('input[name="user_assignment"]:checked').value;
    document.getElementById('user-existing').style.display = val === 'existing' ? '' : 'none';
    document.getElementById('user-new').style.display      = val === 'new'      ? '' : 'none';
    hnUpdate();
  }};

  document.getElementById('user-existing-select').addEventListener('change', function() {{ hnUpdate(); }});
  ['new_email', 'new_password'].forEach(function(id) {{
    document.getElementById(id).addEventListener('input', function() {{ hnUpdate(); }});
  }});
  var unameEl = document.getElementById('new_username');
  var dnameEl = document.getElementById('new_display_name');
  unameEl.addEventListener('input', function() {{ usernameTouched = true; hnUpdate(); }});
  dnameEl.addEventListener('input', function() {{
    if (!usernameTouched) unameEl.value = toSlug(dnameEl.value);
    hnUpdate();
  }});

  toggleUserFields();
}})();
</script>"#,
        hostname            = crate::html_escape(&data.hostname),
        existing_checked    = checked("existing"),
        new_checked         = checked("new"),
        existing_opts       = existing_opts,
        new_username        = crate::html_escape(&data.new_username),
        new_email           = crate::html_escape(&data.new_email),
        new_display_name    = crate::html_escape(&data.new_display_name),
    );

    crate::admin_page("New Site", "/admin/sites", flash, &content, ctx)
}
