use serde::Deserialize;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct PageFrontMatter {
    pub identifier: Option<String>,
    pub name: Option<String>,
    pub tags: Option<Vec<String>>,
    pub modified_datetime: Option<String>,
    pub created_datetime: Option<String>,
}
