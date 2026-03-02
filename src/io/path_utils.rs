use std::path::Path;

/// Centralized utility to ensure all paths within the Chasqui ecosystem use
/// consistent Unix-style forward slashes, regardless of the host OS.
pub fn normalize_path<P: AsRef<Path>>(path: P) -> String {
    path.as_ref()
        .to_string_lossy()
        .replace("\\", "/")
}

/// Resolves logical path components like '.' and '..' to produce a clean,
/// canonical identifier-style string.
pub fn normalize_logical_path<P: AsRef<Path>>(path: P) -> String {
    use std::path::Component;
    let mut components = Vec::new();

    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                components.pop();
            }
            Component::Normal(c) => {
                components.push(c.to_string_lossy().to_string());
            }
            _ => {}
        }
    }

    components.join("/")
}

/// Specialized sanitization for user-provided identifiers (e.g. from frontmatter).
/// It ensures that identifiers cannot be used for path traversal or other malicious purposes.
pub fn sanitize_identifier(id: &str) -> String {
    let normalized = normalize_logical_path(id);
    normalized.trim_start_matches('/').to_string()
}

/// Helper to generate a logical identifier from a physical path.
pub fn path_to_identifier(path: &Path, strip_extension: bool) -> String {
    let path_str = if strip_extension {
        path.with_extension("")
            .to_string_lossy()
            .to_string()
    } else {
        path.to_string_lossy().to_string()
    };

    path_str.replace("\\", "/")
}
