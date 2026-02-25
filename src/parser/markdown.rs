use crate::parser::model::PageFrontMatter;
use anyhow::{anyhow, Result};
use gray_matter::{engine::YAML, Matter};
use pulldown_cmark::{html, Event, Options as CmarkOptions, Parser, Tag};

// extracts YAML frontmatter and returns the typed metadata alongside the raw markdown body
pub fn extract_frontmatter(md_content: &str, filename: &str) -> Result<(PageFrontMatter, String)> {
    let matter = Matter::<YAML>::new();

    // explicitly tell 'parse' with epic turbofish syntax to use our PageFrontMatter struct for <D>
    let parsed_matter = matter
        .parse::<PageFrontMatter>(md_content)
        .map_err(|e| anyhow!("Failed to parse frontmatter in {}: {}", filename, e))?;

    let frontmatter = parsed_matter.data.unwrap_or_default();

    Ok((frontmatter, parsed_matter.content))
}

// compiles markdown content into HTML, and returns a list of all found links
pub fn compile_markdown_to_html(markdown_content: &str) -> Result<(String, Vec<String>)> {
    let mut options = CmarkOptions::empty();
    options.insert(CmarkOptions::ENABLE_STRIKETHROUGH);
    options.insert(CmarkOptions::ENABLE_TABLES);

    let parser = Parser::new_ext(markdown_content, options);

    let mut html_content = String::new();
    let mut extracted_links = Vec::new();

    let event_iterator = parser.map(|event| {
        if let Event::Start(Tag::Link { dest_url, .. }) = &event {
            extracted_links.push(dest_url.to_string());
        }
        event
    });

    html::push_html(&mut html_content, event_iterator);

    Ok((html_content, extracted_links))
}
