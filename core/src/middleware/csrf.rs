//! Same-origin CSRF protection for authentication and authenticated mutations.
//!
//! Modern browsers send `Origin` on form POSTs. Requiring an exact origin/host
//! match also blocks same-site attacks from an untrusted sibling subdomain,
//! which SameSite cookies alone do not prevent.

use axum::{
    extract::Request,
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

fn protected_path(path: &str) -> bool {
    path.starts_with("/admin")
        || path.starts_with("/account")
        || matches!(path, "/login" | "/subscribe" | "/recover")
        || path.starts_with("/recover/")
        || path.starts_with("/subscribe/")
        || protected_public_account_mutation(path)
}

fn protected_public_account_mutation(path: &str) -> bool {
    let mut segments = path.trim_start_matches('/').split('/');
    matches!(
        (segments.next(), segments.next(), segments.next()),
        (
            Some(slug),
            Some("comment" | "save" | "unsave"),
            None
        ) if !slug.is_empty() && !matches!(slug, "form" | "poll")
    )
}

/// Reject cross-origin state-changing requests on auth/account/admin routes.
pub async fn same_origin(request: Request, next: Next) -> Response {
    if request.method() == Method::GET
        || request.method() == Method::HEAD
        || request.method() == Method::OPTIONS
        || !protected_path(request.uri().path())
    {
        return next.run(request).await;
    }

    let headers = request.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let forwarded_proto = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim);

    let Some(host) = host else {
        return (StatusCode::FORBIDDEN, "CSRF validation failed").into_response();
    };
    let Some(origin) = origin else {
        return (StatusCode::FORBIDDEN, "CSRF validation failed").into_response();
    };

    let http_origin = format!("http://{host}");
    let https_origin = format!("https://{host}");
    let valid = match forwarded_proto {
        Some("https") => origin == https_origin,
        Some("http") => origin == http_origin,
        // In direct/local operation either scheme is acceptable; the exact Host
        // must still match. Production Caddy supplies X-Forwarded-Proto.
        _ => origin == http_origin || origin == https_origin,
    };

    if !valid {
        tracing::warn!(host, origin, "blocked cross-origin state-changing request");
        return (StatusCode::FORBIDDEN, "CSRF validation failed").into_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::protected_path;

    #[test]
    fn protects_auth_and_authenticated_areas() {
        assert!(protected_path("/admin/login"));
        assert!(protected_path("/account/profile/update"));
        assert!(protected_path("/login"));
        assert!(protected_path("/recover/token"));
        assert!(protected_path("/subscribe/confirm/token"));
        assert!(protected_path("/example-post/comment"));
        assert!(protected_path("/example-post/save"));
        assert!(protected_path("/example-post/unsave"));
        assert!(!protected_path("/form/contact"));
        assert!(!protected_path("/form/save"));
        assert!(!protected_path("/save"));
    }
}
