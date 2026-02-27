use crate::parser::model::PageFrontMatter;
use anyhow::{Result, anyhow};
use gray_matter::{Matter, engine::YAML};
use pulldown_cmark::{Event, Options as CmarkOptions, Parser, Tag, html};

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
                Ok((PageFrontMatter::default(), body_content.trim_start().to_string()))
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

    let parser = Parser::new_ext(markdown_content, options);

    let mut html_content = String::new();

    // parse AST -> for link
    let event_iterator = parser.map(|event| {
        if let Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) = event
        {
            let new_url = resolver(&dest_url);
            Event::Start(Tag::Link {
                link_type,
                dest_url: new_url.into(),
                title,
                id,
            })
        } else {
            event
        }
    });

    html::push_html(&mut html_content, event_iterator);

    Ok(html_content)
}
