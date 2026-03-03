use crate::io::verify_relative_path;
use std::path::Path;

#[test]
fn test_io_link_jailbreak_prevention() {
    let root = Path::new("/content");

    // 1. Straight traversal from a root file
    // file: /content/a.md (depth 0)
    // link: ../outside -> depth -1 (Breach)
    assert!(verify_relative_path(root, Path::new("a.md"), Path::new("../outside")).is_err());

    // 2. Traversal after entering a folder
    // file: /content/blog/post.md (depth 1)
    // link: ../../outside -> depth 1 -> 0 -> -1 (Breach)
    assert!(
        verify_relative_path(root, Path::new("blog/post.md"), Path::new("../../outside")).is_err()
    );

    // 3. Valid deep traversal (staying within root)
    // file: /content/blog/post.md (depth 1)
    // link: ../about.md -> depth 1 -> 0 (Safe)
    assert!(
        verify_relative_path(root, Path::new("blog/post.md"), Path::new("../about.md")).is_ok()
    );

    // 4. Using deep base_depth
    // file: /content/a/b/c/d.md (depth 3)
    // link: ../../../../outside -> depth 3 -> 2 -> 1 -> 0 -> -1 (Breach)
    assert!(
        verify_relative_path(
            root,
            Path::new("a/b/c/d.md"),
            Path::new("../../../../outside")
        )
        .is_err()
    );

    // 5. Valid complex relative link
    // file: /content/a/b/c/d.md (depth 3)
    // link: ../../../z.md -> depth 3 -> 2 -> 1 -> 0 (Safe)
    assert!(
        verify_relative_path(root, Path::new("a/b/c/d.md"), Path::new("../../../z.md")).is_ok()
    );
}

#[test]
fn test_io_path_utils_jailbreak_prevention() {
    use crate::io::path_utils::{normalize_logical_path, sanitize_identifier};

    // 1. Good paths should resolve correctly
    assert_eq!(normalize_logical_path("blog/post"), "blog/post");
    assert_eq!(normalize_logical_path("a/b/../c"), "a/c");
    assert_eq!(normalize_logical_path("./a/b"), "a/b");

    // 2. Traversal attempts (Bad paths)
    // normalize_logical_path still "clamps" to the root, which is safe.
    assert_eq!(normalize_logical_path("../outside"), "outside");
    
    // 3. Sanitization (STRICT)
    // Should reject traversal and slugify
    assert_eq!(sanitize_identifier("../../../secret"), "");
    assert_eq!(sanitize_identifier("blog/My Post!"), "blog/my-post");
    assert_eq!(sanitize_identifier("/absolute/path"), "absolute/path");
    assert_eq!(sanitize_identifier("  space  "), "space");
}
