use crate::ChasquiConfig;
use crate::features::pages::model::{DbOperationReport, DbPage};
use anyhow::{Result, anyhow};
use gray_matter::{Matter, engine::YAML};
use pulldown_cmark::{Event, Options as CmarkOptions, Parser, Tag, html};
use serde::Deserialize;
use sqlx::types::chrono::NaiveDateTime;
use sqlx::{Pool, Sqlite};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{env, fs};
use walkdir::WalkDir;
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

pub async fn get_entry_by_filename(
    filename: &str,
    pool: &Pool<Sqlite>,
) -> sqlx::Result<Option<DbPage>> {
    sqlx::query_as::<_, DbPage>(
        r#"
        SELECT * FROM pages WHERE filename = ?
        "#,
    )
    .bind(filename)
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

pub fn build_valid_files_set(content_dir: &Path) -> HashSet<String> {
    let mut valid_files = HashSet::new();

    // we only care about successful reads, filter_map over Ok()
    for entry in WalkDir::new(content_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file()
            && entry.path().extension().and_then(|s| s.to_str()) == Some("md")
        {
            if let Ok(relative) = entry.path().strip_prefix(content_dir) {
                // normalize to forward slashes for cross-platform consistency
                let normalized = relative.to_string_lossy().replace("\\", "/");
                valid_files.insert(normalized);
            }
        }
    }
    valid_files
}

pub fn process_md_dir(
    md_path: &Path,
    pages_from_db: Vec<&DbPage>,
    config: &ChasquiConfig,
) -> Result<Vec<(DbPage, DbOperationReport)>> {
    let mut page_operations: Vec<(DbPage, DbOperationReport)> = Vec::new();
    let db_pages_map = pages_to_hashmap(pages_from_db);

    // build the set of valid files
    let valid_files = build_valid_files_set(md_path);

    for result_entry in WalkDir::new(md_path) {
        let entry = match result_entry {
            Ok(val) => val,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        // We use config.content_dir to safely strip the prefix
        let relative_path = entry
            .path()
            .strip_prefix(&config.content_dir)
            .unwrap_or(entry.path());
        let filename = relative_path.to_string_lossy().to_string();

        let db_page_opt = db_pages_map.get(&filename).cloned();

        // 3. Pass the config and the valid_files set into the single file processor
        match process_single_file(entry.path(), db_page_opt, config, &valid_files) {
            Ok(page_report) => {
                page_operations.push(page_report);
            }
            Err(e) => {
                eprintln!("Error occurred processing page {}: {}", filename, e);
            }
        };
    }

    Ok(page_operations)
}

// process a directory entry, identify if it's a page, and identify necessary action
// additionally, report which db operation is appropriate (single responsibility)
// returns error if unable to read file, unable to process frontmatter, or any links to other pages are broken
//  TODO: break this function down! this is huge
pub fn process_single_file(
    path: &Path,
    db_page_opt: Option<DbPage>,
    config: &ChasquiConfig,
    valid_files: &HashSet<String>,
) -> Result<(DbPage, DbOperationReport)> {
    // 1. Read file from disk
    let md_content = fs::read_to_string(path)
        .map_err(|e| anyhow!("Unable to read file {}: {}", path.display(), e))?;

    // 2. Resolve relative path safely using config
    let relative_path = path.strip_prefix(&config.content_dir).unwrap_or(path);
    let filename = relative_path.to_string_lossy().to_string();

    // 3. Extract OS metadata
    let metadata_result = fs::metadata(path);
    let os_modified =
        get_property_from_metadata(&metadata_result, &MetadataDateTimeOptions::Modified).ok();
    let os_created =
        get_property_from_metadata(&metadata_result, &MetadataDateTimeOptions::Created).ok();

    // 4. Pass ingredients to the pure core
    parse_markdown_to_db_page(
        &filename,
        &md_content,
        os_modified,
        os_created,
        db_page_opt,
        config,
        valid_files,
    )
}

// extracts YAML frontmatter and returns the typed metadata alongside the raw markdown body
fn extract_frontmatter(md_content: &str, filename: &str) -> Result<(PageFrontMatter, String)> {
    let matter = Matter::<YAML>::new();

    // explicitly tell 'parse' with epic turbofish syntax to use our PageFrontMatter struct for <D>
    let parsed_matter = matter
        .parse::<PageFrontMatter>(md_content)
        .map_err(|e| anyhow!("Failed to parse frontmatter in {}: {}", filename, e))?;

    let frontmatter = parsed_matter.data.unwrap_or_default();

    Ok((frontmatter, parsed_matter.content))
}

// compiles markdown content into HTML, explicitly validating and rewriting internal links
// if a link is broken, compilation immediately halts and returns an Error
fn compile_markdown_to_html(
    current_file_path: &Path,
    filename: &str,
    markdown_content: &str,
    valid_files: &HashSet<String>,
) -> Result<String> {
    let mut options = CmarkOptions::empty();
    options.insert(CmarkOptions::ENABLE_STRIKETHROUGH);
    options.insert(CmarkOptions::ENABLE_TABLES);

    let parser = Parser::new_ext(markdown_content, options);
    let mut rewrote_events = Vec::new();

    // iterate over the event stream
    for event in parser {
        match event {
            // is this the start of a link?
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let dest_str = dest_url.to_string();

                // pass the link the validator
                match validate_and_rewrite_link(current_file_path, &dest_str, valid_files) {
                    Ok(new_dest) => {
                        // take the link the validator gave back and push it in place of the old
                        rewrote_events.push(Event::Start(Tag::Link {
                            link_type,
                            dest_url: new_dest.into(),
                            title,
                            id,
                        }));
                    }
                    Err(e) => {
                        // woah, this internal link is invalid.
                        // we don't want to push this page.
                        // immediately abort the entire function and return the error.
                        return Err(anyhow!("In {}: {}", filename, e));
                    }
                }
            }
            // all other events pass through untouched
            _ => rewrote_events.push(event),
        }
    }

    let mut html_content = String::new();
    html::push_html(&mut html_content, rewrote_events.into_iter());

    Ok(html_content)
}

pub fn parse_markdown_to_db_page(
    filename: &str,
    md_content: &str,
    os_modified: Option<NaiveDateTime>,
    os_created: Option<NaiveDateTime>,
    db_page_opt: Option<DbPage>,
    config: &ChasquiConfig,
    valid_files: &HashSet<String>,
) -> Result<(DbPage, DbOperationReport)> {
    // hash content and early exit if md content hash is the same
    let file_md_content_hash = format!("{:016x}", xxh3_64(md_content.as_bytes()));
    if let Some(db_page) = &db_page_opt {
        if db_page.md_content_hash == file_md_content_hash {
            return Ok((db_page.clone(), DbOperationReport::NoChange));
        }
    }

    // extract frontmatter
    let (frontmatter, content_body) = extract_frontmatter(md_content, filename)?;

    // resolve identifier
    let default_identifier = if config.strip_extensions {
        Path::new(filename)
            .with_extension("")
            .to_string_lossy()
            .to_string()
    } else {
        filename.to_string()
    };
    let identifier = frontmatter.identifier.unwrap_or(default_identifier);

    // resolve dates with OS metadata
    let final_modified_datetime = resolve_datetime(frontmatter.modified_datetime, os_modified);
    let final_created_datetime = resolve_datetime(frontmatter.created_datetime, os_created);

    // setup tags and names
    let name = frontmatter.name;
    let tags = frontmatter
        .tags
        .map(|t| serde_json::to_string(&t).unwrap_or_default());

    // AST -> HTML
    let html_content =
        compile_markdown_to_html(Path::new(filename), filename, &content_body, valid_files)?;

    // 7. Package for Database
    let operation = if db_page_opt.is_some() {
        DbOperationReport::Update
    } else {
        DbOperationReport::Insert
    };

    let new_page = DbPage {
        identifier,
        filename: filename.to_string(),
        name,
        html_content,
        md_content: content_body,
        md_content_hash: file_md_content_hash,
        tags,
        modified_datetime: final_modified_datetime,
        created_datetime: final_created_datetime,
    };

    Ok((new_page, operation))
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

fn validate_and_rewrite_link(
    current_file_path: &Path,
    dest: &str,
    valid_files: &HashSet<String>,
) -> Result<String> {
    // ignore external links and anchor links
    if dest.starts_with("http://")
        || dest.starts_with("https://")
        || dest.starts_with("mailto:")
        || dest.starts_with('#')
    {
        return Ok(dest.to_string());
    }

    // strip any query parameters or fragments (e.g., index.md#section -> index.md)
    let path_part = dest.split('#').next().unwrap_or(dest);
    let path_part = path_part.split('?').next().unwrap_or(path_part);

    // resolve the path mathematically in memory using 'lexical' joining
    let mut target_md_path = if path_part.starts_with('/') {
        PathBuf::from(path_part.trim_start_matches('/'))
    } else {
        let parent_dir = current_file_path.parent().unwrap_or_else(|| Path::new("")); // If no parent, it's at the root
        parent_dir.join(path_part)
    };

    // handle extensions
    if target_md_path.extension().and_then(|e| e.to_str()) == Some("html")
        || target_md_path.extension().is_none()
    {
        target_md_path.set_extension("md");
    }

    // clean the path to handle `../` mathematically (e.g., "folder/../index.md" -> "index.md")
    // we use a small helper here to parse the components without hitting the disk
    let normalized_path = normalize_path_lexically(&target_md_path);
    let normalized_string = normalized_path.to_string_lossy().replace("\\", "/");

    if !valid_files.contains(&normalized_string) {
        return Err(anyhow!(
            "Broken internal link: '{}' resolves to '{}', which does not exist.",
            dest,
            normalized_string
        ));
    }

    // convert the file path to a root-relative web URL
    let web_url = normalized_path
        .with_extension("")
        .to_string_lossy()
        .to_string()
        .replace("\\", "/");

    // astro explicitly treats undefined as our root "/".
    if web_url == "index" {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", web_url))
    }
}

// helper to mathematically resolve `.` and `..` without touching the filesystem
fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::Normal(c) => components.push(c),
            _ => components.push(component.as_os_str()),
        }
    }
    components.into_iter().collect()
}

enum MetadataDateTimeOptions {
    Modified,
    Created,
}

fn get_property_from_metadata(
    metadata_result: &std::io::Result<fs::Metadata>,
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
