use crate::domain::Page;
use crate::features::pages::model::{DbPage, JsonPage};
use chrono::NaiveDateTime;

// create a page for the purposes of testing
fn create_test_page() -> Page {
    Page {
        identifier: "test-slug".to_string(),
        filename: "test.md".to_string(),
        name: Some("Test Page".to_string()),
        html_content: "<h1>Hello</h1>".to_string(),
        md_content: "# Hello".to_string(),
        md_content_hash: "hash123".to_string(),
        tags: vec!["rust".to_string(), "api".to_string()],
        modified_datetime: NaiveDateTime::parse_from_str(
            "2023-01-01 12:00:00",
            "%Y-%m-%d %H:%M:%S",
        )
        .ok(),
        created_datetime: NaiveDateTime::parse_from_str("2023-01-01 10:00:00", "%Y-%m-%d %H:%M:%S")
            .ok(),
    }
}

// test the system's ability to convert Page -> DbPage (important for saving Pages to the database)
// as well as insure a Vec<String> will convert to a JSON string
// in the future, there will be more types of Pages for MySQL and Postgres's databases, and these
// tests will have to be updated
#[test]
fn test_page_to_db_page_serialization() {
    // create a generic Page based on our example page function
    let page = create_test_page();
    // convert it into a DbPage
    let db_page: DbPage = (&page).into();

    assert_eq!(db_page.identifier, "test-slug");
    // verify tags became a JSON string (necessary for storing in sqlite)
    assert_eq!(db_page.tags, Some(r#"["rust","api"]"#.to_string()));
}

// test the system's ability to convert DbPage -> Page and JSON vectorization
#[test]
fn test_db_page_to_page_deserialization() {
    // create a page with the tags a JSON string
    let db_page = DbPage {
        identifier: "db-slug".to_string(),
        filename: "db.md".to_string(),
        name: None,
        html_content: "".to_string(),
        md_content: "".to_string(),
        md_content_hash: "".to_string(),
        tags: Some(r#"["tag1","tag2"]"#.to_string()),
        modified_datetime: None,
        created_datetime: None,
    };

    // convert it into a page
    let page: Page = db_page.try_into().expect("Should convert from DB model");

    assert_eq!(page.identifier, "db-slug");
    // assert that the conversion properly vectorized the tags
    assert_eq!(page.tags, vec!["tag1".to_string(), "tag2".to_string()]);
}

// test Page -> JsonPage and JSON stringification(???)
#[test]
fn test_page_to_json_page_formatting() {
    let page = create_test_page();
    let json_page: JsonPage = (&page).into();

    // verify date formatting
    assert_eq!(
        json_page.modified_datetime,
        Some("2023-01-01 12:00:00".to_string())
    );
    // verify tags are a native vector for the API
    assert_eq!(json_page.tags, vec!["rust".to_string(), "api".to_string()]);
}

// to be honest i don't know why this is here
#[test]
fn test_tags_round_trip_empty() {
    let mut page = create_test_page();
    page.tags = vec![]; // Empty tags

    let db_page: DbPage = (&page).into();
    assert_eq!(db_page.tags, None); // Should be None in DB

    let round_trip_page: Page = db_page.try_into().expect("Should convert back");
    assert!(round_trip_page.tags.is_empty());
}

// ensure the system fails when the tags in a DbPage are malformed
// (would be real funny if it didn't fail)
#[test]
fn test_malformed_db_tags_fails() {
    let db_page = DbPage {
        identifier: "bad".to_string(),
        filename: "bad.md".to_string(),
        name: None,
        html_content: "".to_string(),
        md_content: "".to_string(),
        md_content_hash: "".to_string(),
        tags: Some("not-json".to_string()), // Malformed JSON
        modified_datetime: None,
        created_datetime: None,
    };

    let result: Result<Page, _> = db_page.try_into();
    assert!(result.is_err());
}
