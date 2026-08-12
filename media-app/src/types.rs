use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct GridItem {
    pub id: String,
    pub filename: String,
    #[serde(rename = "type")]
    pub type_key: String,
    #[serde(rename = "isImage")]
    pub is_image: bool,
    pub path: String,
    pub alt: String,
    pub title: String,
    pub caption: String,
    pub size: String,
    pub dims: String,
    pub uploader: String,
    pub uploaded_at: String,
    #[allow(dead_code)]
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GridFolder {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GridTypeCounts {
    pub all: i64,
    pub image: i64,
    pub video: i64,
    pub audio: i64,
    pub document: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GridResponse {
    pub items: Vec<GridItem>,
    pub folders: Vec<GridFolder>,
    pub type_counts: GridTypeCounts,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}
