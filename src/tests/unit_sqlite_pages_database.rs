use crate::database::PageRepository;
use crate::database::sqlite::SqliteRepository;
use crate::domain::Page;
use chrono::NaiveDateTime;
use sqlx::sqlite::SqlitePoolOptions;

// create a sqlite database in memory to test against
// TODO: we might see something closer to how the actual system will perform in a real-time environment by *also doing tests where
// the sqlite database is on the disk.* Some blogs will be too big to fit into memory!
async fn setup_test_db() -> SqliteRepository {
    // Connect to a fresh in-memory database
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        // here's where we establish the database in memory
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    // run migrations to create pages schema
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    SqliteRepository::new(pool)
}

// create a fake page
// TODO: we might see something closer to how the actual system will perform in a real-time
// environment by having well-populated `html_content` rows. how well will the system perform with
// 15,000 word blog posts?
fn create_mock_page(identifier: &str, filename: &str) -> Page {
    Page {
        identifier: identifier.to_string(),
        filename: filename.to_string(),
        name: Some("Test".to_string()),
        html_content: "<p>Hello</p>".to_string(),
        md_content: "# Hello".to_string(),
        md_content_hash: "hash".to_string(),
        tags: vec!["rust".to_string()],
        modified_datetime: NaiveDateTime::parse_from_str(
            "2023-01-01 12:00:00",
            "%Y-%m-%d %H:%M:%S",
        )
        .ok(),
        created_datetime: NaiveDateTime::parse_from_str("2023-01-01 12:00:00", "%Y-%m-%d %H:%M:%S")
            .ok(),
    }
}

// test the database's ability to save and retrieve pages
#[tokio::test]
async fn test_sqlite_save_and_retrieve() {
    // establish a connection to the in-memory database
    let repo = setup_test_db().await;

    // make a mock page
    let page = create_mock_page("slug-1", "file1.md");
    // save it to the mock database
    repo.save_page(&page).await.expect("Should save page");
    // try to get it back
    let retrieved = repo
        .get_page_by_identifier("slug-1")
        .await
        .expect("Should query")
        .expect("Should find page");

    // assert that we have gotten this page back
    assert_eq!(retrieved.identifier, "slug-1");
    assert_eq!(retrieved.tags, vec!["rust"]);
}

// test the database's ability to update a page
#[tokio::test]
async fn test_sqlite_upsert_logic() {
    // establish a connection to the in-memory database
    let repo = setup_test_db().await;

    // create a page, save it
    let mut page = create_mock_page("slug-1", "file1.md");
    repo.save_page(&page).await.unwrap();

    // change the content of the page and perform another save
    page.html_content = "<h1>Updated</h1>".to_string();
    // save the changes
    repo.save_page(&page).await.unwrap();

    // get the page back (hopefully now updated)
    let retrieved = repo
        .get_page_by_identifier("slug-1")
        .await
        .unwrap()
        .unwrap();
    // assert that we have gotten the page back in updated form
    assert_eq!(retrieved.html_content, "<h1>Updated</h1>");
}

// test that the database will not accept pages with the same identifier
#[tokio::test]
async fn test_sqlite_unique_identifier_constraint() {
    let repo = setup_test_db().await;

    let p1 = create_mock_page("shared-slug", "file1.md");
    repo.save_page(&p1).await.unwrap();

    // create a second page with the SAME identifier, DIFFERENT filename and attempt to save it
    // this should fail because `identifier` is defined as unique
    let p2 = create_mock_page("shared-slug", "file2.md");
    let result = repo.save_page(&p2).await;

    // assert that the operation failed
    assert!(
        result.is_err(),
        "Should fail due to unique identifier constraint"
    );
}

// test that the database will delete pages properly
#[tokio::test]
async fn test_sqlite_delete() {
    let repo = setup_test_db().await;
    let page = create_mock_page("slug", "file.md");
    repo.save_page(&page).await.unwrap();

    // attempt to delete the page
    repo.delete_page("file.md").await.unwrap();

    // try to get the deleted page (hopefully we can't)
    let retrieved = repo.get_page_by_identifier("slug").await.unwrap();
    // assert that we got nothing back
    assert!(retrieved.is_none());
}
