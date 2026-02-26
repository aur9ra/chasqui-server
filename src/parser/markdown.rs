use crate::parser::model::PageFrontMatter;
use anyhow::{Result, anyhow};
use gray_matter::{Matter, engine::YAML};
use pulldown_cmark::{Event, Options as CmarkOptions, Parser, Tag, html};

// extracts YAML frontmatter and returns the typed metadata alongside the raw markdown body
pub fn extract_frontmatter(md_content: &str, filename: &str) -> Result<(PageFrontMatter, String)> {
    let matter = Matter::<YAML>::new();

    // Soft Check: If it doesn't look like it has frontmatter delimiters, don't even try to parse
    if !md_content.starts_with("---") {
        return Ok((PageFrontMatter::default(), md_content.to_string()));
    }

    // explicitly tell 'parse' with epic turbofish syntax to use our PageFrontMatter struct for <D>
    let parsed_matter = match matter.parse::<PageFrontMatter>(md_content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Warning: Malformed YAML frontmatter in {}. Using defaults. Error: {}", filename, e);
            // If parsing fails, we need to manually find where the frontmatter ends to return the content
            // or just return the whole thing if we can't be sure.
            // gray_matter's failure usually means the '---' blocks were there but the content was garbage.
            return Ok((PageFrontMatter::default(), md_content.to_string()));
        }
    };

    let frontmatter = parsed_matter.data.unwrap_or_default();

    Ok((frontmatter, parsed_matter.content))
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
