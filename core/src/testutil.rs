use crate::io::{ContentMetadata, ContentReader, SyncFile};
use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct MockLargeStream {
    size: u64,
    pos: u64,
}

impl Read for MockLargeStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.size - self.pos;
        if remaining == 0 {
            return Ok(0);
        }
        let to_read = std::cmp::min(buf.len(), remaining as usize);
        for i in 0..to_read {
            buf[i] = 0;
        }
        self.pos += to_read as u64;
        Ok(to_read)
    }
}

impl Seek for MockLargeStream {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(n) => self.pos = std::cmp::min(n, self.size),
            SeekFrom::Current(n) => {
                let new_pos = self.pos as i64 + n;
                self.pos = std::cmp::max(0, std::cmp::min(new_pos, self.size as i64)) as u64;
            }
            SeekFrom::End(n) => {
                let new_pos = self.size as i64 + n;
                self.pos = std::cmp::max(0, std::cmp::min(new_pos, self.size as i64)) as u64;
            }
        }
        Ok(self.pos)
    }
}

#[derive(Clone)]
struct MockFile {
    content: Vec<u8>,
    metadata: ContentMetadata,
    virtual_size: Option<u64>,
}

#[derive(Clone)]
pub struct MockContentReader {
    files: Arc<Mutex<HashMap<PathBuf, MockFile>>>,
}

impl MockContentReader {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_virtual_large_file(&self, path: &str, size: u64) {
        let mut files = self.files.lock().unwrap();
        files.insert(
            PathBuf::from(path),
            MockFile {
                content: Vec::new(),
                metadata: ContentMetadata {
                    modified: None,
                    created: None,
                    size,
                },
                virtual_size: Some(size),
            },
        );
    }

    pub fn add_file(&self, path: &str, content: &str) {
        self.add_file_with_metadata(path, content, None, None);
    }

    pub fn add_binary_file(&self, path: &str, content: Vec<u8>) {
        let mut files = self.files.lock().unwrap();
        let size = content.len() as u64;
        files.insert(
            PathBuf::from(path),
            MockFile {
                content,
                metadata: ContentMetadata {
                    modified: None,
                    created: None,
                    size,
                },
                virtual_size: None,
            },
        );
    }

    pub fn add_file_with_metadata(
        &self,
        path: &str,
        content: &str,
        modified: Option<NaiveDateTime>,
        created: Option<NaiveDateTime>,
    ) {
        let mut files = self.files.lock().unwrap();
        let size = content.len() as u64;
        files.insert(
            PathBuf::from(path),
            MockFile {
                content: content.as_bytes().to_vec(),
                metadata: ContentMetadata { modified, created, size },
                virtual_size: None,
            },
        );
    }

    pub fn load_real_file(&self, virtual_path: &str, real_path: &Path) {
        let content = std::fs::read(real_path).expect("Failed to read real file for mock");
        let fs_metadata = std::fs::metadata(real_path).expect("Failed to read metadata for mock");

        let modified = fs_metadata.modified().ok().map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.naive_utc()
        });
        let created = fs_metadata.created().ok().map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.naive_utc()
        });

        let mut files = self.files.lock().unwrap();
        files.insert(
            PathBuf::from(virtual_path),
            MockFile {
                content,
                metadata: ContentMetadata {
                    modified,
                    created,
                    size: fs_metadata.len(),
                },
                virtual_size: None,
            },
        );
    }
}

#[async_trait]
impl ContentReader for MockContentReader {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        let files = self.files.lock().unwrap();
        let file = files
            .get(path)
            .ok_or_else(|| anyhow::anyhow!("File not found in mock: {:?}", path))?;
        
        if file.virtual_size.is_some() {
            return Err(anyhow::anyhow!("Cannot read virtual large file as string"));
        }

        String::from_utf8(file.content.clone())
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in mock file: {}", e))
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        let files = self.files.lock().unwrap();
        let file = files
            .get(path)
            .ok_or_else(|| anyhow::anyhow!("File not found in mock: {:?}", path))?;

        if let Some(size) = file.virtual_size {
            if size > 1024 * 1024 * 50 {
                return Err(anyhow::anyhow!("Safety: Mock refuses to read_bytes for virtual file > 50MB. Use streaming!"));
            }
            return Ok(vec![0u8; size as usize]);
        }

        Ok(file.content.clone())
    }

    async fn open_file(&self, path: &Path) -> Result<SyncFile> {
        let files = self.files.lock().unwrap();
        let file = files
            .get(path)
            .ok_or_else(|| anyhow::anyhow!("File not found in mock: {:?}", path))?;

        if let Some(size) = file.virtual_size {
            Ok(Box::new(MockLargeStream { size, pos: 0 }))
        } else {
            Ok(Box::new(std::io::Cursor::new(file.content.clone())))
        }
    }

    async fn get_hash(&self, path: &Path) -> Result<String> {
        let files = self.files.lock().unwrap();
        let file = files
            .get(path)
            .ok_or_else(|| anyhow::anyhow!("File not found in mock: {:?}", path))?;

        use xxhash_rust::xxh3::Xxh3;
        let mut hasher = Xxh3::new();

        if let Some(total_size) = file.virtual_size {
            let limit = std::cmp::min(total_size, 1024 * 1024);
            let chunk_size = 64 * 1024;
            let mut remaining = limit;
            let chunk = vec![0u8; chunk_size];
            
            while remaining > 0 {
                let to_write = std::cmp::min(remaining, chunk_size as u64);
                hasher.update(&chunk[..to_write as usize]);
                remaining -= to_write;
            }
        } else {
            hasher.update(&file.content);
        }

        Ok(format!("{:016x}", hasher.digest()))
    }

    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata> {
        let files = self.files.lock().unwrap();
        let file = files
            .get(path)
            .ok_or_else(|| anyhow::anyhow!("Metadata not found in mock: {:?}", path))?;
        
        let mut meta = file.metadata.clone();
        meta.size = file.virtual_size.unwrap_or(file.content.len() as u64);
        Ok(meta)
    }

    async fn list_markdown_files(&self, _root: &Path) -> Result<Vec<PathBuf>> {
        let files = self.files.lock().unwrap();
        Ok(files
            .keys()
            .filter(|p| p.extension().map_or(false, |e| e == "md"))
            .cloned()
            .collect())
    }

    async fn list_all_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let files = self.files.lock().unwrap();
        Ok(files.keys().filter(|p| p.starts_with(root)).cloned().collect())
    }

    async fn list_files_by_extension(&self, _root: &Path, _extension: String) {}
}