use crate::parser::markdown::{
    apply_nginx_prefix, extract_frontmatter, is_external_url, precompile_markdown,
};

#[test]
fn test_extract_frontmatter_valid() {
    let input = "---\nidentifier: test-id\ntags:\n  - rust\n  - tests\n---\n# Hello World";
    let (fm, body) = extract_frontmatter(input, "test.md").expect("Should parse valid frontmatter");

    assert_eq!(fm.identifier, Some("test-id".to_string()));
    assert_eq!(fm.tags, Some(vec!["rust".to_string(), "tests".to_string()]));
    assert_eq!(body.trim(), "# Hello World");
}

#[test]
fn test_extract_frontmatter_no_fm() {
    let input = "# Just Content";
    let (fm, body) =
        extract_frontmatter(input, "test.md").expect("Should handle missing frontmatter");

    assert!(fm.identifier.is_none());
    assert_eq!(body.trim(), "# Just Content");
}

#[test]
fn test_precompile_markdown_link_resolution() {
    let markdown_with_link = "Check out [my post](post.md)";

    let result = precompile_markdown(
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
    .expect("Should precompile");

    assert!(result.contains("[my post](/post)"));
}

#[test]
fn test_precompile_markdown_image_url_resolution() {
    let input = "![An image](photo.jpg)";
    let result = precompile_markdown(input, |url| format!("/resolved/{}", url), false).unwrap();

    assert!(result.contains("](/resolved/photo.jpg)"));
    assert!(result.contains("[An image]"));
}

#[test]
fn test_precompile_markdown_nginx_prefix_images() {
    let input = "![Photo](photo.jpg)";
    let result = precompile_markdown(input, |url| url.to_string(), true).unwrap();

    assert!(result.contains("/images/photo.jpg"));
}

#[test]
fn test_precompile_markdown_nginx_prefix_videos() {
    let input = "![Demo](demo.mp4)";
    let result = precompile_markdown(input, |url| url.to_string(), true).unwrap();

    assert!(result.contains("/videos/demo.mp4"));
}

#[test]
fn test_precompile_markdown_nginx_prefix_audio() {
    let input = "![Song](tune.mp3)";
    let result = precompile_markdown(input, |url| url.to_string(), true).unwrap();

    assert!(result.contains("/audio/tune.mp3"));
}

#[test]
fn test_precompile_markdown_nginx_prefix_external_urls_skipped() {
    for url in [
        "http://example.com/img.jpg",
        "https://cdn.example.com/video.mp4",
        "mailto:test@example.com",
        "//cdn.example.com/audio.mp3",
    ] {
        let input = format!("[Link]({})", url);
        let result = precompile_markdown(&input, |u| u.to_string(), true).unwrap();
        assert!(
            !result.contains("/images/")
                && !result.contains("/videos/")
                && !result.contains("/audio/"),
            "External URL {} should not be prefixed, got: {}",
            url,
            result
        );
    }
}

#[test]
fn test_precompile_markdown_nginx_prefix_disabled() {
    let input = "![Photo](photo.jpg)";
    let result = precompile_markdown(input, |url| url.to_string(), false).unwrap();

    assert!(result.contains("(photo.jpg)"));
    assert!(!result.contains("/images/"));
}

#[test]
fn test_precompile_markdown_preserves_structure() {
    let input = "# Title\n\nParagraph with **bold** and *italic*.\n\n- Item 1\n- Item 2";
    let result = precompile_markdown(input, |url| url.to_string(), false).unwrap();

    assert!(result.contains("# Title"));
    assert!(result.contains("bold"));
    assert!(result.contains("Item 1"));
}

#[test]
fn test_extract_frontmatter_malformed_fields() {
    let broken_tags_md = "---\ntags: ['broken!]\n---\nhello world!";
    let (fm, body) =
        extract_frontmatter(broken_tags_md, "test.md").expect("Should not crash on malformed YAML");

    assert!(fm.tags.is_none());
    assert_eq!(body.trim(), "hello world!");
}

#[test]
fn test_parsing_malformed_frontmatter() {
    let unclosed_frontmatter_md = "---\nunfinished frontmatter";
    let (fm, body) = extract_frontmatter(unclosed_frontmatter_md, "test.md").unwrap();
    assert!(fm.identifier.is_none());
    assert_eq!(body, unclosed_frontmatter_md);

    let expected_body = "Hello world!";
    let gibberish_frontmatter_md = format!("---\n::br()k=n y@ml: :;;\n---\n{}", &expected_body);
    let (fm, body) = extract_frontmatter(&gibberish_frontmatter_md, "test.md").unwrap();
    assert!(fm.identifier.is_none());
    assert_eq!(body.trim(), expected_body);

    let empty_frontmatter_md = format!("---\n---\n{}", &expected_body);
    let (fm, body) = extract_frontmatter(&empty_frontmatter_md, "test.md").unwrap();
    assert!(fm.identifier.is_none());
    assert_eq!(body.trim(), expected_body);

    let huge_content = "a".repeat(1024 * 1024);
    let huge_frontmatter_md = format!("---\n{}\n---\n{}", huge_content, &expected_body);
    let (fm, body) = extract_frontmatter(&huge_frontmatter_md, "test.md").unwrap();
    assert!(fm.identifier.is_none());
    assert_eq!(body.trim(), expected_body);
}

#[test]
fn test_apply_nginx_prefix_disabled() {
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
    assert_eq!(apply_nginx_prefix("demo.mp4", true), "/videos/demo.mp4");
    assert_eq!(apply_nginx_prefix("/clip.mov", true), "/videos/clip.mov");
    assert_eq!(
        apply_nginx_prefix("media/movie.webm", true),
        "/videos/media/movie.webm"
    );
}

#[test]
fn test_apply_nginx_prefix_audio() {
    assert_eq!(apply_nginx_prefix("song.mp3", true), "/audio/song.mp3");
    assert_eq!(apply_nginx_prefix("/track.wav", true), "/audio/track.wav");
    assert_eq!(
        apply_nginx_prefix("music/album.ogg", true),
        "/audio/music/album.ogg"
    );
}

#[test]
fn test_apply_nginx_prefix_external_urls_skipped() {
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
    assert_eq!(apply_nginx_prefix("file.txt", true), "file.txt");
    assert_eq!(apply_nginx_prefix("script.js", true), "script.js");
    assert_eq!(apply_nginx_prefix("/data/file.csv", true), "/data/file.csv");
}

#[test]
fn test_is_external_url() {
    assert!(is_external_url("http://example.com"));
    assert!(is_external_url("https://example.com"));
    assert!(is_external_url("mailto:test@example.com"));
    assert!(is_external_url("//cdn.example.com"));

    assert!(!is_external_url("/images/photo.jpg"));
    assert!(!is_external_url("photo.jpg"));
    assert!(!is_external_url("../assets/video.mp4"));
    assert!(!is_external_url("./relative/path.png"));
}
