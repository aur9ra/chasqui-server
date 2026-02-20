use crate::features::pages::model::DbPage;
use crate::features::pages::repo::{get_pages_from_db, process_md_dir, process_page_operations};
use axum::Router;
use dotenv;
use sqlx::Sqlite;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePoolOptions;
use std::{env::var, path::Path};

mod features;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // determine environment variables
    dotenv::dotenv().ok();

    // init environment variables
    let db_url =
        var("DATABASE_URL").expect("Failed to determine DATABASE_URL from environment variables");
    let db_url_str = db_url.as_str();

    let max_connections = var("MAX_CONNECTIONS")
        .ok()
        .and_then(|val| val.parse::<u32>().ok())
        .unwrap_or(15);

    // verify db exists
    if !Sqlite::database_exists(db_url_str).await.unwrap_or(false) {
        println!("Unable to connect to database at {}, creating...", db_url);
        match Sqlite::create_database(db_url_str).await {
            Ok(_) => println!("Successfully created database at {}.", db_url),
            Err(e) => panic!(
                "Unable to create database at {}. Error details: {}",
                db_url, e
            ),
        };
    }

    // connect to our db
    let pool = match SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect(db_url_str)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            panic!("Failed to create pool on {}: {}", db_url, e);
        }
    };

    // run migrations
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations.");

    // init pages, sync with db
    let md_path = Path::new("./content/md");

    // get current pages in db
    let db_pages = get_pages_from_db(&pool).await.unwrap();
    let borrowable_db_pages: Vec<&DbPage> = db_pages.iter().collect();

    // scan the directory and determine what needs to be inserted/updated/deleted
    let page_operations = process_md_dir(md_path, borrowable_db_pages).unwrap();

    // execute the database operations
    process_page_operations(&pool, page_operations)
        .await
        .unwrap();

    println!("Sync complete. Starting server...");

    let app = Router::new()
        .merge(features::pages::router())
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Server listening on http://127.0.0.1:3000");

    axum::serve(listener, app).await?;

    Ok(())
}
