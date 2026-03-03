use std::path::Path;

/// Centralized utility to ensure all paths within the Chasqui ecosystem use
/// consistent Unix-style forward slashes, regardless of the host OS.
pub fn normalize_path<P: AsRef<Path>>(path: P) -> String {
    path.as_ref().to_string_lossy().replace("\\", "/")
}

/// Resolves logical path components like '.' and '..' to produce a clean,
/// canonical identifier-style string.
///
/// it would be good to pass our base path here, so we can not get something invalid
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
/// It ensures that identifiers are valid URL components and cannot be used for path traversal.
pub fn sanitize_identifier(id: &str) -> String {
    // 1. Strict rejection of traversal
    if id.contains("..") {
        return "".to_string();
    }

    // 2. Logical normalization (handles things like ./a)
    let normalized = normalize_logical_path(id);

    // 3. URL-safe slugification
    let mut result = String::with_capacity(normalized.len());
    let mut last_was_dash = false;

    for c in normalized.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if c == '/' || c == '.' || c == '_' {
            result.push(c);
            last_was_dash = false;
        } else if c.is_whitespace() || c == '-' || c == ' ' {
            if !last_was_dash && !result.is_empty() {
                result.push('-');
                last_was_dash = true;
            }
        }
    }

    // Trim leading/trailing dashes and slashes for a clean URL component
    let final_id = result.trim_matches('-').trim_start_matches('/');
    final_id.to_string()
}

/// Helper to generate a logical identifier from a physical path.
pub fn path_to_identifier(path: &Path, strip_extension: bool) -> String {
    let raw = if strip_extension {
        path.with_extension("").to_string_lossy().to_string()
    } else {
        path.to_string_lossy().to_string()
    };

    sanitize_identifier(&raw)
}
