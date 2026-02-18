use crate::features::pages::model::DbPage;
use anyhow::{Result, anyhow};
use markdown::{self, Options, to_html_with_options};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::{Executor, Pool, Sqlite};
use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};
use walkdir::WalkDir;

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

// when loading from disk, there may not be existing information for the name, tags, etc. in
// the metadata, so we will ask the user for these fields.
pub fn new_page(
    _filename: String,
    _html_content: String,
    _md_content: String,
    _name: Option<String>,
    _tags: Option<String>,
    _modified_datetime: Option<NaiveDateTime>,
    _created_datetime: Option<NaiveDateTime>,
    ask_user: bool,
) -> DbPage {
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

    DbPage {
        filename: _filename,
        name: name,
        tags: tags,
        html_content: _html_content,
        md_content: _md_content,
        created_datetime: created_datetime,
        modified_datetime: modified_datetime,
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

pub async fn get_pages_from_db(pool: &Pool<Sqlite>) -> sqlx::Result<Vec<DbPage>> {
    let get_pages_status = sqlx::query_as!(DbPage, r#"SELECT 
                                                        filename,
                                                        name,
                                                        html_content,
                                                        md_content,
                                                        tags,
                                                        modified_datetime as "modified_datetime: NaiveDateTime",
                                                        created_datetime as "created_datetime: NaiveDateTime"
                                                    FROM pages"#).fetch_all(pool).await?;
    Ok(get_pages_status)
}

// iterate over the files at md_path.
// for each file, as well as the file's corresponding entry in the database,
// this function determines what page info should make it to the database.
pub fn process_md_dir(md_path: &Path, pages_from_db: Vec<&DbPage>) -> Result<Vec<DbPage>> {
    // todo!("Process pages in db that aren't in folder (drop this file?)");
    let md_location_prefix = Path::new("./content/md/");
    let mut pages: Vec<DbPage> = Vec::new();

    let pages_from_db_hashmap = pages_to_hashmap(pages_from_db);

    let include_ext = std::env::var("FILENAME_INCLUDE_EXTENSION")
        .map(|v| v == "true")
        .unwrap_or(false);

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

        let relative_path = entry
            .path()
            .strip_prefix(md_location_prefix)
            .unwrap_or(entry.path());

        // todo: add config options to change how filename is generated
        const filename_include_expression: bool = false;
        let filename: String = if include_ext {
            relative_path.to_string_lossy().to_string()
        } else {
            relative_path
                .with_extension("")
                .to_string_lossy()
                .to_string()
        };

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

        // transform markdown to html (for me, astro should take care of this, but what if the front-end
        // doesn't)
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
        let file: DbPage = new_page(
            filename.to_string(),
            html_content,
            md_content,
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

pub fn pages_to_hashmap(pages: Vec<&DbPage>) -> HashMap<&String, DbPage> {
    let mut h: HashMap<&String, DbPage> = HashMap::new();
    for page in pages {
        h.insert(&page.filename, page.clone());
    }
    h
}

pub async fn insert_from_vec_pages(
    pool: &Pool<Sqlite>,
    files_pages: Vec<&DbPage>,
    db_pages: Vec<&DbPage>,
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
                            md_content = ?,
                            tags = ?,
                            modified_datetime = ?,
                            created_datetime = ?
                        WHERE filename = ?
                        "#,
                        file_page.name,
                        file_page.html_content,
                        file_page.md_content,
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
                    md_content,
                    tags,
                    modified_datetime,
                    created_datetime
                )
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
                    file_page.filename,
                    file_page.name,
                    file_page.html_content,
                    file_page.md_content,
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

enum MetadataDateTimeOptions {
    Modified,
    Created,
}

fn get_property_from_metadata(
    metadata: &fs::Metadata,
    options: &MetadataDateTimeOptions,
) -> Result<NaiveDateTime> {
    let systime = match options {
        MetadataDateTimeOptions::Modified => metadata.modified(),
        MetadataDateTimeOptions::Created => metadata.created(),
    };

    let cleaned_systime = match systime {
        Ok(val) => val,
        Err(e) => return Err(anyhow!("Failed to get time from metadata: {}", e)),
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
