use crate::features::pages::model::{DbOperationReport, DbPage};
use anyhow::{Result, anyhow};
use markdown::{self, Options, to_html_with_options};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::{Executor, Pool, Sqlite};
use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};
use walkdir::{DirEntry, WalkDir};
use xxhash_rust::xxh3::xxh3_64;

pub async fn get_entry_by_name(name: &str, pool: &Pool<Sqlite>) -> sqlx::Result<Option<DbPage>> {
    let supply_default_entry: bool = true;
    let default_name: &str = "index.md";
    let supplied_name = match supply_default_entry && name.is_empty() {
        true => default_name,
        false => name,
    };
    // here, we use the query_as function (rather than the query macro)
    sqlx::query_as::<_, DbPage>(
        r#"
        SELECT * FROM pages WHERE filename LIKE ?
        "#,
    )
    .bind(supplied_name)
    .fetch_optional(pool)
    .await
}

// ask the user for input through the stdin
// return Some(input) if input != stop_str, otherwise None
fn ask_user_stdin_optional(question: &str, stop_str: &str) -> Option<String> {
    let user_input = ask_user_stdin(&question);
    match user_input == stop_str {
        true => None,
        false => Some(user_input),
    }
}

// ask the user for input through the stdin
// if something goes wrong, continue to ask
fn ask_user_stdin(question: &impl std::fmt::Display) -> String {
    let stdin = io::stdin();
    println!("{}", question);

    let input = &mut String::new();

    loop {
        match stdin.read_line(input) {
            Ok(_) => return input.trim_matches(char::is_control).to_owned(),
            // was there somehow an error reading from stdin?
            Err(e) => {
                eprintln!("Failed to read stdin. Error: {}", e);
                input.clear();
                continue;
            }
        }
    }
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

        let operation_report = process_dir_entry(&entry, &db_pages_map);
        match operation_report {
            Ok(page_report) => {
                page_operations.push(page_report);
            }
            Err(e) => return Err(anyhow!("Error occurred processing page: {}", e)),
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

        // this error isn't actually important, it just means
        // this isn't a file
        Err(e) => {
            return Err(anyhow!(
                "Entry {} is not a readable file",
                &entry.path().display()
            ));
        }
    };
    let path = entry.path();
    let relative_path = path.strip_prefix(md_location_prefix).unwrap_or(&path);

    let filename = relative_path.to_string_lossy().to_string().to_owned();
    // there's got to be a better way to do this?

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
        Ok(m) => m,
        Err(_) => {
            todo!(
                "Implement metadata collection for the case where we are unable to retrieve it from the file."
            )
        }
    };

    // get modified time from the OS file properties
    let modified_from_metadata =
        get_property_from_metadata(&metadata, &MetadataDateTimeOptions::Modified).ok();

    // get created time from the OS file properties
    let created_from_metadata =
        get_property_from_metadata(&metadata, &MetadataDateTimeOptions::Created).ok();

    // md -> html
    let html_content = match to_html_with_options(&md_content, &Options::gfm()) {
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
                md_content: md_content,
                md_content_hash: file_md_content_hash,
                tags: tags,
                modified_datetime: modified_from_metadata,
                created_datetime: created_from_metadata,
            },
            DbOperationReport::Update,
        ));
    } else {
        // CREATING NEW PAGE IN DB

        // present the user: we've got a new file, give us some info..?
        // one day this will be a ui. today it is taken care of in a couple dozen lines
        let mut truncated_md_content = md_content.clone();
        truncated_md_content.truncate(100);
        println!("\nDetected new page ({}).", filename);
        println!("Preview:");
        println!("{}\n", truncated_md_content);

        // get identifier, name, and tags from user.
        let identifier = ask_user_stdin(&String::from("Please provide an identifier."));
        let name = ask_user_stdin_optional(
            &String::from("Please provide a name (optional, enter to skip)."),
            "\n",
        );
        let mut tags_vec: Vec<String> = Vec::new();
        const QUIT_STR: &str = ""; // TODO put this into .env
        println!("Please provide any number of tags.");
        loop {
            let question = format!("Input tag #{} (or enter to stop)", tags_vec.len());
            if let Some(tag) = ask_user_stdin_optional(&question, QUIT_STR) {
                tags_vec.push(tag);
                continue;
            } else {
                // the user is done adding tags.
                break;
            }
        }
        let tags = match tags_vec.len() {
            0 => None,
            _ => Some(serde_json::to_string(&tags_vec).unwrap_or("".to_string())),
        };

        return Ok((
            DbPage {
                identifier: identifier,
                filename: filename.to_owned(),
                name: name,
                html_content: html_content,
                md_content: md_content,
                md_content_hash: file_md_content_hash,
                tags: tags,
                modified_datetime: modified_from_metadata,
                created_datetime: created_from_metadata,
            },
            DbOperationReport::Insert,
        ));
    }
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
    metadata: &fs::Metadata,
    options: &MetadataDateTimeOptions,
) -> Result<NaiveDateTime> {
    // depending on user's provided options, attempt to get modified/created data from metadata
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
    let time: u64 = match sys_time.duration_since(std::time::UNIX_EPOCH) {
        Ok(val) => val.as_secs(),
        Err(_) => return Err(anyhow!("Failed to convert system time to chrono")),
    };

    let naive_dt = NaiveDateTime::from_timestamp(time as i64, 0);

    Ok(naive_dt)
}
