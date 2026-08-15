//! Public form submission handler.
//!
//! `POST /form/{name}` — accepts any HTML form, stores fields as JSONB,
//! and redirects back with `?submitted={name}`.
//!
//! Fields whose name starts with `_` (e.g. `_honeypot`) are stripped before
//! storage so they never persist.

use std::collections::HashMap;

use axum::{
    extract::{ConnectInfo, Form, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect},
};

use crate::app_state::AppState;
use crate::mail::{self, EmailMessage};
use crate::middleware::site::CurrentSite;
use crate::models::form_def;
use crate::models::form_submission::{self, create, CreateFormSubmission};

/// `POST /form/{name}` — store a form submission and redirect.
pub async fn submit(
    State(state): State<AppState>,
    current_site: CurrentSite,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
    Path(name): Path<String>,
    Form(fields): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    // Honeypot / internal field stripping — drop any key starting with `_`
    let data: HashMap<String, String> = fields
        .into_iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .collect();

    // If this form has been administratively blocked, redirect with ?blocked=1
    if form_submission::is_blocked(&state.db, current_site.site.id, &name).await {
        let referer = headers
            .get(axum::http::header::REFERER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("/");
        let base = referer.split('?').next().unwrap_or(referer);
        return Redirect::to(&format!("{}?blocked=1", base));
    }

    // Skip storing empty submissions (all fields blank after stripping)
    let is_empty = data.values().all(|v| v.trim().is_empty());

    if !is_empty {
        // Best-effort IP extraction.
        // In production, Caddy sets X-Real-IP (or X-Forwarded-For).
        // In development (direct connection, no proxy), fall back to the TCP peer address.
        let ip = headers
            .get("x-real-ip")
            .or_else(|| headers.get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
            .or_else(|| Some(peer_addr.ip().to_string()));

        // Built before `data` is moved into the submission record below —
        // only used if the form has a notify_email set.
        let notify_body = data.iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join("\n");

        // Fetched before `data` moves below — needed for both the
        // admin-notify email and the submitter-confirmation email.
        let form = form_def::get_by_slug(&state.db, current_site.site.id, &name).await.ok().flatten();

        // The submitter's own address, if this form collects one: the
        // submitted value of its first `email`-type field.
        let submitter_email = form.as_ref().and_then(|f| {
            f.fields.iter()
                .find(|field| field.field_type == "email")
                .and_then(|field| data.get(&field.name))
                .filter(|v| !v.trim().is_empty())
                .cloned()
        });

        let input = CreateFormSubmission {
            site_id: current_site.site.id,
            form_name: name.clone(),
            data: serde_json::to_value(&data).unwrap_or(serde_json::Value::Object(Default::default())),
            ip_address: ip,
        };

        if let Err(e) = create(&state.db, input).await {
            tracing::error!("form submit '{}' error: {:?}", name, e);
        }

        // Fire-and-forget: don't make the visitor's redirect wait on an
        // outbound HTTP call to Mailgun.
        if let Some(form) = form {
            let site_id = current_site.site.id;
            let form_id = form.id;
            let provider_id = form.email_provider_id;

            if let Some(to) = form.settings.notify_email {
                let state = state.clone();
                let subject = format!("New submission: {}", form.name);
                tokio::spawn(async move {
                    let msg = EmailMessage { to: &to, subject: &subject, text: &notify_body, form_id: Some(form_id), provider_id };
                    if let Err(e) = mail::send_for_site(&state, site_id, msg).await {
                        tracing::error!("form notify email failed: {e:?}");
                    }
                });
            }

            if form.settings.confirm_submitter {
                if let Some(to) = submitter_email {
                    let state = state.clone();
                    let subject = fill_template(&form.settings.confirm_subject, &data);
                    let body = fill_template(&form.settings.confirm_body, &data);
                    tokio::spawn(async move {
                        let msg = EmailMessage { to: &to, subject: &subject, text: &body, form_id: Some(form_id), provider_id };
                        if let Err(e) = mail::send_for_site(&state, site_id, msg).await {
                            tracing::error!("form confirmation email failed: {e:?}");
                        }
                    });
                } else {
                    tracing::warn!(
                        "form '{}' has confirm_submitter enabled but no email-type field value was submitted",
                        name
                    );
                }
            }
        }
    }

    // Redirect back to the page that submitted the form, appending
    // ?submitted={name}. Using the form's own name (not just "1") lets a
    // page with multiple embedded forms show the success message for only
    // the one actually submitted — see FormDef::render_html's inline
    // script. Existing hand-written themes checking `{% if
    // request.query.submitted %}` still work unchanged since any non-empty
    // value is truthy.
    // Fall back to "/" if the Referer header is missing or unparseable.
    let referer = headers
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/");

    // Strip any existing query string from the referer before appending ours.
    let base = referer.split('?').next().unwrap_or(referer);
    Redirect::to(&format!(
        "{}?submitted={}",
        base,
        crate::handlers::admin::appearance::url_encode_param(&name)
    ))
}

/// Replace `{{field_name}}` tokens in a confirmation subject/body with the
/// matching submitted value. Unknown tokens are left as-is rather than
/// blanked out, so a typo'd field name is obvious to the admin instead of
/// silently disappearing from the email.
fn fill_template(template: &str, data: &HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (key, value) in data {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    out
}
