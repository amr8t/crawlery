//! Session management for persisting cookies and headers across crawls.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionCookie {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionData {
    #[serde(default)]
    pub cookies: Vec<SessionCookie>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<String>,
}

impl SessionData {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            eprintln!(
                "Warning: Session file not found: {}, using empty session",
                path.display()
            );
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut data = self.clone();
        data.saved_at = Some(chrono::Utc::now().to_rfc3339());
        let json = serde_json::to_string_pretty(&data)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
