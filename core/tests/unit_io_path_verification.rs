use chasqui_core::io::verify_relative_path;
use chasqui_core::io::path_utils::{normalize_logical_path, sanitize_identifier};
use std::path::Path;

#[test]
fn test_io_link_jailbreak_prevention() {
    let root = Path::new("/content");

    assert!(verify_relative_path(root, Path::new("a.md"), Path::new("../outside")).is_err());

    assert!(
        verify_relative_path(root, Path::new("blog/post.md"), Path::new("../../outside")).is_err()
    );

    assert!(
        verify_relative_path(root, Path::new("blog/post.md"), Path::new("../about.md")).is_ok()
    );

    assert!(
        verify_relative_path(
            root,
            Path::new("a/b/c/d.md"),
            Path::new("../../../../outside")
        )
        .is_err()
    );

    assert!(
        verify_relative_path(root, Path::new("a/b/c/d.md"), Path::new("../../../z.md")).is_ok()
    );
}

#[test]
fn test_io_path_utils_jailbreak_prevention() {
    assert_eq!(normalize_logical_path("blog/post"), "blog/post");
    assert_eq!(normalize_logical_path("a/b/../c"), "a/c");
    assert_eq!(normalize_logical_path("./a/b"), "a/b");

    assert_eq!(normalize_logical_path("../outside"), "outside");
    
    assert_eq!(sanitize_identifier("../../../secret"), "");
    assert_eq!(sanitize_identifier("blog/My Post!"), "blog/my-post");
    assert_eq!(sanitize_identifier("/absolute/path"), "absolute/path");
    assert_eq!(sanitize_identifier("  space  "), "space");
}