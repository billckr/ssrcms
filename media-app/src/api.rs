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
