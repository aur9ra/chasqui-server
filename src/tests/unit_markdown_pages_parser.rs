use crate::parser::markdown::{
    apply_nginx_prefix, compile_markdown_to_html, extract_frontmatter, is_external_url,
};

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
    let result = compile_markdown_to_html(input, |link| link.to_string(), false)
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
    let markdown_with_link = "Check out [my post](post.md)";

    // simulate a resolver that "knows" about our pages and turns .md files into slugs
    let result = compile_markdown_to_html(
        markdown_with_link,
        |link| {
            if link.ends_with(".md") {
                format!("/{}", link.replace(".md", ""))
            } else {
                link.to_string()
            }
        },
        false,
    )
    .expect("Should compile");

    // assert that [my post](post.md) became <a href="/post">my post</a>
    assert!(result.contains(r#"<a href="/post">my post</a>"#));
}

#[test]
fn test_extract_frontmatter_malformed_fields() {
    // YAML syntax is OK, but types/values are invalid for our struct
    let broken_tags_md = "---\ntags: ['broken!]\n---\nhello world!";
    let (fm, body) =
        extract_frontmatter(broken_tags_md, "test.md").expect("Should not crash on malformed YAML");

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

#[test]
fn test_polymorphic_embedding_image() {
    let input = "![An image](photo.jpg)";
    let result =
        compile_markdown_to_html(input, |url| format!("/resolved/{}", url), false).unwrap();

    // Should render as standard <img>
    assert!(result.contains(r#"<img src="/resolved/photo.jpg" alt="An image" />"#));
}

#[test]
fn test_polymorphic_embedding_video() {
    let input = "![A cool video](demo.mp4)";
    let result =
        compile_markdown_to_html(input, |url| format!("/resolved/{}", url), false).unwrap();

    // Should render as <video> with aria-label
    assert!(result.contains(r#"<video controls aria-label="A cool video">"#));
    assert!(result.contains(r#"<source src="/resolved/demo.mp4" type="video/mp4">"#));
}

#[test]
fn test_polymorphic_embedding_audio() {
    let input = "![Sweet music](tune.mp3)";
    let result =
        compile_markdown_to_html(input, |url| format!("/resolved/{}", url), false).unwrap();

    // Should render as <audio> with aria-label
    assert!(result.contains(r#"<audio controls aria-label="Sweet music">"#));
    assert!(result.contains(r#"<source src="/resolved/tune.mp3" type="audio/mpeg">"#));
}

#[test]
fn test_polymorphic_embedding_case_insensitivity() {
    let input = "![Video](CLIP.MOV)";
    let result = compile_markdown_to_html(input, |url| url.to_string(), false).unwrap();

    assert!(result.contains("<video"));
    assert!(result.contains(r#"src="CLIP.MOV""#));
}

// --- Direct tests for apply_nginx_prefix helper ---

#[test]
fn test_apply_nginx_prefix_disabled() {
    // When disabled, URLs should pass through unchanged
    assert_eq!(apply_nginx_prefix("photo.jpg", false), "photo.jpg");
    assert_eq!(
        apply_nginx_prefix("/images/photo.jpg", false),
        "/images/photo.jpg"
    );
    assert_eq!(
        apply_nginx_prefix("https://example.com/img.jpg", false),
        "https://example.com/img.jpg"
    );
}

#[test]
fn test_apply_nginx_prefix_images() {
    // Images get /images/ prefix
    assert_eq!(apply_nginx_prefix("photo.jpg", true), "/images/photo.jpg");
    assert_eq!(apply_nginx_prefix("/photo.jpg", true), "/images/photo.jpg");
    assert_eq!(
        apply_nginx_prefix("subdir/photo.png", true),
        "/images/subdir/photo.png"
    );
    assert_eq!(apply_nginx_prefix("CAPS.JPG", true), "/images/CAPS.JPG");
}

#[test]
fn test_apply_nginx_prefix_videos() {
    // Videos get /videos/ prefix
    assert_eq!(apply_nginx_prefix("demo.mp4", true), "/videos/demo.mp4");
    assert_eq!(apply_nginx_prefix("/clip.mov", true), "/videos/clip.mov");
    assert_eq!(
        apply_nginx_prefix("media/movie.webm", true),
        "/videos/media/movie.webm"
    );
}

#[test]
fn test_apply_nginx_prefix_audio() {
    // Audio gets /audio/ prefix
    assert_eq!(apply_nginx_prefix("song.mp3", true), "/audio/song.mp3");
    assert_eq!(apply_nginx_prefix("/track.wav", true), "/audio/track.wav");
    assert_eq!(
        apply_nginx_prefix("music/album.ogg", true),
        "/audio/music/album.ogg"
    );
}

#[test]
fn test_apply_nginx_prefix_external_urls_skipped() {
    // External URLs should never get prefixed
    let external_urls = [
        "http://example.com/img.jpg",
        "https://cdn.example.com/video.mp4",
        "mailto:test@example.com",
        "//cdn.example.com/audio.mp3",
    ];

    for url in external_urls {
        assert_eq!(
            apply_nginx_prefix(url, true),
            url,
            "External URL {} should not be prefixed",
            url
        );
    }
}

#[test]
fn test_apply_nginx_prefix_unknown_extensions() {
    // Unknown extensions pass through unchanged
    assert_eq!(apply_nginx_prefix("file.txt", true), "file.txt");
    assert_eq!(apply_nginx_prefix("script.js", true), "script.js");
    assert_eq!(apply_nginx_prefix("/data/file.csv", true), "/data/file.csv");
}

// --- Direct tests for is_external_url helper ---

#[test]
fn test_is_external_url() {
    // External URLs
    assert!(is_external_url("http://example.com"));
    assert!(is_external_url("https://example.com"));
    assert!(is_external_url("mailto:test@example.com"));
    assert!(is_external_url("//cdn.example.com"));

    // Internal URLs
    assert!(!is_external_url("/images/photo.jpg"));
    assert!(!is_external_url("photo.jpg"));
    assert!(!is_external_url("../assets/video.mp4"));
    assert!(!is_external_url("./relative/path.png"));
}
