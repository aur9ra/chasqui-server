use crate::parser::markdown::{compile_markdown_to_html, extract_frontmatter};

// test that the system can properly pull out YAML frontmatter from a markdown file
// frontmatter is the "identity" of the page, where the writer defines things like tags and slugs
#[test]
fn test_extract_frontmatter_valid() {
    // a mock file content with yaml at the top
    let input = "---\nidentifier: test-id\ntags:\n  - rust\n  - tests\n---\n# Hello World";
    // try to pull the metadata and the body apart
    let (fm, body) = extract_frontmatter(input, "test.md").expect("Should parse valid frontmatter");

    // check if the metadata matches what we wrote
    assert_eq!(fm.identifier, Some("test-id".to_string()));
    assert_eq!(fm.tags, Some(vec!["rust".to_string(), "tests".to_string()]));
    // check if the body is still intact
    assert_eq!(body.trim(), "# Hello World");
}

// ensure the system doesn't crash if a writer forgets the frontmatter
// in this case, the system should just see it as a plain markdown file
#[test]
fn test_extract_frontmatter_no_fm() {
    let input = "# Just Content";
    let (fm, body) = extract_frontmatter(input, "test.md").expect("Should handle missing frontmatter");

    // metadata should be empty
    assert!(fm.identifier.is_none());
    // body should be exactly what we put in
    assert_eq!(body.trim(), "# Just Content");
}

// test the actual markdown -> html compilation
// this is the "visual" part of the engine that prepares the page for the browser
#[test]
fn test_compile_markdown_basic() {
    let input = "# Title\nThis is a [link](test.md)";
    
    // compile it! the resolver just returns the link as-is for this simple test
    let result = compile_markdown_to_html(input, |link| link.to_string())
        .expect("Should compile markdown");

    // assert that markdown headers became html h1 tags
    assert!(result.contains("<h1>Title</h1>"));
    // assert that markdown links became html anchor tags
    assert!(result.contains(r#"<a href="test.md">link</a>"#));
}

// test the "Link Resolver" logic
// this is a critical feature: it allows writers to use file names in their markdown,
// and the system "fixes" them on-the-fly to use the public slug/identifier
#[test]
fn test_compile_markdown_with_resolver() {
    let input = "Check out [my post](post.md)";
    
    // simulate a resolver that "knows" about our pages and turns .md files into slugs
    let result = compile_markdown_to_html(input, |link| {
        if link.ends_with(".md") {
            format!("/{}", link.replace(".md", ""))
        } else {
            link.to_string()
        }
    }).expect("Should compile");

    // assert that [my post](post.md) became <a href="/post">my post</a>
    assert!(result.contains(r#"<a href="/post">my post</a>"#));
}
