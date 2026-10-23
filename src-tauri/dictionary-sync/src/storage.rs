//! Local metadata contains dictionary data, but never an authorization token.
use crate::{github::Repository, Dictionary};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SyncState {
    pub client_id: String,
    pub app_slug: String,
    pub repository: Option<Repository>,
    pub baseline: Option<Dictionary>,
    pub last_sync: Option<u64>,
}

pub fn load(directory: &Path) -> Result<SyncState, String> {
    use std::io::Read;
    let file = match std::fs::File::open(directory.join("state.json")) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SyncState::default()),
        Err(_) => return Err("local_storage".into()),
    };
    let mut bytes = vec![];
    file.take((crate::MAX_BYTES * 2 + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "local_storage")?;
    if bytes.len() > crate::MAX_BYTES * 2 {
        return Err("local_state_invalid".into());
    }
    let state: SyncState = serde_json::from_slice(&bytes).map_err(|_| "local_state_invalid")?;
    if let Some(d) = &state.baseline {
        crate::validate(d).map_err(|_| "local_state_invalid")?;
    }
    Ok(state)
}

pub fn save(directory: &Path, state: &SyncState) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(directory).map_err(|_| "local_storage")?;
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| "local_storage")?;
    serde_json::to_writer_pretty(&mut file, state).map_err(|_| "local_storage")?;
    file.flush().map_err(|_| "local_storage")?;
    file.as_file().sync_all().map_err(|_| "local_storage")?;
    file.persist(directory.join("state.json"))
        .map_err(|_| "local_storage")?;
    Ok(())
}

/// Retain the latest twenty pre-sync snapshots. They contain dictionary data only.
pub fn backup(directory: &Path, dictionary: &Dictionary) -> Result<(), String> {
    use std::io::Write;
    let bytes = crate::encode(dictionary)?;
    let backups = directory.join("backups");
    std::fs::create_dir_all(&backups).map_err(|_| "local_storage")?;
    let mut file = tempfile::Builder::new()
        .prefix("dictionary-")
        .suffix(".json")
        .tempfile_in(&backups)
        .map_err(|_| "local_storage")?;
    file.write_all(&bytes).map_err(|_| "local_storage")?;
    file.as_file().sync_all().map_err(|_| "local_storage")?;
    file.keep().map_err(|_| "local_storage")?;
    let mut entries: Vec<_> = std::fs::read_dir(&backups)
        .map_err(|_| "local_storage")?
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with("dictionary-")
                && e.path().extension().is_some_and(|e| e == "json")
        })
        .collect();
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    let excess = entries.len().saturating_sub(20);
    for entry in entries.into_iter().take(excess) {
        std::fs::remove_file(entry.path()).map_err(|_| "local_storage")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_roundtrip_and_corruption_is_not_a_fresh_start() {
        let dir = tempfile::tempdir().unwrap();
        let state = SyncState {
            client_id: "Iv1.example".into(),
            baseline: Some(Dictionary::default()),
            ..SyncState::default()
        };
        save(dir.path(), &state).unwrap();
        assert_eq!(load(dir.path()).unwrap(), state);
        std::fs::write(dir.path().join("state.json"), b"broken").unwrap();
        assert_eq!(load(dir.path()), Err("local_state_invalid".into()));
    }
}
