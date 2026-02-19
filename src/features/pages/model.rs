use chrono::NaiveDateTime;
use derive_more::derive::Display;
use serde::{Deserialize, Serialize};

#[derive(sqlx::FromRow, Eq, PartialEq, Clone, Display)]
#[display("{}", filename)]
pub struct DbPage {
    pub identifier: String,
    pub filename: String,
    pub name: Option<String>,
    pub html_content: String,
    pub md_content: String,
    pub md_content_hash: String,
    pub tags: Option<String>,
    pub modified_datetime: Option<NaiveDateTime>,
    pub created_datetime: Option<NaiveDateTime>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonPage {
    pub identifier: String,
    pub filename: String,
    pub name: Option<String>,
    pub html_content: String,
    pub md_content: String,
    pub md_content_hash: String,
    pub tags: Option<String>,
    pub modified_datetime: Option<String>,
    pub created_datetime: Option<String>,
}

pub enum DbOperationReport {
    NoChange,
    Update,
    Delete,
    Insert,
}
