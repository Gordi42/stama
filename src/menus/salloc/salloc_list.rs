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

    pub fn get_mut(&mut self, index: usize) -> eyre::Result<&mut T> {
        let entry = self
            .entries
            .get_mut(index)
            .ok_or_else(|| eyre::eyre!("Index out of bounds."))?;
        Ok(entry)
    }

    pub fn set_list(&mut self, list: Vec<T>) {
        self.entries = list;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    // =======================================================================
    //            FILE OPERATIONS
    // =======================================================================

    pub fn save(&self, filename: Option<&str>) -> eyre::Result<()> {
        let home = std::env::var("HOME")?;
        let config_dir = format!("{}/{}", home, CONFIG_DIR);
        std::fs::create_dir_all(&config_dir)?;
        let file = match filename {
            Some(name) => format!("{}/{}", config_dir, name),
            None => format!("{}/{}", config_dir, FILENAME),
        };
        let toml_str = toml::to_string(&self)?;
        // write the toml string to the file
        // if the file exists, it should be overwritten
        std::fs::write(file, toml_str)?;
        Ok(())
    }

    pub fn load(filename: Option<&str>) -> eyre::Result<SallocList<T>>
    where
        for<'de> T: Deserialize<'de>,
    {
        let home = std::env::var("HOME")?;
        let file = match filename {
            Some(name) => format!("{}/{}/{}", home, CONFIG_DIR, name),
            None => format!("{}/{}/{}", home, CONFIG_DIR, FILENAME),
        };
        // if the file does not exist, return an empty list
        if !std::path::Path::new(&file).exists() {
            return Ok(SallocList::new());
        }
        // otherwise, load the list
        let toml_str = std::fs::read_to_string(file)?;
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
        let mut list: SallocList<SallocEntry> = SallocList::new();
        let entry = SallocEntry::new();
        list.push(entry);
        list.save(Some("test_save.toml")).unwrap();
        let home = std::env::var("HOME").unwrap();
        let file = format!("{}/{}/test_save.toml", home, CONFIG_DIR);
        assert!(std::path::Path::new(&file).exists());
    }

    #[test]
    fn test_load() {
        // Create the list
        let mut list: SallocList<SallocEntry> = SallocList::new();
        let mut entry = SallocEntry::new();
        entry.preset_name = "test".to_string();
        list.push(entry);
        // Save the list
        list.save(Some("test_load.toml")).unwrap();

        // Load the list
        let loaded_list: SallocList<SallocEntry> =
            SallocList::load(Some("test_load.toml")).unwrap();

        // Test the loaded list
        assert_eq!(loaded_list.len(), 1);
        let entry = loaded_list.get(0).unwrap();
        assert_eq!(entry.preset_name, "test");
    }
}
