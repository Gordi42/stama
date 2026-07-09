use std::path::{Path, PathBuf};

use color_eyre::eyre;
use serde::{Deserialize, Serialize};

/// The directory where the config files are saved
/// full path: $HOME/{CONFIG_DIR}
const CONFIG_DIR: &str = ".config/stama";

/// The filename where the salloc list is saved
/// full path: $HOME/{CONFIG_DIR}/{FILENAME}
const FILENAME: &str = "salloc_list.toml";

/// A list of salloc entries that can be saved and loaded
/// from a TOML file
#[derive(Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct SallocList<T> {
    pub entries: Vec<T>,
}

impl<T: Serialize> SallocList<T> {
    // =======================================================================
    //             CONSTRUCTORS
    // =======================================================================
    pub fn new() -> SallocList<T> {
        SallocList {
            entries: Vec::new(),
        }
    }

    // =======================================================================
    //   MAIN METHODS
    // =======================================================================

    pub fn push(&mut self, item: T) {
        self.entries.push(item);
    }

    pub fn get(&self, index: usize) -> eyre::Result<&T> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| eyre::eyre!("Index out of bounds."))?;
        Ok(entry)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // =======================================================================
    //            FILE OPERATIONS
    // =======================================================================

    /// Compute the default config file path:
    /// $HOME/{CONFIG_DIR}/{filename or FILENAME}
    fn default_path(filename: Option<&str>) -> eyre::Result<PathBuf> {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(home)
            .join(CONFIG_DIR)
            .join(filename.unwrap_or(FILENAME)))
    }

    /// Save the list to the default config location
    pub fn save(&self, filename: Option<&str>) -> eyre::Result<()> {
        self.save_to_path(&Self::default_path(filename)?)
    }

    /// Save the list to the given path, creating parent directories
    /// if necessary
    pub fn save_to_path(&self, path: &Path) -> eyre::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string(&self)?;
        // write the toml string to the file
        // if the file exists, it should be overwritten
        std::fs::write(path, toml_str)?;
        Ok(())
    }

    /// Load the list from the default config location
    pub fn load(filename: Option<&str>) -> eyre::Result<SallocList<T>>
    where
        for<'de> T: Deserialize<'de>,
    {
        Self::load_from_path(&Self::default_path(filename)?)
    }

    /// Load the list from the given path
    /// If the file does not exist, return an empty list
    pub fn load_from_path(path: &Path) -> eyre::Result<SallocList<T>>
    where
        for<'de> T: Deserialize<'de>,
    {
        if !path.exists() {
            return Ok(SallocList::new());
        }
        let toml_str = std::fs::read_to_string(path)?;
        let list: SallocList<T> = toml::from_str(&toml_str)?;
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menus::salloc::salloc_entry::SallocEntry;

    #[test]
    fn test_new() {
        let list: SallocList<SallocEntry> = SallocList::new();
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_push() {
        let mut list: SallocList<SallocEntry> = SallocList::new();
        let entry = SallocEntry::new();
        list.push(entry);
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_get() {
        let mut list: SallocList<SallocEntry> = SallocList::new();
        let entry = SallocEntry::new();
        list.push(entry);
        let entry_ref: &SallocEntry = list.get(0).unwrap();
        assert_eq!(entry_ref.preset_name, "new");
    }

    #[test]
    fn test_save() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test_save.toml");
        let mut list: SallocList<SallocEntry> = SallocList::new();
        let entry = SallocEntry::new();
        list.push(entry);
        list.save_to_path(&file).unwrap();
        assert!(file.exists());
    }

    #[test]
    fn test_load() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test_load.toml");
        // Create the list
        let mut list: SallocList<SallocEntry> = SallocList::new();
        let mut entry = SallocEntry::new();
        entry.preset_name = "test".to_string();
        list.push(entry);
        // Save the list
        list.save_to_path(&file).unwrap();

        // Load the list
        let loaded_list: SallocList<SallocEntry> = SallocList::load_from_path(&file).unwrap();

        // Test the loaded list
        assert_eq!(loaded_list.len(), 1);
        let entry = loaded_list.get(0).unwrap();
        assert_eq!(entry.preset_name, "test");
    }

    #[test]
    fn test_load_missing_file_returns_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("does_not_exist.toml");
        let loaded_list: SallocList<SallocEntry> = SallocList::load_from_path(&file).unwrap();
        assert!(loaded_list.is_empty());
    }

    #[test]
    fn test_save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nested/dir/list.toml");
        let list: SallocList<SallocEntry> = SallocList::new();
        list.save_to_path(&file).unwrap();
        assert!(file.exists());
    }
}
