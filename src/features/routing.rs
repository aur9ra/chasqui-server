use crate::config::ChasquiConfig;

/// Applies Chasqui's global routing rules to a URL path to determine its logical identifier.
pub fn path_to_identifier(config: &ChasquiConfig, path: &str) -> String {
    let normalized = path.trim_start_matches('/').trim_end_matches('/');
    
    // 1. Home Page Rule: Map empty or default paths to the home identifier.
    if config.serve_home && (normalized.is_empty() || normalized == config.home_identifier) {
        return config.home_identifier.clone();
    }

    normalized.to_string()
}

/// Attempts to find a matching identifier by trying common URL variations 
/// (e.g., stripping .html or .md extensions).
pub fn get_identifier_variants(raw_id: &str) -> Vec<String> {
    let mut variants = vec![raw_id.to_string()];
    
    if let Some(stripped) = raw_id.strip_suffix(".html") {
        variants.push(stripped.to_string());
    }
    if let Some(stripped) = raw_id.strip_suffix(".md") {
        variants.push(stripped.to_string());
    }
    
    variants
}
