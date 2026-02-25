use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub identifier: String,
    pub filename: String,
    pub name: Option<String>,
    pub html_content: String,
    pub md_content: String,
    pub md_content_hash: String,
    pub tags: Vec<String>,
    pub modified_datetime: Option<NaiveDateTime>,
    pub created_datetime: Option<NaiveDateTime>,
}
