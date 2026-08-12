use crate::types::GridResponse;

/// Fetch the filtered grid data from the same-origin admin API.
pub async fn fetch_grid(
    folder_id: Option<&str>,
    type_filter: Option<&str>,
    page: i64,
) -> Result<GridResponse, String> {
    let mut qs: Vec<String> = vec![format!("page={page}")];
    if let Some(f) = folder_id {
        qs.push(format!("folder_id={f}"));
    }
    if let Some(t) = type_filter {
        qs.push(format!("type={t}"));
    }
    let url = format!("/admin/api/media/grid?{}", qs.join("&"));

    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<GridResponse>()
        .await
        .map_err(|e| e.to_string())
}

/// POST /admin/media/folders/new. Redirects on success server-side (the
/// endpoint predates this island); we just check the response arrived ok —
/// `fetch` follows the redirect transparently and we don't care about the
/// resulting HTML body.
pub async fn create_folder(name: &str) -> Result<(), String> {
    let body = format!("name={}", urlencoding_component(name));
    let resp = gloo_net::http::Request::post("/admin/media/folders/new")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(format!("server returned {}", resp.status())) }
}

/// POST /admin/media/folders/{id}/delete.
pub async fn delete_folder(id: &str, delete_media: bool) -> Result<(), String> {
    let body = format!("delete_media={}", if delete_media { "true" } else { "false" });
    let resp = gloo_net::http::Request::post(&format!("/admin/media/folders/{id}/delete"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(format!("server returned {}", resp.status())) }
}

/// Minimal x-www-form-urlencoded value encoder — folder names are already
/// restricted to alphanumerics/hyphens by `sanitize_folder_name` before this
/// is ever called, but encode defensively rather than assume that holds.
fn urlencoding_component(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
                c.to_string()
            } else {
                c.to_string().bytes().map(|b| format!("%{b:02X}")).collect()
            }
        })
        .collect()
}
