use std::{fs, path::PathBuf};

use anyhow::{Context as _, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    pub id: Uuid,
    pub text: String,
    pub created_at: DateTime<Local>,
}

impl Transcript {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            text: text.into(),
            created_at: Local::now(),
        }
    }
}

/// Transcripts persisted as a JSON file, newest first.
pub struct Store {
    path: PathBuf,
    pub items: Vec<Transcript>,
}

impl Store {
    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rs-speech-to-text")
            .join("transcripts.json")
    }

    pub fn load(path: PathBuf) -> Result<Self> {
        let items = match fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json)
                .with_context(|| format!("parsing {}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Ok(Self { path, items })
    }

    pub fn add(&mut self, transcript: Transcript) {
        self.items.insert(0, transcript);
    }

    pub fn remove(&mut self, id: Uuid) {
        self.items.retain(|t| t.id != id);
    }

    /// Drops transcripts created before `cutoff`. Returns how many were removed.
    pub fn prune_older_than(&mut self, cutoff: DateTime<Local>) -> usize {
        let before = self.items.len();
        self.items.retain(|t| t.created_at >= cutoff);
        before - self.items.len()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&self.items)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("stt-test-{}", Uuid::new_v4()));
        let path = dir.join("transcripts.json");

        let mut store = Store::load(path.clone()).unwrap();
        assert!(store.items.is_empty());

        let a = Transcript::new("first");
        let b = Transcript::new("second");
        store.add(a.clone());
        store.add(b.clone());
        store.save().unwrap();

        let mut loaded = Store::load(path.clone()).unwrap();
        assert_eq!(loaded.items, vec![b.clone(), a.clone()]);

        loaded.remove(b.id);
        loaded.save().unwrap();
        assert_eq!(Store::load(path).unwrap().items, vec![a]);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn prune_drops_old_transcripts() {
        let path = std::env::temp_dir().join(format!("stt-test-{}.json", Uuid::new_v4()));
        let mut store = Store::load(path).unwrap();

        let now = Local::now();
        let mut old = Transcript::new("old");
        old.created_at = now - chrono::Duration::days(120);
        let fresh = Transcript::new("fresh");
        store.add(old);
        store.add(fresh.clone());

        let cutoff = now.checked_sub_months(chrono::Months::new(3)).unwrap();
        assert_eq!(store.prune_older_than(cutoff), 1);
        assert_eq!(store.items, vec![fresh]);
    }
}
