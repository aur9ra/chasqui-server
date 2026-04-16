use crate::database::sqlite::SqliteRepository;
use crate::features::pages::model::Page;
use chrono::NaiveDateTime;
use sqlx::sqlite::SqlitePoolOptions;

async fn setup_test_db() -> SqliteRepository {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    SqliteRepository::new(pool)
}

fn create_mock_page(identifier: &str, filename: &str) -> Page {
    Page {
        identifier: identifier.to_string(),
        filename: filename.to_string(),
        name: Some("Test".to_string()),
        md_content: "# Hello".to_string(),
        content_hash: "hash".to_string(),
        tags: vec!["rust".to_string()],
        modified_datetime: NaiveDateTime::parse_from_str(
            "2023-01-01 12:00:00",
            "%Y-%m-%d %H:%M:%S",
        )
        .ok(),
        created_datetime: NaiveDateTime::parse_from_str("2023-01-01 12:00:00", "%Y-%m-%d %H:%M:%S")
            .ok(),
        file_path: std::path::PathBuf::from(format!("/content/{}", filename)),
        new_path: None,
    }
}

#[tokio::test]
async fn test_sqlite_save_and_retrieve() {
    let repo = setup_test_db().await;

    let page = create_mock_page("slug-1", "file1.md");
    repo.save_page(&page).await.expect("Should save page");
    let retrieved = repo
        .get_page_by_identifier("slug-1")
        .await
        .expect("Should query")
        .expect("Should find page");

    assert_eq!(retrieved.identifier, "slug-1");
    assert_eq!(retrieved.tags, vec!["rust"]);
}

#[tokio::test]
async fn test_sqlite_upsert_logic() {
    let repo = setup_test_db().await;

    let mut page = create_mock_page("slug-1", "file1.md");
    repo.save_page(&page).await.unwrap();

    page.md_content = "# Updated".to_string();
    page.content_hash = "hash2".to_string();
    repo.save_page(&page).await.unwrap();

    let retrieved = repo
        .get_page_by_identifier("slug-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.md_content, "# Updated");
    assert_eq!(retrieved.content_hash, "hash2");
}

#[tokio::test]
async fn test_sqlite_unique_identifier_constraint() {
    let repo = setup_test_db().await;

    let p1 = create_mock_page("shared-slug", "file1.md");
    repo.save_page(&p1).await.unwrap();

    let p2 = create_mock_page("shared-slug", "file2.md");
    let result = repo.save_page(&p2).await;

    assert!(
        result.is_err(),
        "Should fail due to unique identifier constraint"
    );
}

#[tokio::test]
async fn test_sqlite_delete() {
    let repo = setup_test_db().await;
    let page = create_mock_page("slug", "file.md");
    repo.save_page(&page).await.unwrap();

    repo.delete_page("file.md").await.unwrap();

    let retrieved = repo.get_page_by_identifier("slug").await.unwrap();
    assert!(retrieved.is_none());
}