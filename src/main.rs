use anyhow::{Result, anyhow};
use axum::{Router, routing::get};
use dotenv;
use sqlx::Sqlite;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashMap;
use std::{env::var, path::Path};

use crate::pages::{Page, get_pages_from_db, insert_from_vec_pages};

mod db;
mod pages;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // determine environment variables
    dotenv::dotenv().ok();

    let db_url = match var("DATABASE_URL") {
        Ok(val) => val,
        Err(e) => {
            panic!("Failed to determine database_url from env: {}", e);
        }
    };
    let db_url_str = db_url.as_str();

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
        .max_connections(15)
        .connect(db_url_str)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            panic!("Failed to create pool on {}: {}", db_url, e);
        }
    };

    let md_path = Path::new("./content/md");
    pages::init_db_check(&pool).await;
    let db_pages = get_pages_from_db(&pool).await.unwrap();
    let borrowable_db_pages: Vec<&Page> = db_pages.iter().collect();
    println!("retrieved {} pages from db", db_pages.len());
    let files_pages = pages::process_md_dir(md_path, borrowable_db_pages.clone()).unwrap();
    insert_from_vec_pages(
        &pool,
        files_pages.iter().collect(),
        borrowable_db_pages.clone(),
    )
    .await;

    Ok(())
}
