use crate::parser::model::PageFrontMatter;
use anyhow::{Result, anyhow};
use gray_matter::{Matter, engine::YAML};
use pulldown_cmark::{Event, Options as CmarkOptions, Parser, Tag, html};

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

// compiles markdown content into HTML, and resolves links on-the-fly using the provided resolver
pub fn compile_markdown_to_html<F>(markdown_content: &str, mut resolver: F) -> Result<String>
where
    F: FnMut(&str) -> Option<String>,
{
    let mut options = CmarkOptions::empty();
    options.insert(CmarkOptions::ENABLE_STRIKETHROUGH);
    options.insert(CmarkOptions::ENABLE_TABLES);

    let parser = Parser::new_ext(markdown_content, options);

    let mut html_content = String::new();
    let mut broken_links = Vec::new();

    // parse AST -> for link
    let event_iterator = parser.map(|event| {
        if let Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) = event
        {
            match resolver(&dest_url) {
                Some(new_url) => Event::Start(Tag::Link {
                    link_type,
                    dest_url: new_url.into(),
                    title,
                    id,
                }),
                None => {
                    broken_links.push(dest_url.to_string());
                    Event::Start(Tag::Link {
                        link_type,
                        dest_url,
                        title,
                        id,
                    })
                }
            }
        } else {
            event
        }
    });

    html::push_html(&mut html_content, event_iterator);

    if !broken_links.is_empty() {
        return Err(anyhow!(
            "Broken internal links detected: {}",
            broken_links.join(", ")
        ));
    }

    Ok(html_content)
}
