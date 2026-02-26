use crate::config::ChasquiConfig;
use std::collections::HashMap;

// the manifest represents our in-memory knowledge of the database
// during edit events, this will be edited before the SyncCache (for routes) and db.
pub struct Manifest {
    pub filename_to_identifier: HashMap<String, String>,
    pub identifier_to_filename: HashMap<String, String>,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            filename_to_identifier: HashMap::new(),
            identifier_to_filename: HashMap::new(),
        }
    }

    pub fn insert(&mut self, filename: String, identifier: String) {
        self.filename_to_identifier
            .insert(filename.clone(), identifier.clone());
        self.identifier_to_filename.insert(identifier, filename);
    }

    pub fn remove_by_filename(&mut self, filename: &str) {
        if let Some(identifier) = self.filename_to_identifier.remove(filename) {
            self.identifier_to_filename.remove(&identifier);
        }
    }

    pub fn get_identifier_for_filename(&self, filename: &str) -> Option<String> {
        self.filename_to_identifier.get(filename).cloned()
    }

    pub fn get_filename_for_identifier(&self, identifier: &str) -> Option<String> {
        self.identifier_to_filename.get(identifier).cloned()
    }

    pub fn has_identifier(&self, identifier: &str) -> bool {
        self.identifier_to_filename.contains_key(identifier)
    }

    // this function is called by the AST parser on all anchors.
    // this function will give the AST parser links that will navigate to the identifier and catch
    // errors
    // this function will also ignore any external links or mailtos
    pub fn resolve_link(&self, link: &str, current_filename: &str, config: &ChasquiConfig) -> String {
        // filter external and anchor-only links
        if link.starts_with("http://")
            || link.starts_with("https://")
            || link.starts_with("mailto:")
            || link.starts_with('#')
        {
            return link.to_string();
        }

        // normalize by stripping fragments
        let parts: Vec<&str> = link.split('#').collect();
        let raw_lookup = parts[0];
        let fragment = parts.get(1).map(|f| format!("#{}", f)).unwrap_or_default();

        // Handle relative vs literal paths
        let lookup_key = if raw_lookup.starts_with("./") || raw_lookup.starts_with("../") {
            let mut base = std::path::PathBuf::from(current_filename);
            base.pop();
            let joined = base.join(raw_lookup);
            normalize_path_string(&joined)
        } else {
            raw_lookup.trim_start_matches('/').to_string()
        };

        // attempt to lookup the link by filename & identifier
        let resolved_identifier = if let Some(identifier) = self.filename_to_identifier.get(&lookup_key)
        {
            Some(identifier.clone())
        } else if self.identifier_to_filename.contains_key(&lookup_key) {
            Some(lookup_key.to_string())
        } else {
            None
        };
        // return the "fixed" link that will navigate to the page the writer intended, or the
        // original if broken
        match resolved_identifier {
            Some(id) => {
                if config.serve_home && id == config.home_identifier {
                    format!("/{}", fragment)
                } else {
                    format!("/{}{}", id, fragment)
                }
            }
            None => link.to_string(),
        }
    }
}

pub fn normalize_path_string(path: &std::path::Path) -> String {
    use std::path::Component;
    let mut components = Vec::new();

    for component in path.components() {
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
