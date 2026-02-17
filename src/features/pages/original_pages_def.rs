use anyhow::{Result, anyhow};
use derive_more::derive::Display;
use markdown::{self, Options, to_html_with_options};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::{Executor, Pool, Sqlite};
use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};
use walkdir::WalkDir;

// todo: hash field
// (won't rebuild files that haven't changed)
#[derive(sqlx::FromRow, Eq, PartialEq, Clone, Display)]
#[display("{}", filename)]
pub struct Page {
    filename: String,
    name: Option<String>,
    html_content: String,
    tags: Option<String>,
    modified_datetime: Option<NaiveDateTime>,
    created_datetime: Option<NaiveDateTime>,
}

impl Page {
    async fn create_new_entry(
        &self,
        pool: &Pool<Sqlite>,
    ) -> sqlx::Result<sqlx::sqlite::SqliteQueryResult> {
        // todo: I want to change `html_content` to `md_content`.
        // This will require manually updating the CRUD functions.
        // we must provide the strings exactly for this compile time guarantee.
        let create_new_entry_status = sqlx::query!(
            r#"
            INSERT INTO pages (
            filename,
            name,
            html_content,
            tags,
            modified_datetime,
            created_datetime
            )
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            self.filename,
            self.name,
            self.html_content,
            self.tags,
            self.modified_datetime,
            self.created_datetime
        )
        .execute(pool)
        .await;

        create_new_entry_status
    }

    async fn get_entry(&self, pool: &Pool<Sqlite>) -> sqlx::Result<Option<Page>> {
        // here, we use the query_as function (rather than the query macro)
        let get_entry_status = sqlx::query_as::<_, Page>(
            r#"
        SELECT * FROM pages WHERE filename LIKE ?
        "#,
        )
        .bind(&self.filename)
        .fetch_optional(pool)
        .await;

        get_entry_status
    }

    async fn update_entry(
        &self,
        pool: &Pool<Sqlite>,
    ) -> sqlx::Result<sqlx::sqlite::SqliteQueryResult> {
        let update_entry_status = sqlx::query!(
            r#"
            UPDATE pages
            SET
            name = ?,
            html_content = ?,
            tags = ?,
            modified_datetime = ?,
            created_datetime = ?
            WHERE filename = ?
            "#,
            self.name,
            self.html_content,
            self.tags,
            self.modified_datetime,
            self.created_datetime,
            self.filename,
        )
        .execute(pool)
        .await;

        update_entry_status
    }

    async fn delete_entry(
        &self,
        pool: &Pool<Sqlite>,
    ) -> sqlx::Result<sqlx::sqlite::SqliteQueryResult> {
        let delete_entry_status = sqlx::query!(
            r#"
        DELETE FROM pages WHERE filename = ?"#,
            self.filename
        )
        .execute(pool)
        .await;

        delete_entry_status
    }
}

impl Page {
    // when loading from disk, there may not be existing information for the name, tags, etc. in
    // the metadata, so we will ask the user for these fields.
    pub fn new(
        _filename: String,
        _html_content: String,
        _name: Option<String>,
        _tags: Option<String>,
        _modified_datetime: Option<NaiveDateTime>,
        _created_datetime: Option<NaiveDateTime>,
        ask_user: bool,
    ) -> Page {
        // track if we have notified the user of a file that needs input
        // i.e. "hey, this file {} needs some data"
        let mut presented_user = false;
        let mut truncated_html_content = _html_content.clone();
        truncated_html_content.truncate(100);

        let mut present_user_file_if_hasnt = || {
            if !presented_user {
                presented_user = true;
                println!("\nDetected new page ({}).", _filename);
                println!("Preview:");
                println!("{}\n", truncated_html_content)
            }
        };

        // get the name of the page from the user
        let name = match _name {
            Some(val) => Some(val),
            None => {
                present_user_file_if_hasnt();
                if ask_user {
                    ask_user_stdin_optional("Please provide a name: (or enter for no name)", "")
                } else {
                    None
                }
            }
        };

        // get tags, store as serialized json
        let tags = match _tags {
            Some(val) => Some(val),
            None => {
                if ask_user {
                    // here's our list of tags we'll serialize
                    let mut tags: Vec<String> = Vec::new();
                    const QUIT_STR: &str = "";
                    println!("Please provide any number of tags.");
                    // it's true, with the power of serialization, we can handle an arbitrary number of
                    // tags. let's continually ask the user for some
                    loop {
                        let question = format!("Input tag #{} (or enter to stop)", tags.len());
                        if let Some(tag) = ask_user_stdin_optional(&question, QUIT_STR) {
                            tags.push(tag);
                            continue;
                        } else {
                            // the user is done adding tags.
                            break;
                        }
                    }

                    match tags.len() {
                        0 => None,
                        _ => Some(serde_json::to_string(&tags).unwrap_or("".to_string())),
                    }
                } else {
                    None
                }

                // todo: more sophisticated measures for a deserialization failure, but it's 4:37
                // am and i want something working and this match is hideous
            }
        };

        // get modified time (optional)
        // todo implement this
        let modified_datetime: Option<NaiveDateTime> = match _modified_datetime {
            Some(val) => Some(val),
            None => {
                eprintln!("modified_datetime creation from cli is not yet implemented");
                None
            }
        };

        // get created time (optional)
        // todo implement this
        let created_datetime: Option<NaiveDateTime> = match _created_datetime {
            Some(val) => Some(val),
            None => {
                eprintln!("created_datetime creation from cli is not yet implemented");
                None
            }
        };

        Self {
            filename: _filename,
            name: name,
            tags: tags,
            html_content: _html_content,
            created_datetime: created_datetime,
            modified_datetime: modified_datetime,
        }
    }
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

// before we do any ops with the db, we need to make sure we have got a pages table
pub async fn init_db_check(pool: &Pool<Sqlite>) -> sqlx::Result<()> {
    let status = pool
        .fetch_one(sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='pages'",
        ))
        .await;

