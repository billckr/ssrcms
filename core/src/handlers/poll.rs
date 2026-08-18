//! Public poll voting handlers.
//!
//! `POST /poll/{slug}` — records a vote and redirects back with
//! `?voted={slug}` or `?already_voted={slug}` (PRG pattern, same idiom as
//! `handlers/form.rs`'s `?submitted={name}`).
//! `GET /poll/{slug}/results` — JSON tally, fetched client-side by the
//! embed script generated in `PollDef::render_html`.
//!
//! Vote dedupe reuses the signed-cookie pattern from `post_unlock.rs`: a
//! long-lived signed cookie `poll_voted_{poll_id}` carries a random voter
//! token. Unlike the post-unlock cookie (session-only, existence alone
//! means "unlocked"), this one is deliberately long-lived — the whole point
//! is surviving across browser sessions — and its value is looked up
//! against `poll_votes` rather than trusted blindly, since a poll with
//! `vote_protection: CookieAndIp` also needs to check IP even when the
//! cookie is present but doesn't match a real vote yet.

use std::collections::HashMap;

use axum::{
    extract::{ConnectInfo, Form, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Json, Redirect},
};
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use serde::Serialize;

use crate::app_state::AppState;
use crate::middleware::site::CurrentSite;
use crate::models::poll_def;
use crate::models::poll_vote::{self, RecordVote};

fn cookie_name(poll_id: uuid::Uuid) -> String {
    format!("poll_voted_{poll_id}")
}

fn client_ip(headers: &HeaderMap, peer_addr: std::net::SocketAddr) -> Option<String> {
    headers
        .get("x-real-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| Some(peer_addr.ip().to_string()))
}

fn redirect_back(headers: &HeaderMap, param: &str, slug: &str) -> Redirect {
    let referer = headers
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/");
    let base = referer.split('?').next().unwrap_or(referer);
    Redirect::to(&format!("{base}?{param}={}", crate::handlers::admin::themes::url_encode_param(slug)))
}

#[derive(serde::Deserialize)]
pub struct VoteForm {
    pub option: String,
}

/// `POST /poll/{slug}` — record a vote and redirect.
pub async fn submit(
    State(state): State<AppState>,
    current_site: CurrentSite,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
    Path(slug): Path<String>,
    jar: SignedCookieJar,
    Form(form): Form<VoteForm>,
) -> impl IntoResponse {
    let Ok(Some(poll)) = poll_def::get_by_slug(&state.db, current_site.site.id, &slug).await else {
        return (jar, redirect_back(&headers, "already_voted", &slug)).into_response();
    };

    // Reject a vote for an option that isn't actually one of this poll's
    // options — a tampered/replayed request, or a stale form from before
    // the poll's options were edited.
    if !poll.options.iter().any(|o| o.key == form.option) {
        return (jar, redirect_back(&headers, "already_voted", &slug)).into_response();
    }

    let ip = client_ip(&headers, peer_addr);
    let existing_token = jar.get(&cookie_name(poll.id)).map(|c| c.value().to_string());
    let voter_token = existing_token.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let already = poll_vote::has_voted(&state.db, poll.id, &voter_token, ip.as_deref(), poll.settings.vote_protection).await;

    let redirect = if already {
        redirect_back(&headers, "already_voted", &slug)
    } else {
        let inserted = poll_vote::record_vote(&state.db, RecordVote {
            poll_id: poll.id,
            site_id: current_site.site.id,
            option_key: &form.option,
            voter_token: &voter_token,
            ip_address: ip.as_deref(),
        }).await.unwrap_or(false);
        redirect_back(&headers, if inserted { "voted" } else { "already_voted" }, &slug)
    };

    // Always (re-)set the cookie, even on an already-voted redirect, so a
    // visitor who had lost the cookie but was caught by the IP check gets a
    // consistent token going forward. 10-year Max-Age: unlike the
    // post-unlock cookie, this one deliberately survives browser restarts —
    // persisting the "don't let this browser vote twice" state is the
    // entire point.
    let cookie = Cookie::build((cookie_name(poll.id), voter_token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(3650))
        .build();

    (jar.add(cookie), redirect).into_response()
}

#[derive(Serialize)]
struct ResultOptionJson {
    key: String,
    label: String,
    votes: i64,
}

#[derive(Serialize)]
struct ResultsJson {
    total: i64,
    options: Vec<ResultOptionJson>,
}

/// `GET /poll/{slug}/results` — public JSON tally, fetched client-side by
/// the embed script's post-vote swap (see `PollDef::render_html`). No auth
/// — poll results are meant to be visible to anyone who can already see the
/// embedded poll.
pub async fn results(
    State(state): State<AppState>,
    current_site: CurrentSite,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let Ok(Some(poll)) = poll_def::get_by_slug(&state.db, current_site.site.id, &slug).await else {
        return Json(ResultsJson { total: 0, options: Vec::new() });
    };
    let tally: HashMap<String, i64> = poll_vote::tally(&state.db, poll.id).await.unwrap_or_default().into_iter().collect();
    let options: Vec<ResultOptionJson> = poll.options.iter().map(|o| ResultOptionJson {
        key: o.key.clone(),
        label: o.label.clone(),
        votes: tally.get(&o.key).copied().unwrap_or(0),
    }).collect();
    let total = options.iter().map(|o| o.votes).sum();
    Json(ResultsJson { total, options })
}
