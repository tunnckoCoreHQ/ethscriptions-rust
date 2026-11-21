// SPDX-License-Identifier: MPL-2.0

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::ApiError;

const DAY_SECONDS: u64 = 86_400;
const WEEK_SECONDS: u64 = DAY_SECONDS * 7;

#[derive(Debug, Clone, Copy)]
pub enum CacheTtl {
    Day,
    Week,
}

impl CacheTtl {
    fn as_duration(self) -> Duration {
        match self {
            Self::Day => Duration::from_secs(DAY_SECONDS),
            Self::Week => Duration::from_secs(WEEK_SECONDS),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    expires_at: u64,
    value: T,
}

/// File cache of upstream results: one JSON file per entry, at `<root>/<namespace>/<key>.json`.
#[derive(Debug, Clone, Default)]
pub struct Cache {
    /// Cache directory. `None` reads it from the environment on each use:
    /// `$ETHS_CACHE_DIR`, else `$XDG_CACHE_HOME/eths`, else `$HOME/.cache/eths`.
    pub root: Option<PathBuf>,
}

impl Cache {
    pub fn load<T>(&self, namespace: &str, key: &str) -> Result<Option<T>, ApiError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let path = self.file_path(namespace, key)?;
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read(&path).map_err(|e| {
            ApiError::internal(format!(
                "Failed reading cache file '{}': {e}",
                path.display()
            ))
        })?;
        let entry: CacheEntry<T> = serde_json::from_slice(&content).map_err(|e| {
            ApiError::internal(format!(
                "Failed parsing cache file '{}': {e}",
                path.display()
            ))
        })?;

        if entry.expires_at <= now_epoch_seconds() {
            let _ = fs::remove_file(&path);
            return Ok(None);
        }

        Ok(Some(entry.value))
    }

    pub fn store<T>(
        &self,
        namespace: &str,
        key: &str,
        ttl: CacheTtl,
        value: &T,
    ) -> Result<(), ApiError>
    where
        T: Serialize,
    {
        let path = self.file_path(namespace, key)?;
        ensure_parent_dir(&path)?;

        let entry = CacheEntry {
            expires_at: now_epoch_seconds() + ttl.as_duration().as_secs(),
            value,
        };
        let bytes = serde_json::to_vec(&entry)
            .map_err(|e| ApiError::internal(format!("Failed serializing cache entry: {e}")))?;
        fs::write(&path, bytes).map_err(|e| {
            ApiError::internal(format!(
                "Failed writing cache file '{}': {e}",
                path.display()
            ))
        })
    }

    fn file_path(&self, namespace: &str, key: &str) -> Result<PathBuf, ApiError> {
        let root = match &self.root {
            Some(root) => root.clone(),
            None => cache_root()?,
        };
        Ok(root.join(namespace).join(format!("{key}.json")))
    }
}

fn ensure_parent_dir(path: &Path) -> Result<(), ApiError> {
    let Some(parent) = path.parent() else {
        return Err(ApiError::internal("Cache path has no parent directory"));
    };
    fs::create_dir_all(parent).map_err(|e| {
        ApiError::internal(format!(
            "Failed creating cache dir '{}': {e}",
            parent.display()
        ))
    })
}

fn cache_root() -> Result<PathBuf, ApiError> {
    if let Ok(dir) = std::env::var("ETHS_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }

    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(dir).join("eths"));
    }

    let Ok(home) = std::env::var("HOME") else {
        return Err(ApiError::internal(
            "Cannot determine cache directory; set ETHS_CACHE_DIR or HOME",
        ));
    };
    Ok(PathBuf::from(home).join(".cache").join("eths"))
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