    let table_exists: bool = status.is_ok();

    if !table_exists {
        let table_execute_status = pool
            .execute(sqlx::query(
                "CREATE TABLE IF NOT EXISTS pages (
                    filename            TEXT NOT NULL UNIQUE PRIMARY KEY,
                    name                TEXT,
                    html_content        TEXT NOT NULL,
                    tags                TEXT,
                    modified_datetime   INTEGER,
                    created_datetime    INTEGER
                     )",
            ))
            .await;

        match table_execute_status {
            Ok(_) => println!("Successfully created 'pages' in db."),
            Err(e) => eprintln!("Failed to create 'pages' in db: {}", e),
        }
    }
    Ok(())
}

pub async fn get_pages_from_db(pool: &Pool<Sqlite>) -> sqlx::Result<Vec<Page>> {
    let get_pages_status = sqlx::query_as!(Page, r#"SELECT 
                                                        filename,
                                                        name,
                                                        html_content,
                                                        tags,
                                                        modified_datetime as "modified_datetime: NaiveDateTime",
                                                        created_datetime as "created_datetime: NaiveDateTime"
                                                    FROM pages"#).fetch_all(pool).await?;

    Ok(get_pages_status)
}

// iterate over the files at md_path.
// for each file, as well as the file's corresponding entry in the database,
// this function determines what page info should make it to the database.
pub fn process_md_dir(md_path: &Path, pages_from_db: Vec<&Page>) -> Result<Vec<Page>> {
    println!("HashMap length: {}", pages_from_db.len());
    let mut pages: Vec<Page> = Vec::new();

    let pages_from_db_hashmap = pages_to_hashmap(pages_from_db);

    for result_entry in WalkDir::new(md_path) {
        let entry = match result_entry {
            Ok(val) => val,

            // somehow this is not a valid entry
            Err(_) => continue,
        };

        // if it's a file, cool, otherwise, skip
        let md_content = match fs::read_to_string(entry.path()) {
            Ok(content) => content,

            // this error isn't actually important, it just means
            // this isn't a file
            Err(_) => continue,
        };

        let filename = entry.path().to_str().unwrap();

        println!("== Processing {} ==", entry.path().display());

        // at this point, we are currently "operating" over a page that we would like the user to
        // see. we also want to expose features with these pages to write to the db.
        // important attributes for pages include:
        // time created
        // time edited
        // tags
        // stylized name
        // etc...

        // get Page from db
        let db_page = pages_from_db_hashmap.get(&filename.to_string());

        // if it is not present in the db, ask the user!
        // but first, let's get stuff from the file itself
        // get file attributes
        let metadata = match fs::metadata(entry.path()) {
            Ok(val) => val,
            Err(_) => continue,
        };

        // get modified time from the OS file properties
        let modified_from_metadata =
            get_property_from_metadata(&metadata, &MetadataDateTimeOptions::Modified).ok();

        // get created time from the OS file properties
        let created_from_metadata =
            get_property_from_metadata(&metadata, &MetadataDateTimeOptions::Created).ok();

        // transform markdown to html
        let html_content = match to_html_with_options(&md_content, &Options::gfm()) {
            Ok(val) => val,
            Err(e) => {
                return Err(anyhow!(
                    "Failed to convert md to html. Error details: {}",
                    e
                ));
            }
        };

        // get name from db if possible
        let name = match db_page {
            Some(val) => val.name.clone(),
            None => None,
        };

        // get tags from db if possible
        let tags = match db_page {
            Some(val) => val.tags.clone(),
            None => None,
        };

        // if there are blank fields left, we will use the new_ask_user
        // Page constructor to ask the user IF there was no corresponding page in the db
        // (this is a new file in the directory)
        // if it is in the db, the null value is likely intentional
        let file: Page = Page::new(
            filename.to_string(),
            html_content,
            name,
            tags,
            modified_from_metadata,
            created_from_metadata,
            !db_page.is_some(),
        );

        pages.push(file);
    }

    Ok(pages)
}

pub fn pages_to_hashmap(pages: Vec<&Page>) -> HashMap<&String, Page> {
    let mut h: HashMap<&String, Page> = HashMap::new();
    for page in pages {
        h.insert(&page.filename, page.clone());
    }
    h
}

pub async fn insert_from_vec_pages(
    pool: &Pool<Sqlite>,
    files_pages: Vec<&Page>,
    db_pages: Vec<&Page>,
) {
    // we want to be able to easily retrieve info from the db pages
    let db_hashmap = pages_to_hashmap(db_pages);

    for file_page in files_pages {
        // is a version of this page in the database?
        let db_page_option = db_hashmap.get(&file_page.filename);

        match db_page_option {
            Some(db_page) => {
                let db_page_owned = db_page.to_owned();
                // yes, there is a version of this page in the db. are they the same?
                if *file_page == db_page_owned {
                    // yes, they are the same.
                    continue;
                } else {
                    // no, they are not the same.
                    // update the database accordingly.. todo
                    let query = sqlx::query!(
                        r#"
                        UPDATE pages
                        SET
                            name = ?,
                            html_content = ?,
                            tags = ?,
                            modified_datetime = ?,
                            created_datetime = ?
                        WHERE filename = ?
                        "#,
                        file_page.name,
                        file_page.html_content,
                        file_page.tags,
                        file_page.modified_datetime,
                        file_page.created_datetime,
                        file_page.filename,
                    );

                    let update_status = pool.execute(query).await;
                    match update_status {
                        Ok(_) => {
                            println!("Successfully updated {} in db.", file_page.filename);
                        }
                        Err(e) => {
                            println!("Failed to update {} in db: {}", file_page.filename, e);
                        }
                    }
                }
            }
            None => {
                // no, there is not a version of this page in the db.

                // create query
                let query = sqlx::query!(
                    r#"
                INSERT INTO pages (
                    filename,
                    name,
                    html_content,
                    tags,
                    modified_datetime,
                    created_datetime
                )
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
                    file_page.filename,
                    file_page.name,
                    file_page.html_content,
                    file_page.tags,
                    file_page.modified_datetime,
                    file_page.created_datetime
                );

                let insert_status = pool.execute(query).await;
                match insert_status {
                    Ok(_) => {
                        println!("Successfully pushed {} to db.", file_page.filename);
                    }
                    Err(e) => {
                        eprintln!("Failed to push {} to db: {}", file_page.filename, e);
                    }
                }
            }
        }
    }
}

const DEFAULT_PAGE_NAME: &str = "";
const DEFAULT_PAGE_TAGS: &str = "";

enum MetadataDateTimeOptions {
    Modified,
    Accessed,
    Created,
}

fn get_property_from_metadata(
    metadata: &fs::Metadata,
    options: &MetadataDateTimeOptions,
) -> Result<NaiveDateTime> {
    let systime = match options {
        MetadataDateTimeOptions::Modified => metadata.modified(),
        MetadataDateTimeOptions::Accessed => metadata.accessed(),
        MetadataDateTimeOptions::Created => metadata.created(),
    };

    let cleaned_systime = match systime {
        Ok(val) => val,
        Err(e) => return Err(anyhow!("Failed to get time from metadata")),
    };

    let modified_datetime = match system_time_to_chrono(&cleaned_systime) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    return Ok(modified_datetime);
}

fn system_time_to_chrono(sys_time: &std::time::SystemTime) -> Result<NaiveDateTime> {
    let time: u64 = match sys_time.duration_since(std::time::UNIX_EPOCH) {
        Ok(val) => val.as_secs(),
        Err(_) => return Err(anyhow!("Failed to convert system time to chrono")),
    };

    let naive_dt = NaiveDateTime::from_timestamp(time as i64, 0);

    Ok(naive_dt)
}
