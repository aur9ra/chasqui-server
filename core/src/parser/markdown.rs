use crate::parser::model::PageFrontMatter;
use anyhow::Result;
use gray_matter::{engine::YAML, Matter};
use pulldown_cmark::{Event, Options as CmarkOptions, Parser, Tag};
use pulldown_cmark_to_cmark::cmark;
use std::collections::HashMap;

pub fn is_external_url(url: &str) -> bool {
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("mailto:")
        || url.starts_with("//")
}

pub fn apply_nginx_prefix(url: &str, enable_prefixes: bool) -> String {
    if !enable_prefixes || is_external_url(url) {
        return url.to_string();
    }

    let extension = url.rsplit('.').next().map(|s| s.to_lowercase());

    let prefix_map = get_media_nginx_prefix_map();

    if let Some(ext) = extension {
        if let Some(prefix) = prefix_map.get(ext.as_str()) {
            let clean_url = url.trim_start_matches('/');
            return format!("{}{}", prefix, clean_url);
        }
    }

    url.to_string()
}

fn get_media_nginx_prefix_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();

    for ext in ["mp4", "mov", "webm", "mkv", "ogv", "avi"] {
        map.insert(ext, "/videos/");
    }

    for ext in ["mp3", "wav", "ogg", "flac", "m4a", "aac", "opus"] {
        map.insert(ext, "/audio/");
    }

    for ext in [
        "jpg", "jpeg", "png", "webp", "gif", "heic", "svg", "ico", "tiff", "bmp",
    ] {
        map.insert(ext, "/images/");
    }

    map
}

pub fn extract_frontmatter(md_content: &str, filename: &str) -> Result<(PageFrontMatter, String)> {
    if !md_content.starts_with("---") {
        return Ok((PageFrontMatter::default(), md_content.to_string()));
    }

    if let Some(end_offset) = md_content[3..].find("---") {
        let closing_start = end_offset + 3;
        let body_start = closing_start + 3;

        let frontmatter_block = &md_content[..body_start];
        let body_content = &md_content[body_start..];

        let matter = Matter::<YAML>::new();
        return match matter.parse::<PageFrontMatter>(frontmatter_block) {
            Ok(parsed) => Ok((
                parsed.data.unwrap_or_default(),
                body_content.trim_start().to_string(),
            )),
            Err(e) => {
                eprintln!(
                    "Warning: Malformed YAML frontmatter in {}. Using defaults. Error: {}",
                    filename, e
                );
                Ok((
                    PageFrontMatter::default(),
                    body_content.trim_start().to_string(),
                ))
            }
        };
    }

    Ok((PageFrontMatter::default(), md_content.to_string()))
}

pub fn precompile_markdown<F>(
    markdown_content: &str,
    mut resolver: F,
    nginx_media_prefixes: bool,
) -> Result<String>
where
    F: FnMut(&str) -> String,
{
    let mut options = CmarkOptions::empty();
    options.insert(CmarkOptions::ENABLE_STRIKETHROUGH);
    options.insert(CmarkOptions::ENABLE_TABLES);

    let parser = Parser::new_ext(markdown_content, options);

    let mut events: Vec<Event> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let resolved_url = resolver(&dest_url);
                let prefixed_url = apply_nginx_prefix(&resolved_url, nginx_media_prefixes);
                events.push(Event::Start(Tag::Link {
                    link_type,
                    dest_url: prefixed_url.into(),
                    title,
                    id,
                }));
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let resolved_url = resolver(&dest_url);
                let prefixed_url = apply_nginx_prefix(&resolved_url, nginx_media_prefixes);
                events.push(Event::Start(Tag::Image {
                    link_type,
                    dest_url: prefixed_url.into(),
                    title,
                    id,
                }));
            }
            _ => events.push(event),
        }
    }

    let mut output = String::new();
    cmark(events.into_iter(), &mut output)?;

    Ok(output)
}