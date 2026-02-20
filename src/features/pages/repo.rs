use crate::features::pages::model::{DbOperationReport, DbPage};
use anyhow::{Result, anyhow};
use gray_matter::{Matter, ParsedEntity, engine::YAML};
use markdown::{self, Options, to_html_with_options};
use serde::Deserialize;
use sqlx::types::chrono::NaiveDateTime;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::Path;
use std::{env, fs};
use walkdir::{DirEntry, WalkDir};
use xxhash_rust::xxh3::xxh3_64;

#[derive(Deserialize, Debug, Default)]
struct PageFrontMatter {
    identifier: Option<String>,
    name: Option<String>,
    tags: Option<Vec<String>>,
    modified_datetime: Option<String>,
    created_datetime: Option<String>,
}

pub async fn get_entry_by_identifier(
    identifier: &str,
    pool: &Pool<Sqlite>,
) -> sqlx::Result<Option<DbPage>> {
    sqlx::query_as::<_, DbPage>(
        r#"
        SELECT * FROM pages WHERE identifier LIKE ?
        "#,
    )
    .bind(identifier)
    .fetch_optional(pool)
    .await
}

pub async fn get_pages_from_db(pool: &Pool<Sqlite>) -> sqlx::Result<Vec<DbPage>> {
    let get_pages_status = sqlx::query_as!(DbPage, r#"SELECT 
                                                        identifier,
                                                        filename,
                                                        name,
                                                        html_content,
                                                        md_content,
                                                        md_content_hash,
                                                        tags,
                                                        modified_datetime as "modified_datetime: NaiveDateTime",
                                                        created_datetime as "created_datetime: NaiveDateTime"
                                                    FROM pages"#).fetch_all(pool).await?;
    Ok(get_pages_status)
}

// iterate over the files at md_path.
// generate a list of operations that need to be performed to get the db in sync with our md files.
pub fn process_md_dir(
    md_path: &Path,
    pages_from_db: Vec<&DbPage>,
) -> Result<Vec<(DbPage, DbOperationReport)>> {
    let mut page_operations: Vec<(DbPage, DbOperationReport)> = Vec::new();

    let db_pages_map = pages_to_hashmap(pages_from_db);

    for result_entry in WalkDir::new(md_path) {
        let entry = match result_entry {
            Ok(val) => val,

            // somehow this is not a valid entry
            Err(_) => continue,
        };

        // skip anything that isn't a file
        if !entry.file_type().is_file() {
            continue;
        }

        // work with only markdown files (for now)
        if entry.path().extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        let operation_report = process_dir_entry(&entry, &db_pages_map);
        match operation_report {
            Ok(page_report) => {
                page_operations.push(page_report);
            }
            Err(e) => {
                eprintln!("Error occurred processing page: {}", e);
            }
        };
    }

    Ok(page_operations)
    // TODO: Add delete operations when a db page no longer exists in the file system.
    // TODO: Allow a config setting to ask the user whether or not to keep the file.
    // TODO: Identify "moves" with some confidence.
}

// process a directory entry, identify if it's a page, and identify necessary action
// additionally, report which db operation is appropriate (single responsibility)
fn process_dir_entry(
    entry: &DirEntry,
    db_pages: &HashMap<&String, DbPage>,
) -> Result<(DbPage, DbOperationReport)> {
    let md_location_prefix = Path::new("./content/md/");

    // if it's a file, cool, otherwise, skip
    let md_content = match fs::read_to_string(&entry.path()) {
        Ok(content) => content,

        // unable to read file
        Err(e) => {
            return Err(anyhow!(
                "Unable to read file {}: {}",
                &entry.path().display(),
                e
            ));
        }
    };

    // get path
    let path = entry.path();
    let relative_path = path.strip_prefix(md_location_prefix).unwrap_or(&path);

    let filename = relative_path.to_string_lossy().to_string().to_owned();
    // there's got to be a better way to do this?

    // parse frontmatter and seperate it from the content
    let matter = Matter::<YAML>::new();
    let parsed_matter: ParsedEntity = matter
        .parse(&md_content)
        .map_err(|e| anyhow!("Failed to parse frontmatter in {}: {}", filename, e))?;

    // deserialize frontmatter into our struct
    let frontmatter = match parsed_matter.data {
        Some(pod) => pod.deserialize::<PageFrontMatter>().unwrap_or_default(),
        None => PageFrontMatter::default(),
    };

    // convert tags into a JSON string for db
    let tags = frontmatter
        .tags
        .map(|t| serde_json::to_string(&t).unwrap_or_default());

    // determine the identifier
    let strip_extension = env::var("DEFAULT_IDENTIFIER_STRIP_EXTENSION")
        .unwrap_or_else(|_| "false".to_string())
        == "true";

    let default_identifier = if strip_extension {
        // if path is "pages/post1.md", this extracts "pages/post1"
        relative_path
            .with_extension("")
            .to_string_lossy()
            .to_string()
    } else {
        filename.clone()
    };

    let identifier = frontmatter.identifier.unwrap_or(default_identifier);

    let name = frontmatter.name;

    let file_md_content_hash = format!("{:016x}", xxh3_64(md_content.as_bytes()));

    // try to get the corresponding db page, if exists
    let db_page_opt = db_pages.get(&filename);

    // to save on computation, let's peek into the corresponding database page (if it exists) to
    // see if the hash of the content is the same
    if let Some(db_page) = db_page_opt
        && db_page.md_content_hash == file_md_content_hash
    {
        return Ok((db_page.to_owned(), DbOperationReport::NoChange));
    };

    let metadata = match fs::metadata(entry.path()) {
        Ok(m) => Ok(m),
        Err(_) => {
            eprintln!("Warning: Could not read OS metadata for {}", filename);
            Err(anyhow!("Failed to read metadata"))
        }
    };

    // get modified/created time from the OS file properties
    let os_modified =
        get_property_from_metadata(&metadata, &MetadataDateTimeOptions::Modified).ok();
    let os_created = get_property_from_metadata(&metadata, &MetadataDateTimeOptions::Created).ok();

    // resolve modified/created times
    let final_modified_datetime = resolve_datetime(frontmatter.modified_datetime, os_modified);
    let final_created_datetime = resolve_datetime(frontmatter.created_datetime, os_created);

    // md -> html (critical: use parsed_matter, as md_content includes yaml)
    let html_content = match to_html_with_options(&parsed_matter.content, &Options::gfm()) {
        Ok(html) => html,
        Err(e) => {
            return Err(anyhow!(
                "Failed to convert md to html. Error details: {}",
                e
            ));
        }
    };

    if let Some(db_page) = db_page_opt {
        // UPDATING EXISTING PAGE IN DB

        let identifier = db_page.identifier.to_owned();
        let name = db_page.name.to_owned();
        let tags = db_page.tags.to_owned();

        return Ok((
            DbPage {
                identifier: identifier,
                filename: filename.to_owned(),
                name: name,
                html_content: html_content,
                md_content: parsed_matter.content,
                md_content_hash: file_md_content_hash,
                tags: tags,
                modified_datetime: final_modified_datetime,
                created_datetime: final_created_datetime,
            },
            DbOperationReport::Update,
        ));
    } else {
        // CREATING NEW PAGE IN DB

        return Ok((
            DbPage {
                identifier: identifier,
                filename: filename.to_owned(),
                name: name,
                html_content: html_content,
                md_content: parsed_matter.content,
                md_content_hash: file_md_content_hash,
                tags: tags,
                modified_datetime: final_modified_datetime,
                created_datetime: final_created_datetime,
            },
            DbOperationReport::Insert,
        ));
    }
}

fn resolve_datetime(
    frontmatter_date: Option<String>,
    os_date: Option<NaiveDateTime>,
) -> Option<NaiveDateTime> {
    // tier 1: try to use frontmatter data
    if let Some(date_str) = frontmatter_date {
        // attempt to parse RFC3339
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&date_str) {
            return Some(dt.naive_utc());
        }

        // fallback to YYYY-MM-DD
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
            return Some(dt.and_hms_opt(0, 0, 0).unwrap_or_default());
        }
    }

    // tier 2 & 3
    os_date
}

