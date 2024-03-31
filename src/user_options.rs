use color_eyre::eyre::{self, Result};
use std::fs::{self, File};
use std::io::prelude::*;
use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserOptions {
    pub refresh_rate: usize,          // Refresh rate in milliseconds
    pub show_completed_jobs: bool,  // Show completed jobs
    pub confirm_before_quit: bool,  // Confirm before quitting
    pub confirm_before_kill: bool,  // Confirm before killing a job
    pub external_editor: String,    // External editor command (e.g. "vim")
    pub dummy_jobs: bool,            // if dummy jobs should be created
}

impl Default for UserOptions {
    fn default() -> Self {
        Self {
            refresh_rate: 250,
            show_completed_jobs: true,
            confirm_before_quit: false,
            confirm_before_kill: true,
            external_editor: "vim".to_string(),
            dummy_jobs: false,
        }
    }
}

// ====================================================================
//  LOADING AND SAVING
// ====================================================================

impl UserOptions {
    pub fn load_from_file() -> Result<Self> {
        let file_dir = get_file_dir()?;
        let file_path = get_file_path(&file_dir);

        let contents = fs::read_to_string(file_path)?;
        let user_options = toml::from_str(&contents)?;
        Ok(user_options)
    }


    pub fn load() -> Self {
        if !file_exists() {
            return Self::default();
        }
        match Self::load_from_file() {
            Ok(user_options) => user_options,
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        match self.try_save() {
            Ok(_) => (),
            Err(_) => (),
        }
    }

    pub fn try_save(&self) -> Result<()> {
        let file_dir = get_file_dir()?;
        touch_dir(&file_dir)?;
        let file_path = get_file_path(&file_dir);

        let toml = toml::to_string(self)?;
        let mut file = File::create(file_path)?;
        file.write_all(toml.as_bytes())?;
        Ok(())
    }

}

fn get_file_dir() -> Result<String> {
    let home = std::env::var("HOME");
    let home = match home {
        Ok(path) => path,
        Err(_) => return Err(eyre::eyre!(
                "Could not find HOME environment variable")),
    };
    Ok(format!("{}/.config/stama", home))
}

fn get_file_path(file_dir: &str) -> String {
    format!("{}/config.toml", file_dir)
}

fn file_exists() -> bool {
    let file_dir = match get_file_dir() {
        Ok(file_dir) => file_dir,
        Err(_) => return false,
    };
    let file_path = get_file_path(&file_dir);
    std::path::Path::new(&file_path).exists()
}

fn touch_dir(file_dir: &str) -> Result<()> {
    // create the directory
    match fs::create_dir_all(file_dir) {
        Ok(_) => Ok(()),
        Err(_) => Err(eyre::eyre!("Could not create directory")),
    }
}


