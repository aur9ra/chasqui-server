use crate::parser::model::PageFrontMatter;
use anyhow::Result;
use gray_matter::{Matter, engine::YAML};
use pulldown_cmark::{Event, Options as CmarkOptions, Parser, Tag, TagEnd, html};

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
pub fn compile_markdown_to_html<F>(markdown_content: &str, mut resolver: F) -> Result<String>
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
                let new_url = resolver(&dest_url);
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
                    events.push(Event::Html(
                        format!(
                            r#"<video controls aria-label="{}"><source src="{}" type="video/mp4">Your browser does not support the video tag.</video>"#,
                            alt, new_url
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
                    events.push(Event::Html(
                        format!(
                            r#"<audio controls aria-label="{}"><source src="{}" type="audio/mpeg">Your browser does not support the audio tag.</audio>"#,
                            alt, new_url
                        )
                        .into(),
                    ));
                } else {
                    events.push(Event::Start(Tag::Image {
                        link_type,
                        dest_url: new_url.into(),
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
