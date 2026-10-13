//! Preview-first cleanup for old files directly inside the current user's temp folder.
//! This deliberately does not recurse into folders, follow symlinks, or delete automatically.

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_MIN_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const MAX_PREVIEW_ITEMS: usize = 100;
const PREVIEW_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CleanupPreviewItem {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CleanupPreview {
    pub root_label: String,
    pub generated_at: String,
    pub min_age_days: u64,
    pub files_found: usize,
    pub total_bytes: u64,
    pub truncated: bool,
    pub items: Vec<CleanupPreviewItem>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CleanupResult {
    pub deleted_count: usize,
    pub deleted_bytes: u64,
    pub skipped_count: usize,
    pub completed_at: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    path: PathBuf,
    size_bytes: u64,
    modified: SystemTime,
}

#[derive(Debug)]
pub struct CleanupCache {
    root: PathBuf,
    created: Instant,
    min_age: Duration,
    items: HashMap<String, Candidate>,
}

pub fn build_preview() -> Result<(CleanupPreview, CleanupCache)> {
    build_preview_at(&std::env::temp_dir(), DEFAULT_MIN_AGE)
}

fn build_preview_at(root_path: &Path, min_age: Duration) -> Result<(CleanupPreview, CleanupCache)> {
    let root = root_path.canonicalize().map_err(|e| {
        CoreError::Internal(format!("cannot access the current user's temp folder: {e}"))
    })?;
    let now = SystemTime::now();
    let cutoff = now.checked_sub(min_age).unwrap_or(UNIX_EPOCH);
    let entries = fs::read_dir(&root).map_err(|e| {
        CoreError::Internal(format!("cannot scan the current user's temp folder: {e}"))
    })?;

    let mut candidates: Vec<(String, Candidate)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        // metadata() follows links; symlink_metadata() does not. Only direct regular files are allowed.
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if modified > cutoff || modified > now || now.duration_since(modified).is_err() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        candidates.push((
            name,
            Candidate {
                path,
                size_bytes: metadata.len(),
                modified,
            },
        ));
    }

    candidates.sort_by(|a, b| {
        b.1.size_bytes
            .cmp(&a.1.size_bytes)
            .then_with(|| a.1.modified.cmp(&b.1.modified))
    });
    let truncated = candidates.len() > MAX_PREVIEW_ITEMS;
    candidates.truncate(MAX_PREVIEW_ITEMS);
    let files_found = candidates.len();
    let total_bytes = candidates
        .iter()
        .fold(0_u64, |acc, (_, c)| acc.saturating_add(c.size_bytes));
    let mut cache_items = HashMap::new();
    let mut items = Vec::with_capacity(candidates.len());
    for (idx, (name, candidate)) in candidates.into_iter().enumerate() {
        let id = format!("temp-{idx:03}");
        let modified_at = chrono::DateTime::<chrono::Utc>::from(candidate.modified).to_rfc3339();
        items.push(CleanupPreviewItem {
            id: id.clone(),
            name,
            size_bytes: candidate.size_bytes,
            modified_at,
        });
        cache_items.insert(id, candidate);
    }

    let preview = CleanupPreview {
        root_label: "Current user's temporary folder".into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        min_age_days: min_age.as_secs() / (24 * 60 * 60),
        files_found,
        total_bytes,
        truncated,
        items,
        note: "Only regular files directly inside the current user's temp folder are listed. Folders and symbolic links are never followed. Files younger than the stated age are excluded. Files in use may be skipped by Windows.".into(),
    };
    let cache = CleanupCache {
        root,
        created: Instant::now(),
        min_age,
        items: cache_items,
    };
    Ok((preview, cache))
}

pub fn delete_selected(cache: &mut CleanupCache, ids: &[String]) -> Result<CleanupResult> {
    if cache.created.elapsed() > PREVIEW_TTL {
        return Err(CoreError::InvalidConfig(
            "cleanup preview expired; scan again before deleting".into(),
        ));
    }
    if ids.is_empty() {
        return Err(CoreError::InvalidConfig(
            "select at least one previewed file".into(),
        ));
    }
    if ids.len() > MAX_PREVIEW_ITEMS {
        return Err(CoreError::InvalidConfig("too many files selected".into()));
    }
    let mut unique = std::collections::HashSet::new();
    if ids
        .iter()
        .any(|id| !unique.insert(id) || !cache.items.contains_key(id))
    {
        return Err(CoreError::InvalidConfig(
            "selection does not match the latest cleanup preview".into(),
        ));
    }

    let cutoff = SystemTime::now()
        .checked_sub(cache.min_age)
        .unwrap_or(UNIX_EPOCH);
    let mut deleted_count = 0_usize;
    let mut deleted_bytes = 0_u64;
    let mut skipped_count = 0_usize;
    for id in ids {
        let Some(candidate) = cache.items.remove(id) else {
            skipped_count += 1;
            continue;
        };
        if candidate.path.parent() != Some(cache.root.as_path()) {
            skipped_count += 1;
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&candidate.path) else {
            skipped_count += 1;
            continue;
        };
        if !meta.file_type().is_file() || meta.file_type().is_symlink() {
            skipped_count += 1;
            continue;
        }
        let Ok(modified) = meta.modified() else {
            skipped_count += 1;
            continue;
        };
        // Refuse to delete a path whose file changed after the preview, even if its name
        // and age still look eligible. This avoids silently deleting a replacement file.
        if modified > cutoff
            || modified > SystemTime::now()
            || modified != candidate.modified
            || meta.len() != candidate.size_bytes
        {
            skipped_count += 1;
            continue;
        }
        match fs::remove_file(&candidate.path) {
            Ok(()) => {
                deleted_count += 1;
                deleted_bytes = deleted_bytes.saturating_add(meta.len());
            }
            Err(_) => skipped_count += 1,
        }
    }
    Ok(CleanupResult {
        deleted_count,
        deleted_bytes,
        skipped_count,
        completed_at: chrono::Utc::now().to_rfc3339(),
        message: "Windows may refuse to remove a file that is still in use. TurboFix does not terminate apps to force a deletion.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn preview_only_includes_direct_old_files_and_deletion_requires_preview_ids() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = fs::File::create(dir.path().join("temporary.bin")).unwrap();
        file.write_all(b"hello world").unwrap();
        drop(file);
        let sub = dir.path().join("subfolder");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.bin"), b"keep nested file").unwrap();

        // Zero cutoff makes newly-created files eligible in a deterministic isolated test directory.
        let (preview, mut cache) = build_preview_at(dir.path(), Duration::ZERO).unwrap();
        assert_eq!(preview.files_found, 1);
        assert_eq!(preview.items[0].name, "temporary.bin");
        assert_eq!(preview.total_bytes, 11);
        assert!(delete_selected(&mut cache, &["not-previewed".into()]).is_err());

        let id = preview.items[0].id.clone();
        let result = delete_selected(&mut cache, &[id]).unwrap();
        assert_eq!(result.deleted_count, 1);
        assert_eq!(result.deleted_bytes, 11);
        assert!(!dir.path().join("temporary.bin").exists());
        assert!(sub.join("nested.bin").exists());
    }

    #[test]
    fn changed_file_is_skipped_after_preview() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("replace.me"), b"old").unwrap();
        let (preview, mut cache) = build_preview_at(dir.path(), Duration::ZERO).unwrap();
        assert_eq!(preview.files_found, 1);
        fs::write(
            dir.path().join("replace.me"),
            b"new content with another size",
        )
        .unwrap();

        let result = delete_selected(&mut cache, &[preview.items[0].id.clone()]).unwrap();
        assert_eq!(result.deleted_count, 0);
        assert_eq!(result.skipped_count, 1);
        assert!(dir.path().join("replace.me").exists());
    }

    #[test]
    fn duplicate_or_empty_selection_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.tmp"), b"a").unwrap();
        let (preview, mut cache) = build_preview_at(dir.path(), Duration::ZERO).unwrap();
        assert!(delete_selected(&mut cache, &[]).is_err());
        if let Some(item) = preview.items.first() {
            assert!(delete_selected(&mut cache, &[item.id.clone(), item.id.clone()]).is_err());
        }
    }
}
