use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

/// User settings persisted as `settings.json` in the config directory.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub openai_api_key: Option<String>,
}

impl Settings {
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rs-speech-to-text")
            .join("settings.json")
    }

    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(json) => {
                serde_json::from_str(&json).with_context(|| format!("parsing {}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }

    /// The saved key, falling back to the `OPENAI_API_KEY` environment variable.
    pub fn api_key(&self) -> Option<String> {
        self.openai_api_key
            .clone()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| {
                std::env::var("OPENAI_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("stt-settings-{}", uuid::Uuid::new_v4()));
        let path = dir.join("settings.json");

        assert_eq!(Settings::load(&path).unwrap(), Settings::default());

        let settings = Settings {
            openai_api_key: Some("sk-test".into()),
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        fs::remove_dir_all(dir).unwrap();
    }
}
