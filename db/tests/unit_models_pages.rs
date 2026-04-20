use chasqui_core::features::pages::model::Page;
use chasqui_db::repo::pages::DbPage;
use chrono::NaiveDateTime;
use std::path::PathBuf;

fn create_test_page() -> Page {
    Page {
        identifier: "test-slug".to_string(),
        filename: "test.md".to_string(),
        name: Some("Test Page".to_string()),
        md_content: "# Hello".to_string(),
        content_hash: "hash123".to_string(),
        tags: vec!["rust".to_string(), "api".to_string()],
        modified_datetime: NaiveDateTime::parse_from_str(
            "2023-01-01 12:00:00",
            "%Y-%m-%d %H:%M:%S",
        )
        .ok(),
        created_datetime: NaiveDateTime::parse_from_str("2023-01-01 10:00:00", "%Y-%m-%d %H:%M:%S")
            .ok(),
        file_path: PathBuf::from("/content/test.md"),
        new_path: None,
    }
}

#[test]
fn test_page_to_db_page_serialization() {
    let page = create_test_page();
    let db_page: DbPage = (&page).into();

    assert_eq!(db_page.identifier, "test-slug");
    assert_eq!(db_page.tags, Some(r#"["rust","api"]"#.to_string()));
    assert_eq!(db_page.content_hash, "hash123");
}

#[test]
fn test_db_page_to_page_deserialization() {
    let db_page = DbPage {
        identifier: "db-slug".to_string(),
        filename: "db.md".to_string(),
        name: None,
        md_content: "".to_string(),
        content_hash: "".to_string(),
        tags: Some(r#"["tag1","tag2"]"#.to_string()),
        modified_datetime: None,
        created_datetime: None,
        file_path: "/content/db.md".to_string(),
        new_path: None,
    };

    let page: Page = db_page.try_into().expect("Should convert from DB model");

    assert_eq!(page.identifier, "db-slug");
    assert_eq!(page.tags, vec!["tag1".to_string(), "tag2".to_string()]);
}

#[test]
fn test_tags_round_trip_empty() {
    let mut page = create_test_page();
    page.tags = vec![];

    let db_page: DbPage = (&page).into();
    assert_eq!(db_page.tags, None);

    let round_trip_page: Page = db_page.try_into().expect("Should convert back");
    assert!(round_trip_page.tags.is_empty());
}

#[test]
fn test_malformed_db_tags_fails() {
    let db_page = DbPage {
        identifier: "bad".to_string(),
        filename: "bad.md".to_string(),
        name: None,
        md_content: "".to_string(),
        content_hash: "".to_string(),
        tags: Some("not-json".to_string()),
        modified_datetime: None,
        created_datetime: None,
        file_path: "/content/bad.md".to_string(),
        new_path: None,
    };

    let result: Result<Page, _> = db_page.try_into();
    assert!(result.is_err());
}