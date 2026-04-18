use std::path::Path;

pub fn normalize_path<P: AsRef<Path>>(path: P) -> String {
    path.as_ref().to_string_lossy().replace("\\", "/")
}

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

pub fn sanitize_identifier(id: &str) -> String {
    if id.contains("..") {
        return "".to_string();
    }

    let normalized = normalize_logical_path(id);

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

    let final_id = result.trim_matches('-').trim_start_matches('/');
    final_id.to_string()
}

pub fn path_to_identifier(path: &Path, strip_extension: bool) -> String {
    let raw = if strip_extension {
        path.with_extension("").to_string_lossy().to_string()
    } else {
        path.to_string_lossy().to_string()
    };

    sanitize_identifier(&raw)
}