use crate::parser::model::PageFrontMatter;
use anyhow::Result;
use gray_matter::{engine::YAML, Matter};
use pulldown_cmark::{html, Event, Options as CmarkOptions, Parser, Tag, TagEnd};
use std::collections::HashMap;

fn get_media_nginx_prefix_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();

    // video extensions
    for ext in ["mp4", "mov", "webm", "mkv", "ogv", "avi"] {
        map.insert(ext, "/videos/");
    }

    // audio extensions
    for ext in ["mp3", "wav", "ogg", "flac", "m4a", "aac", "opus"] {
        map.insert(ext, "/audio/");
    }

    // umage extensions
    for ext in [
        "jpg", "jpeg", "png", "webp", "gif", "heic", "svg", "ico", "tiff", "bmp",
    ] {
        map.insert(ext, "/images/");
    }

    map
}

pub fn is_external_url(url: &str) -> bool {
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("mailto:")
        || url.starts_with("//")
    // is this exhaustive?
    // this feels awful
}

// applies NGINX media prefix based on file extension and config
// only applies to internal media urls (e.g. not mailto:..., https://...)
pub fn apply_nginx_prefix(url: &str, enable_prefixes: bool) -> String {
    if !enable_prefixes || is_external_url(url) {
        return url.to_string();
    }

    // get filename after last dot
    let extension = url.rsplit('.').next().map(|s| s.to_lowercase());

    let prefix_map = get_media_nginx_prefix_map();

    if let Some(ext) = extension {
        if let Some(prefix) = prefix_map.get(ext.as_str()) {
            // strip leading slash
            let clean_url = url.trim_start_matches('/');
            return format!("{}{}", prefix, clean_url);
        }
    }

    // if there isn't a recognized extension, return as-is
    url.to_string()
}

// extracts YAML frontmatter and returns the typed metadata alongside the raw markdown body
pub fn extract_frontmatter(md_content: &str, filename: &str) -> Result<(PageFrontMatter, String)> {
    // Soft Check: If it doesn't start with delimiters, it's just content
    if !md_content.starts_with("---") {
        return Ok((PageFrontMatter::default(), md_content.to_string()));
    }

    // Find the closing delimiter. We search after the first "---" (3 chars)
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

    // No closing "---" found: treat the whole thing as content
    Ok((PageFrontMatter::default(), md_content.to_string()))
}

// compiles markdown content into HTML, and resolves links on-the-fly using the provided resolver
pub fn compile_markdown_to_html<F>(
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
    // enable GFM markdown

    let parser = Parser::new_ext(markdown_content, options);

    let mut html_content = String::new();
    let mut events = Vec::new();
    let mut iter = parser.into_iter();

    while let Some(event) = iter.next() {
        match event {
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let resolved_url = resolver(&dest_url);
                let lower_url = dest_url.to_lowercase();

                if lower_url.ends_with(".mp4") || lower_url.ends_with(".mov") {
                    // Extract alt text from subsequent events
                    let mut alt = String::new();
                    while let Some(next_event) = iter.next() {
                        match next_event {
                            Event::End(TagEnd::Image) => break,
                            Event::Text(t) => alt.push_str(&t),
                            _ => {}
                        }
                    }
                    let prefixed_url = apply_nginx_prefix(&resolved_url, nginx_media_prefixes);
                    events.push(Event::Html(
                        format!(
                            r#"<video controls aria-label="{}"><source src="{}" type="video/mp4">Your browser does not support the video tag.</video>"#,
                            alt, prefixed_url
                        )
                        .into(),
                    ));
                } else if lower_url.ends_with(".mp3")
                    || lower_url.ends_with(".wav")
                    || lower_url.ends_with(".ogg")
                {
                    // Extract alt text
                    let mut alt = String::new();
                    while let Some(next_event) = iter.next() {
                        match next_event {
                            Event::End(TagEnd::Image) => break,
                            Event::Text(t) => alt.push_str(&t),
                            _ => {}
                        }
                    }
                    let prefixed_url = apply_nginx_prefix(&resolved_url, nginx_media_prefixes);
                    events.push(Event::Html(
                        format!(
                            r#"<audio controls aria-label="{}"><source src="{}" type="audio/mpeg">Your browser does not support the audio tag.</audio>"#,
                            alt, prefixed_url
                        )
                        .into(),
                    ));
                } else {
                    let prefixed_url = apply_nginx_prefix(&resolved_url, nginx_media_prefixes);
                    events.push(Event::Start(Tag::Image {
                        link_type,
                        dest_url: prefixed_url.into(),
                        title,
                        id,
                    }));
                }
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let new_url = resolver(&dest_url);
                events.push(Event::Start(Tag::Link {
                    link_type,
                    dest_url: new_url.into(),
                    title,
                    id,
                }));
            }
            _ => events.push(event),
        }
    }

    html::push_html(&mut html_content, events.into_iter());

    Ok(html_content)
}
