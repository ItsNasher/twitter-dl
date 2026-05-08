use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoVariant {
    pub label: String,
    pub url: String,
    pub bitrate: u64,
}