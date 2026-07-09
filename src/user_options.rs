use color_eyre::eyre::{self, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

// `#[serde(default)]` makes every missing field fall back to the value
// from `UserOptions::default()`. This way a config file written by an
// older stama version (or by a future version with additional fields)
// still loads instead of silently resetting all options to defaults.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(default)]
pub struct UserOptions {
    pub refresh_rate: usize,       // Refresh rate in milliseconds
    pub show_completed_jobs: bool, // Show completed jobs
    pub confirm_before_quit: bool, // Confirm before quitting
    pub confirm_before_kill: bool, // Confirm before killing a job
    pub external_editor: String,   // External editor command (e.g. "vim")
}

impl Default for UserOptions {
    fn default() -> Self {
        Self {
            refresh_rate: 250,
            show_completed_jobs: true,
            confirm_before_quit: false,
            confirm_before_kill: true,
            external_editor: "vim".to_string(),
        }
    }
}

// ====================================================================
//  LOADING AND SAVING
// ====================================================================

impl UserOptions {
    /// Loads the options from the default config path.
    /// Falls back to the defaults if the file is missing or fails to
    /// parse. A file that failed to parse is protected against being
    /// overwritten by a later save (see `try_save_to_path`).
    pub fn load() -> Self {
        match default_config_path() {
            Ok(path) => Self::load_from_path(&path),
            Err(_) => Self::default(),
        }
    }

    /// Loads the options from the given path, falling back to the
    /// defaults if the file is missing or fails to parse.
    fn load_from_path(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        Self::read_from_path(path).unwrap_or_default()
    }

    /// Reads and parses the options from the given path.
    fn read_from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let user_options = toml::from_str(&contents)?;
        Ok(user_options)
    }

    /// Saves the options to the default config path, ignoring errors.
    /// Use `try_save` when the error should be surfaced to the user.
    pub fn save(&self) {
        // errors while saving are intentionally ignored here; there is
        // no message channel available at this layer (yet)
        let _ = self.try_save();
    }

    /// Saves the options to the default config path.
    pub fn try_save(&self) -> Result<()> {
        let path = default_config_path()?;
        self.try_save_to_path(&path)
    }

    /// Saves the options to the given path.
    ///
    /// Refuses to overwrite an existing file that does not parse as a
    /// valid config: such a file made `load` fall back to the defaults,
    /// and overwriting it would silently destroy the user's (possibly
    /// hand-edited) configuration. The user has to fix or remove the
    /// corrupt file first.
    fn try_save_to_path(&self, path: &Path) -> Result<()> {
        if path.exists() && Self::read_from_path(path).is_err() {
            return Err(eyre::eyre!(
                "refusing to overwrite the config file '{}' because it could \
                 not be parsed; fix or remove it first",
                path.display()
            ));
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| {
                eyre::eyre!("could not create directory '{}': {}", dir.display(), e)
            })?;
        }
        let toml = toml::to_string(self)?;
        let mut file = File::create(path)?;
        file.write_all(toml.as_bytes())?;
        Ok(())
    }
}

/// Returns the default config file path: `$HOME/.config/stama/config.toml`.
fn default_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| eyre::eyre!("Could not find HOME environment variable"))?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("stama")
        .join("config.toml"))
}

// ====================================================================
// TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Creates options that differ from the defaults in every field.
    fn non_default_options() -> UserOptions {
        UserOptions {
            refresh_rate: 500,
            show_completed_jobs: false,
            confirm_before_quit: true,
            confirm_before_kill: false,
            external_editor: "nano".to_string(),
        }
    }

    #[test]
    fn test_toml_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let options = non_default_options();
        options.try_save_to_path(&path).unwrap();
        let loaded = UserOptions::load_from_path(&path);
        assert_eq!(options, loaded);
    }

    #[test]
    fn test_save_creates_missing_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dirs").join("config.toml");

        let options = UserOptions::default();
        options.try_save_to_path(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_missing_file_loads_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.toml");

        let loaded = UserOptions::load_from_path(&path);
        assert_eq!(loaded, UserOptions::default());
    }

    #[test]
    fn test_missing_field_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // a config written by an older stama version that only knows
        // about `refresh_rate`
        fs::write(&path, "refresh_rate = 100\n").unwrap();

        let loaded = UserOptions::load_from_path(&path);
        let defaults = UserOptions::default();
        assert_eq!(loaded.refresh_rate, 100);
        assert_eq!(loaded.show_completed_jobs, defaults.show_completed_jobs);
        assert_eq!(loaded.confirm_before_quit, defaults.confirm_before_quit);
        assert_eq!(loaded.confirm_before_kill, defaults.confirm_before_kill);
        assert_eq!(loaded.external_editor, defaults.external_editor);
    }

    #[test]
    fn test_corrupt_file_loads_defaults_and_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let corrupt = "this is not [valid toml";
        fs::write(&path, corrupt).unwrap();

        // loading a corrupt file falls back to the defaults
        let loaded = UserOptions::load_from_path(&path);
        assert_eq!(loaded, UserOptions::default());

        // saving must refuse to overwrite the corrupt file
        assert!(loaded.try_save_to_path(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), corrupt);
    }
}
