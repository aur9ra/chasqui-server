use crate::domain::Page;
use std::collections::HashMap;

// exists to quickly get a page back for our routes rather than calling the db
pub struct SyncCache {
    pub pages_by_filename: HashMap<String, Page>,
}

impl SyncCache {
    pub fn new() -> Self {
        Self {
            pages_by_filename: HashMap::new(),
        }
    }
}
