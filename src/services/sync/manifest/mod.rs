pub mod claim;

use crate::features::model::FeatureType;
pub use self::claim::ManifestClaim;
use crate::io::path_utils::{normalize_logical_path};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The unified source of truth for all resource identities in the system.
pub struct Manifest {
    pub filenames: HashSet<String>,
    pub file_to_id: HashMap<String, String>,
    pub id_to_file: HashMap<String, String>,
    pub hashes: HashMap<String, String>,
    pub feature_types: HashMap<String, FeatureType>,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            filenames: HashSet::new(),
            file_to_id: HashMap::new(),
            id_to_file: HashMap::new(),
            hashes: HashMap::new(),
            feature_types: HashMap::new(),
        }
    }

    /// Atomically registers a file and its metadata.
    pub fn register_claim(&mut self, claim: ManifestClaim) {
        self.filenames.insert(claim.filename.clone());
        self.hashes
            .insert(claim.filename.clone(), claim.content_hash);
        self.feature_types.insert(claim.filename.clone(), claim.feature_type);

        if let Some(id) = claim.identifier {
            self.file_to_id.insert(claim.filename.clone(), id.clone());
            self.id_to_file.insert(id, claim.filename);
        }
    }

    /// Completely removes a file and its associated metadata.
    pub fn remove_by_filename(&mut self, filename: &str) {
        self.filenames.remove(filename);
        self.hashes.remove(filename);
        self.feature_types.remove(filename);
        if let Some(id) = self.file_to_id.remove(filename) {
            self.id_to_file.remove(&id);
        }
    }

    /// Resolves a relative or literal link using the manifest's knowledge of filenames and identifiers.
    pub fn resolve_link(&self, link: &str, current_filename: &Path, config: &crate::config::ChasquiConfig) -> String {
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
            normalize_logical_path(&joined)
        } else {
            raw_lookup.trim_start_matches('/').to_string()
        };

        // attempt to lookup the link by filename & identifier
        let resolved_identifier = if let Some(identifier) = self.file_to_id.get(&lookup_key)
        {
            Some(identifier.clone())
        } else if self.id_to_file.contains_key(&lookup_key) {
            Some(lookup_key.to_string())
        } else {
            None
        };

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

    /// Processes a batch of paths with their respective mount points and feature types, filters for collisions, and atomically updates the manifest.
    /// Returns the list of valid claims that passed the consensus pass.
    pub async fn register_claims(
        &mut self,
        path_mount_type_triples: Vec<(std::path::PathBuf, std::path::PathBuf, FeatureType)>,
        reader: &dyn crate::io::ContentReader,
        config: &crate::config::ChasquiConfig,
    ) -> Vec<ManifestClaim> {
        let mut potentials = Vec::new();
        let mut id_counts: HashMap<String, usize> = HashMap::new();

        // 1. Collect all potential claims and count occurrences for internal collision detection
        for (path, mount, f_type) in path_mount_type_triples {
            match ManifestClaim::new(&path, &mount, reader, config, self, f_type).await {
                Ok(Some(claim)) => {
                    if let Some(ref id) = claim.identifier {
                        *id_counts.entry(id.clone()).or_insert(0) += 1;
                    }
                    potentials.push(claim);
                }
                Ok(None) => {} // Skip unchanged or ignored files
                Err(e) => eprintln!("Manifest: Failed to generate claim for {:?}: {}", path, e),
            }
        }

        // 2. Filter based on collisions and update manifest
        let mut valid_claims = Vec::new();
        for claim in potentials {
            let mut has_collision = false;

            if let Some(ref id) = claim.identifier {
                // Internal Collision: Two files in the SAME incoming batch claiming same ID
                if *id_counts.get(id).unwrap_or(&0) > 1 {
                    eprintln!("Collision (Internal): Identifier '{}' claimed by multiple files in batch. Rejecting all.", id);
                    has_collision = true;
                }

                // External Collision: Incoming file claiming ID owned by an EXISTING file in the Manifest
                if let Some(existing_file) = self.id_to_file.get(id) {
                    if existing_file != &claim.filename {
                        eprintln!("Collision (External): Identifier '{}' already owned by {}. Rejecting {}.", id, existing_file, claim.filename);
                        has_collision = true;
                    }
                }
            }

            if !has_collision {
                self.register_claim(claim.clone());
                valid_claims.push(claim);
            }
        }

        valid_claims
    }
}
