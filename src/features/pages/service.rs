use crate::config::ChasquiConfig;
use crate::features::pages::model::Page;
use crate::io::path_utils::{normalize_path, sanitize_identifier};
use crate::io::ContentReader;
use crate::parser::markdown::{extract_frontmatter, precompile_markdown};
use crate::services::sync::manifest::Manifest;
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use std::path::Path;

impl Page {
    pub fn resolve_identity(
        relative_path: &Path,
        bytes: &[u8],
        config: &ChasquiConfig,
    ) -> Result<String> {
        let raw_content = String::from_utf8(bytes.to_vec()).context("Invalid UTF-8 in Page")?;
        let filename = normalize_path(relative_path);
        let (fm, _) = extract_frontmatter(&raw_content, &filename)?;
        let id = fm.identifier.unwrap_or_else(|| {
            generate_default_identifier(relative_path, config.page_strip_extension)
        });
        Ok(sanitize_identifier(&id))
    }

    pub async fn new_from_file(
        path: &Path,
        config: &ChasquiConfig,
        reader: &dyn ContentReader,
        manifest: &Manifest,
    ) -> Result<Self> {
        let relative_path = path
            .strip_prefix(&config.pages_dir)
            .or_else(|_| path.strip_prefix(&config.pages_dir.parent().unwrap_or(&config.pages_dir)))
            .map_err(|_| anyhow::anyhow!("File {} is outside of pages dir", path.display()))?;

        let filename = normalize_path(path.strip_prefix(&config.pages_dir).unwrap_or(path));

        let raw_markdown = reader.read_to_string(path).await?;
        let metadata = reader.get_metadata(path).await?;

        let (frontmatter, content_body) = extract_frontmatter(&raw_markdown, &filename)?;

        let identifier = frontmatter
            .identifier
            .map(|id| sanitize_identifier(&id))
            .unwrap_or_else(|| {
                sanitize_identifier(&generate_default_identifier(
                    relative_path,
                    config.page_strip_extension,
                ))
            });

        let content_hash = format!(
            "{:016x}",
            xxhash_rust::xxh3::xxh3_64(raw_markdown.as_bytes())
        );

        let md_content = precompile_markdown(
            &content_body,
            |link| manifest.resolve_link(link, Path::new(&filename), config),
            config.nginx_media_prefixes,
        )?;

        let modified_datetime = resolve_datetime(frontmatter.modified_datetime, metadata.modified);
        let created_datetime = resolve_datetime(frontmatter.created_datetime, metadata.created);

        Ok(Page {
            identifier,
            filename,
            name: frontmatter.name,
            md_content,
            content_hash,
            tags: frontmatter.tags.unwrap_or_default(),
            modified_datetime,
            created_datetime,
            file_path: path.to_path_buf(),
            new_path: None,
        })
    }
}

fn generate_default_identifier(relative_path: &Path, strip_extension: bool) -> String {
    let path_str = if strip_extension {
        relative_path
            .with_extension("")
            .to_string_lossy()
            .to_string()
    } else {
        relative_path.to_string_lossy().to_string()
    };

    path_str.replace("\\", "/")
}

fn resolve_datetime(
    frontmatter_date: Option<String>,
    os_date: Option<NaiveDateTime>,
) -> Option<NaiveDateTime> {
    if let Some(date_str) = frontmatter_date {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&date_str) {
            return Some(dt.naive_utc());
        }
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
            return Some(dt.and_hms_opt(0, 0, 0).unwrap_or_default());
        }
    }
    os_date
}