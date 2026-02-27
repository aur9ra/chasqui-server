use anyhow::{Result, bail, Context};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use std::path::{Component, Path, PathBuf};

pub mod local;

/// A path that has been logically verified to reside within the content root.
pub struct VerifiedPath(PathBuf);

impl VerifiedPath {
    /// Provides access to the underlying path for IO operations.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// The ONLY permitted way to read a file from the disk.
pub fn verified_fs_read_to_string(path: VerifiedPath) -> Result<String> {
    Ok(std::fs::read_to_string(path.as_path())?)
}

/// The ONLY permitted way to read metadata from the disk.
pub fn verified_fs_metadata(path: VerifiedPath) -> Result<std::fs::Metadata> {
    Ok(std::fs::metadata(path.as_path())?)
}

/// Verifies a relative link against a base file path within a root.
/// Uses a depth-counter to prevent traversal above the root.
pub fn verify_relative_path(
    root: &Path,
    base_rel_file: &Path,
    link: &Path,
) -> Result<VerifiedPath> {
    // Start depth is the folder containing the base file
    let mut depth: isize = base_rel_file
        .parent()
        .map(|p| {
            p.components()
                .filter(|c| matches!(c, Component::Normal(_)))
                .count() as isize
        })
        .unwrap_or(0);

    for component in link.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    bail!(
                        "Security Violation: Traversal above root in link: {:?}",
                        link
                    );
                }
            }
            _ => {}
        }
    }

    let mut final_path = root.to_path_buf();
    if let Some(parent) = base_rel_file.parent() {
        final_path.push(parent);
    }
    final_path.push(link);

    Ok(VerifiedPath(final_path))
}

/// Verifies an absolute path is within the root.
pub fn verify_absolute_path(root: &Path, absolute_path: &Path) -> Result<VerifiedPath> {
    if absolute_path.starts_with(root) {
        Ok(VerifiedPath(absolute_path.to_path_buf()))
    } else {
        bail!(
            "Security Violation: Absolute path outside root: {:?}",
            absolute_path
        )
    }
}

#[async_trait]
pub trait ContentReader: Send + Sync {
    async fn read_to_string(&self, path: &Path) -> Result<String>;
    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata>;
    async fn list_markdown_files(&self, root: &Path) -> Result<Vec<PathBuf>>;
}

#[derive(Clone)]
pub struct ContentMetadata {
    pub modified: Option<NaiveDateTime>,
    pub created: Option<NaiveDateTime>,
}
