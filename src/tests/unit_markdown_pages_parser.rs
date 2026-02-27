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
    let (fm, body) =
        extract_frontmatter(input, "test.md").expect("Should handle missing frontmatter");

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
    let result =
        compile_markdown_to_html(input, |link| link.to_string()).expect("Should compile markdown");

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
    let markdown_with_link = "Check out [my post](post.md)";

    // simulate a resolver that "knows" about our pages and turns .md files into slugs
    let result = compile_markdown_to_html(markdown_with_link, |link| {
        if link.ends_with(".md") {
            format!("/{}", link.replace(".md", ""))
        } else {
            link.to_string()
        }
    })
    .expect("Should compile");

    // assert that [my post](post.md) became <a href="/post">my post</a>
    assert!(result.contains(r#"<a href="/post">my post</a>"#));
}

#[test]
fn test_extract_frontmatter_malformed_fields() {
    // YAML syntax is OK, but types/values are invalid for our struct
    let broken_tags_md = "---\ntags: ['broken!]\n---\nhello world!";
    let (fm, body) = extract_frontmatter(broken_tags_md, "test.md").expect("Should not crash on malformed YAML");

    // When gray_matter fails to deserialize into PageFrontMatter, we currently return PageFrontMatter::default()
    assert!(fm.tags.is_none());
    assert_eq!(body.trim(), "hello world!");
}

#[test]
fn test_parsing_malformed_frontmatter() {
    // Case 1: Unclosed Frontmatter
    let unclosed_frontmatter_md = "---\nunfinished frontmatter";
    let (fm, body) = extract_frontmatter(unclosed_frontmatter_md, "test.md").unwrap();
    assert!(fm.identifier.is_none());
    // Should return unchanged content because it couldn't find a pair of delimiters
    assert_eq!(body, unclosed_frontmatter_md);

    // Case 2: Gibberish Frontmatter
    let expected_body = "Hello world!";
    let gibberish_frontmatter_md = format!("---\n::br()k=n y@ml: :;;\n---\n{}", &expected_body);
    let (fm, body) = extract_frontmatter(&gibberish_frontmatter_md, "test.md").unwrap();
    assert!(fm.identifier.is_none());
    assert_eq!(body.trim(), expected_body);

    // Case 3: Empty Frontmatter
    let empty_frontmatter_md = format!("---\n---\n{}", &expected_body);
    let (fm, body) = extract_frontmatter(&empty_frontmatter_md, "test.md").unwrap();
    assert!(fm.identifier.is_none());
    assert_eq!(body.trim(), expected_body);

    // Case 4: Huge Frontmatter (1MB)
    let huge_content = "a".repeat(1024 * 1024);
    let huge_frontmatter_md = format!("---\n{}\n---\n{}", huge_content, &expected_body);
    let (fm, body) = extract_frontmatter(&huge_frontmatter_md, "test.md").unwrap();
    assert!(fm.identifier.is_none());
    assert_eq!(body.trim(), expected_body);
}
