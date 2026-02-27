use std::path::Path;
use crate::io::verify_relative_path;

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
    assert!(verify_relative_path(root, Path::new("blog/post.md"), Path::new("../../outside")).is_err());

    // 3. Valid deep traversal (staying within root)
    // file: /content/blog/post.md (depth 1)
    // link: ../about.md -> depth 1 -> 0 (Safe)
    assert!(verify_relative_path(root, Path::new("blog/post.md"), Path::new("../about.md")).is_ok());

    // 4. Using deep base_depth
    // file: /content/a/b/c/d.md (depth 3)
    // link: ../../../../outside -> depth 3 -> 2 -> 1 -> 0 -> -1 (Breach)
    assert!(verify_relative_path(root, Path::new("a/b/c/d.md"), Path::new("../../../../outside")).is_err());

    // 5. Valid complex relative link
    // file: /content/a/b/c/d.md (depth 3)
    // link: ../../../z.md -> depth 3 -> 2 -> 1 -> 0 (Safe)
    assert!(verify_relative_path(root, Path::new("a/b/c/d.md"), Path::new("../../../z.md")).is_ok());
}