pub fn pages_to_hashmap(pages: Vec<&DbPage>) -> HashMap<&String, DbPage> {
    let mut h: HashMap<&String, DbPage> = HashMap::new();
    for page in pages {
        h.insert(&page.filename, page.clone());
    }
    h
}

pub async fn process_page_operations(
    pool: &Pool<Sqlite>,
    page_operations: Vec<(DbPage, DbOperationReport)>,
) -> sqlx::Result<()> {
    for (db_page, operation) in page_operations {
        match operation {
            DbOperationReport::Insert => {
                sqlx::query!(
                    r#"
                    INSERT INTO pages (
                        identifier,
                        filename,
                        name,
                        html_content,
                        md_content,
                        md_content_hash,
                        tags,
                        modified_datetime,
                        created_datetime
                    )
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                    db_page.identifier,
                    db_page.filename,
                    db_page.name,
                    db_page.html_content,
                    db_page.md_content,
                    db_page.md_content_hash,
                    db_page.tags,
                    db_page.modified_datetime,
                    db_page.created_datetime
                )
                .execute(pool)
                .await?;

                println!("Successfully inserted {} into db.", db_page.filename);
            }
            DbOperationReport::Update => {
                sqlx::query!(
                    r#"
                    UPDATE pages
                    SET
                        identifier = ?,
                        name = ?,
                        html_content = ?,
                        md_content = ?,
                        md_content_hash = ?,
                        tags = ?,
                        modified_datetime = ?,
                        created_datetime = ?
                    WHERE filename = ?
                    "#,
                    db_page.identifier,
                    db_page.name,
                    db_page.html_content,
                    db_page.md_content,
                    db_page.md_content_hash,
                    db_page.tags,
                    db_page.modified_datetime,
                    db_page.created_datetime,
                    db_page.filename
                )
                .execute(pool)
                .await?;

                println!("Successfully updated {} in db.", db_page.filename);
            }
            DbOperationReport::Delete => {
                sqlx::query!(
                    r#"
                    DELETE FROM pages WHERE filename = ?
                    "#,
                    db_page.filename
                )
                .execute(pool)
                .await?;

                println!("Successfully deleted {} from db.", db_page.filename);
            }
            DbOperationReport::NoChange => {
                // Do nothing
            }
        };
    }
    Ok(())
}

enum MetadataDateTimeOptions {
    Modified,
    Created,
}

fn get_property_from_metadata(
    metadata_result: &Result<fs::Metadata>,
    options: &MetadataDateTimeOptions,
) -> Result<NaiveDateTime> {
    // depending on user's provided options, attempt to get modified/created data from metadata
    let metadata = metadata_result
        .as_ref()
        .map_err(|e| anyhow!("Metadata error: {}", e))?;

    let systime = match options {
        MetadataDateTimeOptions::Modified => metadata.modified(),
        MetadataDateTimeOptions::Created => metadata.created(),
    };

    let cleaned_systime = match systime {
        Ok(val) => val,
        Err(e) => return Err(anyhow!("Failed to get time from metadata: {}", e)),
    };

    let dt = match system_time_to_chrono(&cleaned_systime) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    return Ok(dt);
}

fn system_time_to_chrono(sys_time: &std::time::SystemTime) -> Result<NaiveDateTime> {
    let time: u64 = sys_time
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| anyhow!("Failed to convert system time to chrono"))?
        .as_secs();

    let dt = chrono::DateTime::from_timestamp(time as i64, 0)
        .ok_or_else(|| anyhow!("Invalid OS timestamp"))?;

    Ok(dt.naive_utc())
}
